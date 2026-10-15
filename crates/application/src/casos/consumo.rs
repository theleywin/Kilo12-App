//! Casos de uso: el consumo de los dueños (RF-CON).
//!
//! Los dueños se llevan mercancía para su casa. No es una venta —no entra
//! dinero— ni una merma —no se perdió nada—: el valor sale del inventario y
//! se convierte en una **deuda al costo** contra la cuenta que lo cargó. El
//! negocio no es más pobre; tiene otra cosa.
//!
//! ## Las tres reglas que lo sostienen
//!
//! **El costo se congela en la línea.** Kilo12 valora a costo promedio
//! ponderado (D-1), así que el costo del arroz cambia con cada entrada. Si
//! la deuda se calculara leyendo el costo de hoy, la deuda de Ivett
//! cambiaría sola cuando subiera el arroz que se llevó hace un mes. Cada
//! renglón guarda su costo, como `venta_linea` (RF-VTA-13), y el saldo es
//! la suma de lo congelado menos lo pagado.
//!
//! **Liquidar no es un ingreso.** El negocio ya gastó ese dinero cuando
//! compró la mercancía; cuando Ivett lo devuelve, el local **recupera un
//! costo**, no gana. Por eso una liquidación no toca la sesión de caja ni
//! aparece en las cifras de venta del turno: meterla ahí sería inflar
//! ingresos que no existen. Si el dinero entra físicamente a la gaveta, se
//! anota a mano como entrada de efectivo (RF-CAJ-03).
//!
//! **El saldo se mueve en los dos sentidos.** Anular un consumo lo baja;
//! anular una liquidación lo sube. Y puede quedar **negativo**: la cuenta
//! tiene dinero a favor. Eso no se recorta en cero y no se esconde — un
//! saldo negativo oculto es lo que hace que nadie vuelva a confiar en la
//! cifra. Hay cuatro capas que podrían recortarlo, y ninguna lo hace.
//!
//! ## Lo que el consumo NO hace
//!
//! No toca las ventas pasadas, no cuenta como merma —el cierre de caja
//! filtra por tipo de movimiento, así que un consumo no le llega— y no
//! cuenta como movimiento para el informe de productos parados: un producto
//! que solo se consume efectivamente no se vende, y verlo ahí avisa de que
//! algo entra al local solo para gastarse (decisión del dueño).

use std::collections::BTreeMap;

use domain::{
    Cantidad, Dinero, IdPresentacion, IdProducto, Inventario, Movimiento, Ubicacion, UnidadBase,
};

use crate::casos::informes::{peso_de, razon};
use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{
    AnulacionConsumo, ConsumoConfirmado, DescuentoConsumo, LineaConsumo, ProductoConInventario,
    RepositorioConsumo, RepositorioProducto, SaldoCuenta,
};

// ==================================================== lo que llega

/// Un renglón de lo que se llevan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaConsumoPedida {
    pub producto: i64,
    pub presentacion: i64,
    /// Cuántas presentaciones, no unidades base: 2 six-packs son «2».
    pub cantidad: String,
    /// De dónde sale: `BODEGA` o `VITRINA`.
    pub origen: String,
}

/// Lo que llega de la pantalla al registrar un consumo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoRegistrarConsumo {
    /// Cuenta a la que se carga la deuda.
    pub cuenta: i64,
    /// Quién se lo llevó. Puede no ser la dueña de la cuenta.
    pub tomado_por: String,
    pub lineas: Vec<LineaConsumoPedida>,
}

/// Lo que llega al liquidar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoLiquidar {
    pub cuenta: i64,
    pub importe: String,
    pub nota: Option<String>,
}

/// Lo que llega al anular un consumo o una liquidación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoAnularConsumo {
    pub id: i64,
    pub motivo: String,
}

// ================================================= lo que se devuelve

/// Lo que un consumo acaba de generar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumoHecho {
    pub id: i64,
    pub cuenta: String,
    pub tomado_por: String,
    /// Deuda que generó, al costo.
    pub total: String,
    /// Saldo de la cuenta después de cargarlo.
    pub saldo: String,
    pub cuantas_lineas: usize,
}

/// Un renglón tal como quedaría, con su importe ya calculado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaPrevistaConsumo {
    pub producto: i64,
    pub nombre_producto: String,
    pub nombre_presentacion: String,
    /// `Bodega` o `Vitrina`.
    pub origen: String,
    pub cantidad: String,
    pub costo_unitario: String,
    /// Lo que este renglón sumaría a la deuda.
    pub importe: String,
}

/// Lo que un consumo generaría, **sin registrar nada**.
///
/// Existe por la misma razón que la vista previa de una venta: el costo es
/// promedio ponderado y nadie lo lleva en la cabeza, así que «¿cuánto me
/// cuesta llevarme esto?» no se puede contestar sin preguntar. Y de paso se
/// entera de que no hay existencia suficiente **antes** de intentarlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumoPrevisto {
    pub cuenta: String,
    pub tomado_por: String,
    /// Deuda que generaría, al costo.
    pub total: String,
    /// Saldo en que quedaría la cuenta. Sin signo.
    pub saldo_resultante: String,
    /// El saldo resultante queda a favor de la cuenta.
    pub a_favor: bool,
    pub lineas: Vec<LineaPrevistaConsumo>,
}

/// Lo que un consumo haría, ya calculado y sin tocar nada.
#[derive(Debug)]
struct Calculo {
    cuenta: crate::puertos::CuentaConsumo,
    total: Dinero,
    lineas: Vec<LineaConsumo>,
    descuentos: Vec<DescuentoConsumo>,
    /// Unidad base de cada producto, para formatear las cantidades.
    unidades_base: BTreeMap<i64, UnidadBase>,
}

// ============================================== registrar un consumo

/// Registra lo que los dueños se llevaron (RF-CON-01).
#[derive(Debug)]
pub struct RegistrarConsumo<'a, P: RepositorioProducto, C: RepositorioConsumo> {
    productos: &'a P,
    consumos: &'a C,
}

impl<'a, P: RepositorioProducto, C: RepositorioConsumo> RegistrarConsumo<'a, P, C> {
    pub const fn nuevo(productos: &'a P, consumos: &'a C) -> Self {
        Self {
            productos,
            consumos,
        }
    }

    /// Dice lo que el consumo generaría **sin registrar nada**.
    pub fn previsualizar(&self, comando: &ComandoRegistrarConsumo) -> Resultado<ConsumoPrevisto> {
        let calculo = self.calcular(comando)?;

        // El saldo que quedaría: lo que ya debe más lo que se lleva.
        let resultante = self.saldo_de(comando.cuenta)?.sumar(calculo.total)?;

        let lineas = calculo
            .lineas
            .iter()
            .map(|linea| {
                let unidades = linea.cantidad.multiplicar_por_factor(linea.factor)?;
                let decimales = calculo
                    .unidades_base
                    .get(&linea.producto.0)
                    .copied()
                    .map_or(0, decimales_de);

                Ok(LineaPrevistaConsumo {
                    producto: linea.producto.0,
                    nombre_producto: linea.nombre_producto.clone(),
                    nombre_presentacion: linea.nombre_presentacion.clone(),
                    origen: linea.origen.nombre().to_owned(),
                    cantidad: linea.cantidad.formatear(decimales),
                    costo_unitario: linea.costo_unitario.formatear(2),
                    importe: linea.costo_unitario.multiplicar_por(unidades)?.formatear(2),
                })
            })
            .collect::<Resultado<Vec<_>>>()?;

        Ok(ConsumoPrevisto {
            cuenta: calculo.cuenta.nombre,
            tomado_por: comando.tomado_por.trim().to_owned(),
            total: calculo.total.formatear(2),
            saldo_resultante: sin_signo(resultante),
            a_favor: resultante.es_negativo(),
            lineas,
        })
    }

