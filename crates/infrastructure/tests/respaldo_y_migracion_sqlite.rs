//! El respaldo previo a migrar y la reconstrucción del kárdex (migración 9).
//!
//! Estas pruebas cubren la única operación **irreversible** de Kilo12.
//! Todas las demás migraciones hasta ahora solo añadían tablas o columnas;
//! la 9 reescribe `movimiento`, que es el libro que explica por qué la
//! existencia es la que es. Si esa reescritura pierde una fila o reordena
//! los identificadores, el kárdex deja de cuadrar y no hay forma de
//! deducirlo de nada.
//!
//! Dos cosas que solo se pueden probar contra SQLite en disco, nunca en
//! memoria: que el archivo de respaldo aparece de verdad, y que lo que
//! contiene es una base completa y abrible.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use infrastructure::{migraciones, BaseDatos};
use rusqlite::Connection;

// ============================================================ utilería

fn ruta_temporal(etiqueta: &str) -> PathBuf {
    let mut ruta = std::env::temp_dir();
    ruta.push(format!("kilo12-{etiqueta}-{}.db", std::process::id()));
    ruta
}

/// Borra la base y todo lo que la acompaña: los archivos de WAL y los
/// respaldos que estas pruebas hayan dejado.
fn limpiar(ruta: &Path) {
    let _ = std::fs::remove_file(ruta);
    for sufijo in ["-wal", "-shm"] {
        let mut acompanante = ruta.as_os_str().to_owned();
        acompanante.push(sufijo);
        let _ = std::fs::remove_file(PathBuf::from(acompanante));
    }

    // Los respaldos de cualquier versión, más los parciales.
    if let Some(carpeta) = ruta.parent() {
        let prefijo = ruta.file_name().unwrap_or_default().to_owned();
        if let Ok(entradas) = std::fs::read_dir(carpeta) {
            for entrada in entradas.flatten() {
                let nombre = entrada.file_name();
                let nombre = nombre.to_string_lossy();
                let prefijo = prefijo.to_string_lossy();
                if nombre.starts_with(&format!("{prefijo}.previo-v")) {
                    let _ = std::fs::remove_file(entrada.path());
                }
            }
        }
    }
}

fn respaldo_de(ruta: &Path, version: u32) -> PathBuf {
    let mut nombre = ruta.as_os_str().to_owned();
    nombre.push(format!(".previo-v{version}"));
    PathBuf::from(nombre)
}

/// Abre la base cruda, sin pasar por `BaseDatos`, para mirar el esquema sin
/// disparar migraciones.
fn cruda(ruta: &Path) -> Connection {
    Connection::open(ruta).expect("abrir en crudo")
}

fn version_de(conexion: &Connection) -> i64 {
    conexion
        .query_row("PRAGMA user_version", [], |fila| fila.get(0))
        .expect("leer la versión")
}

fn contar(conexion: &Connection, consulta: &str) -> i64 {
    conexion
        .query_row(consulta, [], |fila| fila.get(0))
        .expect("contar")
}

