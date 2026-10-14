//! El consumo de los dueños contra SQLite de verdad (RF-CON).
//!
//! Lo que solo se puede medir aquí y no con dobles:
//!
//! - que el consumo escriba cabecera, renglones, existencia, valor y
//!   asientos **en una sola transacción**, y que si algo falla no quede
//!   nada a medias;
//! - que el saldo sea la suma que dice ser, con el filtro de anulación a
//!   los dos lados y sin que un `JOIN` multiplique filas;
//! - que el consumo **no llegue a la merma del cierre de caja**, que es el
//!   descuadre que toda esta feature evita;
//! - que el kárdex muestre el consumo y su reversa;
//! - y que un producto consumido no se pueda borrar del catálogo.

use std::sync::Arc;

use application::casos::{
    AbrirCaja, AnularConsumo, AnularPagoConsumo, ComandoAbrirCaja, ComandoAnularConsumo,
    ComandoLiquidar, ComandoRegistrarConsumo, ComandoRegistrarProducto, ConsultarCaja,
    ConsultarConsumo, ConsultarKardex, ConsultarProducto, Destino, LineaConsumoPedida,
    LiquidarDeuda, QuitarDelCatalogo, RegistrarConsumo, RegistrarProducto,
};
use infrastructure::{BaseDatos, RepositorioConsumoSqlite, RepositorioProductoSqlite};

/// Los dos repositorios sobre la MISMA base, como en la aplicación.
struct Entorno {
    base: Arc<BaseDatos>,
    productos: RepositorioProductoSqlite,
    consumos: RepositorioConsumoSqlite,
}

impl Entorno {
    fn en_memoria() -> Self {
        let base = Arc::new(BaseDatos::en_memoria().expect("abrir la base en memoria"));
        Self {
            productos: RepositorioProductoSqlite::nuevo(Arc::clone(&base)),
            consumos: RepositorioConsumoSqlite::nuevo(Arc::clone(&base)),
            base,
        }
    }

    fn filas(&self, tabla: &str) -> i64 {
        let consulta = format!("SELECT COUNT(*) FROM {tabla}");
        self.base
            .con(|conexion| Ok(conexion.query_row(&consulta, [], |fila| fila.get(0))?))
            .expect("contar filas")
    }

    /// Cien libras de arroz en la bodega a 120.00 la libra.
    fn arroz(&self) -> (i64, i64) {
        let producto = RegistrarProducto::nuevo(&self.productos)
            .ejecutar(ComandoRegistrarProducto {
                sku: String::new(),
                nombre: "Arroz blanco".to_owned(),
                unidad_base: "lb".to_owned(),
                precio_unitario: "180.00".to_owned(),
                costo_unitario: Some("120.00".to_owned()),
                cantidad_almacen: Some("100".to_owned()),
                cantidad_vitrina: None,
                stock_minimo: None,
                objetivo_vitrina: None,
            })
            .expect("registrar")
            .0;

        let ficha = ConsultarProducto::nuevo(&self.productos)
            .ejecutar(producto)
            .expect("ficha");

        (producto, ficha.presentaciones[0].id)
    }

    /// Carga un consumo a la cuenta de Ivett, que es la 1.
    fn consumir(&self, lineas: Vec<LineaConsumoPedida>) -> application::Resultado<i64> {
        RegistrarConsumo::nuevo(&self.productos, &self.consumos)
            .ejecutar(ComandoRegistrarConsumo {
                cuenta: 1,
                tomado_por: "Yaneisy".to_owned(),
                lineas,
            })
            .map(|hecho| hecho.id)
    }

    fn saldo_de_ivett(&self) -> String {
        ConsultarConsumo::nuevo(&self.consumos)
            .panel()
            .expect("panel")
            .cuentas[0]
            .saldo
            .clone()
    }

    fn existencia_de(&self, producto: i64) -> String {
        ConsultarProducto::nuevo(&self.productos)
            .ejecutar(producto)
            .expect("ficha")
            .existencia_total
    }
}

fn de_bodega(producto: i64, presentacion: i64, cantidad: &str) -> LineaConsumoPedida {
    LineaConsumoPedida {
        producto,
        presentacion,
        cantidad: cantidad.to_owned(),
        origen: "BODEGA".to_owned(),
    }
}

// ================================================== registrar