    pub fn ejecutar(&self, comando: ComandoRegistrarConsumo) -> Resultado<ConsumoHecho> {
        let calculo = self.calcular(&comando)?;
        let tomado_por = comando.tomado_por.trim();

        // El motivo del asiento lleva la cuenta y quién lo tomó: el kárdex
        // se lee solo (RF-INV-05) y «Consumo» a secas obligaría a cruzarlo
        // con otra tabla para saber de quién era.
        let id = self.consumos.registrar_consumo(&ConsumoConfirmado {
            cuenta: calculo.cuenta.id,
            tomado_por,
            total_costo: calculo.total,
            lineas: &calculo.lineas,
            descuentos: &calculo.descuentos,
        })?;

        Ok(ConsumoHecho {
            id,
            cuenta: calculo.cuenta.nombre,
            tomado_por: tomado_por.to_owned(),
            total: calculo.total.formatear(2),
            saldo: self.saldo_de(comando.cuenta)?.formatear(2),
            cuantas_lineas: calculo.lineas.len(),
        })
    }

    /// Hace todas las cuentas sin escribir nada.
    ///
    /// La vista previa y el registro pasan por aquí: si cada uno calculara
    /// lo suyo, el cartel podría decir una cifra y la deuda quedar en otra.
    fn calcular(&self, comando: &ComandoRegistrarConsumo) -> Resultado<Calculo> {
        if comando.lineas.is_empty() {
            return Err(ErrorAplicacion::ConsumoVacio);
        }

        let tomado_por = comando.tomado_por.trim();
        if tomado_por.is_empty() {
            return Err(ErrorAplicacion::Dominio(
                domain::ErrorDominio::TextoObligatorio("quién se lo llevó"),
            ));
        }

        let cuenta = self.cuenta_activa(comando.cuenta)?;

        // La existencia se va descontando por producto: si el mismo
        // producto aparece en dos líneas, la segunda tiene que ver lo que
        // dejó la primera y no el saldo original.
        let mut inventarios: BTreeMap<i64, Inventario> = BTreeMap::new();
        // Lo que sale, agrupado por (producto, ubicación): un asiento del
        // kárdex explica lo que salió de UN estante.
        let mut salidas: BTreeMap<(i64, Ubicacion), (Cantidad, Dinero)> = BTreeMap::new();
        let mut lineas = Vec::with_capacity(comando.lineas.len());
        let mut unidades_base: BTreeMap<i64, UnidadBase> = BTreeMap::new();
        let mut total = Dinero::CERO;

        for pedida in &comando.lineas {
            let ProductoConInventario {
                producto,
                inventario,
            } = self.productos.obtener(IdProducto(pedida.producto))?.ok_or(
                ErrorAplicacion::NoEncontrado {
                    entidad: "producto",
                    id: pedida.producto,
                },
            )?;

            // A diferencia de vender, aquí NO se exige que el producto esté
            // a la venta: retirar algo del mostrador no significa que en
            // casa no se coma. Solo se exige que la presentación exista.
            let presentacion = producto
                .presentacion(IdPresentacion(pedida.presentacion))
                .ok_or(ErrorAplicacion::Dominio(
                    domain::ErrorDominio::PresentacionNoEncontrada,
                ))?;

            let cantidad: Cantidad = pedida.cantidad.trim().parse()?;
            // Media lata no existe, tampoco al llevársela (RF-CAT-03).
            producto.validar_cantidad(cantidad)?;

            let origen: Ubicacion = pedida.origen.trim().parse()?;
            let estado = inventarios.entry(pedida.producto).or_insert(inventario);

            // El costo se congela ANTES de descontar: es el costo con el
            // que se llevó, no el que quede después.
            let unidades = cantidad.multiplicar_por_factor(presentacion.factor())?;
            let salida = estado.registrar_salida(unidades, origen)?;
            let costo_unitario = salida.costo_unitario;
            *estado = salida.inventario;

            total = total.sumar(salida.valor_salida)?;

            salidas
                .entry((pedida.producto, origen))
                .and_modify(|(acumulado, _)| {
                    *acumulado = acumulado.sumar(unidades).unwrap_or(*acumulado);
                })
                .or_insert((unidades, costo_unitario));

            unidades_base.insert(pedida.producto, producto.unidad_base());

            lineas.push(LineaConsumo {
                producto: IdProducto(pedida.producto),
                presentacion: IdPresentacion(pedida.presentacion),
                nombre_producto: producto.nombre().to_owned(),
                nombre_presentacion: presentacion.nombre().to_owned(),
                origen,
                cantidad,
                factor: presentacion.factor(),
                costo_unitario,
            });
        }

        let motivo = format!("Consumo de {} · tomó {tomado_por}", cuenta.nombre);
        let descuentos = Self::armar_descuentos(&inventarios, &salidas, &motivo)?;

        Ok(Calculo {
            cuenta,
            total,
            lineas,
            descuentos,
            unidades_base,
        })
    }

    /// Un asiento por `(producto, origen)`, no uno por línea.
    ///
    /// Del estante salió una sola vez, aunque en la pantalla se escribieran
    /// dos renglones del mismo producto. Pero si un renglón salió de la
    /// bodega y otro de la vitrina, son dos asientos: el kárdex explica
    /// saldos por ubicación.
    fn armar_descuentos(
        inventarios: &BTreeMap<i64, Inventario>,
        salidas: &BTreeMap<(i64, Ubicacion), (Cantidad, Dinero)>,
        motivo: &str,
    ) -> Resultado<Vec<DescuentoConsumo>> {
        let mut descuentos = Vec::with_capacity(salidas.len());

        for ((producto, origen), (cantidad, costo)) in salidas {
            let inventario = inventarios
                .get(producto)
                .copied()
                .unwrap_or(Inventario::VACIO);

            descuentos.push(DescuentoConsumo {
                producto: IdProducto(*producto),
                inventario,
                movimiento: Movimiento::consumo(*cantidad, *costo, *origen, motivo)?,
            });
        }

        Ok(descuentos)
    }

    fn cuenta_activa(&self, id: i64) -> Resultado<crate::puertos::CuentaConsumo> {
        let cuenta = self
            .consumos
            .cuentas(true)?
            .into_iter()
            .find(|cuenta| cuenta.id == id)
            .ok_or(ErrorAplicacion::NoEncontrado {
                entidad: "cuenta de consumo",
                id,
            })?;

        if cuenta.activa {
            Ok(cuenta)
        } else {
            Err(ErrorAplicacion::CuentaConsumoInactiva)
        }
    }

    fn saldo_de(&self, cuenta: i64) -> Resultado<Dinero> {
        Ok(self
            .consumos
            .saldos()?
            .into_iter()
            .find(|saldo| saldo.cuenta == cuenta)
            .map_or(Dinero::CERO, |saldo| saldo.saldo))
    }
}

// ===================================================== liquidar deuda

/// Paga toda o parte de la deuda de una cuenta (RF-CON-10).
#[derive(Debug)]
pub struct LiquidarDeuda<'a, C: RepositorioConsumo> {
    consumos: &'a C,
}

impl<'a, C: RepositorioConsumo> LiquidarDeuda<'a, C> {
    pub const fn nuevo(consumos: &'a C) -> Self {
        Self { consumos }
    }

    /// Anota el pago y devuelve el saldo que queda.
    ///
    /// No se comprueba que el importe no pase de la deuda: pagar de más
    /// deja la cuenta **a favor**, que es un estado legítimo y el mismo al
    /// que se llega anulando un consumo ya pagado.
    pub fn ejecutar(&self, comando: ComandoLiquidar) -> Resultado<String> {
        let importe: Dinero = comando.importe.trim().parse()?;
        if !importe.es_positivo() {
            return Err(ErrorAplicacion::PagoNoPositivo);
        }

        let existe = self
            .consumos
            .cuentas(true)?
            .into_iter()
            .any(|cuenta| cuenta.id == comando.cuenta);

        if !existe {
            return Err(ErrorAplicacion::NoEncontrado {
                entidad: "cuenta de consumo",
                id: comando.cuenta,
            });
        }

        let nota = comando
            .nota
            .as_deref()
            .map(str::trim)
            .filter(|nota| !nota.is_empty());

        self.consumos
            .registrar_pago(comando.cuenta, importe, nota)?;

        let saldo = self
            .consumos
            .saldos()?
            .into_iter()
            .find(|saldo| saldo.cuenta == comando.cuenta)
            .map_or(Dinero::CERO, |saldo| saldo.saldo);

        Ok(saldo.formatear(2))
    }
}

