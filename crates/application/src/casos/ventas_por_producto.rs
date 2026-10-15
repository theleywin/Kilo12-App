//! Caso de uso: lo vendido en la sesión, agrupado por producto (RF-VTA-16).
//!
//! La lista de ventas dice qué cobros hubo; esta vista dice **qué salió**.
//! Una fila por cada presentación vendida, y cada presentación cuenta como
//! un producto distinto: el refresco suelto y el six-pack son dos filas y
//! no se suman entre sí.
//!
//! «Hoy» es la sesión de caja de referencia, igual que en las tarjetas de la
//! cabecera: la abierta, o la última cerrada si no hay ninguna.
//!
//! Se agrupa por el identificador de la presentación, no por su nombre, y
//! se enseña el nombre **congelado** en la venta más reciente. Así, si el
//! producto se renombró a media sesión, sus ventas no se parten en dos filas
//! y la fila dice cómo se llama ahora. Y un producto que ya no está en el
//! catálogo sigue apareciendo: lo que se vendió, se vendió.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use domain::IdSesion;

use crate::casos::agregados::agregar_por;
use crate::casos::caja::{sesion_de_referencia, SesionDeReferencia};
use crate::casos::consultar_ventas::formatear_cantidad_vendida;
use crate::error::Resultado;
use crate::puertos::{ConsultaVentasDeSesion, LineaDelPeriodo, RepositorioProducto};

// ==================================================== lo que se devuelve

/// Una presentación de un producto vendida en la sesión.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilaVendida {
    pub producto: i64,
    /// Nombre del producto congelado en la venta más reciente que lo llevó.
    pub nombre_producto: String,
    pub presentacion: i64,
    /// Nombre de la presentación congelado en la venta más reciente.
    pub nombre_presentacion: String,
    /// Lo que salió, en presentaciones: cajas, si es la caja.
    pub cantidad: String,
    pub total: String,
    pub ganancia: String,
}

/// La vista agrupada completa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VentasPorProducto {
    /// A qué sesión se refiere. Vacía si nunca se abrió una caja.
    pub sesion: Option<SesionDeReferencia>,
    /// De la fila que más dinero dejó entrar a la que menos.
    pub filas: Vec<FilaVendida>,
}

// ========================================================= el caso de uso

/// Agrupa por producto y presentación lo vendido en la sesión de referencia.
#[derive(Debug)]
pub struct ConsultarVentasPorProducto<'a, P: RepositorioProducto, V: ConsultaVentasDeSesion> {
    productos: &'a P,
    ventas: &'a V,
}

impl<'a, P: RepositorioProducto, V: ConsultaVentasDeSesion> ConsultarVentasPorProducto<'a, P, V> {
    pub const fn nuevo(productos: &'a P, ventas: &'a V) -> Self {
        Self { productos, ventas }
    }

    pub fn ejecutar(&self) -> Resultado<VentasPorProducto> {
        let Some(sesion) = sesion_de_referencia(self.productos)? else {
            return Ok(VentasPorProducto {
                sesion: None,
                filas: Vec::new(),
            });
        };

        let lineas = self.ventas.lineas_de_sesion(IdSesion(sesion.id))?;

        Ok(VentasPorProducto {
            sesion: Some(sesion),
            filas: agrupar(&lineas)?,
        })
    }
}

// ============================================================ agrupación

/// Los nombres congelados en la venta más reciente que se ha visto.
#[derive(Debug, Clone, Copy)]
struct Reciente<'l> {
    venta: i64,
    producto: &'l str,
    presentacion: &'l str,
}