/// Un producto con mercancía y tres asientos de kárdex, y después la base
/// devuelta al esquema 8: el que tiene quien actualiza desde la versión
/// anterior.
///
/// Deshacer la 9 exige rehacer `movimiento` con su `CHECK` de entonces. Un
/// `DROP TABLE` no sirve: dejaría la tabla ya reconstruida y la prueba
/// estaría migrando sobre un esquema que ya tiene aplicado lo que dice
/// estar probando.
fn base_en_la_version_8(ruta: &Path) {
    {
        let base = BaseDatos::abrir(ruta).expect("crear la base");
        base.con(|conexion| {
            conexion.execute_batch(
                "INSERT INTO producto
                    (id, sku, nombre, unidad_base, valor_total, stock_minimo,
                     objetivo_vitrina, activo, creado_en)
                 VALUES (1, 'ARROZ', 'Arroz blanco', 'lb', 12000000000, 0, 0, 1,
                         '2026-10-01 09:00:00');

                 INSERT INTO existencia (producto_id, ubicacion, cantidad)
                 VALUES (1, 'BODEGA', 100000), (1, 'VITRINA', 0);

                 INSERT INTO movimiento
                    (producto_id, tipo, origen, destino, cantidad, costo_unitario,
                     bodega_resultante, vitrina_resultante, motivo, ocurrido_en)
                 VALUES
                    (1, 'ENTRADA', NULL, 'BODEGA', 100000, 120000000, 100000, 0,
                     NULL, '2026-10-01 09:00:00'),
                    (1, 'TRASPASO', 'BODEGA', 'VITRINA', 30000, 120000000, 70000,
                     30000, NULL, '2026-10-02 10:00:00'),
                    (1, 'MERMA', 'VITRINA', NULL, 2000, 120000000, 70000, 28000,
                     'Se mojó', '2026-10-03 11:00:00');",
            )?;
            Ok(())
        })
        .expect("sembrar");

        // Y se deshace la 9.
        base.con(|conexion| {
            conexion.execute_batch(
                "DROP TABLE consumo_linea;
                 DROP TABLE consumo;
                 DROP TABLE pago_consumo;
                 DROP TABLE cuenta_consumo;

                 CREATE TABLE movimiento_v8 (
                     id                 INTEGER PRIMARY KEY,
                     producto_id        INTEGER NOT NULL REFERENCES producto(id),
                     tipo               TEXT    NOT NULL CHECK (tipo IN
                                        ('ENTRADA','VENTA','TRASPASO','MERMA',
                                         'AJUSTE','DEVOLUCION')),
                     origen             TEXT    CHECK (origen  IN ('BODEGA','VITRINA')),
                     destino            TEXT    CHECK (destino IN ('BODEGA','VITRINA')),
                     cantidad           INTEGER NOT NULL CHECK (cantidad > 0),
                     costo_unitario     INTEGER NOT NULL CHECK (costo_unitario >= 0),
                     bodega_resultante  INTEGER NOT NULL CHECK (bodega_resultante  >= 0),
                     vitrina_resultante INTEGER NOT NULL CHECK (vitrina_resultante >= 0),
                     motivo             TEXT,
                     ocurrido_en        TEXT    NOT NULL
                 );

                 INSERT INTO movimiento_v8
                     (id, producto_id, tipo, origen, destino, cantidad,
                      costo_unitario, bodega_resultante, vitrina_resultante,
                      motivo, ocurrido_en)
                 SELECT id, producto_id, tipo, origen, destino, cantidad,
                        costo_unitario, bodega_resultante, vitrina_resultante,
                        motivo, ocurrido_en
                   FROM movimiento;

                 DROP TABLE movimiento;
                 ALTER TABLE movimiento_v8 RENAME TO movimiento;
                 CREATE INDEX idx_movimiento_producto ON movimiento(producto_id, id);

                 PRAGMA user_version = 8;",
            )?;
            Ok(())
        })
        .expect("volver al esquema 8");
    }
}

/// Los asientos tal como los lee el kárdex: del más reciente al más viejo.
fn asientos(conexion: &Connection) -> Vec<(i64, String, i64)> {
    let mut consulta = conexion
        .prepare("SELECT id, tipo, cantidad FROM movimiento ORDER BY id DESC")
        .expect("preparar");

    let filas = consulta
        .query_map([], |fila| {
            Ok((
                fila.get::<_, i64>(0)?,
                fila.get::<_, String>(1)?,
                fila.get::<_, i64>(2)?,
            ))
        })
        .expect("consultar");

    filas.map(|fila| fila.expect("leer")).collect()
}

// ====================================================== el respaldo

#[test]
fn migrar_deja_un_respaldo_de_la_version_anterior() {
    let ruta = ruta_temporal("respaldo-deja");
    limpiar(&ruta);
    base_en_la_version_8(&ruta);

    let respaldo = respaldo_de(&ruta, 8);
    assert!(!respaldo.exists(), "antes de migrar no debería existir");

    // Al abrirla se resguarda y se migra.
    {
        let base = BaseDatos::abrir(&ruta).expect("migrar");
        assert_eq!(
            base.con(|c| Ok(version_de(c))).expect("versión"),
            9,
            "la base quedó en la versión nueva"
        );
    }

    assert!(respaldo.exists(), "el respaldo tiene que estar ahí");

    // El parcial es el nombre del respaldo MÁS el sufijo, no un cambio de
    // extensión: se escribe ahí y se renombra al final, así que si queda
    // alguno es que el renombrado no ocurrió.
    let mut parcial = respaldo.as_os_str().to_owned();
    parcial.push(".parcial");
    assert!(
        !PathBuf::from(parcial).exists(),
        "no debe quedar ningún parcial"
    );

    limpiar(&ruta);
}