// ================================================= anular un consumo

/// Deshace un consumo mal registrado (RF-CON-16).
///
/// Devuelve la mercancía a la ubicación de la que salió —que no siempre es
/// la vitrina— y **al costo con que salió**, no al de hoy: reingresarla a
/// otro costo movería el valor del inventario por una operación que solo
/// deshace.
///
/// No tiene ventana de tiempo. Las ventas se limitan al turno abierto (D-5)
/// porque mueven dinero; un consumo no toca la caja, así que ese argumento
/// no aplica.
#[derive(Debug)]
pub struct AnularConsumo<'a, P: RepositorioProducto, C: RepositorioConsumo> {
    productos: &'a P,
    consumos: &'a C,
}

impl<'a, P: RepositorioProducto, C: RepositorioConsumo> AnularConsumo<'a, P, C> {
    pub const fn nuevo(productos: &'a P, consumos: &'a C) -> Self {
        Self {
            productos,
            consumos,
        }
    }

    pub fn ejecutar(&self, comando: ComandoAnularConsumo) -> Resultado<()> {
        let motivo = comando.motivo.trim();
        if motivo.is_empty() {
            return Err(ErrorAplicacion::Dominio(
                domain::ErrorDominio::MotivoObligatorio,
            ));
        }

        let (consumo, lineas) =
            self.consumos
                .consumo(comando.id)?
                .ok_or(ErrorAplicacion::NoEncontrado {
                    entidad: "consumo",
                    id: comando.id,
                })?;

        if consumo.anulado_en.is_some() {
            return Err(ErrorAplicacion::ConsumoYaAnulado);
        }

        // Igual que al registrar: el mismo producto en dos renglones tiene
        // que acumular sobre lo que dejó el anterior.
        let mut inventarios: BTreeMap<i64, Inventario> = BTreeMap::new();
        let mut entradas: BTreeMap<(i64, Ubicacion), (Cantidad, Dinero)> = BTreeMap::new();

        for linea in &lineas {
            let id = linea.producto.0;

            let estado = match inventarios.entry(id) {
                std::collections::btree_map::Entry::Occupied(hueco) => hueco.into_mut(),
                std::collections::btree_map::Entry::Vacant(hueco) => {
                    let ProductoConInventario { inventario, .. } = self
                        .productos
                        .obtener(linea.producto)?
                        .ok_or(ErrorAplicacion::NoEncontrado {
                            entidad: "producto",
                            id,
                        })?;
                    hueco.insert(inventario)
                }
            };

            let unidades = linea.cantidad.multiplicar_por_factor(linea.factor)?;

            // Vuelve al costo congelado de la línea, no al de hoy. El
            // inventario recibe el IMPORTE de lo devuelto, no el costo
            // unitario: operar sobre el total evita el redondeo intermedio
            // que RF-COS-13 prohíbe.
            let importe = linea.costo_unitario.multiplicar_por(unidades)?;
            *estado = estado.registrar_entrada(unidades, importe, linea.origen)?;

            entradas
                .entry((id, linea.origen))
                .and_modify(|(acumulado, _)| {
                    *acumulado = acumulado.sumar(unidades).unwrap_or(*acumulado);
                })
                .or_insert((unidades, linea.costo_unitario));
        }

        let explicacion = format!("Anulado: {motivo}");
        let mut reversas = Vec::with_capacity(entradas.len());

        for ((producto, destino), (cantidad, costo)) in &entradas {
            let inventario = inventarios
                .get(producto)
                .copied()
                .unwrap_or(Inventario::VACIO);

            reversas.push(DescuentoConsumo {
                producto: IdProducto(*producto),
                inventario,
                movimiento: Movimiento::consumo_anulado(
                    *cantidad,
                    *costo,
                    *destino,
                    explicacion.as_str(),
                )?,
            });
        }

        self.consumos.anular_consumo(&AnulacionConsumo {
            consumo: comando.id,
            motivo: motivo.to_owned(),
            reversas: &reversas,
        })
    }
}

// ================================================== ver la deuda

/// Una cuenta con su saldo, ya formateado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuentaEnDeuda {
    pub id: i64,
    pub nombre: String,
    pub activa: bool,
    pub consumido: String,
    pub pagado: String,
    /// Sin signo: el signo lo dice `a_favor`, que es lo que la pantalla
    /// necesita para pintarlo distinto.
    pub saldo: String,
    /// La cuenta tiene dinero a favor en vez de deber.
    pub a_favor: bool,
    /// Qué parte de la deuda del local carga esta cuenta, sin el símbolo.
    ///
    /// Se calcula sobre la suma de los saldos que **deben**: una cuenta a
    /// favor no carga nada y lleva cero. Lo calcula Rust porque la pantalla
    /// no divide dinero (DT-1).
    pub porcion: String,
    /// La misma parte de 0 a 1000, para dibujar. Es geometría, no dinero.
    pub peso: i64,
}

/// El primer nivel: el total y las cuentas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelConsumo {
    /// Deuda viva del local, sumando las tres. Las cuentas a favor restan.
    pub total: String,
    pub a_favor: bool,
    pub cuentas: Vec<CuentaEnDeuda>,
}

/// Un consumo tal como se lista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumoListado {
    pub id: i64,
    pub tomado_por: String,
    pub total: String,
    pub ocurrido_en: String,
    pub anulado: bool,
    pub motivo_anulacion: Option<String>,
}

/// Una liquidación tal como se lista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagoListado {
    pub id: i64,
    pub importe: String,
    pub nota: Option<String>,
    pub ocurrido_en: String,
    pub anulado: bool,
    pub motivo_anulacion: Option<String>,
}

/// El segundo nivel: una cuenta con lo que se llevó y lo que pagó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetalleCuenta {
    pub cuenta: CuentaEnDeuda,
    pub consumos: Vec<ConsumoListado>,
    pub pagos: Vec<PagoListado>,
}

/// Un renglón de un consumo: el nivel más fino.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaConsumoListada {
    pub producto: i64,
    pub nombre_producto: String,
    pub nombre_presentacion: String,
    /// `Bodega` o `Vitrina`.
    pub origen: String,
    pub cantidad: String,
    pub costo_unitario: String,
    /// Lo que ese renglón sumó a la deuda.
    pub importe: String,
}

/// Lo consumido de un producto, acumulado: el eje que dice qué se va.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductoConsumido {
    pub producto: i64,
    pub nombre: String,
    /// Unidades base, con los decimales que su unidad admite.
    pub cantidad: String,
    pub costo: String,
    /// Largo relativo de su barra, de 0 a 1000, respecto al que más costó.
    ///
    /// Es geometría, no dinero: la pantalla dibuja con esto y nunca enseña
    /// la cifra. Misma convención que las barras de Informes.
    pub peso: i64,
    /// Con lo que se ordena, en millonésimas. No sale a la pantalla: el
    /// orden es dinero y comparar las cadenas formateadas pondría «9.00»
    /// por encima de «10.00».
    pub orden: i64,
}

/// Consulta la deuda de consumo en sus distintos niveles (RF-CON-12).
#[derive(Debug)]
pub struct ConsultarConsumo<'a, C: RepositorioConsumo> {
    consumos: &'a C,
}

