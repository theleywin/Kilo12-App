//! Casos de uso de la sesión de caja (RF-CAJ).
//!
//! El turno de caja es lo que convierte un montón de ventas sueltas en algo
//! arqueable: sin él no hay contra qué contar los billetes al final del día.
//!
//! Todo lo que se devuelve aquí llega formateado. La pantalla de caja es la
//! que más números enseña de toda la aplicación, y ni uno solo se calcula en
//! JavaScript: un céntimo de diferencia por redondeo aquí es media hora
//! buscando un descuadre que no existe.

use domain::{
    contar_billetes, ArqueoMoneda, Comision, Dinero, ErrorDominio, IdSesion, ModoCierre,
    MovimientoEfectivo, Porcentaje, ResumenCierre, SesionCaja, TasaCambio, TipoMovimientoEfectivo,
    TotalesCaja, DENOMINACIONES_CUP,
};

use crate::casos::vender::CLAVE_TASA;
use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{AcumuladoSesion, CierreConfirmado, RepositorioProducto, SesionRegistrada};

/// Clave con que se guarda el porcentaje de comisión del operador.
pub const CLAVE_COMISION: &str = "comision_operador";

/// Cuántas sesiones se devuelven si no se pide otra cosa.
pub const LIMITE_POR_DEFECTO: usize = 60;

// ===================================================== lo que se devuelve

/// Un movimiento de efectivo, listo para mostrar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovimientoEfectivoListado {
    pub id: i64,
    /// `ENTRADA` o `SALIDA`.
    pub tipo: String,
    pub tipo_nombre: String,
    pub suma: bool,
    pub importe: String,
    pub motivo: String,
    pub ocurrido_en: String,
    pub hora: String,
}

/// Desglose de la venta por forma de pago (RF-CAJ-05c).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesgloseVenta {
    pub efectivo_cup: String,
    pub transferencia: String,
    /// Dólares recibidos, en dólares.
    pub efectivo_usd: String,
    /// Lo que valen esos dólares en pesos.
    pub efectivo_usd_en_cup: String,
    /// Pesos más transferencia.
    pub total_en_pesos: String,
    /// Con los dólares convertidos y sumados.
    pub total_consolidado: String,
}

/// La caja tal como está ahora mismo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstadoCaja {
    pub id: i64,
    pub operador: String,
    pub abierta_en: String,
    pub fondo_inicial: String,
    pub cuantas_ventas: i64,
    pub desglose: DesgloseVenta,
    pub entradas: String,
    pub salidas: String,
    /// Billetes de peso que debería haber ahora mismo (RF-CAJ-04).
    pub efectivo_esperado: String,
    /// Dólares que debería haber, en dólares.
    pub dolares_esperados: String,
    pub movimientos: Vec<MovimientoEfectivoListado>,
}

/// El resumen económico de la sesión (RF-CAJ-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumenEconomico {
    pub venta_total: String,
    pub costo_vendido: String,
    pub merma: String,
    pub ganancia_bruta: String,
    pub comision_porcentaje: String,
    pub comision: String,
    pub ganancia_neta: String,
}

/// El arqueo de una moneda, ya comparado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArqueoListado {
    pub esperado: String,
    pub contado: String,
    /// Positiva es sobrante; negativa, faltante.
    pub diferencia: String,
    pub cuadra: bool,
    pub sobra: bool,
}

/// El cierre, calculado o ya guardado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CierreCalculado {
    pub sesion: i64,
    pub operador: String,
    pub abierta_en: String,
    pub cerrada_en: Option<String>,
    /// `SEPARADO` o `CONSOLIDADO`.
    pub modo: String,
    pub fondo_inicial: String,
    /// Billetes de peso que dejaron las ventas: lo entregado en efectivo
    /// menos el vuelto devuelto. **No** es la venta cobrada en efectivo.
    pub ventas_efectivo: String,
    pub desglose: DesgloseVenta,
    pub entradas: String,
    pub salidas: String,
    pub arqueo_cup: ArqueoListado,
    pub arqueo_usd: ArqueoListado,
    pub cuadra: bool,
    pub economico: ResumenEconomico,
}

/// Una sesión en el historial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SesionListada {
    pub id: i64,
    pub operador: String,
    pub abierta_en: String,
    pub cerrada_en: Option<String>,
    pub fecha: String,
    pub abierta: bool,
}

// ========================================================== abrir la caja