fn agrupar(lineas: &[LineaDelPeriodo]) -> Resultado<Vec<FilaVendida>> {
    // La aritmética de dinero es la misma que la del informe; solo cambia
    // la clave.
    let movidos = agregar_por(lineas, |l| (l.producto.0, l.presentacion.0))?;

    let mut nombres: BTreeMap<(i64, i64), Reciente<'_>> = BTreeMap::new();
    for linea in lineas {
        let visto = Reciente {
            venta: linea.venta,
            producto: &linea.nombre_producto,
            presentacion: &linea.nombre_presentacion,
        };
        let reciente = nombres
            .entry((linea.producto.0, linea.presentacion.0))
            .or_insert(visto);
        // Con `>=` gana el último renglón leído dentro de la misma venta,
        // así que el resultado no depende del orden en que lleguen.
        if visto.venta >= reciente.venta {
            *reciente = visto;
        }
    }

    let mut filas = movidos
        .iter()
        .map(|(&(producto, presentacion), movido)| {
            let nombre = nombres.get(&(producto, presentacion));
            Ok((
                movido.importe,
                FilaVendida {
                    producto,
                    nombre_producto: nombre.map_or_else(String::new, |r| r.producto.to_owned()),
                    presentacion,
                    nombre_presentacion: nombre
                        .map_or_else(String::new, |r| r.presentacion.to_owned()),
                    cantidad: formatear_cantidad_vendida(movido.cantidad),
                    total: movido.importe.formatear(2),
                    ganancia: movido.ganancia()?.formatear(2),
                },
            ))
        })
        .collect::<Resultado<Vec<_>>>()?;

    // Se ordena por el importe, no por su texto: comparar cadenas pondría
    // «9.00» por encima de «100.00».
    filas.sort_by(|(a_importe, a), (b_importe, b)| {
        Reverse(a_importe)
            .cmp(&Reverse(b_importe))
            .then_with(|| a.nombre_producto.cmp(&b.nombre_producto))
            .then_with(|| a.nombre_presentacion.cmp(&b.nombre_presentacion))
            .then_with(|| a.producto.cmp(&b.producto))
            .then_with(|| a.presentacion.cmp(&b.presentacion))
    });

    Ok(filas.into_iter().map(|(_, fila)| fila).collect())
}

#[cfg(test)]
mod pruebas {
    use domain::{IdPresentacion, IdProducto, UnidadBase};

    use super::*;
    use crate::dobles::{ProductosEnMemoria, VentasDeSesionEnMemoria};

    /// Un renglón vendido: `(producto, presentación)`, sus nombres
    /// congelados, cantidad, factor, precio y costo unitario.
    fn linea(
        venta: i64,
        (producto, presentacion): (i64, i64),
        (nombre_producto, nombre_presentacion): (&str, &str),
        cantidad: &str,
        factor: &str,
        precio: &str,
        costo: &str,
    ) -> LineaDelPeriodo {
        LineaDelPeriodo {
            venta,
            producto: IdProducto(producto),
            presentacion: IdPresentacion(presentacion),
            nombre_producto: nombre_producto.to_owned(),
            nombre_presentacion: nombre_presentacion.to_owned(),
            cantidad: cantidad.parse().expect("cantidad"),
            factor: factor.parse().expect("factor"),
            precio: precio.parse().expect("precio"),
            costo_unitario: costo.parse().expect("costo"),
        }
    }

