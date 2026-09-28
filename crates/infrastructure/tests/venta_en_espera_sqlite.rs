//! La venta en espera contra SQLite de verdad (RF-VTA-14).
//!
//! Lo que se mide aquí no se puede medir con dobles: que los renglones
//! caigan en cascada con su cabecera, que el borrón total respete el orden
//! de las claves foráneas, que la migración se aplique sobre una base vieja
//! y, sobre todo, que cobrar una espera la consuma **en la misma
//! transacción** que registra la venta.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use application::casos::{
    AbrirCaja, BorrarTodo, ComandoAbrirCaja, ComandoDejarEnEspera, ComandoRegistrarProducto,
    ComandoVender, ConsultarKardex, ConsultarProducto, ConsultarVentas, DejarVentaEnEspera,
    EliminarVentaEnEspera, LineaPedida, ListarProductos, ListarVentasEnEspera, PagoPedido,
    RegistrarProducto, RetomarVentaEnEspera, Vender, CLAVE_MANTENIMIENTO,
};
use infrastructure::{
    migraciones, BaseDatos, RepositorioProductoSqlite, RepositorioVentaEnEsperaSqlite,
};

/// Los dos repositorios sobre la MISMA base, como en la aplicación.
struct Entorno {
    base: Arc<BaseDatos>,
    productos: RepositorioProductoSqlite,
    esperas: RepositorioVentaEnEsperaSqlite,
}

impl Entorno {
    fn sobre(base: BaseDatos) -> Self {
        let base = Arc::new(base);
        Self {
            productos: RepositorioProductoSqlite::nuevo(Arc::clone(&base)),
            esperas: RepositorioVentaEnEsperaSqlite::nuevo(Arc::clone(&base)),
            base,
        }
    }

    fn en_memoria() -> Self {
        Self::sobre(BaseDatos::en_memoria().expect("abrir la base en memoria"))
    }

    /// Cuenta las filas de una tabla, sin pasar por ningún caso de uso.
    fn filas(&self, tabla: &str) -> i64 {
        let consulta = format!("SELECT COUNT(*) FROM {tabla}");
        self.base
            .con(|conexion| Ok(conexion.query_row(&consulta, [], |fila| fila.get(0))?))
            .expect("contar filas")
    }

    /// Versión de esquema que dejó la migración.
    fn version(&self) -> i64 {
        self.base
            .con(|conexion| Ok(conexion.query_row("PRAGMA user_version", [], |fila| fila.get(0))?))
            .expect("leer la versión")
    }