#[test]
fn un_consumo_escribe_cabecera_renglones_existencia_valor_y_asiento() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();

    entorno
        .consumir(vec![de_bodega(producto, presentacion, "2")])
        .expect("consumir");

    assert_eq!(entorno.filas("consumo"), 1);
    assert_eq!(entorno.filas("consumo_linea"), 1);
    assert_eq!(entorno.saldo_de_ivett(), "240.00", "2 lb × 120.00");
    assert_eq!(entorno.existencia_de(producto), "98.000");

    // El kárdex: la entrada de apertura más el consumo.
    let kardex = ConsultarKardex::nuevo(&entorno.productos)
        .ejecutar(producto, 10)
        .expect("kárdex");

    assert_eq!(kardex.len(), 2);
    // El código es lo que guarda la base; el nombre, lo que se lee en
    // pantalla. Se comprueban los dos: el segundo demuestra que el tipo
    // nuevo tiene su brazo de presentación y no sale en crudo.
    assert_eq!(kardex[0].tipo, "CONSUMO");
    assert_eq!(kardex[0].tipo_nombre, "Consumo");
    assert!(!kardex[0].es_entrada, "un consumo resta existencia");
    assert_eq!(kardex[0].origen.as_deref(), Some("Bodega"));
    assert_eq!(kardex[0].destino, None);
    assert_eq!(kardex[0].cantidad, "2.000");
    assert_eq!(kardex[0].costo_unitario, "120.00");
    assert_eq!(
        kardex[0].motivo.as_deref(),
        Some("Consumo de Ivett · tomó Yaneisy"),
        "el kárdex se lee solo"
    );
}

#[test]
fn si_el_consumo_falla_no_queda_nada_a_medias() {
    // La segunda línea pide más de lo que hay. El caso de uso lo rechaza
    // antes de llamar al puerto, así que no debe haber ni cabecera ni
    // movimiento ni existencia tocada.
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();

    let error = entorno
        .consumir(vec![
            de_bodega(producto, presentacion, "2"),
            de_bodega(producto, presentacion, "500"),
        ])
        .expect_err("no hay 502 libras");

    assert_eq!(error.codigo(), "EXISTENCIA_INSUFICIENTE");
    assert_eq!(entorno.filas("consumo"), 0);
    assert_eq!(entorno.filas("consumo_linea"), 0);
    assert_eq!(entorno.existencia_de(producto), "100.000", "intacta");
    assert_eq!(
        entorno.filas("movimiento"),
        1,
        "solo la entrada de apertura"
    );
}

#[test]
fn el_consumo_no_se_puede_cargar_a_una_cuenta_que_no_existe() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();

    let error = RegistrarConsumo::nuevo(&entorno.productos, &entorno.consumos)
        .ejecutar(ComandoRegistrarConsumo {
            cuenta: 99,
            tomado_por: "Nadie".to_owned(),
            lineas: vec![de_bodega(producto, presentacion, "1")],
        })
        .expect_err("esa cuenta no existe");

    assert_eq!(error.codigo(), "NO_ENCONTRADO");
    assert_eq!(entorno.filas("consumo"), 0);
}

// =============================================== el saldo

#[test]
fn el_saldo_no_se_multiplica_con_varios_consumos_y_varios_pagos() {
    // El error clásico: unir consumos y pagos en un solo `JOIN` produce
    // una fila por cada combinación, y cada importe se suma varias veces.
    // Tres consumos y dos pagos darían seis filas.
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();

    for _ in 0..3 {
        entorno
            .consumir(vec![de_bodega(producto, presentacion, "1")])
            .expect("consumir");
    }

    for _ in 0..2 {
        LiquidarDeuda::nuevo(&entorno.consumos)
            .ejecutar(ComandoLiquidar {
                cuenta: 1,
                importe: "100.00".to_owned(),
                nota: None,
            })
            .expect("liquidar");
    }

    // 3 × 120.00 − 2 × 100.00 = 160.00. Si se multiplicaran las filas
    // saldría 720.00 − 600.00 = 120.00, u otra cifra cualquiera.
    assert_eq!(entorno.saldo_de_ivett(), "160.00");
}

