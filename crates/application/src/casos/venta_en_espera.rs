//! Casos de uso: la venta en espera (RF-VTA-14).
//!
//! Apartar una venta a medio armar para atender a otro cliente, verlas,
//! retomar una y descartarla. Cobrarla no está aquí: es una venta como
//! cualquier otra y pasa por [`Vender`](crate::casos::Vender), que borra la
//! espera en la misma operación que registra la venta.
//!
//! Tres reglas atraviesan todos estos casos:
//!
//! - **No hace falta caja abierta para apartar.** Apartar no mueve dinero
//!   ni mercancía; cobrar sí, y por eso cobrar sí la exige.
//! - **No se reserva existencia.** La mercancía sigue en la vitrina.
//! - **No se congela nada.** Al retomar se calcula con el precio de hoy,
//!   con la misma cuenta que usa la venta en curso.

use domain::{Cantidad, IdPresentacion, IdProducto, IdVentaEnEspera, LineaEnEspera, VentaEnEspera};

use crate::casos::formatear_cantidad;
use crate::casos::previsualizar_venta::PrevisualizarVenta;
use crate::casos::vender::LineaPedida;
use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{ProductoConInventario, RepositorioProducto, RepositorioVentaEnEspera};

/// Lo que llega de la pantalla al pulsar «Pendiente».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoDejarEnEspera {
    /// Texto libre para reconocerla después. Opcional.
    pub nota: Option<String>,
    pub lineas: Vec<LineaPedida>,
}

/// Aparta la venta en curso.
#[derive(Debug)]
pub struct DejarVentaEnEspera<'a, P: RepositorioProducto, E: RepositorioVentaEnEspera> {
    productos: &'a P,
    esperas: &'a E,
}

impl<'a, P: RepositorioProducto, E: RepositorioVentaEnEspera> DejarVentaEnEspera<'a, P, E> {
    pub const fn nuevo(productos: &'a P, esperas: &'a E) -> Self {
        Self { productos, esperas }
    }

    /// Guarda la venta en espera y devuelve su identificador.
    ///
    /// Comprueba que cada renglón tenga sentido —el producto existe, la
    /// presentación es suya, la cantidad es admisible— porque retomar
    /// basura no le sirve a nadie. Lo que **no** comprueba es la
    /// existencia en vitrina: apartar no descuenta, y lo que falte se verá
    /// al retomar.
    pub fn ejecutar(&self, comando: ComandoDejarEnEspera) -> Resultado<i64> {
        let mut lineas = Vec::with_capacity(comando.lineas.len());

        for pedida in &comando.lineas {
            let ProductoConInventario { producto, .. } = self
                .productos
                .obtener(IdProducto(pedida.producto))?
                .ok_or(ErrorAplicacion::NoEncontrado {
                    entidad: "producto",
                    id: pedida.producto,
                })?;

            producto
                .presentacion(IdPresentacion(pedida.presentacion))
                .ok_or(ErrorAplicacion::Dominio(
                    domain::ErrorDominio::PresentacionNoEncontrada,
                ))?;

            let cantidad: Cantidad = pedida.cantidad.trim().parse()?;
            producto.validar_cantidad(cantidad)?;

            lineas.push(LineaEnEspera::nueva(
                IdProducto(pedida.producto),
                IdPresentacion(pedida.presentacion),
                cantidad,
            )?);
        }

        let espera = VentaEnEspera::nueva(comando.nota.as_deref(), lineas)?;
        Ok(self.esperas.guardar(&espera)?.0)
    }
}

/// Una venta en espera en la lista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VentaEnEsperaListada {
    pub id: i64,
    pub nota: Option<String>,
    /// Cuándo se apartó, como la guarda la base: `YYYY-MM-DD HH:MM:SS`.
    /// La antigüedad se enseña a partir de aquí.
    pub creada_en: String,
    pub cuantas_lineas: i64,
}

/// Lista las ventas en espera, de la más antigua a la más reciente.
#[derive(Debug)]
pub struct ListarVentasEnEspera<'a, E: RepositorioVentaEnEspera> {
    esperas: &'a E,
}

impl<'a, E: RepositorioVentaEnEspera> ListarVentasEnEspera<'a, E> {
    pub const fn nuevo(esperas: &'a E) -> Self {
        Self { esperas }
    }

