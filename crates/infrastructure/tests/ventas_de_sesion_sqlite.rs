//! Las ventas de la sesión contra SQLite de verdad (RF-VTA-16).
//!
//! «Hoy» en la pantalla de ventas es el turno de caja, no el calendario.
//! Lo que se mide aquí no se puede medir con dobles: que la consulta se
//! acote por `venta.sesion_id` y no por la fecha, que las anuladas se
//! queden fuera y que las tarjetas digan lo mismo que el arqueo.

use std::sync::Arc;

use application::casos::vender::CLAVE_TASA;
use application::casos::{
    AbrirCaja, AgregarPresentacion, AnularVenta, CerrarCaja, ComandoAbrirCaja,
    ComandoAgregarPresentacion, ComandoAnularVenta, ComandoCerrarCaja, ComandoRegistrarProducto,
    ComandoVender, ConsultarCaja, ConsultarProducto, ConsultarVentas, ConsultarVentasPorProducto,
    HistorialCajas, HistorialVentas, LineaPedida, PagoPedido, RegistrarProducto, Vender,
    VentasPorProducto,
};
use application::puertos::RepositorioProducto;
use domain::Dinero;
use infrastructure::{BaseDatos, RepositorioProductoSqlite};

/// El repositorio y la base, para poder retocar fechas a mano.
struct Entorno {
    base: Arc<BaseDatos>,
    repositorio: RepositorioProductoSqlite,
}

impl Entorno {
    fn nuevo() -> Self {
        let base = Arc::new(BaseDatos::en_memoria().expect("abrir la base en memoria"));
        Self {
            repositorio: RepositorioProductoSqlite::nuevo(Arc::clone(&base)),
            base,
        }
    }

    fn ejecutar_sql(&self, sql: &str) {
        self.base
            .con(|conexion| Ok(conexion.execute_batch(sql)?))
            .expect("retocar la base");
    }

    fn abrir_caja(&self) -> i64 {
        AbrirCaja::nuevo(&self.repositorio)
            .ejecutar(ComandoAbrirCaja {
                operador: "Yaneisy".to_owned(),
                fondo_inicial: "500.00".to_owned(),
            })
            .expect("abrir la caja")
    }

    fn cerrar_caja(&self) -> application::casos::CierreCalculado {
        CerrarCaja::nuevo(&self.repositorio)
            .ejecutar(ComandoCerrarCaja {
                contado_cup: "0.00".to_owned(),
                contado_usd: "0.00".to_owned(),
                modo: String::new(),
            })
            .expect("cerrar la caja")
    }

    /// Da de alta un producto con 100 unidades en vitrina y devuelve su id
    /// y el de su presentación predeterminada.
    fn producto(&self, nombre: &str, precio: &str, costo: &str) -> (i64, i64) {
        let producto = RegistrarProducto::nuevo(&self.repositorio)
            .ejecutar(ComandoRegistrarProducto {
                sku: String::new(),
                nombre: nombre.to_owned(),
                unidad_base: "unidad".to_owned(),
                precio_unitario: precio.to_owned(),
                costo_unitario: Some(costo.to_owned()),
                cantidad_almacen: None,
                cantidad_vitrina: Some("100".to_owned()),
                stock_minimo: None,
                objetivo_vitrina: None,
            })
            .expect("registrar")
            .0;

        let ficha = ConsultarProducto::nuevo(&self.repositorio)
            .ejecutar(producto)
            .expect("ficha");
        (producto, ficha.presentaciones[0].id)
    }

    /// Añade una presentación y devuelve su id.
    fn presentacion(&self, producto: i64, nombre: &str, factor: &str, precio: &str) -> i64 {
        AgregarPresentacion::nuevo(&self.repositorio)
            .ejecutar(ComandoAgregarPresentacion {
                producto,
                nombre: nombre.to_owned(),
                factor: factor.to_owned(),
                precio: precio.to_owned(),
                codigo_barras: None,
            })
            .expect("agregar presentación");

        ConsultarProducto::nuevo(&self.repositorio)
            .ejecutar(producto)
            .expect("ficha")
            .presentaciones
            .iter()
            .find(|p| p.nombre == nombre)
            .expect("la presentación nueva")
            .id
    }