#[test]
fn el_saldo_sobrevive_a_cerrar_la_aplicacion() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();
    entorno
        .consumir(vec![de_bodega(producto, presentacion, "2")])
        .expect("consumir");

    // Otro repositorio sobre la misma base: lo que se lee viene de la
    // base, no de memoria.
    let otro = RepositorioConsumoSqlite::nuevo(Arc::clone(&entorno.base));
    let panel = ConsultarConsumo::nuevo(&otro).panel().expect("panel");

    assert_eq!(panel.total, "240.00");
    assert_eq!(panel.cuentas.len(), 3);
    assert_eq!(panel.cuentas[0].nombre, "Ivett");
}

#[test]
fn liquidar_de_mas_deja_el_saldo_a_favor_y_la_base_lo_guarda_negativo() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();
    entorno
        .consumir(vec![de_bodega(producto, presentacion, "1")])
        .expect("consumir");

    LiquidarDeuda::nuevo(&entorno.consumos)
        .ejecutar(ComandoLiquidar {
            cuenta: 1,
            importe: "200.00".to_owned(),
            nota: Some("de más".to_owned()),
        })
        .expect("liquidar");

    let panel = ConsultarConsumo::nuevo(&entorno.consumos)
        .panel()
        .expect("panel");

    assert_eq!(panel.cuentas[0].saldo, "80.00");
    assert!(panel.cuentas[0].a_favor, "200.00 − 120.00 a favor");
    assert!(panel.a_favor, "y el total del local");
}

// =============================================== anular

#[test]
fn anular_un_consumo_devuelve_la_mercancia_al_estante_del_que_salio() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();
    let consumo = entorno
        .consumir(vec![de_bodega(producto, presentacion, "2")])
        .expect("consumir");

    AnularConsumo::nuevo(&entorno.productos, &entorno.consumos)
        .ejecutar(ComandoAnularConsumo {
            id: consumo,
            motivo: "Era de Claudia".to_owned(),
        })
        .expect("anular");

    assert_eq!(entorno.saldo_de_ivett(), "0.00");
    assert_eq!(entorno.existencia_de(producto), "100.000");

    let kardex = ConsultarKardex::nuevo(&entorno.productos)
        .ejecutar(producto, 10)
        .expect("kárdex");

    assert_eq!(kardex[0].tipo, "CONSUMO_ANULADO");
    assert_eq!(kardex[0].tipo_nombre, "Consumo anulado");
    assert!(kardex[0].es_entrada, "la reversa suma existencia");
    assert_eq!(
        kardex[0].destino.as_deref(),
        Some("Bodega"),
        "volvió a la bodega"
    );
    assert_eq!(kardex[0].motivo.as_deref(), Some("Anulado: Era de Claudia"));
    // El registro no se borra: queda por su rastro.
    assert_eq!(entorno.filas("consumo"), 1);
}

#[test]
fn la_base_impide_anular_el_mismo_consumo_dos_veces() {
    // La garantía no es el `if` del caso de uso: es el `WHERE anulado_en IS
    // NULL` del `UPDATE`. Esta prueba se salta la aplicación y llama al
    // puerto directamente, que es lo que haría un error de programación.
    use application::puertos::{AnulacionConsumo, RepositorioConsumo};

    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();
    let consumo = entorno
        .consumir(vec![de_bodega(producto, presentacion, "2")])
        .expect("consumir");

    let anular = || {
        entorno.consumos.anular_consumo(&AnulacionConsumo {
            consumo,
            motivo: "Error".to_owned(),
            reversas: &[],
        })
    };

    anular().expect("la primera sí");
    assert!(anular().is_err(), "la segunda no");
}

#[test]
fn anular_un_pago_sube_el_saldo_y_la_base_lo_deja_a_la_vista() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();
    entorno
        .consumir(vec![de_bodega(producto, presentacion, "2")])
        .expect("consumir");

    LiquidarDeuda::nuevo(&entorno.consumos)
        .ejecutar(ComandoLiquidar {
            cuenta: 1,
            importe: "240.00".to_owned(),
            nota: None,
        })
        .expect("liquidar");
    assert_eq!(entorno.saldo_de_ivett(), "0.00");

    AnularPagoConsumo::nuevo(&entorno.consumos)
        .ejecutar(ComandoAnularConsumo {
            id: 1,
            motivo: "Se teclearon 240 por 24".to_owned(),
        })
        .expect("anular el pago");

    assert_eq!(entorno.saldo_de_ivett(), "240.00");

    let detalle = ConsultarConsumo::nuevo(&entorno.consumos)
        .cuenta(1, 10)
        .expect("detalle");

    assert_eq!(detalle.pagos.len(), 1, "sigue a la vista");
    assert!(detalle.pagos[0].anulado);
    assert_eq!(
        detalle.pagos[0].motivo_anulacion.as_deref(),
        Some("Se teclearon 240 por 24")
    );
}