    pub fn ejecutar(&self) -> Resultado<Vec<VentaEnEsperaListada>> {
        Ok(self
            .esperas
            .listar()?
            .into_iter()
            .map(|resumida| VentaEnEsperaListada {
                id: resumida.id.0,
                nota: resumida.nota,
                creada_en: resumida.creada_en,
                cuantas_lineas: resumida.cuantas_lineas,
            })
            .collect())
    }
}

/// Por qué un renglón retomado no se puede cobrar tal cual.
///
/// Retomar no falla por un renglón malo: carga la venta entera y marca lo
/// que hay que corregir. Perder toda la venta porque se desactivó un
/// producto sería castigar al cliente por un cambio de catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemaLinea {
    /// El producto ya no está en el catálogo.
    ProductoNoEncontrado,
    /// El producto se desactivó después de apartar la venta.
    ProductoInactivo,
    /// La presentación ya no pertenece al producto.
    PresentacionNoEncontrada,
    /// La presentación se desactivó después de apartar la venta.
    PresentacionInactiva,
    /// La cantidad apartada ya no es admisible para el producto.
    CantidadInvalida,
    /// La vitrina no da para este renglón, contando los anteriores.
    SinExistencia,
}

impl ProblemaLinea {
    /// Código estable, pensado para que la interfaz reaccione (DT-7).
    pub const fn codigo(&self) -> &'static str {
        match self {
            Self::ProductoNoEncontrado => "PRODUCTO_NO_ENCONTRADO",
            Self::ProductoInactivo => "PRODUCTO_INACTIVO",
            Self::PresentacionNoEncontrada => "PRESENTACION_NO_ENCONTRADA",
            Self::PresentacionInactiva => "PRESENTACION_INACTIVA",
            Self::CantidadInvalida => "CANTIDAD_INVALIDA",
            Self::SinExistencia => "SIN_EXISTENCIA",
        }
    }
}

impl core::fmt::Display for ProblemaLinea {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::ProductoNoEncontrado => "Este producto ya no existe",
            Self::ProductoInactivo => "Este producto se desactivó: ya no se vende",
            Self::PresentacionNoEncontrada => "Esta presentación ya no existe",
            Self::PresentacionInactiva => "Esta presentación se desactivó: ya no se vende",
            Self::CantidadInvalida => "La cantidad ya no es válida para este producto",
            Self::SinExistencia => "No hay suficiente en vitrina",
        })
    }
}

/// Un renglón retomado, calculado con el precio de hoy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaRetomada {
    pub producto: i64,
    pub presentacion: i64,
    /// Vacío si el producto ya no existe.
    pub nombre_producto: String,
    /// Vacío si la presentación ya no existe.
    pub nombre_presentacion: String,
    pub cantidad: String,
    /// Precio de hoy. Solo en los renglones que se pueden calcular.
    pub precio: Option<String>,
    /// Precio × cantidad.
    pub importe: Option<String>,
    /// Lo que saldría de la vitrina por este renglón.
    pub unidades_base: Option<String>,
    /// Qué impide cobrarlo, si algo lo impide.
    pub problema: Option<ProblemaLinea>,
}

/// Una venta en espera lista para volver a la pantalla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VentaRetomada {
    pub id: i64,
    pub nota: Option<String>,
    pub creada_en: String,
    pub lineas: Vec<LineaRetomada>,
    /// Suma de los renglones que se pueden calcular, al precio de hoy.
    pub total: String,
    /// Algún renglón hay que corregirlo antes de cobrar.
    pub hay_problemas: bool,
}

/// Recupera una venta en espera para seguir con ella.
///
/// **No la borra.** Sigue en la lista hasta que se cobra o se elimina: si
/// el cliente se arrepiente a medio camino, la venta no se pierde.
#[derive(Debug)]
pub struct RetomarVentaEnEspera<'a, P: RepositorioProducto, E: RepositorioVentaEnEspera> {
    productos: &'a P,
    esperas: &'a E,
}

impl<'a, P: RepositorioProducto, E: RepositorioVentaEnEspera> RetomarVentaEnEspera<'a, P, E> {
    pub const fn nuevo(productos: &'a P, esperas: &'a E) -> Self {
        Self { productos, esperas }
    }