    /// Refresco a 80 con 20 latas en vitrina. Devuelve producto y
    /// presentación.
    fn refresco(&self) -> (i64, i64) {
        let producto = RegistrarProducto::nuevo(&self.productos)
            .ejecutar(ComandoRegistrarProducto {
                sku: String::new(),
                nombre: "Refresco 500 ml".to_owned(),
                unidad_base: "unidad".to_owned(),
                precio_unitario: "80.00".to_owned(),
                costo_unitario: Some("41.67".to_owned()),
                cantidad_almacen: None,
                cantidad_vitrina: Some("20".to_owned()),
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

    fn abrir_caja(&self) {
        AbrirCaja::nuevo(&self.productos)
            .ejecutar(ComandoAbrirCaja {
                operador: "Yaneisy".to_owned(),
                fondo_inicial: "500.00".to_owned(),
            })
            .expect("abrir la caja");
    }

    fn dejar(&self, nota: Option<&str>, lineas: Vec<LineaPedida>) -> i64 {
        DejarVentaEnEspera::nuevo(&self.productos, &self.esperas)
            .ejecutar(ComandoDejarEnEspera {
                nota: nota.map(str::to_owned),
                lineas,
            })
            .expect("dejar en espera")
    }

    fn cobrar(
        &self,
        lineas: Vec<LineaPedida>,
        paga: &str,
        espera_id: Option<i64>,
    ) -> application::Resultado<i64> {
        Vender::nuevo(&self.productos)
            .ejecutar(ComandoVender {
                lineas,
                pagos: vec![PagoPedido {
                    metodo: "EFECTIVO_CUP".to_owned(),
                    entregado: paga.to_owned(),
                }],
                espera_id,
            })
            .map(|hecha| hecha.folio)
    }

    fn en_vitrina(&self) -> String {
        ListarProductos::nuevo(&self.productos)
            .ejecutar(false)
            .expect("listar el catálogo")
            .remove(0)
            .en_vitrina
    }

    fn cuantas_ventas(&self) -> usize {
        ConsultarVentas::nuevo(&self.productos)
            .ejecutar(10)
            .expect("listar las ventas")
            .ventas
            .len()
    }

    fn cuantas_esperas(&self) -> usize {
        ListarVentasEnEspera::nuevo(&self.esperas)
            .ejecutar()
            .expect("listar las esperas")
            .len()
    }
}

fn linea(producto: i64, presentacion: i64, cantidad: &str) -> LineaPedida {
    LineaPedida {
        producto,
        presentacion,
        cantidad: cantidad.to_owned(),
    }
}

fn ruta_temporal(etiqueta: &str) -> PathBuf {
    std::env::temp_dir().join(format!("kilo12-{etiqueta}-{}.db", std::process::id()))
}

/// Borra el archivo y los auxiliares que deja el modo WAL.
fn limpiar(ruta: &Path) {
    for sufijo in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{sufijo}", ruta.display()));
    }
}

// ============================================== guardar y leer

#[test]
fn una_espera_se_guarda_se_lista_se_retoma_y_se_elimina() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.refresco();

    // Sin caja abierta: apartar no la necesita.
    let id = entorno.dejar(
        Some("  la señora del pan "),
        vec![
            linea(producto, presentacion, "2"),
            linea(producto, presentacion, "3"),
        ],
    );

    let lista = ListarVentasEnEspera::nuevo(&entorno.esperas)
        .ejecutar()
        .expect("listar");
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].id, id);
    assert_eq!(lista[0].nota.as_deref(), Some("la señora del pan"));
    assert_eq!(lista[0].cuantas_lineas, 2);
    // La fecha la pone la base: `YYYY-MM-DD HH:MM:SS`.
    assert_eq!(lista[0].creada_en.len(), 19);

    let retomada = RetomarVentaEnEspera::nuevo(&entorno.productos, &entorno.esperas)
        .ejecutar(id)
        .expect("retomar");
    let cantidades: Vec<&str> = retomada
        .lineas
        .iter()
        .map(|l| l.cantidad.as_str())
        .collect();
    // En el orden en que se armó.
    assert_eq!(cantidades, ["2", "3"]);
    assert_eq!(retomada.total, "400.00");
    assert!(!retomada.hay_problemas);

    // Apartar y retomar no tocan la vitrina ni el kárdex.
    assert_eq!(entorno.en_vitrina(), "20");
    assert_eq!(
        ConsultarKardex::nuevo(&entorno.productos)
            .ejecutar(producto, 50)
            .expect("kárdex")
            .len(),
        1
    );
    // Y retomar no la borra.
    assert_eq!(entorno.cuantas_esperas(), 1);

    EliminarVentaEnEspera::nuevo(&entorno.esperas)
        .ejecutar(id)
        .expect("eliminar");
    assert_eq!(entorno.cuantas_esperas(), 0);
}

#[test]
fn varias_esperas_se_listan_de_la_mas_vieja_a_la_mas_nueva() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.refresco();

    let primera = entorno.dejar(Some("mesa 1"), vec![linea(producto, presentacion, "1")]);
    let segunda = entorno.dejar(None, vec![linea(producto, presentacion, "1")]);

    let lista = ListarVentasEnEspera::nuevo(&entorno.esperas)
        .ejecutar()
        .expect("listar");
    let ids: Vec<i64> = lista.iter().map(|espera| espera.id).collect();

    assert_eq!(ids, [primera, segunda]);
    assert_eq!(lista[1].nota, None);
}