#[test]
fn el_ciclo_completo_cierra_en_cero_contra_la_base() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();

    let consumo = entorno
        .consumir(vec![de_bodega(producto, presentacion, "2")])
        .expect("consumir");
    assert_eq!(entorno.saldo_de_ivett(), "240.00");

    LiquidarDeuda::nuevo(&entorno.consumos)
        .ejecutar(ComandoLiquidar {
            cuenta: 1,
            importe: "240.00".to_owned(),
            nota: None,
        })
        .expect("liquidar");
    assert_eq!(entorno.saldo_de_ivett(), "0.00");

    AnularConsumo::nuevo(&entorno.productos, &entorno.consumos)
        .ejecutar(ComandoAnularConsumo {
            id: consumo,
            motivo: "No era suyo".to_owned(),
        })
        .expect("anular el consumo");
    let panel = ConsultarConsumo::nuevo(&entorno.consumos)
        .panel()
        .expect("panel");
    assert_eq!(panel.cuentas[0].saldo, "240.00");
    assert!(panel.cuentas[0].a_favor);

    AnularPagoConsumo::nuevo(&entorno.consumos)
        .ejecutar(ComandoAnularConsumo {
            id: 1,
            motivo: "Y el pago tampoco".to_owned(),
        })
        .expect("anular el pago");

    assert_eq!(entorno.saldo_de_ivett(), "0.00", "cerrado");
    assert_eq!(
        entorno.existencia_de(producto),
        "100.000",
        "y el arroz volvió"
    );
}

// ================================= lo que NO debe pasar: tocar la caja

#[test]
fn el_consumo_no_cuenta_como_merma_en_el_cierre_de_caja() {
    // Es el descuadre que toda esta feature evita. Si el consumo se hubiera
    // registrado como merma, el arqueo diría que el local perdió mercancía
    // que en realidad tiene apuntada como deuda.
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();

    // Se consulta `acumulado_de_sesion` directamente: es LA consulta que
    // filtra `tipo = 'MERMA'` por ventana de tiempo, o sea la que se habría
    // roto si el consumo se hubiera colado como merma.
    use application::puertos::RepositorioProducto;
    use domain::IdSesion;

    let sesion = AbrirCaja::nuevo(&entorno.productos)
        .ejecutar(ComandoAbrirCaja {
            operador: "Yaneisy".to_owned(),
            fondo_inicial: "500.00".to_owned(),
        })
        .expect("abrir la caja");

    entorno
        .consumir(vec![de_bodega(producto, presentacion, "2")])
        .expect("consumir");

    let acumulado = entorno
        .productos
        .acumulado_de_sesion(IdSesion(sesion))
        .expect("acumulado");

    assert_eq!(
        acumulado.merma_costo.formatear(2),
        "0.00",
        "el consumo no es merma"
    );
    assert_eq!(acumulado.total_vendido.formatear(2), "0.00", "ni una venta");
    assert_eq!(acumulado.costo_vendido.formatear(2), "0.00");
    assert_eq!(acumulado.cuantas_ventas, 0);
}

#[test]
fn liquidar_no_entra_en_el_efectivo_del_turno() {
    // Liquidar no es un ingreso: el negocio recupera un costo que ya había
    // gastado. Si entrara en el arqueo, inflaría ingresos que no existen.
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();

    AbrirCaja::nuevo(&entorno.productos)
        .ejecutar(ComandoAbrirCaja {
            operador: "Yaneisy".to_owned(),
            fondo_inicial: "500.00".to_owned(),
        })
        .expect("abrir la caja");

    entorno
        .consumir(vec![de_bodega(producto, presentacion, "2")])
        .expect("consumir");
    LiquidarDeuda::nuevo(&entorno.consumos)
        .ejecutar(ComandoLiquidar {
            cuenta: 1,
            importe: "240.00".to_owned(),
            nota: None,
        })
        .expect("liquidar");

    let caja = ConsultarCaja::nuevo(&entorno.productos)
        .ejecutar()
        .expect("caja")
        .expect("hay una abierta");

    assert_eq!(caja.efectivo_esperado, "500.00", "solo el fondo inicial");
    assert_eq!(entorno.filas("movimiento_efectivo"), 0);
}