/// Lo que llega de la pantalla al abrir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoAbrirCaja {
    pub operador: String,
    /// Billetes con que empieza la gaveta.
    pub fondo_inicial: String,
}

/// Abre la sesión de caja (RF-CAJ-01).
#[derive(Debug)]
pub struct AbrirCaja<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> AbrirCaja<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoAbrirCaja) -> Resultado<i64> {
        let fondo: Dinero = comando.fondo_inicial.trim().parse()?;
        let sesion = SesionCaja::abrir(comando.operador.trim(), fondo)?;

        Ok(self.repositorio.abrir_sesion(&sesion)?.0)
    }
}

// ================================================ mirar la caja en vivo

/// Consulta la caja abierta (RF-CAJ-04).
#[derive(Debug)]
pub struct ConsultarCaja<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> ConsultarCaja<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    /// Devuelve la caja abierta, o nada si no hay ninguna.
    ///
    /// «No hay caja abierta» no es un error: es el estado normal antes de
    /// empezar el día, y la pantalla tiene que poder dibujarlo.
    pub fn ejecutar(&self) -> Resultado<Option<EstadoCaja>> {
        let Some(registrada) = self.repositorio.sesion_abierta()? else {
            return Ok(None);
        };

        let id = identificador(&registrada)?;
        let acumulado = self.repositorio.acumulado_de_sesion(id)?;
        let totales = totales_de(&acumulado)?;
        let fondo = registrada.sesion.fondo_inicial();

        // El mismo cálculo que hará el cierre, para que lo que se ve
        // durante el turno y lo que sale al cerrar no puedan discrepar.
        let esperado = fondo
            .sumar(totales.efectivo_cup_en_caja())?
            .sumar(acumulado.entradas)?
            .restar(acumulado.salidas)?;

        let movimientos = self
            .repositorio
            .movimientos_efectivo(id)?
            .into_iter()
            .map(|registrado| {
                let movimiento = &registrado.movimiento;
                MovimientoEfectivoListado {
                    id: registrado.id,
                    tipo: movimiento.tipo().como_texto().to_owned(),
                    tipo_nombre: movimiento.tipo().nombre().to_owned(),
                    suma: movimiento.tipo().suma(),
                    importe: movimiento.importe().formatear(2),
                    motivo: movimiento.motivo().to_owned(),
                    hora: hora_de(&registrado.ocurrido_en).to_owned(),
                    ocurrido_en: registrado.ocurrido_en,
                }
            })
            .collect();

        Ok(Some(EstadoCaja {
            id: id.0,
            operador: registrada.sesion.operador().to_owned(),
            abierta_en: registrada.abierta_en,
            fondo_inicial: fondo.formatear(2),
            cuantas_ventas: acumulado.cuantas_ventas,
            desglose: desglose_de(&totales)?,
            entradas: acumulado.entradas.formatear(2),
            salidas: acumulado.salidas.formatear(2),
            efectivo_esperado: esperado.formatear(2),
            dolares_esperados: totales.efectivo_usd().formatear(2),
            movimientos,
        }))
    }
}

// ============================================ mover efectivo a mano

/// Lo que llega al registrar una entrada o salida de efectivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoMoverEfectivo {
    /// `ENTRADA` o `SALIDA`.
    pub tipo: String,
    pub importe: String,
    /// Obligatorio: sin él, un retiro es indistinguible de un faltante.
    pub motivo: String,
}

/// Registra un movimiento de efectivo ajeno a la venta (RF-CAJ-03).
#[derive(Debug)]
pub struct MoverEfectivo<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> MoverEfectivo<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoMoverEfectivo) -> Resultado<()> {
        let registrada = self.exigir_abierta()?;
        let id = identificador(&registrada)?;

        let tipo: TipoMovimientoEfectivo = comando.tipo.trim().parse()?;
        let importe: Dinero = comando.importe.trim().parse()?;
        let movimiento = MovimientoEfectivo::nuevo(tipo, importe, comando.motivo.trim())?;

        self.repositorio
            .registrar_movimiento_efectivo(id, &movimiento)
    }

    fn exigir_abierta(&self) -> Resultado<SesionRegistrada> {
        self.repositorio
            .sesion_abierta()?
            .ok_or(ErrorAplicacion::Dominio(ErrorDominio::SinSesionAbierta))
    }
}

// ======================================== contar los billetes

/// Un renglón del recuento: cuántos billetes de una denominación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaConteo {
    /// Valor del billete, en pesos enteros.
    pub denominacion: i64,
    pub cuantos: i64,
    /// Lo que suman esos billetes.
    pub importe: String,
}