impl<'a, C: RepositorioConsumo> ConsultarConsumo<'a, C> {
    pub const fn nuevo(consumos: &'a C) -> Self {
        Self { consumos }
    }

    /// Nivel 1 y 2: el total del local y el saldo de cada cuenta.
    pub fn panel(&self) -> Resultado<PanelConsumo> {
        let saldos = self.consumos.saldos()?;

        let deuda = deuda_viva(&saldos)?;
        let mut total = Dinero::CERO;
        let mut cuentas = Vec::with_capacity(saldos.len());

        for saldo in saldos {
            total = total.sumar(saldo.saldo)?;
            cuentas.push(describir_cuenta(saldo, deuda)?);
        }

        Ok(PanelConsumo {
            total: sin_signo(total),
            a_favor: total.es_negativo(),
            cuentas,
        })
    }

    /// Nivel 3: los consumos y los pagos de una cuenta.
    pub fn cuenta(&self, cuenta: i64, limite: usize) -> Resultado<DetalleCuenta> {
        let saldos = self.consumos.saldos()?;
        let deuda = deuda_viva(&saldos)?;

        let saldo = saldos
            .into_iter()
            .find(|saldo| saldo.cuenta == cuenta)
            .ok_or(ErrorAplicacion::NoEncontrado {
                entidad: "cuenta de consumo",
                id: cuenta,
            })?;

        let consumos = self
            .consumos
            .consumos_de_cuenta(cuenta, limite)?
            .into_iter()
            .map(|registrado| ConsumoListado {
                id: registrado.id,
                tomado_por: registrado.tomado_por,
                total: registrado.total_costo.formatear(2),
                ocurrido_en: registrado.ocurrido_en,
                anulado: registrado.anulado_en.is_some(),
                motivo_anulacion: registrado.motivo_anulacion,
            })
            .collect();

        let pagos = self
            .consumos
            .pagos_de_cuenta(cuenta, limite)?
            .into_iter()
            .map(|registrado| PagoListado {
                id: registrado.id,
                importe: registrado.importe.formatear(2),
                nota: registrado.nota,
                ocurrido_en: registrado.ocurrido_en,
                anulado: registrado.anulado_en.is_some(),
                motivo_anulacion: registrado.motivo_anulacion,
            })
            .collect();

        Ok(DetalleCuenta {
            cuenta: describir_cuenta(saldo, deuda)?,
            consumos,
            pagos,
        })
    }

    /// Nivel 4: los renglones de un consumo concreto.
    pub fn renglones(&self, consumo: i64) -> Resultado<Vec<LineaConsumoListada>> {
        let (_, lineas) = self
            .consumos
            .consumo(consumo)?
            .ok_or(ErrorAplicacion::NoEncontrado {
                entidad: "consumo",
                id: consumo,
            })?;

        lineas
            .into_iter()
            .map(|linea| {
                let unidades = linea.cantidad.multiplicar_por_factor(linea.factor)?;
                Ok(LineaConsumoListada {
                    producto: linea.producto.0,
                    nombre_producto: linea.nombre_producto,
                    nombre_presentacion: linea.nombre_presentacion,
                    origen: linea.origen.nombre().to_owned(),
                    // En presentaciones, que es como se pidió, y con los
                    // decimales que la unidad del producto admite.
                    cantidad: linea.cantidad.formatear(decimales_de(linea.unidad_base)),
                    costo_unitario: linea.costo_unitario.formatear(2),
                    importe: linea.costo_unitario.multiplicar_por(unidades)?.formatear(2),
                })
            })
            .collect()
    }

    /// El eje extra: qué producto se va más en consumo.
    ///
    /// `cuenta` en blanco suma las tres. Sin fechas mira todo el historial,
    /// que es la pregunta que se hace de verdad: «¿qué es lo que se está
    /// yendo?», no «¿qué se fue en marzo?».
    pub fn por_producto(
        &self,
        cuenta: Option<i64>,
        desde: Option<&str>,
        hasta: Option<&str>,
    ) -> Resultado<Vec<ProductoConsumido>> {
        let lineas = self.consumos.lineas_consumidas(cuenta, desde, hasta)?;

        // La agregación se hace AQUÍ y no en SQL: multiplicar cantidad por
        // factor y por costo con enteros escalados pierde milésimas en la
        // división intermedia, y ese es el redondeo que todo el dominio se
        // toma el trabajo de evitar (DT-5).
        let mut acumulado: BTreeMap<i64, (String, UnidadBase, Cantidad, Dinero)> = BTreeMap::new();

        for linea in lineas {
            let unidades = linea.cantidad.multiplicar_por_factor(linea.factor)?;
            let costo = linea.costo_unitario.multiplicar_por(unidades)?;

            match acumulado.entry(linea.producto.0) {
                std::collections::btree_map::Entry::Occupied(mut hueco) => {
                    let (_, _, cantidad, importe) = hueco.get_mut();
                    *cantidad = cantidad.sumar(unidades)?;
                    *importe = importe.sumar(costo)?;
                }
                std::collections::btree_map::Entry::Vacant(hueco) => {
                    hueco.insert((linea.nombre_producto, linea.unidad_base, unidades, costo));
                }
            }
        }

        // El techo es lo que más costó: las barras se miden contra ello.
        let techo = acumulado
            .values()
            .map(|(_, _, _, costo)| *costo)
            .max()
            .unwrap_or(Dinero::CERO);

        let mut filas: Vec<ProductoConsumido> = acumulado
            .into_iter()
            .map(
                |(producto, (nombre, unidad, cantidad, costo))| ProductoConsumido {
                    producto,
                    nombre,
                    cantidad: format!(
                        "{} {}",
                        cantidad.formatear(decimales_de(unidad)),
                        unidad.simbolo()
                    ),
                    costo: costo.formatear(2),
                    peso: peso_de(costo, techo),
                    orden: costo.millonesimas(),
                },
            )
            .collect();

        // De lo que más costó a lo que menos: la pregunta es qué se está
        // yendo, y eso se mide en dinero, no en unidades.
        filas.sort_by_key(|fila| std::cmp::Reverse(fila.orden));
        Ok(filas)
    }
}

/// Cuántos decimales admite una unidad.
///
/// «3 latas» y «3.500 lb» son las dos correctas; «3.000 latas» solo
/// consigue que el ojo tropiece.
const fn decimales_de(unidad: UnidadBase) -> u32 {
    if unidad.admite_fracciones() {
        3
    } else {
        0
    }
}

/// Describe una cuenta para la pantalla, con su parte de la deuda.
///
/// La construye un solo sitio: el panel y el detalle de una cuenta tienen
/// que enseñar exactamente las mismas cifras, y dos constructores se
/// separan en cuanto alguien añade un campo a uno.
fn describir_cuenta(saldo: SaldoCuenta, deuda: Dinero) -> Resultado<CuentaEnDeuda> {
    Ok(CuentaEnDeuda {
        id: saldo.cuenta,
        nombre: saldo.nombre,
        activa: saldo.activa,
        consumido: saldo.consumido.formatear(2),
        pagado: saldo.pagado.formatear(2),
        saldo: sin_signo(saldo.saldo),
        a_favor: saldo.saldo.es_negativo(),
        porcion: razon(saldo.saldo.o_cero_si_negativo(), deuda)?.formatear(1),
        peso: peso_de(saldo.saldo, deuda),
    })
}

/// Lo que se debe entre todas las cuentas que deben algo.
///
/// Es el denominador del reparto, y no es el neto: si una cuenta tiene
/// dinero a favor, restarla haría que las otras sumaran más del cien por
/// cien.
fn deuda_viva(saldos: &[SaldoCuenta]) -> Resultado<Dinero> {
    let mut deuda = Dinero::CERO;

    for saldo in saldos {
        if saldo.saldo.es_positivo() {
            deuda = deuda.sumar(saldo.saldo)?;
        }
    }

    Ok(deuda)
}