    /// Cobra una venta en efectivo pagando de sobra y devuelve su id.
    fn vender(&self, lineas: &[(i64, i64, &str)]) -> i64 {
        self.cobrar(
            lineas,
            vec![PagoPedido {
                metodo: "EFECTIVO_CUP".to_owned(),
                entregado: "100000.00".to_owned(),
            }],
        )
    }

    fn cobrar(&self, lineas: &[(i64, i64, &str)], pagos: Vec<PagoPedido>) -> i64 {
        let folio = Vender::nuevo(&self.repositorio)
            .ejecutar(ComandoVender {
                lineas: lineas
                    .iter()
                    .map(|(producto, presentacion, cantidad)| LineaPedida {
                        producto: *producto,
                        presentacion: *presentacion,
                        cantidad: (*cantidad).to_owned(),
                    })
                    .collect(),
                pagos,
                espera_id: None,
            })
            .expect("cobrar")
            .folio;

        self.base
            .con(|conexion| {
                Ok(
                    conexion.query_row("SELECT id FROM venta WHERE folio = ?1", [folio], |f| {
                        f.get(0)
                    })?,
                )
            })
            .expect("id de la venta")
    }

    fn historial(&self) -> HistorialVentas {
        ConsultarVentas::nuevo(&self.repositorio)
            .ejecutar(100)
            .expect("consultar las ventas")
    }

    fn agrupadas(&self) -> VentasPorProducto {
        ConsultarVentasPorProducto::nuevo(&self.repositorio, &self.repositorio)
            .ejecutar()
            .expect("agrupar")
    }
}

fn dinero(texto: &str) -> Dinero {
    texto.parse().expect("importe")
}

/// Suma las filas de la vista agrupada. El núcleo no da este total: se
/// calcula aquí solo para comprobar que cuadra con las tarjetas.
fn total_agrupado(vista: &VentasPorProducto) -> Dinero {
    vista
        .filas
        .iter()
        .try_fold(Dinero::CERO, |suma, f| suma.sumar(dinero(&f.total)))
        .expect("sumar")
}

#[test]
fn sin_ninguna_caja_no_hay_sesion_y_no_es_un_error() {
    let entorno = Entorno::nuevo();

    let historial = entorno.historial();
    assert_eq!(historial.sesion, None);
    assert_eq!(historial.hoy.cuantas, 0);
    assert_eq!(historial.hoy.total, "0.00");

    let vista = entorno.agrupadas();
    assert_eq!(vista.sesion, None);
    assert!(vista.filas.is_empty());
}

#[test]
fn las_ventas_de_otra_sesion_no_aparecen() {
    let entorno = Entorno::nuevo();
    let (arroz, libra) = entorno.producto("Arroz", "180.00", "100.00");
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00");

    entorno.abrir_caja();
    entorno.vender(&[(arroz, libra, "5")]);
    entorno.cerrar_caja();

    let segunda = entorno.abrir_caja();
    entorno.vender(&[(refresco, lata, "2")]);

    let historial = entorno.historial();
    let sesion = historial.sesion.expect("hay sesión");
    assert_eq!(sesion.id, segunda);
    assert!(sesion.abierta);
    assert_eq!(historial.hoy.cuantas, 1);
    assert_eq!(historial.hoy.total, "160.00");
    assert_eq!(historial.hoy.ganancia, "80.00");
    // La lista sigue siendo el historial completo: las dos ventas.
    assert_eq!(historial.ventas.len(), 2);

    let vista = entorno.agrupadas();
    let nombres: Vec<&str> = vista
        .filas
        .iter()
        .map(|f| f.nombre_producto.as_str())
        .collect();
    assert_eq!(nombres, ["Refresco"]);
}