#[test]
fn el_respaldo_es_una_base_completa_y_del_esquema_viejo() {
    // Lo que hace que un respaldo sirva no es que el archivo exista: es que
    // se pueda abrir y tenga dentro lo que había. Con WAL activo, copiar el
    // `.db` a secas puede dejar un archivo que parece válido y no lo es.
    let ruta = ruta_temporal("respaldo-valido");
    limpiar(&ruta);
    base_en_la_version_8(&ruta);

    let antes = {
        let conexion = cruda(&ruta);
        asientos(&conexion)
    };

    BaseDatos::abrir(&ruta).expect("migrar");

    let respaldo = cruda(&respaldo_de(&ruta, 8));

    // Se quedó en la versión 8, con sus datos y SIN lo que trajo la 9.
    assert_eq!(version_de(&respaldo), 8);
    assert_eq!(asientos(&respaldo), antes, "el kárdex entero");
    assert_eq!(contar(&respaldo, "SELECT COUNT(*) FROM producto"), 1);
    assert_eq!(
        contar(
            &respaldo,
            "SELECT COUNT(*) FROM sqlite_master
              WHERE type = 'table' AND name = 'cuenta_consumo'"
        ),
        0,
        "el respaldo es de ANTES de la migración"
    );

    limpiar(&ruta);
}

#[test]
fn una_base_al_dia_no_deja_respaldo() {
    // Resguardar en cada arranque sería coste sin motivo.
    let ruta = ruta_temporal("respaldo-al-dia");
    limpiar(&ruta);

    BaseDatos::abrir(&ruta).expect("crear");
    let respaldo = respaldo_de(&ruta, migraciones::version_esperada() as u32);
    BaseDatos::abrir(&ruta).expect("reabrir ya migrada");

    assert!(!respaldo.exists());
    limpiar(&ruta);
}

#[test]
fn una_base_nueva_no_deja_respaldo() {
    // En la versión 0 no hay ni tablas: las crea la migración 1. No hay
    // nada que perder, así que no hay nada que resguardar.
    let ruta = ruta_temporal("respaldo-nueva");
    limpiar(&ruta);

    BaseDatos::abrir(&ruta).expect("crear");

    for version in 0..=migraciones::version_esperada() as u32 {
        assert!(
            !respaldo_de(&ruta, version).exists(),
            "no debería haber respaldo de la v{version}"
        );
    }

    limpiar(&ruta);
}

// ========================================= la reconstrucción del kárdex

#[test]
fn la_reconstruccion_conserva_los_asientos_con_su_id_y_su_orden() {
    // El `id` es alias de `rowid` y el kárdex se lee con `ORDER BY id DESC`.
    // Si la reconstrucción reasignara identificadores, el historial saldría
    // en otro orden sin que nada avisara.
    let ruta = ruta_temporal("kardex-reconstruido");
    limpiar(&ruta);
    base_en_la_version_8(&ruta);

    let antes = {
        let conexion = cruda(&ruta);
        asientos(&conexion)
    };
    assert_eq!(antes.len(), 3, "los tres asientos sembrados");

    BaseDatos::abrir(&ruta).expect("migrar");

    let despues = {
        let conexion = cruda(&ruta);
        asientos(&conexion)
    };

    assert_eq!(despues, antes, "mismos asientos, mismos id, mismo orden");
    limpiar(&ruta);
}

#[test]
fn la_reconstruccion_deja_el_indice_del_kardex_en_pie() {
    // Al borrar la tabla vieja se va su índice con ella. Sin recrearlo, el
    // kárdex de un producto pasaría a recorrer la tabla entera.
    let ruta = ruta_temporal("kardex-indice");
    limpiar(&ruta);
    base_en_la_version_8(&ruta);

    BaseDatos::abrir(&ruta).expect("migrar");

    let conexion = cruda(&ruta);
    assert_eq!(
        contar(
            &conexion,
            "SELECT COUNT(*) FROM sqlite_master
              WHERE type = 'index' AND name = 'idx_movimiento_producto'"
        ),
        1
    );

    limpiar(&ruta);
}

#[test]
fn el_esquema_nuevo_admite_el_consumo_y_sigue_rechazando_lo_que_no_existe() {
    // La defensa en dos niveles de DT-9: el enum de Rust impide construir
    // un tipo inválido, y el `CHECK` lo impide incluso a quien escriba por
    // debajo del dominio.
    let ruta = ruta_temporal("kardex-check");
    limpiar(&ruta);
    base_en_la_version_8(&ruta);
    BaseDatos::abrir(&ruta).expect("migrar");

    let conexion = cruda(&ruta);
    let asentar = |tipo: &str| {
        conexion.execute(
            "INSERT INTO movimiento
                (producto_id, tipo, origen, destino, cantidad, costo_unitario,
                 bodega_resultante, vitrina_resultante, motivo, ocurrido_en)
             VALUES (1, ?1, 'VITRINA', NULL, 1000, 120000000, 70000, 27000,
                     'Consumo de Ivett', '2026-10-14 12:00:00')",
            [tipo],
        )
    };

    assert!(asentar("CONSUMO").is_ok(), "el consumo ya cabe");
    assert!(asentar("CONSUMO_ANULADO").is_ok(), "y su reversa también");
    assert!(
        asentar("REGALO").is_err(),
        "lo que no existe sigue sin caber"
    );

    limpiar(&ruta);
}