/// Formatea un importe sin su signo.
///
/// El signo no se pierde: viaja aparte, en `a_favor`. Un «-500.00» en
/// pantalla se lee como un error de cálculo; «500.00 a favor» se lee como
/// lo que es.
fn sin_signo(importe: Dinero) -> String {
    let texto = importe.formatear(2);
    texto.strip_prefix('-').unwrap_or(&texto).to_owned()
}

// ============================================== anular una liquidación

/// Deshace una liquidación mal registrada (RF-CON-17).
///
/// Sube el saldo otra vez. No lleva clave de mantenimiento: es la
/// corrección de un error de tecleo, no una operación que destruya cifras
/// irreconstruibles. Sin ella, la única salida a un «500» donde iban «50»
/// sería registrar un consumo que nunca ocurrió, o sea meter una mentira en
/// los datos para tapar un error de dedo.
#[derive(Debug)]
pub struct AnularPagoConsumo<'a, C: RepositorioConsumo> {
    consumos: &'a C,
}

impl<'a, C: RepositorioConsumo> AnularPagoConsumo<'a, C> {
    pub const fn nuevo(consumos: &'a C) -> Self {
        Self { consumos }
    }

    pub fn ejecutar(&self, comando: ComandoAnularConsumo) -> Resultado<()> {
        let motivo = comando.motivo.trim();
        if motivo.is_empty() {
            return Err(ErrorAplicacion::Dominio(
                domain::ErrorDominio::MotivoObligatorio,
            ));
        }

        let pago = self
            .consumos
            .pago(comando.id)?
            .ok_or(ErrorAplicacion::NoEncontrado {
                entidad: "liquidación",
                id: comando.id,
            })?;

        if pago.anulado_en.is_some() {
            return Err(ErrorAplicacion::PagoYaAnulado);
        }

        self.consumos.anular_pago(comando.id, motivo)
    }
}

#[cfg(test)]
mod pruebas {
    use domain::{TipoMovimiento, UnidadBase};

    use super::*;
    use crate::dobles::{ConsumosEnMemoria, ProductosEnMemoria};

    /// Cien libras de arroz a 120.00 la libra, y veinticuatro refrescos a
    /// 41.67 la unidad, con un six-pack como segunda presentación.
    fn catalogo() -> ProductosEnMemoria {
        ProductosEnMemoria::default()
            .con_producto(
                1,
                "Arroz blanco",
                UnidadBase::Libra,
                &[(10, "Libra", "1", "180.00")],
                "100",
            )
            .con_valor(1, "12000.00")
            .con_producto(
                2,
                "Refresco",
                UnidadBase::Unidad,
                &[
                    (20, "Unidad", "1", "80.00"),
                    (21, "Six-pack", "6", "450.00"),
                ],
                "24",
            )
            .con_valor(2, "1000.08")
    }

    fn linea(producto: i64, presentacion: i64, cantidad: &str) -> LineaConsumoPedida {
        LineaConsumoPedida {
            producto,
            presentacion,
            cantidad: cantidad.to_owned(),
            origen: "VITRINA".to_owned(),
        }
    }

    /// Los dos dobles sobre el MISMO catálogo, como en la aplicación: el
    /// adaptador del consumo escribe la existencia y el valor él solo.
    fn entorno() -> (std::rc::Rc<ProductosEnMemoria>, ConsumosEnMemoria) {
        let productos = std::rc::Rc::new(catalogo());
        let consumos = ConsumosEnMemoria::sobre(std::rc::Rc::clone(&productos));
        (productos, consumos)
    }

    fn entorno_con(
        productos: ProductosEnMemoria,
    ) -> (std::rc::Rc<ProductosEnMemoria>, ConsumosEnMemoria) {
        let productos = std::rc::Rc::new(productos);
        let consumos = ConsumosEnMemoria::sobre(std::rc::Rc::clone(&productos));
        (productos, consumos)
    }

    fn comando(
        cuenta: i64,
        tomado_por: &str,
        lineas: Vec<LineaConsumoPedida>,
    ) -> ComandoRegistrarConsumo {
        ComandoRegistrarConsumo {
            cuenta,
            tomado_por: tomado_por.to_owned(),
            lineas,
        }
    }

    /// Ivett carga dos libras de arroz y un six-pack: 240.00 + 250.02.
    fn consumo_de_ivett(
        productos: &ProductosEnMemoria,
        consumos: &ConsumosEnMemoria,
    ) -> ConsumoHecho {
        RegistrarConsumo::nuevo(productos, consumos)
            .ejecutar(comando(
                1,
                "Yaneisy",
                vec![linea(1, 10, "2"), linea(2, 21, "1")],
            ))
            .expect("registrar el consumo")
    }

    // -------------------------------------------- registrar un consumo

    #[test]
    fn un_consumo_genera_deuda_al_costo_y_dice_quien_lo_tomo() {
        let (productos, consumos) = entorno();

        let hecho = consumo_de_ivett(&productos, &consumos);

        // 2 lb × 120.00 = 240.00; 6 u × 41.67 = 250.02.
        assert_eq!(hecho.total, "490.02");
        assert_eq!(hecho.cuenta, "Ivett");
        assert_eq!(
            hecho.tomado_por, "Yaneisy",
            "la cuenta no es quien se lo llevó"
        );
        assert_eq!(hecho.saldo, "490.02");
        assert_eq!(hecho.cuantas_lineas, 2);
    }

    #[test]
    fn el_consumo_descuenta_la_existencia_de_donde_salio() {
        let (productos, consumos) = entorno();
        consumo_de_ivett(&productos, &consumos);

        // Del arroz salieron 2 lb de las 100; del refresco, 6 de 24.
        assert_eq!(
            productos
                .inventario_de(1)
                .existencias()
                .en(Ubicacion::Vitrina)
                .formatear(3),
            "98.000"
        );
        assert_eq!(
            productos
                .inventario_de(2)
                .existencias()
                .en(Ubicacion::Vitrina)
                .formatear(0),
            "18"
        );
    }

    #[test]
    fn el_valor_del_inventario_baja_justo_la_deuda_que_se_genero() {
        // El negocio no es más pobre: convirtió inventario en deuda.
        let (productos, consumos) = entorno();
        let antes = productos.inventario_de(1).valor_total();

        consumo_de_ivett(&productos, &consumos);

        let despues = productos.inventario_de(1).valor_total();
        assert_eq!(antes.restar(despues).expect("resta").formatear(2), "240.00");
    }

    /// El invariante que sostiene todo el módulo.
    #[test]
    fn la_deuda_no_se_mueve_cuando_el_costo_cambia_despues() {
        let (productos, consumos) = entorno();
        let hecho = consumo_de_ivett(&productos, &consumos);
        assert_eq!(hecho.total, "490.02");

        // Llega arroz mucho más caro: el costo promedio sube.
        let inventario = productos.inventario_de(1);
        let encarecido = inventario
            .registrar_entrada(
                "100".parse().expect("cantidad"),
                "30000.00".parse().expect("importe"),
                Ubicacion::Vitrina,
            )
            .expect("entrada");
        assert!(
            encarecido.costo_unitario().expect("costo")
                > inventario.costo_unitario().expect("costo")
        );

        // Y la deuda de Ivett sigue siendo byte a byte la misma.
        let panel = ConsultarConsumo::nuevo(&consumos).panel().expect("panel");
        assert_eq!(panel.cuentas[0].saldo, "490.02");
        assert_eq!(panel.cuentas[0].consumido, "490.02");
    }

    #[test]
    fn un_asiento_por_estante_no_uno_por_renglon() {
        // Dos renglones del mismo producto desde el mismo sitio salieron
        // del estante una sola vez.
        let (productos, consumos) = entorno();

        RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(
                1,
                "Ivett",
                vec![linea(1, 10, "2"), linea(1, 10, "3")],
            ))
            .expect("registrar");