#[test]
fn eliminar_una_espera_arrastra_sus_renglones() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.refresco();
    let id = entorno.dejar(
        None,
        vec![
            linea(producto, presentacion, "1"),
            linea(producto, presentacion, "2"),
        ],
    );
    assert_eq!(entorno.filas("venta_en_espera_linea"), 2);

    EliminarVentaEnEspera::nuevo(&entorno.esperas)
        .ejecutar(id)
        .expect("eliminar");

    // La cascada la aplica la base: no queda ningún renglón huérfano.
    assert_eq!(entorno.filas("venta_en_espera"), 0);
    assert_eq!(entorno.filas("venta_en_espera_linea"), 0);

    let error = EliminarVentaEnEspera::nuevo(&entorno.esperas)
        .ejecutar(id)
        .expect_err("ya no existe");
    assert_eq!(error.codigo(), "VENTA_EN_ESPERA_NO_ENCONTRADA");
}

#[test]
fn retomar_una_espera_que_no_existe_da_un_error_claro() {
    let entorno = Entorno::en_memoria();

    let error = RetomarVentaEnEspera::nuevo(&entorno.productos, &entorno.esperas)
        .ejecutar(404)
        .expect_err("no existe");

    assert_eq!(error.codigo(), "VENTA_EN_ESPERA_NO_ENCONTRADA");
}

#[test]
fn la_base_tampoco_admite_una_nota_vacia() {
    // La regla vive en el dominio; la restricción en la base existe por si
    // un error de programación se la saltara (DT-9).
    let entorno = Entorno::en_memoria();

    let resultado = entorno.base.con(|conexion| {
        Ok(conexion.execute(
            "INSERT INTO venta_en_espera (nota, creada_en) VALUES ('', '2026-09-27 10:00:00')",
            [],
        )?)
    });

    assert!(resultado.is_err());
}

#[test]
fn vaciar_la_aplicacion_se_lleva_las_esperas() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.refresco();
    entorno.dejar(Some("mesa 2"), vec![linea(producto, presentacion, "1")]);

    // Si las esperas no se borraran antes que los productos, la clave
    // foránea de sus renglones haría fallar el borrón entero.
    BorrarTodo::nuevo(&entorno.productos)
        .ejecutar(CLAVE_MANTENIMIENTO)
        .expect("borrar");

    assert_eq!(entorno.filas("venta_en_espera"), 0);
    assert_eq!(entorno.filas("venta_en_espera_linea"), 0);
    assert_eq!(entorno.filas("producto"), 0);
}

// ============================================== migración

#[test]
fn una_base_nueva_nace_con_las_tablas_de_espera() {
    let entorno = Entorno::en_memoria();

    assert_eq!(entorno.version(), 7);
    assert_eq!(migraciones::version_esperada(), 7);
    assert_eq!(entorno.filas("venta_en_espera"), 0);
    assert_eq!(entorno.filas("venta_en_espera_linea"), 0);
}

#[test]
fn la_migracion_se_aplica_sobre_una_base_de_la_version_anterior() {
    let ruta = ruta_temporal("migracion-espera");
    limpiar(&ruta);

    // Una base con datos, devuelta al esquema 6: el que tiene quien
    // actualiza desde la versión anterior.
    {
        let entorno = Entorno::sobre(BaseDatos::abrir(&ruta).expect("abrir la base en disco"));
        entorno.refresco();
        entorno
            .base
            .con(|conexion| {
                conexion.execute_batch(
                    "DROP TABLE venta_en_espera_linea;
                     DROP TABLE venta_en_espera;
                     PRAGMA user_version = 6;",
                )?;
                Ok(())
            })
            .expect("volver al esquema 6");
    }

    // Al abrirla se migra sola y los datos siguen ahí.
    let id = {
        let entorno = Entorno::sobre(BaseDatos::abrir(&ruta).expect("reabrir y migrar"));
        assert_eq!(entorno.version(), 7);
        assert_eq!(entorno.en_vitrina(), "20");

        let ficha = ConsultarProducto::nuevo(&entorno.productos)
            .ejecutar(1)
            .expect("ficha");
        entorno.dejar(None, vec![linea(1, ficha.presentaciones[0].id, "1")])
    };

    // Abrir una base ya migrada no vuelve a migrar ni pierde nada.
    {
        let entorno = Entorno::sobre(BaseDatos::abrir(&ruta).expect("reabrir ya migrada"));
        let lista = ListarVentasEnEspera::nuevo(&entorno.esperas)
            .ejecutar()
            .expect("listar");
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].id, id);
    }

    limpiar(&ruta);
}

// ============================================== cobrar una espera