#[test]
fn las_tres_cuentas_de_consumo_nacen_con_la_migracion() {
    let base = Arc::new(BaseDatos::en_memoria().expect("abrir en memoria"));

    let nombres: Vec<String> = base
        .con(|conexion| {
            let mut consulta = conexion.prepare("SELECT nombre FROM cuenta_consumo ORDER BY id")?;
            let filas = consulta.query_map([], |fila| fila.get::<_, String>(0))?;
            Ok(filas.map(|fila| fila.expect("leer")).collect())
        })
        .expect("listar cuentas");

    assert_eq!(nombres, ["Ivett", "Claudia", "Inés"]);
}

#[test]
fn las_cuentas_nacen_activas_y_su_nombre_no_se_repite() {
    let base = BaseDatos::en_memoria().expect("abrir en memoria");

    base.con(|conexion| {
        assert_eq!(
            contar(
                conexion,
                "SELECT COUNT(*) FROM cuenta_consumo WHERE activa = 1"
            ),
            3
        );
        assert!(
            conexion
                .execute("INSERT INTO cuenta_consumo (nombre) VALUES ('Ivett')", [])
                .is_err(),
            "dos cuentas con el mismo nombre serían dos deudas indistinguibles"
        );
        Ok(())
    })
    .expect("comprobar");
}

// ======================== actualizar desde la versión publicada

/// Versión de esquema que dejó la `v0.1.0`, la que está instalada en la
/// tienda. Si sale otra publicación, esta constante sube con ella.
const VERSION_PUBLICADA: usize = 6;

/// Crea una base con el esquema de la versión **publicada**, que es la que
/// tiene instalada quien va a actualizar.
fn base_en_la_version_publicada(ruta: &Path) {
    let conexion = cruda(ruta);

    migraciones::aplicar_hasta(&conexion, VERSION_PUBLICADA).expect("aplicar las migraciones");

    // Datos como los que tendría una tienda en marcha: catálogo, existencia,
    // kárdex y una venta cobrada en su turno de caja.
    conexion
        .execute_batch(
            "INSERT INTO producto
               (id, sku, nombre, unidad_base, valor_total, stock_minimo,
                objetivo_vitrina, activo, creado_en)
             VALUES (1, 'REFRESCO', 'Refresco', 'unidad', 400000000, 0, 0, 1,
                     '2026-09-01 08:00:00');

             INSERT INTO presentacion
               (id, producto_id, nombre, factor, precio, es_predeterminada,
                codigo_barras, activa)
             VALUES (1, 1, 'Unidad', 1000, 80000000, 1, NULL, 1);

             INSERT INTO existencia (producto_id, ubicacion, cantidad)
             VALUES (1, 'BODEGA', 0), (1, 'VITRINA', 10000);

             INSERT INTO movimiento
               (id, producto_id, tipo, origen, destino, cantidad, costo_unitario,
                bodega_resultante, vitrina_resultante, motivo, ocurrido_en)
             VALUES (1, 1, 'ENTRADA', NULL, 'VITRINA', 20000, 40000000,
                     0, 20000, NULL, '2026-09-01 08:05:00'),
                    (2, 1, 'VENTA', 'VITRINA', NULL, 10000, 40000000,
                     0, 10000, NULL, '2026-09-01 10:00:00');

             INSERT INTO sesion_caja
               (id, operador, fondo_inicial, estado, abierta_en, cerrada_en)
             VALUES (1, 'Yaneisy', 500000000, 'CERRADA',
                     '2026-09-01 08:00:00', '2026-09-01 20:00:00');

             INSERT INTO venta
               (id, folio, total, costo_total, vuelto, sesion_id, ocurrido_en,
                anulada_en, motivo_anulacion)
             VALUES (1, 1, 800000000, 400000000, 0, 1,
                     '2026-09-01 10:00:00', NULL, NULL);

             INSERT INTO venta_linea
               (venta_id, producto_id, presentacion_id, nombre_producto,
                nombre_presentacion, cantidad, factor, precio, costo_unitario)
             VALUES (1, 1, 1, 'Refresco', 'Unidad', 10000, 1000, 80000000, 40000000);",
        )
        .expect("sembrar la tienda");
}