/// El recuento completo, ya sumado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConteoCalculado {
    pub lineas: Vec<LineaConteo>,
    pub total: String,
    /// Cuántos billetes hay en total. Ayuda a detectar un cero de más.
    pub cuantos_billetes: i64,
}

/// Cuenta los billetes de la gaveta (RF-CAJ-05).
///
/// La cajera teclea cuántos billetes tiene de cada valor y el total sale de
/// aquí. Sumar doce productos en la pantalla sería tentador y también sería
/// la única cifra del cierre calculada fuera del núcleo.
#[derive(Debug)]
pub struct ContarEfectivo;

impl ContarEfectivo {
    /// Denominaciones con que se cuenta, de menor a mayor.
    pub const fn denominaciones() -> [i64; 12] {
        DENOMINACIONES_CUP
    }

    /// Suma un recuento.
    ///
    /// `cuantos` llega en el mismo orden que [`Self::denominaciones`]. Un
    /// hueco vacío cuenta como cero: la cajera no tiene por qué escribir
    /// ceros en los billetes que no tiene.
    pub fn ejecutar(cuantos: &[i64]) -> Resultado<ConteoCalculado> {
        let recuento: Vec<(i64, i64)> = DENOMINACIONES_CUP
            .iter()
            .enumerate()
            .map(|(indice, valor)| (*valor, cuantos.get(indice).copied().unwrap_or(0)))
            .collect();

        let total = contar_billetes(&recuento)?;

        let lineas = recuento
            .iter()
            .map(|(valor, cuantos)| {
                Ok(LineaConteo {
                    denominacion: *valor,
                    cuantos: *cuantos,
                    importe: Dinero::desde_unidades(valor * cuantos)?.formatear(2),
                })
            })
            .collect::<Resultado<Vec<_>>>()?;

        Ok(ConteoCalculado {
            cuantos_billetes: recuento.iter().map(|(_, cuantos)| cuantos).sum(),
            lineas,
            total: total.formatear(2),
        })
    }
}

// ================================================== cerrar y previsualizar

/// Lo que llega de la pantalla de cierre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoCerrarCaja {
    /// Billetes de peso contados físicamente.
    pub contado_cup: String,
    /// Dólares contados, en dólares.
    pub contado_usd: String,
    /// `SEPARADO` o `CONSOLIDADO` (RF-CAJ-05b).
    pub modo: String,
}

/// Calcula y confirma el cierre de la sesión (RF-CAJ-05).
#[derive(Debug)]
pub struct CerrarCaja<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> CerrarCaja<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    /// Enseña cómo quedaría el cierre sin cerrar nada.
    ///
    /// Usa exactamente el mismo cálculo que [`Self::ejecutar`]: la vista
    /// previa y el cierre no pueden decir cosas distintas, porque quien
    /// cuenta los billetes decide mirando la primera.
    pub fn previsualizar(&self, comando: &ComandoCerrarCaja) -> Resultado<CierreCalculado> {
        let (registrada, cierre, acumulado) = self.calcular(comando)?;
        presentar_cierre(&registrada, &cierre, None, &acumulado)
    }

    pub fn ejecutar(&self, comando: ComandoCerrarCaja) -> Resultado<CierreCalculado> {
        let (registrada, cierre, acumulado) = self.calcular(&comando)?;
        self.repositorio.cerrar_sesion(&cierre)?;

        // Se relee para devolver la hora de cierre que puso la base, en vez
        // de inventarla aquí con otro reloj.
        let guardada = self
            .repositorio
            .sesion(cierre.sesion)?
            .unwrap_or(registrada);
        let cerrada = guardada.cerrada_en.clone();

        presentar_cierre(&guardada, &cierre, cerrada, &acumulado)
    }

    fn calcular(
        &self,
        comando: &ComandoCerrarCaja,
    ) -> Resultado<(SesionRegistrada, CierreConfirmado, AcumuladoSesion)> {
        let registrada = self
            .repositorio
            .sesion_abierta()?
            .ok_or(ErrorAplicacion::Dominio(ErrorDominio::SinSesionAbierta))?;

        let id = identificador(&registrada)?;
        let acumulado = self.repositorio.acumulado_de_sesion(id)?;
        let totales = totales_de(&acumulado)?;

        let contado_cup: Dinero = comando.contado_cup.trim().parse()?;
        let contado_usd: Dinero = comando.contado_usd.trim().parse()?;
        // El cierre va siempre consolidado: la venta total es la suma de
        // las tres formas de cobro con los dólares ya convertidos. El modo
        // separado sigue existiendo porque las sesiones cerradas con él
        // deben poder leerse tal como se cerraron (RF-CAJ-08), pero ya no
        // se elige al cerrar.
        let modo = match comando.modo.trim() {
            "SEPARADO" => ModoCierre::Separado,
            _ => ModoCierre::Consolidado,
        };

        let resumen = ResumenCierre::calcular(
            totales,
            modo,
            registrada.sesion.fondo_inicial(),
            acumulado.entradas,
            acumulado.salidas,
            contado_cup,
            contado_usd,
        )?;

        // La base de la comisión es la venta total, no la ganancia: así el
        // operador verifica su importe sin ver los costos de compra (D-6).
        let comision = Comision::calcular(acumulado.total_vendido, self.porcentaje_comision()?)?;

        let confirmado = CierreConfirmado {
            sesion: id,
            resumen,
            costo_vendido: acumulado.costo_vendido,
            merma_costo: acumulado.merma_costo,
            comision,
        };

        Ok((registrada, confirmado, acumulado))
    }

    /// Porcentaje de comisión vigente, o cero si no se ha configurado.
    ///
    /// Que no esté puesto no puede impedir cerrar la caja: contar el dinero
    /// es lo importante, y una comisión de cero es una respuesta honesta.
    fn porcentaje_comision(&self) -> Resultado<i64> {
        let Some(guardado) = self.repositorio.configuracion(CLAVE_COMISION)? else {
            return Ok(0);
        };

        let porcentaje: Porcentaje = guardado.trim().parse()?;
        Ok(porcentaje.diezmilesimas())
    }
}