    pub fn ejecutar(&self, id: i64) -> Resultado<VentaRetomada> {
        let registrada =
            self.esperas
                .obtener(IdVentaEnEspera(id))?
                .ok_or(ErrorAplicacion::Dominio(
                    domain::ErrorDominio::VentaEnEsperaNoEncontrada,
                ))?;

        let mut lineas = Vec::with_capacity(registrada.espera.lineas().len());
        // Los renglones sanos, con su posición, para calcularlos todos de
        // una vez con la misma cuenta que la venta en curso.
        let mut sanas: Vec<(usize, LineaPedida)> = Vec::new();

        for (posicion, apartada) in registrada.espera.lineas().iter().enumerate() {
            let linea = self.revisar(apartada)?;
            if linea.problema.is_none() {
                sanas.push((
                    posicion,
                    LineaPedida {
                        producto: linea.producto,
                        presentacion: linea.presentacion,
                        cantidad: apartada.cantidad().formatear(3),
                    },
                ));
            }
            lineas.push(linea);
        }

        // El precio, el importe y la existencia salen de la vista previa:
        // si mañana cambia cómo se calcula la venta en curso, lo retomado
        // cambia con ella y no hay dos cuentas que se contradigan.
        let (posiciones, pedidas): (Vec<usize>, Vec<LineaPedida>) = sanas.into_iter().unzip();
        let total = if pedidas.is_empty() {
            domain::Dinero::CERO.formatear(2)
        } else {
            let prevista = PrevisualizarVenta::nuevo(self.productos).ejecutar(pedidas)?;
            for (posicion, calculada) in posiciones.into_iter().zip(prevista.lineas) {
                if let Some(linea) = lineas.get_mut(posicion) {
                    linea.cantidad = calculada.cantidad;
                    linea.precio = Some(calculada.precio);
                    linea.importe = Some(calculada.importe);
                    linea.unidades_base = Some(calculada.unidades_base);
                    if calculada.sin_existencia {
                        linea.problema = Some(ProblemaLinea::SinExistencia);
                    }
                }
            }
            prevista.total
        };

        let hay_problemas = lineas.iter().any(|linea| linea.problema.is_some());

        Ok(VentaRetomada {
            id: registrada.id.0,
            nota: registrada.espera.nota().map(str::to_owned),
            creada_en: registrada.creada_en,
            lineas,
            total,
            hay_problemas,
        })
    }

    /// Mira si un renglón apartado se sigue pudiendo vender, sin precio
    /// todavía.
    fn revisar(&self, apartada: &LineaEnEspera) -> Resultado<LineaRetomada> {
        let mut linea = LineaRetomada {
            producto: apartada.producto().0,
            presentacion: apartada.presentacion().0,
            nombre_producto: String::new(),
            nombre_presentacion: String::new(),
            cantidad: apartada.cantidad().formatear(3),
            precio: None,
            importe: None,
            unidades_base: None,
            problema: None,
        };

        let producto = match self.productos.obtener(apartada.producto())? {
            Some(ProductoConInventario { producto, .. }) => producto,
            None => {
                linea.problema = Some(ProblemaLinea::ProductoNoEncontrado);
                return Ok(linea);
            }
        };

        linea.nombre_producto = producto.nombre().to_owned();
        linea.cantidad = formatear_cantidad(apartada.cantidad(), &producto);

        let presentacion = producto.presentacion(apartada.presentacion());
        if let Some(presentacion) = presentacion {
            linea.nombre_presentacion = presentacion.nombre().to_owned();
        }

        linea.problema = if !producto.esta_activo() {
            Some(ProblemaLinea::ProductoInactivo)
        } else {
            match presentacion {
                None => Some(ProblemaLinea::PresentacionNoEncontrada),
                Some(presentacion) if !presentacion.esta_activa() => {
                    Some(ProblemaLinea::PresentacionInactiva)
                }
                Some(_) => producto
                    .validar_cantidad(apartada.cantidad())
                    .err()
                    .map(|_| ProblemaLinea::CantidadInvalida),
            }
        };

        Ok(linea)
    }
}

/// Descarta una venta en espera sin cobrarla.
#[derive(Debug)]
pub struct EliminarVentaEnEspera<'a, E: RepositorioVentaEnEspera> {
    esperas: &'a E,
}

