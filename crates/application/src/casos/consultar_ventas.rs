//! Casos de uso: qué se vendió (RF-VTA-16).
//!
//! Dos preguntas distintas, y conviene no mezclarlas:
//!
//! - **¿Cómo va el día?** Un par de cifras que se miran de reojo: cuántas
//!   ventas, cuánto entró, cuánto se ganó.
//! - **¿Qué pasó en esta venta?** El detalle de un cobro concreto, con sus
//!   líneas y las formas en que se pagó.
//!
//! La ganancia no se guarda: se deduce restando el costo al total. Y ese
//! costo es el que tenía la mercancía **cuando se vendió**, congelado en su
//! línea (RF-VTA-13). Recalcularlo con el costo de hoy haría que la
//! ganancia de ayer cambiara sola cada vez que llega una remesa.

use domain::{Cantidad, Dinero};

use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{DetalleVenta, RepositorioProducto, VentaRegistrada};

/// Cuántas ventas se devuelven si no se pide otra cosa.
pub const LIMITE_POR_DEFECTO: usize = 100;

/// Una venta en la lista, resumida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VentaListada {
    pub id: i64,
    pub folio: i64,
    /// Se anuló: sigue en el historial pero no cuenta para nada.
    pub anulada: bool,
    pub total: String,
    /// Total menos costo congelado.
    pub ganancia: String,
    /// Fecha y hora tal como la guardó la base.
    pub ocurrido_en: String,
    /// Solo la hora, que es lo que distingue una venta de otra en el día.
    pub hora: String,
    pub fecha: String,
}

/// Lo vendido hoy, para la cabecera de la pantalla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumenDelDia {
    pub cuantas: i64,
    pub total: String,
    pub ganancia: String,
}

/// La pantalla de ventas, de una sola consulta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistorialVentas {
    pub hoy: ResumenDelDia,
    pub ventas: Vec<VentaListada>,
}

/// Una línea de una venta ya cobrada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaVendida {
    pub producto: i64,
    pub nombre_producto: String,
    pub nombre_presentacion: String,
    pub cantidad: String,
    pub precio: String,
    pub importe: String,
    /// Lo que costó la mercancía que salió por esta línea.
    pub costo: String,
    pub ganancia: String,
}

/// Una forma en que se pagó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagoHecho {
    pub metodo: String,
    pub metodo_nombre: String,
    /// Lo que entregó el cliente, en su moneda.
    pub entregado: String,
    pub moneda: String,
    /// Tasa congelada, solo en los pagos en dólares.
    pub tasa: Option<String>,
    /// Cuánto valió en pesos.
    pub equivalente: String,
}

/// Una venta con todo su detalle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VentaDetallada {
    pub id: i64,
    pub folio: i64,
    /// Se anuló: sigue en el historial, pero no cuenta para nada.
    pub anulada: bool,
    pub motivo_anulacion: Option<String>,
    pub total: String,
    pub costo_total: String,
    pub ganancia: String,
    pub vuelto: String,
    pub entregado: String,
    pub ocurrido_en: String,
    pub fecha: String,
    pub hora: String,
    pub lineas: Vec<LineaVendida>,
    pub pagos: Vec<PagoHecho>,
}

/// Consulta el historial de ventas.
#[derive(Debug)]
pub struct ConsultarVentas<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> ConsultarVentas<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, limite: usize) -> Resultado<HistorialVentas> {
        let resumen = self.repositorio.resumen_de_hoy()?;
        let ventas = self.repositorio.listar_ventas(limite)?;

        let listadas = ventas
            .into_iter()
            .map(|venta| {
                let VentaRegistrada {
                    id,
                    folio,
                    total,
                    costo_total,
                    ocurrido_en,
                    anulada,
                    ..
                } = venta;

                Ok(VentaListada {
                    id,
                    folio,
                    anulada,
                    total: total.formatear(2),
                    ganancia: total.restar(costo_total)?.formatear(2),
                    fecha: fecha_de(&ocurrido_en).to_owned(),
                    hora: hora_de(&ocurrido_en).to_owned(),
                    ocurrido_en,
                })
            })
            .collect::<Resultado<Vec<_>>>()?;

        Ok(HistorialVentas {
            hoy: ResumenDelDia {
                cuantas: resumen.cuantas,
                total: resumen.total.formatear(2),
                ganancia: resumen.total.restar(resumen.costo_total)?.formatear(2),
            },
            ventas: listadas,
        })
    }
}