/// Actualizar desde la versión publicada conserva todo lo que había.
///
/// Es el salto que de verdad va a ocurrir en la tienda: la `v0.1.0` dejó el
/// esquema en 6 y esta compilación espera 9, así que al abrir se encadenan
/// tres migraciones, y la última **reconstruye el kárdex**. Lo que aquí se
/// comprueba no es que cada migración funcione por separado —eso ya está
/// cubierto— sino que las tres seguidas no se pisan y que al otro lado
/// siguen estando el catálogo, la existencia, los asientos y las ventas.
#[test]
fn actualizar_desde_la_version_publicada_conserva_los_datos() {
    let ruta = ruta_temporal("actualizacion-v6");
    limpiar(&ruta);
    base_en_la_version_publicada(&ruta);

    {
        let conexion = cruda(&ruta);
        assert_eq!(
            version_de(&conexion),
            6,
            "la base de partida es la publicada"
        );
    }

    // Esto es lo que hace la aplicación al arrancar por primera vez tras
    // instalar la versión nueva.
    let base = BaseDatos::abrir(&ruta).expect("abrir y migrar");

    base.con(|conexion| {
        assert_eq!(version_de(conexion), 9, "el esquema queda al día");

        // El catálogo y su inventario, intactos.
        assert_eq!(contar(conexion, "SELECT COUNT(*) FROM producto"), 1);
        assert_eq!(
            contar(conexion, "SELECT valor_total FROM producto WHERE id = 1"),
            400_000_000
        );
        assert_eq!(
            contar(
                conexion,
                "SELECT cantidad FROM existencia WHERE producto_id = 1 AND ubicacion = 'VITRINA'"
            ),
            10_000
        );

        // El kárdex: las dos filas, con su identificador y su orden. Es lo
        // que reescribe la migración 9 y lo único irreparable si se pierde.
        // Se leen como las lee el kárdex, del asiento más nuevo al más
        // viejo, porque es ese orden el que la reconstrucción podría haber
        // alterado al reasignar los identificadores.
        let asientos = asientos(conexion);
        assert_eq!(
            asientos,
            vec![
                (2, "VENTA".to_owned(), 10_000),
                (1, "ENTRADA".to_owned(), 20_000)
            ]
        );

        // La venta, su renglón y su turno.
        assert_eq!(contar(conexion, "SELECT COUNT(*) FROM venta"), 1);
        assert_eq!(contar(conexion, "SELECT COUNT(*) FROM venta_linea"), 1);
        assert_eq!(contar(conexion, "SELECT COUNT(*) FROM sesion_caja"), 1);
        assert_eq!(
            contar(conexion, "SELECT total FROM venta WHERE id = 1"),
            800_000_000
        );

        // Y lo que trajeron las migraciones nuevas está en su sitio: las
        // tres cuentas de consumo sembradas por la 9.
        assert_eq!(contar(conexion, "SELECT COUNT(*) FROM cuenta_consumo"), 3);
        assert_eq!(contar(conexion, "SELECT COUNT(*) FROM venta_en_espera"), 0);
        assert_eq!(contar(conexion, "SELECT COUNT(*) FROM historial_costo"), 0);

        Ok(())
    })
    .expect("comprobar");

    // La base queda sana: ni corrupción ni referencias rotas.
    {
        let conexion = cruda(&ruta);
        let integridad: String = conexion
            .query_row("PRAGMA integrity_check", [], |fila| fila.get(0))
            .expect("integrity_check");
        assert_eq!(integridad, "ok");
        assert_eq!(
            contar(&conexion, "SELECT COUNT(*) FROM pragma_foreign_key_check"),
            0
        );
    }

    // Y antes de tocar nada dejó la copia de seguridad del estado viejo,
    // que es la red si la actualización hubiera salido mal.
    let respaldo = respaldo_de(&ruta, 6);
    assert!(respaldo.exists(), "falta el respaldo previo a migrar");
    let viejo = cruda(&respaldo);
    assert_eq!(version_de(&viejo), 6, "el respaldo es del esquema anterior");
    assert_eq!(contar(&viejo, "SELECT COUNT(*) FROM venta"), 1);

    drop(base);
    limpiar(&ruta);
}