impl<'a, E: RepositorioVentaEnEspera> EliminarVentaEnEspera<'a, E> {
    pub const fn nuevo(esperas: &'a E) -> Self {
        Self { esperas }
    }

    pub fn ejecutar(&self, id: i64) -> Resultado<()> {
        if self.esperas.eliminar(IdVentaEnEspera(id))? {
            Ok(())
        } else {
            Err(ErrorAplicacion::Dominio(
                domain::ErrorDominio::VentaEnEsperaNoEncontrada,
            ))
        }
    }
}

#[cfg(test)]
mod pruebas {
    use domain::UnidadBase;

    use super::*;
    use crate::dobles::{EsperasEnMemoria, ProductosEnMemoria};

    /// Refresco suelto a 80 y en six-pack a 300, con 20 latas en vitrina;
    /// y arroz por libra a 180, con 10 libras.
    fn catalogo() -> ProductosEnMemoria {
        ProductosEnMemoria::default()
            .con_producto(
                1,
                "Refresco 500 ml",
                UnidadBase::Unidad,
                &[
                    (10, "Unidad", "1", "80.00"),
                    (11, "Six-pack", "6", "300.00"),
                ],
                "20",
            )
            .con_producto(
                2,
                "Arroz",
                UnidadBase::Libra,
                &[(20, "Libra", "1", "180.00")],
                "10",
            )
    }

    fn cantidad(texto: &str) -> Cantidad {
        texto.parse().expect("cantidad válida")
    }

    fn pedida(producto: i64, presentacion: i64, cantidad: &str) -> LineaPedida {
        LineaPedida {
            producto,
            presentacion,
            cantidad: cantidad.to_owned(),
        }
    }

    fn dejar(
        productos: &ProductosEnMemoria,
        esperas: &EsperasEnMemoria,
        nota: Option<&str>,
        lineas: Vec<LineaPedida>,
    ) -> Resultado<i64> {
        DejarVentaEnEspera::nuevo(productos, esperas).ejecutar(ComandoDejarEnEspera {
            nota: nota.map(str::to_owned),
            lineas,
        })
    }

    #[test]
    fn apartar_no_exige_caja_abierta_ni_toca_la_vitrina() {
        // El catálogo de prueba no tiene caja: si apartar la exigiera,
        // esto fallaría con SIN_SESION_ABIERTA.
        let productos = catalogo();
        let esperas = EsperasEnMemoria::default();

        let id = dejar(
            &productos,
            &esperas,
            Some("  la señora del pan "),
            vec![pedida(1, 11, "2"), pedida(2, 20, "1.5")],
        )
        .expect("apartar");

        let guardada = esperas
            .obtener(IdVentaEnEspera(id))
            .expect("leer")
            .expect("existe");
        assert_eq!(guardada.espera.nota(), Some("la señora del pan"));
        assert_eq!(guardada.espera.lineas().len(), 2);
        assert_eq!(guardada.espera.lineas()[1].cantidad(), cantidad("1.5"));

        // Nada salió de la vitrina: apartar no reserva.
        let refresco = productos
            .obtener(IdProducto(1))
            .expect("leer")
            .expect("existe");
        assert_eq!(refresco.inventario.existencias().vitrina(), cantidad("20"));
    }

    #[test]
    fn no_se_aparta_una_venta_vacia() {
        let error = dejar(&catalogo(), &EsperasEnMemoria::default(), None, Vec::new())
            .expect_err("sin renglones no hay espera");

        assert_eq!(error.codigo(), "ESPERA_VACIA");
    }

    #[test]
    fn no_se_aparta_lo_que_no_tiene_sentido() {
        let productos = catalogo();
        let esperas = EsperasEnMemoria::default();

        let casos = [
            (pedida(99, 10, "1"), "NO_ENCONTRADO"),
            (pedida(1, 20, "1"), "PRESENTACION_NO_ENCONTRADA"),
            (pedida(1, 10, "0"), "CANTIDAD_NO_POSITIVA"),
            (pedida(1, 10, "1.5"), "CANTIDAD_FRACCIONARIA_NO_PERMITIDA"),
            (pedida(1, 10, "tres"), "TEXTO_NUMERICO_INVALIDO"),
        ];

        for (linea, codigo) in casos {
            let error = dejar(&productos, &esperas, None, vec![linea]).expect_err("debe fallar");
            assert_eq!(error.codigo(), codigo);
        }

        // Ninguno de los intentos dejó nada guardado.
        assert!(esperas.listar().expect("listar").is_empty());
    }