/// Consulta una venta concreta.
#[derive(Debug)]
pub struct ConsultarVenta<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> ConsultarVenta<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, id: i64) -> Resultado<VentaDetallada> {
        let DetalleVenta {
            venta,
            lineas,
            pagos,
        } = self
            .repositorio
            .detalle_venta(id)?
            .ok_or(ErrorAplicacion::NoEncontrado {
                entidad: "venta",
                id,
            })?;

        let vendidas = lineas
            .into_iter()
            .map(|linea| {
                let importe = linea.precio.multiplicar_por(linea.cantidad)?;
                // El costo se cuenta sobre unidades base: una caja de 24 se
                // llevó 24 veces el costo unitario, no una.
                let unidades = linea.cantidad.multiplicar_por_factor(linea.factor)?;
                let costo = linea.costo_unitario.multiplicar_por(unidades)?;

                Ok(LineaVendida {
                    producto: linea.producto.0,
                    nombre_producto: linea.nombre_producto,
                    nombre_presentacion: linea.nombre_presentacion,
                    cantidad: formatear_cantidad_vendida(linea.cantidad),
                    precio: linea.precio.formatear(2),
                    importe: importe.formatear(2),
                    costo: costo.formatear(2),
                    ganancia: importe.restar(costo)?.formatear(2),
                })
            })
            .collect::<Resultado<Vec<_>>>()?;

        let mut entregado = Dinero::CERO;
        let mut hechos = Vec::with_capacity(pagos.len());

        for pago in pagos {
            entregado = entregado.sumar(pago.equivalente_cup)?;

            hechos.push(PagoHecho {
                metodo: pago.metodo.como_texto().to_owned(),
                metodo_nombre: pago.metodo.nombre().to_owned(),
                entregado: pago.entregado.formatear(2),
                moneda: pago.metodo.moneda().simbolo().to_owned(),
                tasa: pago.tasa.map(|t| t.formatear(2)),
                equivalente: pago.equivalente_cup.formatear(2),
            });
        }

        Ok(VentaDetallada {
            id: venta.id,
            folio: venta.folio,
            anulada: venta.anulada,
            motivo_anulacion: venta.motivo_anulacion.clone(),
            total: venta.total.formatear(2),
            costo_total: venta.costo_total.formatear(2),
            ganancia: venta.total.restar(venta.costo_total)?.formatear(2),
            vuelto: venta.vuelto.formatear(2),
            entregado: entregado.formatear(2),
            fecha: fecha_de(&venta.ocurrido_en).to_owned(),
            hora: hora_de(&venta.ocurrido_en).to_owned(),
            ocurrido_en: venta.ocurrido_en,
            lineas: vendidas,
            pagos: hechos,
        })
    }
}

/// Parte de fecha de un `YYYY-MM-DD HH:MM:SS`.
fn fecha_de(sello: &str) -> &str {
    sello.split(' ').next().unwrap_or(sello)
}

/// Parte de hora, sin segundos: a nadie le importa el segundo en que cobró.
fn hora_de(sello: &str) -> &str {
    let resto = sello.split(' ').nth(1).unwrap_or("");
    resto.get(..5).unwrap_or(resto)
}

/// Formatea la cantidad vendida sin arrastrar ceros inútiles.
///
/// Aquí no se tiene el producto delante para saber si era a granel, así que
/// se deduce de la propia cifra: si es entera, se escribe entera.
fn formatear_cantidad_vendida(cantidad: Cantidad) -> String {
    if cantidad.es_entera() {
        cantidad.formatear(0)
    } else {
        cantidad.formatear(3)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_fecha_y_la_hora_salen_del_sello_de_la_base() {
        assert_eq!(fecha_de("2026-09-23 19:41:10"), "2026-09-23");
        assert_eq!(hora_de("2026-09-23 19:41:10"), "19:41");
    }

    #[test]
    fn un_sello_raro_no_revienta() {
        assert_eq!(fecha_de("2026-09-23"), "2026-09-23");
        assert_eq!(hora_de("2026-09-23"), "");
    }

    #[test]
    fn las_cantidades_enteras_no_arrastran_decimales() {
        let tres: Cantidad = "3".parse().expect("cantidad válida");
        let media: Cantidad = "1.5".parse().expect("cantidad válida");

        assert_eq!(formatear_cantidad_vendida(tres), "3");
        assert_eq!(formatear_cantidad_vendida(media), "1.500");
    }
}