    /// Refresco suelto a 80 y en six-pack a 300 (costo 40 la lata), y arroz
    /// por libra a 180 (costo 100). Caja 7 abierta.
    fn catalogo() -> ProductosEnMemoria {
        ProductosEnMemoria::default()
            .con_producto(
                1,
                "Refresco",
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
            .con_sesion_abierta(7, "2026-09-28 08:00:00")
    }

    const REFRESCO: (&str, &str) = ("Refresco", "Unidad");
    const SIXPACK: (&str, &str) = ("Refresco", "Six-pack");
    const ARROZ: (&str, &str) = ("Arroz", "Libra");

    fn consultar(
        productos: &ProductosEnMemoria,
        ventas: &VentasDeSesionEnMemoria,
    ) -> VentasPorProducto {
        ConsultarVentasPorProducto::nuevo(productos, ventas)
            .ejecutar()
            .expect("consultar")
    }

    fn por_presentacion(vista: &VentasPorProducto, id: i64) -> &FilaVendida {
        vista
            .filas
            .iter()
            .find(|f| f.presentacion == id)
            .expect("presentación")
    }

    #[test]
    fn cada_presentacion_es_su_propia_fila() {
        let productos = catalogo();
        let ventas = VentasDeSesionEnMemoria::default()
            // Venta 1: 2 latas sueltas y un six-pack.
            .con_linea(7, linea(1, (1, 10), REFRESCO, "2", "1", "80.00", "40.00"))
            .con_linea(7, linea(1, (1, 11), SIXPACK, "1", "6", "300.00", "40.00"))
            // Venta 2: otras 3 latas.
            .con_linea(7, linea(2, (1, 10), REFRESCO, "3", "1", "80.00", "40.00"));

        let vista = consultar(&productos, &ventas);

        // Dos filas del mismo producto, sin nada que las sume.
        assert_eq!(vista.filas.len(), 2);

        // Sueltas: 5 latas, 400 de venta, 400 − 5 × 40 = 200 de ganancia.
        let sueltas = &vista.filas[0];
        assert_eq!(sueltas.producto, 1);
        assert_eq!(sueltas.nombre_producto, "Refresco");
        assert_eq!(sueltas.presentacion, 10);
        assert_eq!(sueltas.nombre_presentacion, "Unidad");
        assert_eq!(sueltas.cantidad, "5");
        assert_eq!(sueltas.total, "400.00");
        assert_eq!(sueltas.ganancia, "200.00");

        // Six-pack: 1 caja de 6, 300 − 6 × 40 = 60. La salida va en cajas.
        let caja = &vista.filas[1];
        assert_eq!(caja.producto, 1);
        assert_eq!(caja.nombre_producto, "Refresco");
        assert_eq!(caja.presentacion, 11);
        assert_eq!(caja.cantidad, "1");
        assert_eq!(caja.total, "300.00");
        assert_eq!(caja.ganancia, "60.00");
    }

    #[test]
    fn varios_renglones_de_la_misma_presentacion_se_suman_en_una_fila() {
        let productos = catalogo();
        let ventas = VentasDeSesionEnMemoria::default()
            // La misma venta con dos renglones de la misma presentación.
            .con_linea(7, linea(1, (1, 10), REFRESCO, "1", "1", "80.00", "40.00"))
            .con_linea(7, linea(1, (1, 10), REFRESCO, "1", "1", "80.00", "40.00"))
            .con_linea(7, linea(2, (1, 10), REFRESCO, "2", "1", "80.00", "40.00"));

        let vista = consultar(&productos, &ventas);

        assert_eq!(vista.filas.len(), 1);
        assert_eq!(vista.filas[0].cantidad, "4");
        assert_eq!(vista.filas[0].total, "320.00");
    }

    #[test]
    fn las_filas_se_ordenan_por_total_sin_agrupar_por_producto() {
        let productos = catalogo();
        let ventas = VentasDeSesionEnMemoria::default()
            // Refresco suelto: 80.
            .con_linea(7, linea(1, (1, 10), REFRESCO, "1", "1", "80.00", "40.00"))
            // Six-pack: 2 × 300 = 600.
            .con_linea(7, linea(2, (1, 11), SIXPACK, "2", "6", "300.00", "40.00"))
            // Arroz: 2 × 180 = 360.
            .con_linea(7, linea(3, (2, 20), ARROZ, "2", "1", "180.00", "100.00"));

        let vista = consultar(&productos, &ventas);

        // El arroz queda entre las dos presentaciones del refresco: manda
        // el total de cada fila, no el del producto.
        let orden: Vec<(&str, &str)> = vista
            .filas
            .iter()
            .map(|f| (f.nombre_producto.as_str(), f.nombre_presentacion.as_str()))
            .collect();
        assert_eq!(orden, [SIXPACK, ARROZ, REFRESCO]);
    }

    #[test]
    fn a_igual_total_desempata_el_nombre_y_no_el_texto_del_importe() {
        let productos = catalogo();
        let ventas = VentasDeSesionEnMemoria::default()
            // 9.00 y 100.00: como texto, «9.00» iría primero.
            .con_linea(7, linea(1, (1, 10), REFRESCO, "1", "1", "9.00", "1.00"))
            .con_linea(7, linea(2, (2, 20), ARROZ, "1", "1", "100.00", "1.00"))
            // Mismo total que el arroz: desempata el nombre del producto.
            .con_linea(7, linea(3, (1, 11), SIXPACK, "1", "6", "100.00", "1.00"));

        let vista = consultar(&productos, &ventas);

        let presentaciones: Vec<i64> = vista.filas.iter().map(|f| f.presentacion).collect();
        assert_eq!(presentaciones, [20, 11, 10]);
    }

    #[test]
    fn lo_vendido_a_granel_se_suma_con_decimales() {
        let productos = catalogo();
        let ventas = VentasDeSesionEnMemoria::default()
            .con_linea(7, linea(1, (2, 20), ARROZ, "1.5", "1", "180.00", "100.00"))
            .con_linea(7, linea(2, (2, 20), ARROZ, "2", "1", "180.00", "100.00"));

        let arroz = &consultar(&productos, &ventas).filas[0];

        assert_eq!(arroz.cantidad, "3.500");
        // 3.5 × 180 = 630; 630 − 3.5 × 100 = 280.
        assert_eq!(arroz.total, "630.00");
        assert_eq!(arroz.ganancia, "280.00");
    }

    #[test]
    fn un_producto_que_ya_no_esta_en_el_catalogo_sigue_apareciendo() {
        let productos = catalogo();
        productos.quitar_producto(2);
        let ventas = VentasDeSesionEnMemoria::default()
            .con_linea(7, linea(1, (2, 20), ARROZ, "2", "1", "180.00", "100.00"));

        let vista = consultar(&productos, &ventas);

        assert_eq!(vista.filas.len(), 1);
        let arroz = &vista.filas[0];
        // Con sus nombres congelados.
        assert_eq!(arroz.nombre_producto, "Arroz");
        assert_eq!(arroz.nombre_presentacion, "Libra");
        assert_eq!(arroz.cantidad, "2");
        assert_eq!(arroz.total, "360.00");
    }

    #[test]
    fn si_se_renombro_se_ensena_el_nombre_de_la_venta_mas_reciente() {
        let productos = catalogo();
        // Llegan desordenadas a propósito: manda la venta, no el orden.
        let ventas = VentasDeSesionEnMemoria::default()
            .con_linea(
                7,
                linea(
                    5,
                    (1, 10),
                    ("Refresco cola", "Lata"),
                    "1",
                    "1",
                    "80.00",
                    "40.00",
                ),
            )
            .con_linea(7, linea(2, (1, 10), REFRESCO, "1", "1", "80.00", "40.00"));

        let vista = consultar(&productos, &ventas);

        // Una sola fila: se agrupa por identificador, no por nombre.
        assert_eq!(vista.filas.len(), 1);
        let fila = por_presentacion(&vista, 10);
        assert_eq!(fila.nombre_producto, "Refresco cola");
        assert_eq!(fila.nombre_presentacion, "Lata");
        assert_eq!(fila.cantidad, "2");
    }

    #[test]
    fn solo_cuenta_la_sesion_de_referencia() {
        let productos = catalogo();
        let ventas = VentasDeSesionEnMemoria::default()
            .con_linea(6, linea(1, (2, 20), ARROZ, "9", "1", "180.00", "100.00"))
            .con_linea(7, linea(2, (1, 10), REFRESCO, "1", "1", "80.00", "40.00"));

        let vista = consultar(&productos, &ventas);

        assert_eq!(vista.filas.len(), 1);
        assert_eq!(vista.filas[0].nombre_producto, "Refresco");
    }

    #[test]
    fn con_la_caja_cerrada_se_ensena_la_ultima_sesion_cerrada() {
        let productos = ProductosEnMemoria::default()
            .con_sesion_cerrada(3, "2026-09-26 08:00:00", "2026-09-26 20:00:00")
            .con_sesion_cerrada(4, "2026-09-27 08:00:00", "2026-09-27 20:30:00");
        let ventas = VentasDeSesionEnMemoria::default()
            .con_linea(3, linea(1, (2, 20), ARROZ, "1", "1", "180.00", "100.00"))
            .con_linea(4, linea(2, (1, 10), REFRESCO, "2", "1", "80.00", "40.00"));

        let vista = consultar(&productos, &ventas);

        assert_eq!(
            vista.sesion,
            Some(SesionDeReferencia {
                id: 4,
                abierta: false,
                abierta_en: "2026-09-27 08:00:00".to_owned(),
                cerrada_en: Some("2026-09-27 20:30:00".to_owned()),
            })
        );
        assert_eq!(vista.filas.len(), 1);
        assert_eq!(vista.filas[0].nombre_producto, "Refresco");
        assert_eq!(vista.filas[0].cantidad, "2");
    }

    #[test]
    fn la_sesion_abierta_manda_sobre_las_cerradas() {
        let productos =
            catalogo().con_sesion_cerrada(6, "2026-09-27 08:00:00", "2026-09-27 20:30:00");

        let vista = consultar(&productos, &VentasDeSesionEnMemoria::default());

        let sesion = vista.sesion.expect("hay sesión");
        assert_eq!(sesion.id, 7);
        assert!(sesion.abierta);
        assert_eq!(sesion.cerrada_en, None);
        assert!(vista.filas.is_empty());
    }

    #[test]
    fn sin_ninguna_sesion_no_hay_nada_que_agrupar_y_no_es_un_error() {
        let productos = ProductosEnMemoria::default();
        let ventas = VentasDeSesionEnMemoria::default()
            .con_linea(1, linea(1, (1, 10), REFRESCO, "1", "1", "80.00", "40.00"));

        let vista = consultar(&productos, &ventas);

        assert_eq!(vista.sesion, None);
        assert!(vista.filas.is_empty());
    }
}