    #[test]
    fn apartar_no_mira_la_existencia() {
        // 25 latas con solo 20 en vitrina: se aparta igual, y el faltante
        // se avisará al retomar.
        let productos = catalogo();
        let esperas = EsperasEnMemoria::default();

        assert!(dejar(&productos, &esperas, None, vec![pedida(1, 10, "25")]).is_ok());
    }

    #[test]
    fn la_nota_demasiado_larga_se_rechaza() {
        let larga = "a".repeat(domain::LARGO_MAXIMO_NOTA.saturating_add(1));
        let error = dejar(
            &catalogo(),
            &EsperasEnMemoria::default(),
            Some(&larga),
            vec![pedida(1, 10, "1")],
        )
        .expect_err("la nota se pasa");

        assert_eq!(error.codigo(), "NOTA_DEMASIADO_LARGA");
    }

    #[test]
    fn se_pueden_tener_varias_a_la_vez_y_se_listan_en_orden() {
        let productos = catalogo();
        let esperas = EsperasEnMemoria::default();

        let primera = dejar(
            &productos,
            &esperas,
            Some("mesa 1"),
            vec![pedida(1, 10, "1")],
        )
        .expect("apartar");
        let segunda = dejar(
            &productos,
            &esperas,
            None,
            vec![pedida(1, 10, "1"), pedida(2, 20, "2")],
        )
        .expect("apartar");

        let lista = ListarVentasEnEspera::nuevo(&esperas)
            .ejecutar()
            .expect("listar");

        assert_eq!(
            lista,
            vec![
                VentaEnEsperaListada {
                    id: primera,
                    nota: Some("mesa 1".to_owned()),
                    creada_en: "2026-09-27 10:15:00".to_owned(),
                    cuantas_lineas: 1,
                },
                VentaEnEsperaListada {
                    id: segunda,
                    nota: None,
                    creada_en: "2026-09-27 10:15:00".to_owned(),
                    cuantas_lineas: 2,
                },
            ]
        );
    }

    #[test]
    fn retomar_calcula_con_el_precio_de_hoy_y_no_borra_la_espera() {
        let productos = catalogo();
        let esperas = EsperasEnMemoria::default();
        let id = dejar(
            &productos,
            &esperas,
            Some("mesa 3"),
            vec![pedida(1, 11, "2"), pedida(2, 20, "1.5")],
        )
        .expect("apartar");

        let retomada = RetomarVentaEnEspera::nuevo(&productos, &esperas)
            .ejecutar(id)
            .expect("retomar");

        assert_eq!(retomada.id, id);
        assert_eq!(retomada.nota.as_deref(), Some("mesa 3"));
        assert!(!retomada.hay_problemas);
        // 2 six-packs a 300 más 1,5 lb a 180.
        assert_eq!(retomada.total, "870.00");

        let six_pack = &retomada.lineas[0];
        assert_eq!(six_pack.nombre_producto, "Refresco 500 ml");
        assert_eq!(six_pack.nombre_presentacion, "Six-pack");
        assert_eq!(six_pack.cantidad, "2");
        assert_eq!(six_pack.precio.as_deref(), Some("300.00"));
        assert_eq!(six_pack.importe.as_deref(), Some("600.00"));
        assert_eq!(six_pack.unidades_base.as_deref(), Some("12"));
        assert_eq!(six_pack.problema, None);

        assert_eq!(retomada.lineas[1].cantidad, "1.500");

        // Sigue en la lista hasta que se cobre o se elimine.
        assert_eq!(esperas.listar().expect("listar").len(), 1);
    }