// ========================================================== el historial

/// Consulta las sesiones y sus cierres (RF-CAJ-07).
#[derive(Debug)]
pub struct HistorialCajas<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> HistorialCajas<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn listar(&self, limite: usize) -> Resultado<Vec<SesionListada>> {
        Ok(self
            .repositorio
            .listar_sesiones(limite)?
            .into_iter()
            .map(|registrada| SesionListada {
                id: registrada.sesion.id().map_or(0, |id| id.0),
                operador: registrada.sesion.operador().to_owned(),
                fecha: fecha_de(&registrada.abierta_en).to_owned(),
                abierta: registrada.sesion.esta_abierta(),
                abierta_en: registrada.abierta_en,
                cerrada_en: registrada.cerrada_en,
            })
            .collect())
    }

    /// Devuelve el cierre congelado de una sesión.
    ///
    /// No se recalcula nada: se lee lo que se guardó al cerrar. Si el dueño
    /// cambió la tasa o la comisión después, este cierre sigue diciendo lo
    /// que se pactó entonces (RF-CAJ-08).
    pub fn cierre(&self, id: i64) -> Resultado<CierreCalculado> {
        let sesion = IdSesion(id);

        let registrada = self
            .repositorio
            .sesion(sesion)?
            .ok_or(ErrorAplicacion::NoEncontrado {
                entidad: "sesión de caja",
                id,
            })?;

        let cierre = self
            .repositorio
            .cierre_de_sesion(sesion)?
            .ok_or(ErrorAplicacion::Dominio(ErrorDominio::SesionCerrada))?;

        // Las entradas y salidas se resuman de sus movimientos, que en una
        // sesión cerrada ya no cambian: no hay riesgo de que el histórico
        // se mueva bajo los pies.
        let acumulado = self.repositorio.acumulado_de_sesion(sesion)?;
        let cerrada = registrada.cerrada_en.clone();

        presentar_cierre(&registrada, &cierre, cerrada, &acumulado)
    }
}

// ============================================================= ayudantes

fn identificador(registrada: &SesionRegistrada) -> Resultado<IdSesion> {
    registrada
        .sesion
        .id()
        .ok_or(ErrorAplicacion::Dominio(ErrorDominio::SinSesionAbierta))
}

fn totales_de(acumulado: &AcumuladoSesion) -> Resultado<TotalesCaja> {
    TotalesCaja::nuevos(
        acumulado.total_vendido,
        acumulado.transferencia,
        acumulado.efectivo_usd,
        acumulado.efectivo_usd_en_cup,
        acumulado.efectivo_cup_neto,
    )
    .map_err(ErrorAplicacion::from)
}