// ============================= el eje por producto y el catálogo

#[test]
fn el_eje_por_producto_sale_de_la_base_sin_multiplicar_en_sql() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();

    entorno
        .consumir(vec![de_bodega(producto, presentacion, "2")])
        .expect("consumir");
    entorno
        .consumir(vec![de_bodega(producto, presentacion, "3")])
        .expect("consumir");

    let filas = ConsultarConsumo::nuevo(&entorno.consumos)
        .por_producto(None, None, None)
        .expect("por producto");

    assert_eq!(filas.len(), 1, "un producto, dos consumos");
    assert_eq!(filas[0].nombre, "Arroz blanco");
    assert_eq!(filas[0].cantidad, "5.000 lb");
    assert_eq!(filas[0].costo, "600.00");
}

#[test]
fn lo_anulado_no_aparece_en_el_eje_por_producto() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();
    let consumo = entorno
        .consumir(vec![de_bodega(producto, presentacion, "2")])
        .expect("consumir");

    AnularConsumo::nuevo(&entorno.productos, &entorno.consumos)
        .ejecutar(ComandoAnularConsumo {
            id: consumo,
            motivo: "Error".to_owned(),
        })
        .expect("anular");

    assert!(ConsultarConsumo::nuevo(&entorno.consumos)
        .por_producto(None, None, None)
        .expect("por producto")
        .is_empty());
}

#[test]
fn un_producto_consumido_no_se_puede_borrar_del_catalogo() {
    // El consumo deja asiento en el kárdex, así que la clave foránea
    // impide borrarlo: solo se retira (RF-CAT-07). Encaja sin que haya
    // habido que escribir nada para ello.
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();

    entorno
        .consumir(vec![de_bodega(producto, presentacion, "100")])
        .expect("consumir todo");
    assert_eq!(entorno.existencia_de(producto), "0.000");

    let hecho = QuitarDelCatalogo::nuevo(&entorno.productos)
        .ejecutar(producto)
        .expect("quitar");

    assert_eq!(hecho.destino, Destino::Retirar);
    assert_eq!(hecho.movimientos, 2, "la entrada y el consumo");
    assert_eq!(hecho.ventas, 0, "nunca se vendió");
}

#[test]
fn los_renglones_de_un_consumo_se_leen_con_su_unidad() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();
    let consumo = entorno
        .consumir(vec![de_bodega(producto, presentacion, "2.5")])
        .expect("consumir");

    let renglones = ConsultarConsumo::nuevo(&entorno.consumos)
        .renglones(consumo)
        .expect("renglones");

    assert_eq!(renglones.len(), 1);
    // La libra admite fracciones: tres decimales, no cero.
    assert_eq!(renglones[0].cantidad, "2.500");
    assert_eq!(renglones[0].nombre_producto, "Arroz blanco");
    assert_eq!(renglones[0].origen, "Bodega");
    assert_eq!(renglones[0].importe, "300.00");
}

#[test]
fn el_borrado_total_se_lleva_el_consumo() {
    // Sin las cuatro tablas en la lista ordenada del borrón, la clave
    // foránea contra `producto` impediría vaciar el catálogo.
    use application::casos::{BorrarTodo, CLAVE_MANTENIMIENTO};

    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.arroz();
    entorno
        .consumir(vec![de_bodega(producto, presentacion, "2")])
        .expect("consumir");
    LiquidarDeuda::nuevo(&entorno.consumos)
        .ejecutar(ComandoLiquidar {
            cuenta: 1,
            importe: "100.00".to_owned(),
            nota: None,
        })
        .expect("liquidar");

    BorrarTodo::nuevo(&entorno.productos)
        .ejecutar(CLAVE_MANTENIMIENTO)
        .expect("borrar todo");

    assert_eq!(entorno.filas("consumo"), 0);
    assert_eq!(entorno.filas("consumo_linea"), 0);
    assert_eq!(entorno.filas("pago_consumo"), 0);
    assert_eq!(entorno.filas("cuenta_consumo"), 0);
    assert_eq!(entorno.filas("producto"), 0);
}