#[test]
fn una_sesion_que_cruza_la_medianoche_incluye_las_ventas_de_ayer() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00");

    entorno.abrir_caja();
    let de_anoche = entorno.vender(&[(refresco, lata, "1")]);
    entorno.vender(&[(refresco, lata, "2")]);

    // La caja se abrió ayer a las 22:00 y la primera venta fue a las 23:50.
    entorno.ejecutar_sql(&format!(
        "UPDATE sesion_caja SET abierta_en = datetime('now', 'localtime', 'start of day', '-2 hours');
         UPDATE venta SET ocurrido_en = datetime('now', 'localtime', 'start of day', '-10 minutes')
          WHERE id = {de_anoche};"
    ));

    let historial = entorno.historial();
    // Con el corte por calendario saldría 1; con el turno son las dos.
    assert_eq!(historial.hoy.cuantas, 2);
    assert_eq!(historial.hoy.total, "240.00");

    let vista = entorno.agrupadas();
    assert_eq!(vista.filas.len(), 1);
    assert_eq!(vista.filas[0].cantidad, "3");
    assert_eq!(vista.filas[0].total, "240.00");
}

#[test]
fn las_anuladas_no_cuentan_ni_en_las_tarjetas_ni_agrupadas() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00");
    let (arroz, libra) = entorno.producto("Arroz", "180.00", "100.00");

    entorno.abrir_caja();
    entorno.vender(&[(refresco, lata, "1")]);
    let anulada = entorno.vender(&[(refresco, lata, "4"), (arroz, libra, "2")]);

    AnularVenta::nuevo(&entorno.repositorio)
        .ejecutar(ComandoAnularVenta {
            venta: anulada,
            motivo: "error de cobro".to_owned(),
        })
        .expect("anular");

    let historial = entorno.historial();
    assert_eq!(historial.hoy.cuantas, 1);
    assert_eq!(historial.hoy.total, "80.00");

    let vista = entorno.agrupadas();
    assert_eq!(vista.filas.len(), 1);
    let refresco = &vista.filas[0];
    assert_eq!(refresco.nombre_producto, "Refresco");
    assert_eq!(refresco.cantidad, "1");
    assert_eq!(refresco.total, "80.00");
}

#[test]
fn con_la_caja_cerrada_se_ve_la_ultima_sesion() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00");

    let sesion = entorno.abrir_caja();
    entorno.vender(&[(refresco, lata, "3")]);
    entorno.cerrar_caja();

    let historial = entorno.historial();
    let referencia = historial.sesion.expect("la última cerrada");
    assert_eq!(referencia.id, sesion);
    assert!(!referencia.abierta);
    assert!(referencia.cerrada_en.is_some());
    assert_eq!(historial.hoy.total, "240.00");

    let vista = entorno.agrupadas();
    assert_eq!(vista.sesion, Some(referencia));
    assert_eq!(vista.filas[0].total, "240.00");
}

#[test]
fn cada_presentacion_es_una_fila_ordenada_por_total() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00");
    let caja = entorno.presentacion(refresco, "Six-pack", "6", "420.00");

    entorno.abrir_caja();
    entorno.vender(&[(refresco, lata, "2"), (refresco, caja, "1")]);
    entorno.vender(&[(refresco, caja, "2")]);

    let vista = entorno.agrupadas();
    assert_eq!(vista.filas.len(), 2);

    // Six-pack: 3 cajas, 1 260; la salida va en cajas, no en latas.
    let caja_vendida = &vista.filas[0];
    assert_eq!(caja_vendida.presentacion, caja);
    assert_eq!(caja_vendida.nombre_producto, "Refresco");
    assert_eq!(caja_vendida.nombre_presentacion, "Six-pack");
    assert_eq!(caja_vendida.cantidad, "3");
    assert_eq!(caja_vendida.total, "1260.00");
    // 1 260 − 18 × 40 = 540
    assert_eq!(caja_vendida.ganancia, "540.00");

    // Lata: 2, 160 − 2 × 40 = 80.
    let sueltas = &vista.filas[1];
    assert_eq!(sueltas.presentacion, lata);
    assert_eq!(sueltas.nombre_producto, "Refresco");
    assert_eq!(sueltas.nombre_presentacion, "Unidad");
    assert_eq!(sueltas.cantidad, "2");
    assert_eq!(sueltas.total, "160.00");
    assert_eq!(sueltas.ganancia, "80.00");
}