    #[test]
    fn retomar_marca_los_renglones_malos_sin_perder_la_venta() {
        let productos = catalogo()
            .con_producto(
                3,
                "Galletas",
                UnidadBase::Unidad,
                &[(30, "Paquete", "1", "50.00")],
                "5",
            )
            .con_producto(
                4,
                "Aceite",
                UnidadBase::Unidad,
                &[
                    (40, "Botella", "1", "400.00"),
                    (41, "Caja", "12", "4500.00"),
                ],
                "30",
            )
            .con_producto(
                5,
                "Jabón",
                UnidadBase::Unidad,
                &[(50, "Unidad", "1", "60.00")],
                "3",
            );
        let esperas = EsperasEnMemoria::default();
        let id = dejar(
            &productos,
            &esperas,
            None,
            // Uno sano; después, uno cuyo producto se desactiva, otro cuya
            // presentación se desactiva, otro cuyo producto desaparece y
            // uno para el que no alcanza la vitrina.
            vec![
                pedida(1, 10, "1"),
                pedida(3, 30, "1"),
                pedida(4, 41, "1"),
                pedida(5, 50, "1"),
                pedida(1, 10, "25"),
            ],
        )
        .expect("apartar");

        productos.desactivar_producto(3);
        productos.desactivar_presentacion(4, 41);
        productos.quitar_producto(5);

        let retomada = RetomarVentaEnEspera::nuevo(&productos, &esperas)
            .ejecutar(id)
            .expect("retomar no falla por un renglón malo");

        let problemas: Vec<Option<&str>> = retomada
            .lineas
            .iter()
            .map(|linea| linea.problema.map(|p| p.codigo()))
            .collect();
        assert_eq!(
            problemas,
            [
                None,
                Some("PRODUCTO_INACTIVO"),
                Some("PRESENTACION_INACTIVA"),
                Some("PRODUCTO_NO_ENCONTRADO"),
                Some("SIN_EXISTENCIA"),
            ]
        );
        assert!(retomada.hay_problemas);

        // Lo inactivo conserva el nombre para que el aviso diga de qué se
        // trata; lo desaparecido ya no tiene nombre ni precio.
        assert_eq!(retomada.lineas[1].nombre_producto, "Galletas");
        assert_eq!(retomada.lineas[1].precio, None);
        assert_eq!(retomada.lineas[2].nombre_presentacion, "Caja");
        assert!(retomada.lineas[3].nombre_producto.is_empty());
        assert_eq!(retomada.lineas[3].cantidad, "1.000");

        // El faltante se calcula igual que en la venta en curso: 1 + 25
        // latas compiten por 20, y se sigue sabiendo cuánto costaría.
        assert_eq!(retomada.lineas[4].importe.as_deref(), Some("2000.00"));
        assert_eq!(retomada.total, "2080.00");
    }

    #[test]
    fn retomar_una_espera_que_no_existe_lo_dice() {
        let error = RetomarVentaEnEspera::nuevo(&catalogo(), &EsperasEnMemoria::default())
            .ejecutar(42)
            .expect_err("no hay nada que retomar");

        assert_eq!(error.codigo(), "VENTA_EN_ESPERA_NO_ENCONTRADA");
    }

    #[test]
    fn eliminar_la_quita_de_la_lista_y_no_se_elimina_dos_veces() {
        let productos = catalogo();
        let esperas = EsperasEnMemoria::default();
        let id = dejar(&productos, &esperas, None, vec![pedida(1, 10, "1")]).expect("apartar");

        EliminarVentaEnEspera::nuevo(&esperas)
            .ejecutar(id)
            .expect("eliminar");
        assert!(esperas.listar().expect("listar").is_empty());

        let error = EliminarVentaEnEspera::nuevo(&esperas)
            .ejecutar(id)
            .expect_err("ya no está");
        assert_eq!(error.codigo(), "VENTA_EN_ESPERA_NO_ENCONTRADA");
    }

    #[test]
    fn cobrar_desde_una_espera_le_pasa_su_id_al_repositorio() {
        use crate::casos::vender::{ComandoVender, PagoPedido, Vender};

        let productos = catalogo().con_caja_abierta();
        let cobrar = |espera_id| {
            Vender::nuevo(&productos)
                .ejecutar(ComandoVender {
                    lineas: vec![pedida(1, 10, "1")],
                    pagos: vec![PagoPedido {
                        metodo: "EFECTIVO_CUP".to_owned(),
                        entregado: "80.00".to_owned(),
                    }],
                    espera_id,
                })
                .expect("cobrar")
        };

        cobrar(Some(7));
        assert_eq!(
            *productos.espera_cobrada.borrow(),
            Some(Some(IdVentaEnEspera(7)))
        );

        // Una venta normal no consume ninguna espera.
        cobrar(None);
        assert_eq!(*productos.espera_cobrada.borrow(), Some(None));
    }
}