fn desglose_de(totales: &TotalesCaja) -> Resultado<DesgloseVenta> {
    Ok(DesgloseVenta {
        efectivo_cup: totales.efectivo_cup().formatear(2),
        transferencia: totales.transferencia().formatear(2),
        efectivo_usd: totales.efectivo_usd().formatear(2),
        efectivo_usd_en_cup: totales.efectivo_usd_en_cup().formatear(2),
        total_en_pesos: totales.total_en_cup()?.formatear(2),
        total_consolidado: totales.total_consolidado()?.formatear(2),
    })
}

fn arqueo_de(arqueo: &ArqueoMoneda) -> Resultado<ArqueoListado> {
    let diferencia = arqueo.diferencia()?;

    Ok(ArqueoListado {
        esperado: arqueo.esperado.formatear(2),
        contado: arqueo.contado.formatear(2),
        diferencia: diferencia.formatear(2),
        cuadra: diferencia.es_cero(),
        sobra: diferencia.es_positivo(),
    })
}

/// Arma la vista del cierre, tanto el calculado como el ya guardado.
fn presentar_cierre(
    registrada: &SesionRegistrada,
    cierre: &CierreConfirmado,
    cerrada_en: Option<String>,
    acumulado: &AcumuladoSesion,
) -> Resultado<CierreCalculado> {
    let resumen = &cierre.resumen;
    let totales = &resumen.totales;

    // Lo que las ventas dejaron en billetes se deduce del esperado en vez
    // de recalcularlo: así el desglose cuadra con la cifra contra la que se
    // arquea, tanto en la vista previa como al releer una caja cerrada.
    let fondo = registrada.sesion.fondo_inicial();
    let ventas_efectivo = resumen
        .arqueo_cup
        .esperado
        .restar(fondo)?
        .restar(acumulado.entradas)?
        .sumar(acumulado.salidas)?;

    let venta_total = resumen.total_vendido()?;
    let ganancia_bruta = venta_total.restar(cierre.costo_vendido)?;
    let ganancia_neta = ganancia_bruta
        .restar(cierre.merma_costo)?
        .restar(cierre.comision.importe())?;

    Ok(CierreCalculado {
        sesion: cierre.sesion.0,
        operador: registrada.sesion.operador().to_owned(),
        abierta_en: registrada.abierta_en.clone(),
        cerrada_en,
        modo: match resumen.modo {
            ModoCierre::Separado => "SEPARADO".to_owned(),
            ModoCierre::Consolidado => "CONSOLIDADO".to_owned(),
        },
        fondo_inicial: fondo.formatear(2),
        ventas_efectivo: ventas_efectivo.formatear(2),
        desglose: desglose_de(totales)?,
        entradas: acumulado.entradas.formatear(2),
        salidas: acumulado.salidas.formatear(2),
        arqueo_cup: arqueo_de(&resumen.arqueo_cup)?,
        arqueo_usd: arqueo_de(&resumen.arqueo_usd)?,
        cuadra: resumen.cuadra()?,
        economico: ResumenEconomico {
            venta_total: venta_total.formatear(2),
            costo_vendido: cierre.costo_vendido.formatear(2),
            merma: cierre.merma_costo.formatear(2),
            ganancia_bruta: ganancia_bruta.formatear(2),
            comision_porcentaje: Porcentaje::desde_diezmilesimas(cierre.comision.porcentaje())
                .formatear(2),
            comision: cierre.comision.importe().formatear(2),
            ganancia_neta: ganancia_neta.formatear(2),
        },
    })
}

/// Tasa vigente, solo para presentar el cierre consolidado.
pub fn tasa_para_cierre<R: RepositorioProducto>(repositorio: &R) -> Resultado<Option<TasaCambio>> {
    let Some(guardada) = repositorio.configuracion(CLAVE_TASA)? else {
        return Ok(None);
    };

    let cup_por_usd: Dinero = guardada.trim().parse()?;
    Ok(Some(TasaCambio::nueva(cup_por_usd)?))
}

/// Parte de fecha de un `YYYY-MM-DD HH:MM:SS`.
fn fecha_de(sello: &str) -> &str {
    sello.split(' ').next().unwrap_or(sello)
}

/// Parte de hora, sin segundos.
fn hora_de(sello: &str) -> &str {
    let resto = sello.split(' ').nth(1).unwrap_or("");
    resto.get(..5).unwrap_or(resto)
}