        let asientos = consumos.asientos.borrow();
        assert_eq!(asientos[0].1.len(), 1, "un solo asiento");
        assert_eq!(asientos[0].1[0].1.cantidad().formatear(3), "5.000");
        assert_eq!(asientos[0].1[0].1.tipo(), TipoMovimiento::Consumo);
    }

    #[test]
    fn dos_origenes_en_el_mismo_acto_son_dos_asientos() {
        // El aceite de la vitrina y el arroz de la bodega: el kárdex
        // explica saldos por ubicación, así que son dos asientos.
        let (productos, consumos) = entorno_con(catalogo().con_bodega(1, "50"));

        let mut de_bodega = linea(1, 10, "4");
        de_bodega.origen = "BODEGA".to_owned();

        RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(1, "Ivett", vec![linea(1, 10, "2"), de_bodega]))
            .expect("registrar");

        let asientos = consumos.asientos.borrow();
        let mut origenes: Vec<Ubicacion> = asientos[0]
            .1
            .iter()
            .filter_map(|(_, movimiento)| movimiento.origen())
            .collect();
        origenes.sort();

        assert_eq!(origenes, [Ubicacion::Bodega, Ubicacion::Vitrina]);
        assert_eq!(asientos[0].1.len(), 2);
    }

    #[test]
    fn el_asiento_dice_la_cuenta_y_quien_lo_tomo() {
        let (productos, consumos) = entorno();
        consumo_de_ivett(&productos, &consumos);

        let asientos = consumos.asientos.borrow();
        assert_eq!(
            asientos[0].1[0].1.motivo(),
            Some("Consumo de Ivett · tomó Yaneisy")
        );
    }

    #[test]
    fn no_se_consume_mas_de_lo_que_hay() {
        let (productos, consumos) = entorno();

        let error = RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(1, "Ivett", vec![linea(1, 10, "500")]))
            .expect_err("no hay 500 libras");

        assert_eq!(error.codigo(), "EXISTENCIA_INSUFICIENTE");
    }

    #[test]
    fn no_se_consume_de_la_bodega_lo_que_solo_esta_en_la_vitrina() {
        // La existencia es por ubicación: cien libras en vitrina no son
        // cien libras en bodega.
        let (productos, consumos) = entorno();

        let mut de_bodega = linea(1, 10, "2");
        de_bodega.origen = "BODEGA".to_owned();

        let error = RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(1, "Ivett", vec![de_bodega]))
            .expect_err("la bodega está vacía");

        assert_eq!(error.codigo(), "EXISTENCIA_INSUFICIENTE");
    }

    #[test]
    fn un_producto_retirado_de_la_venta_si_se_puede_consumir() {
        // Retirarlo del mostrador no significa que en casa no se coma.
        // Esto es distinto de vender, que lo rechaza.
        let (productos, consumos) = entorno();
        productos.desactivar_producto(1);

        let hecho = RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(1, "Ivett", vec![linea(1, 10, "2")]))
            .expect("un retirado sigue siendo comestible");

        assert_eq!(hecho.total, "240.00");
    }

    #[test]
    fn no_se_carga_nada_a_una_cuenta_retirada() {
        let (productos, consumos) = entorno();
        consumos.desactivar_cuenta(1);

        let error = RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(1, "Ivett", vec![linea(1, 10, "2")]))
            .expect_err("esa cuenta está retirada");

        assert_eq!(error.codigo(), "CUENTA_CONSUMO_INACTIVA");
    }

    #[test]
    fn un_consumo_sin_renglones_no_es_nada() {
        let (productos, consumos) = entorno();

        let error = RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(1, "Ivett", vec![]))
            .expect_err("nada que registrar");

        assert_eq!(error.codigo(), "CONSUMO_VACIO");
    }

    #[test]
    fn hay_que_decir_quien_se_lo_llevo() {
        let (productos, consumos) = entorno();

        let error = RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(1, "   ", vec![linea(1, 10, "2")]))
            .expect_err("sin nombre no se registra");

        assert_eq!(error.codigo(), "TEXTO_OBLIGATORIO");
    }

    // ----------------------------------------------------- liquidar

    #[test]
    fn una_liquidacion_parcial_deja_el_resto_pendiente() {
        let (productos, consumos) = entorno();
        consumo_de_ivett(&productos, &consumos);

        let saldo = LiquidarDeuda::nuevo(&consumos)
            .ejecutar(ComandoLiquidar {
                cuenta: 1,
                importe: "200.00".to_owned(),
                nota: Some("a cuenta".to_owned()),
            })
            .expect("liquidar");

        assert_eq!(saldo, "290.02");
    }

    #[test]
    fn liquidar_el_total_deja_la_cuenta_en_cero() {
        let (productos, consumos) = entorno();
        consumo_de_ivett(&productos, &consumos);

        let saldo = LiquidarDeuda::nuevo(&consumos)
            .ejecutar(ComandoLiquidar {
                cuenta: 1,
                importe: "490.02".to_owned(),
                nota: None,
            })
            .expect("liquidar");

        assert_eq!(saldo, "0.00");
    }

    #[test]
    fn pagar_de_mas_deja_la_cuenta_a_favor() {
        // No se recorta: es un estado legítimo, el mismo al que se llega
        // anulando un consumo ya pagado.
        let (productos, consumos) = entorno();
        consumo_de_ivett(&productos, &consumos);

        LiquidarDeuda::nuevo(&consumos)
            .ejecutar(ComandoLiquidar {
                cuenta: 1,
                importe: "600.00".to_owned(),
                nota: None,
            })
            .expect("liquidar");

        let panel = ConsultarConsumo::nuevo(&consumos).panel().expect("panel");
        assert_eq!(panel.cuentas[0].saldo, "109.98");
        assert!(panel.cuentas[0].a_favor, "tiene dinero a favor");
    }

    #[test]
    fn liquidar_cero_no_liquida_nada() {
        let consumos = ConsumosEnMemoria::default();

        let error = LiquidarDeuda::nuevo(&consumos)
            .ejecutar(ComandoLiquidar {
                cuenta: 1,
                importe: "0".to_owned(),
                nota: None,
            })
            .expect_err("cero no es un pago");

        assert_eq!(error.codigo(), "PAGO_NO_POSITIVO");
    }

    // ------------------------------------------------------ anular

    #[test]
    fn anular_un_consumo_devuelve_la_mercancia_y_baja_la_deuda() {
        let (productos, consumos) = entorno();
        let hecho = consumo_de_ivett(&productos, &consumos);

        AnularConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(ComandoAnularConsumo {
                id: hecho.id,
                motivo: "Se cargó a la cuenta equivocada".to_owned(),
            })
            .expect("anular");

        assert_eq!(consumos.saldo_de(1).formatear(2), "0.00");
        assert_eq!(
            productos
                .inventario_de(1)
                .existencias()
                .en(Ubicacion::Vitrina)
                .formatear(3),
            "100.000",
            "las dos libras volvieron"
        );
        assert_eq!(
            productos.inventario_de(1).valor_total().formatear(2),
            "12000.00",
            "y volvieron al costo con que salieron"
        );
    }

    #[test]
    fn la_reversa_deja_su_propio_asiento() {
        let (productos, consumos) = entorno();
        let hecho = consumo_de_ivett(&productos, &consumos);

        AnularConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(ComandoAnularConsumo {
                id: hecho.id,
                motivo: "Error de dedo".to_owned(),
            })
            .expect("anular");

        let asientos = consumos.asientos.borrow();
        let reversa = &asientos[1].1[0].1;
        assert_eq!(reversa.tipo(), TipoMovimiento::ConsumoAnulado);
        assert_eq!(reversa.motivo(), Some("Anulado: Error de dedo"));
        assert!(reversa.tipo().es_entrada());
    }

    #[test]
    fn la_mercancia_vuelve_al_estante_del_que_salio() {
        let (productos, consumos) = entorno_con(catalogo().con_bodega(1, "50"));

        let mut de_bodega = linea(1, 10, "4");
        de_bodega.origen = "BODEGA".to_owned();

        let hecho = RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(1, "Ivett", vec![de_bodega]))
            .expect("registrar");

        AnularConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(ComandoAnularConsumo {
                id: hecho.id,
                motivo: "Devuelto".to_owned(),
            })
            .expect("anular");

        // A la bodega, no a la vitrina.
        let existencias = productos.inventario_de(1).existencias();
        assert_eq!(existencias.en(Ubicacion::Bodega).formatear(3), "50.000");
        assert_eq!(existencias.en(Ubicacion::Vitrina).formatear(3), "100.000");
    }

    #[test]
    fn un_consumo_no_se_anula_dos_veces() {
        let (productos, consumos) = entorno();
        let hecho = consumo_de_ivett(&productos, &consumos);

        let anular = || {
            AnularConsumo::nuevo(productos.as_ref(), &consumos).ejecutar(ComandoAnularConsumo {
                id: hecho.id,
                motivo: "Error".to_owned(),
            })
        };

        anular().expect("la primera sí");
        assert_eq!(
            anular().expect_err("la segunda no").codigo(),
            "CONSUMO_YA_ANULADO"
        );
    }

    #[test]
    fn anular_un_consumo_ya_pagado_deja_la_cuenta_a_favor() {
        let (productos, consumos) = entorno();
        let hecho = consumo_de_ivett(&productos, &consumos);

        LiquidarDeuda::nuevo(&consumos)
            .ejecutar(ComandoLiquidar {
                cuenta: 1,
                importe: "490.02".to_owned(),
                nota: None,
            })
            .expect("liquidar");

        AnularConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(ComandoAnularConsumo {
                id: hecho.id,
                motivo: "No era de Ivett".to_owned(),
            })
            .expect("anular");

        let panel = ConsultarConsumo::nuevo(&consumos).panel().expect("panel");
        assert_eq!(panel.cuentas[0].saldo, "490.02");
        assert!(panel.cuentas[0].a_favor, "pagó algo que no debía");
        assert!(panel.a_favor, "y el total del local también");
    }

    #[test]
    fn anular_un_pago_sube_la_deuda_otra_vez() {
        let (productos, consumos) = entorno();
        consumo_de_ivett(&productos, &consumos);

        LiquidarDeuda::nuevo(&consumos)
            .ejecutar(ComandoLiquidar {
                cuenta: 1,
                importe: "490.02".to_owned(),
                nota: None,
            })
            .expect("liquidar");
        assert_eq!(consumos.saldo_de(1).formatear(2), "0.00");

        AnularPagoConsumo::nuevo(&consumos)
            .ejecutar(ComandoAnularConsumo {
                id: 1,
                motivo: "Se teclearon 490 por 49".to_owned(),
            })
            .expect("anular el pago");

        assert_eq!(consumos.saldo_de(1).formatear(2), "490.02");
    }

    #[test]
    fn un_pago_no_se_anula_dos_veces() {
        let consumos = ConsumosEnMemoria::default();
        LiquidarDeuda::nuevo(&consumos)
            .ejecutar(ComandoLiquidar {
                cuenta: 1,
                importe: "100.00".to_owned(),
                nota: None,
            })
            .expect("liquidar");

        let anular = || {
            AnularPagoConsumo::nuevo(&consumos).ejecutar(ComandoAnularConsumo {
                id: 1,
                motivo: "Error".to_owned(),
            })
        };

        anular().expect("la primera sí");
        assert_eq!(
            anular().expect_err("la segunda no").codigo(),
            "PAGO_YA_ANULADO"
        );
    }

    /// El ciclo completo tiene que cerrar en cero exacto.
    ///
    /// Es la prueba que caza un recorte en cualquiera de las capas: si
    /// alguna pusiera un suelo en cero, este recorrido no cuadraría.
    #[test]
    fn consumir_pagar_anular_el_consumo_y_anular_el_pago_cierra_en_cero() {
        let (productos, consumos) = entorno();

        let hecho = consumo_de_ivett(&productos, &consumos);
        assert_eq!(consumos.saldo_de(1).formatear(2), "490.02", "debe");

        LiquidarDeuda::nuevo(&consumos)
            .ejecutar(ComandoLiquidar {
                cuenta: 1,
                importe: "490.02".to_owned(),
                nota: None,
            })
            .expect("liquidar");
        assert_eq!(consumos.saldo_de(1).formatear(2), "0.00", "pagado");

        AnularConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(ComandoAnularConsumo {
                id: hecho.id,
                motivo: "No era suyo".to_owned(),
            })
            .expect("anular el consumo");
        assert_eq!(consumos.saldo_de(1).formatear(2), "-490.02", "a favor");

        AnularPagoConsumo::nuevo(&consumos)
            .ejecutar(ComandoAnularConsumo {
                id: 1,
                motivo: "Y el pago tampoco".to_owned(),
            })
            .expect("anular el pago");

        assert_eq!(consumos.saldo_de(1).formatear(2), "0.00", "cerrado");
    }

    #[test]
    fn no_se_anula_sin_decir_por_que() {
        let (productos, consumos) = entorno();
        let hecho = consumo_de_ivett(&productos, &consumos);

        let error = AnularConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(ComandoAnularConsumo {
                id: hecho.id,
                motivo: "  ".to_owned(),
            })
            .expect_err("sin motivo no se anula");

        assert_eq!(error.codigo(), "MOTIVO_OBLIGATORIO");
    }

    // --------------------------------------------------- los niveles

    #[test]
    fn el_panel_suma_las_tres_cuentas() {
        let (productos, consumos) = entorno();
        consumo_de_ivett(&productos, &consumos);

        RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(2, "Claudia", vec![linea(1, 10, "1")]))
            .expect("registrar");

        let panel = ConsultarConsumo::nuevo(&consumos).panel().expect("panel");

        assert_eq!(panel.total, "610.02", "490.02 + 120.00");
        assert!(!panel.a_favor);
        assert_eq!(panel.cuentas.len(), 3, "las tres, deban o no");
        assert_eq!(panel.cuentas[1].nombre, "Claudia");
        assert_eq!(panel.cuentas[1].saldo, "120.00");
        assert_eq!(panel.cuentas[2].saldo, "0.00", "Inés no debe nada");
    }

    // --------------------------------------------- la vista previa

    #[test]
    fn la_vista_previa_dice_lo_que_costaria_sin_registrar_nada() {
        let (productos, consumos) = entorno();

        let prevista = RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .previsualizar(&comando(
                1,
                "Yaneisy",
                vec![linea(1, 10, "2"), linea(2, 21, "1")],
            ))
            .expect("previsualizar");

        assert_eq!(prevista.cuenta, "Ivett");
        assert_eq!(prevista.total, "490.02");
        assert_eq!(prevista.saldo_resultante, "490.02");
        assert!(!prevista.a_favor);
        assert_eq!(prevista.lineas.len(), 2);
        // La libra admite decimales; la lata, no.
        assert_eq!(prevista.lineas[0].cantidad, "2.000");
        assert_eq!(prevista.lineas[0].importe, "240.00");
        assert_eq!(prevista.lineas[1].cantidad, "1");
        assert_eq!(prevista.lineas[1].importe, "250.02");

        // Y no ha tocado nada.
        assert_eq!(
            productos
                .inventario_de(1)
                .existencias()
                .en(Ubicacion::Vitrina)
                .formatear(3),
            "100.000"
        );
        assert_eq!(consumos.saldo_de(1).formatear(2), "0.00");
    }

    #[test]
    fn la_vista_previa_avisa_antes_de_intentarlo() {
        // Es la mitad de su valor: enterarse de que no hay existencia al
        // confirmar es enterarse tarde.
        let (productos, consumos) = entorno();

        let error = RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .previsualizar(&comando(1, "Ivett", vec![linea(1, 10, "500")]))
            .expect_err("no hay 500 libras");

        assert_eq!(error.codigo(), "EXISTENCIA_INSUFICIENTE");
    }

    #[test]
    fn la_vista_previa_suma_sobre_lo_que_ya_se_debe() {
        let (productos, consumos) = entorno();
        consumo_de_ivett(productos.as_ref(), &consumos);

        let prevista = RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .previsualizar(&comando(1, "Ivett", vec![linea(1, 10, "1")]))
            .expect("previsualizar");

        assert_eq!(prevista.total, "120.00", "lo que se lleva ahora");
        assert_eq!(prevista.saldo_resultante, "610.02", "490.02 + 120.00");
    }

    #[test]
    fn la_vista_previa_y_el_registro_dicen_la_misma_cifra() {
        // Si cada uno calculara lo suyo, el cartel podría prometer una
        // cosa y la deuda quedar en otra.
        let (productos, consumos) = entorno();
        let pedido = comando(1, "Yaneisy", vec![linea(1, 10, "2"), linea(2, 21, "1")]);

        let caso = RegistrarConsumo::nuevo(productos.as_ref(), &consumos);
        let prevista = caso.previsualizar(&pedido).expect("previsualizar");
        let hecho = caso.ejecutar(pedido).expect("registrar");

        assert_eq!(prevista.total, hecho.total);
        assert_eq!(prevista.saldo_resultante, hecho.saldo);
    }

    // ------------------------------------------- el reparto de la deuda

    #[test]
    fn el_panel_reparte_la_deuda_entre_quienes_deben() {
        let (productos, consumos) = entorno();
        // Ivett 240.00, Claudia 120.00: dos tercios y un tercio.
        RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(1, "Ivett", vec![linea(1, 10, "2")]))
            .expect("registrar");
        RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(2, "Claudia", vec![linea(1, 10, "1")]))
            .expect("registrar");

        let panel = ConsultarConsumo::nuevo(&consumos).panel().expect("panel");

        assert_eq!(panel.total, "360.00");
        assert_eq!(panel.cuentas[0].porcion, "66.7");
        assert_eq!(panel.cuentas[0].peso, 666);
        assert_eq!(panel.cuentas[1].porcion, "33.3");
        assert_eq!(panel.cuentas[2].porcion, "0.0", "Inés no debe nada");
        assert_eq!(panel.cuentas[2].peso, 0);
    }

    #[test]
    fn una_cuenta_a_favor_no_carga_parte_de_la_deuda() {
        // Si se restara del denominador, las otras sumarían más del cien
        // por cien y la barra se saldría de la pantalla.
        let (productos, consumos) = entorno();
        RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(1, "Ivett", vec![linea(1, 10, "2")]))
            .expect("registrar");
        LiquidarDeuda::nuevo(&consumos)
            .ejecutar(ComandoLiquidar {
                cuenta: 2,
                importe: "500.00".to_owned(),
                nota: None,
            })
            .expect("Claudia paga de más sin deber nada");

        let panel = ConsultarConsumo::nuevo(&consumos).panel().expect("panel");

        assert_eq!(
            panel.cuentas[0].porcion, "100.0",
            "Ivett carga toda la deuda"
        );
        assert_eq!(panel.cuentas[0].peso, 1000);
        assert!(panel.cuentas[1].a_favor);
        assert_eq!(panel.cuentas[1].porcion, "0.0");
        assert_eq!(panel.cuentas[1].peso, 0);
    }

    #[test]
    fn el_detalle_de_una_cuenta_lleva_sus_consumos_y_sus_pagos() {
        let (productos, consumos) = entorno();
        consumo_de_ivett(&productos, &consumos);
        LiquidarDeuda::nuevo(&consumos)
            .ejecutar(ComandoLiquidar {
                cuenta: 1,
                importe: "100.00".to_owned(),
                nota: Some("del cumpleaños".to_owned()),
            })
            .expect("liquidar");

        let detalle = ConsultarConsumo::nuevo(&consumos)
            .cuenta(1, 10)
            .expect("detalle");

        assert_eq!(detalle.cuenta.consumido, "490.02");
        assert_eq!(detalle.cuenta.pagado, "100.00");
        assert_eq!(detalle.cuenta.saldo, "390.02");
        assert_eq!(detalle.consumos.len(), 1);
        assert_eq!(detalle.consumos[0].tomado_por, "Yaneisy");
        assert!(!detalle.consumos[0].anulado);
        assert_eq!(detalle.pagos.len(), 1);
        assert_eq!(detalle.pagos[0].nota.as_deref(), Some("del cumpleaños"));
    }

    #[test]
    fn los_renglones_de_un_consumo_dicen_que_sumo_cada_uno() {
        let (productos, consumos) = entorno();
        let hecho = consumo_de_ivett(&productos, &consumos);

        let renglones = ConsultarConsumo::nuevo(&consumos)
            .renglones(hecho.id)
            .expect("renglones");

        assert_eq!(renglones.len(), 2);
        assert_eq!(renglones[0].nombre_producto, "Arroz blanco");
        assert_eq!(renglones[0].nombre_presentacion, "Libra");
        assert_eq!(renglones[0].origen, "Vitrina");
        assert_eq!(renglones[0].costo_unitario, "120.00");
        assert_eq!(renglones[0].importe, "240.00");
        // El six-pack: seis unidades a 41.67.
        assert_eq!(renglones[1].nombre_presentacion, "Six-pack");
        assert_eq!(renglones[1].importe, "250.02");
    }

    #[test]
    fn el_eje_por_producto_dice_que_se_esta_yendo() {
        let (productos, consumos) = entorno();
        consumo_de_ivett(&productos, &consumos);

        // Claudia se lleva mucho más arroz.
        RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(2, "Claudia", vec![linea(1, 10, "10")]))
            .expect("registrar");

        let filas = ConsultarConsumo::nuevo(&consumos)
            .por_producto(None, None, None)
            .expect("por producto");

        // El arroz primero: 12 lb × 120.00 = 1440.00, contra 250.02.
        assert_eq!(filas[0].nombre, "Arroz blanco");
        assert_eq!(filas[0].costo, "1440.00");
        assert_eq!(filas[1].nombre, "Refresco");
        assert_eq!(filas[1].costo, "250.02");
    }

    #[test]
    fn el_eje_por_producto_se_puede_mirar_de_una_sola_cuenta() {
        let (productos, consumos) = entorno();
        consumo_de_ivett(&productos, &consumos);
        RegistrarConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(comando(2, "Claudia", vec![linea(1, 10, "10")]))
            .expect("registrar");

        let filas = ConsultarConsumo::nuevo(&consumos)
            .por_producto(Some(2), None, None)
            .expect("por producto");

        assert_eq!(filas.len(), 1, "Claudia solo se llevó arroz");
        assert_eq!(filas[0].costo, "1200.00");
    }

    #[test]
    fn lo_anulado_no_cuenta_en_ningun_nivel() {
        let (productos, consumos) = entorno();
        let hecho = consumo_de_ivett(&productos, &consumos);

        AnularConsumo::nuevo(productos.as_ref(), &consumos)
            .ejecutar(ComandoAnularConsumo {
                id: hecho.id,
                motivo: "Error".to_owned(),
            })
            .expect("anular");

        let consulta = ConsultarConsumo::nuevo(&consumos);
        assert_eq!(consulta.panel().expect("panel").total, "0.00");
        assert!(consulta
            .por_producto(None, None, None)
            .expect("por producto")
            .is_empty());

        // Pero sigue a la vista, marcado.
        let detalle = consulta.cuenta(1, 10).expect("detalle");
        assert_eq!(detalle.consumos.len(), 1);
        assert!(detalle.consumos[0].anulado);
        assert_eq!(
            detalle.consumos[0].motivo_anulacion.as_deref(),
            Some("Error")
        );
    }
}