#[test]
fn cobrar_una_espera_la_consume_en_la_misma_operacion() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.refresco();
    entorno.abrir_caja();

    let id = entorno.dejar(Some("mesa 4"), vec![linea(producto, presentacion, "3")]);
    let folio = entorno
        .cobrar(vec![linea(producto, presentacion, "3")], "300.00", Some(id))
        .expect("cobrar la espera");

    assert_eq!(folio, 1);
    assert_eq!(entorno.cuantas_ventas(), 1);
    assert_eq!(entorno.en_vitrina(), "17");
    // La espera y sus renglones desaparecieron con el cobro.
    assert_eq!(entorno.filas("venta_en_espera"), 0);
    assert_eq!(entorno.filas("venta_en_espera_linea"), 0);
}

#[test]
fn cobrar_una_espera_que_ya_no_existe_no_registra_nada() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.refresco();
    entorno.abrir_caja();

    let error = entorno
        .cobrar(
            vec![linea(producto, presentacion, "3")],
            "300.00",
            Some(999),
        )
        .expect_err("esa espera no existe");

    assert_eq!(error.codigo(), "VENTA_EN_ESPERA_NO_ENCONTRADA");
    // Ni venta, ni descuento, ni asiento: la transacción se deshizo entera.
    assert_eq!(entorno.cuantas_ventas(), 0);
    assert_eq!(entorno.en_vitrina(), "20");
    assert_eq!(entorno.filas("movimiento"), 1);
}

#[test]
fn una_espera_no_se_cobra_dos_veces() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.refresco();
    entorno.abrir_caja();
    let id = entorno.dejar(None, vec![linea(producto, presentacion, "2")]);

    entorno
        .cobrar(vec![linea(producto, presentacion, "2")], "200.00", Some(id))
        .expect("primer cobro");
    let error = entorno
        .cobrar(vec![linea(producto, presentacion, "2")], "200.00", Some(id))
        .expect_err("el segundo cobro sería una venta doble");

    assert_eq!(error.codigo(), "VENTA_EN_ESPERA_NO_ENCONTRADA");
    assert_eq!(entorno.cuantas_ventas(), 1);
    assert_eq!(entorno.en_vitrina(), "18");
}

#[test]
fn si_el_cobro_no_pasa_la_espera_sigue_ahi() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.refresco();
    entorno.abrir_caja();

    // 25 latas con 20 en vitrina: se aparta igual, pero no se puede cobrar.
    let id = entorno.dejar(None, vec![linea(producto, presentacion, "25")]);
    let error = entorno
        .cobrar(
            vec![linea(producto, presentacion, "25")],
            "2000.00",
            Some(id),
        )
        .expect_err("no alcanza la vitrina");

    assert_eq!(error.codigo(), "EXISTENCIA_INSUFICIENTE");
    assert_eq!(entorno.cuantas_ventas(), 0);
    assert_eq!(entorno.cuantas_esperas(), 1);
    assert_eq!(entorno.filas("venta_en_espera_linea"), 1);
}

#[test]
fn si_la_venta_falla_dentro_de_la_transaccion_la_espera_no_se_pierde() {
    let entorno = Entorno::en_memoria();
    let (producto, presentacion) = entorno.refresco();
    entorno.abrir_caja();
    let id = entorno.dejar(None, vec![linea(producto, presentacion, "1")]);

    // Una falla provocada DESPUÉS de borrar la espera, a mitad de la
    // transacción: al insertar la venta. Es el caso que de verdad prueba
    // que el borrado y la venta son una sola operación.
    entorno
        .base
        .con(|conexion| {
            conexion.execute_batch(
                "CREATE TRIGGER falla_provocada BEFORE INSERT ON venta
                 BEGIN SELECT RAISE(ABORT, 'falla provocada'); END;",
            )?;
            Ok(())
        })
        .expect("instalar la falla");

    let error = entorno
        .cobrar(vec![linea(producto, presentacion, "1")], "80.00", Some(id))
        .expect_err("la venta no se puede insertar");

    assert_eq!(error.codigo(), "PERSISTENCIA");
    assert_eq!(entorno.cuantas_esperas(), 1);
    assert_eq!(entorno.filas("venta_en_espera_linea"), 1);
    assert_eq!(entorno.en_vitrina(), "20");
}