/// Las tarjetas, la vista agrupada y la caja cuentan lo mismo.
///
/// Si alguna vez discrepan, el dueño ve un «Cobrado» en la pantalla de
/// ventas y otra «Venta total» al cerrar, y ya no se fía de ninguna.
#[test]
fn las_tarjetas_cuadran_con_lo_que_la_caja_dice_que_se_vendio() {
    let entorno = Entorno::nuevo();
    entorno
        .repositorio
        .guardar_configuracion(CLAVE_TASA, "420.00")
        .expect("fijar la tasa");
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "41.67");
    let (arroz, libra) = entorno.producto("Arroz", "180.00", "100.00");

    entorno.abrir_caja();
    entorno.vender(&[(refresco, lata, "3")]);
    // Venta de 1 160 con pago mixto: 300 en efectivo, 100 por
    // transferencia y 2 USD (840). El vuelto, 80, sale en pesos.
    entorno.cobrar(
        &[(refresco, lata, "10"), (arroz, libra, "2")],
        vec![
            PagoPedido {
                metodo: "EFECTIVO_CUP".to_owned(),
                entregado: "300.00".to_owned(),
            },
            PagoPedido {
                metodo: "TRANSFERENCIA".to_owned(),
                entregado: "100.00".to_owned(),
            },
            PagoPedido {
                metodo: "EFECTIVO_USD".to_owned(),
                entregado: "2.00".to_owned(),
            },
        ],
    );
    let anulada = entorno.vender(&[(arroz, libra, "1")]);
    AnularVenta::nuevo(&entorno.repositorio)
        .ejecutar(ComandoAnularVenta {
            venta: anulada,
            motivo: "error".to_owned(),
        })
        .expect("anular");

    // Con la caja abierta: tarjetas frente a la caja en vivo y a la vista
    // previa del cierre.
    let abierta = entorno.historial();
    let caja = ConsultarCaja::nuevo(&entorno.repositorio)
        .ejecutar()
        .expect("consultar la caja")
        .expect("hay caja");
    let previa = CerrarCaja::nuevo(&entorno.repositorio)
        .previsualizar(&ComandoCerrarCaja {
            contado_cup: "0.00".to_owned(),
            contado_usd: "0.00".to_owned(),
            modo: String::new(),
        })
        .expect("previsualizar");

    assert_eq!(abierta.hoy.cuantas, caja.cuantas_ventas);
    assert_eq!(abierta.hoy.total, caja.desglose.total_consolidado);
    assert_eq!(abierta.hoy.total, previa.economico.venta_total);
    assert_eq!(abierta.hoy.ganancia, previa.economico.ganancia_bruta);
    // 3 × 80 + 10 × 80 + 2 × 180
    assert_eq!(abierta.hoy.total, "1400.00");
    assert_eq!(
        total_agrupado(&entorno.agrupadas()),
        dinero(&abierta.hoy.total)
    );

    // Cerrada: tarjetas frente al cierre congelado.
    let cierre = entorno.cerrar_caja();
    let cerrada = entorno.historial();
    let releido = HistorialCajas::nuevo(&entorno.repositorio)
        .cierre(cierre.sesion)
        .expect("releer el cierre");

    assert_eq!(cerrada.hoy, abierta.hoy);
    assert_eq!(cerrada.hoy.total, cierre.economico.venta_total);
    assert_eq!(cerrada.hoy.ganancia, cierre.economico.ganancia_bruta);
    assert_eq!(cerrada.hoy.total, releido.economico.venta_total);
    assert_eq!(cerrada.hoy.ganancia, releido.economico.ganancia_bruta);
    assert_eq!(
        total_agrupado(&entorno.agrupadas()),
        dinero(&cerrada.hoy.total)
    );
}
