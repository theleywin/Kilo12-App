//! Evolución del esquema de la base de datos.
//!
//! Las migraciones se aplican solas al arrancar y se numeran con
//! `PRAGMA user_version` (DT-10). Cada una va dentro de una transacción: si
//! falla, el esquema queda como estaba y la aplicación se niega a arrancar,
//! en lugar de operar sobre datos a medio migrar.
//!
//! **Una migración publicada no se edita jamás.** Un error se corrige con
//! una migración nueva; cambiar una anterior dejaría inconsistentes las
//! bases que ya la aplicaron.

use rusqlite::Connection;

use crate::error::ResultadoInfra;

/// Migraciones en orden. El índice más uno es su número de versión.
const MIGRACIONES: &[&str] = &[
    // 1 — Catálogo y existencias.
    r#"
    CREATE TABLE categoria (
        id      INTEGER PRIMARY KEY,
        nombre  TEXT    NOT NULL UNIQUE,
        activa  INTEGER NOT NULL DEFAULT 1 CHECK (activa IN (0,1))
    );

    CREATE TABLE producto (
        id               INTEGER PRIMARY KEY,
        sku              TEXT    NOT NULL UNIQUE,
        nombre           TEXT    NOT NULL,
        categoria_id     INTEGER REFERENCES categoria(id),
        unidad_base      TEXT    NOT NULL CHECK (unidad_base IN
                                 ('unidad','kg','lb','g','L','ml')),
        es_granel        INTEGER NOT NULL DEFAULT 0 CHECK (es_granel IN (0,1)),
        -- Valor acumulado invertido, en millonésimas. Junto con la suma de
        -- existencias es la fuente de verdad del costo (RF-COS-02).
        valor_total      INTEGER NOT NULL DEFAULT 0 CHECK (valor_total >= 0),
        stock_minimo     INTEGER NOT NULL DEFAULT 0 CHECK (stock_minimo >= 0),
        objetivo_vitrina INTEGER NOT NULL DEFAULT 0 CHECK (objetivo_vitrina >= 0),
        activo           INTEGER NOT NULL DEFAULT 1 CHECK (activo IN (0,1)),
        creado_en        TEXT    NOT NULL
    );

    CREATE TABLE presentacion (
        id                INTEGER PRIMARY KEY,
        producto_id       INTEGER NOT NULL REFERENCES producto(id),
        nombre            TEXT    NOT NULL,
        -- Milésimas: un six-pack es 6000. Debe ser mayor que cero (RF-PRS-14).
        factor            INTEGER NOT NULL CHECK (factor > 0),
        precio            INTEGER NOT NULL CHECK (precio >= 0),
        es_predeterminada INTEGER NOT NULL DEFAULT 0 CHECK (es_predeterminada IN (0,1)),
        codigo_barras     TEXT,
        activa            INTEGER NOT NULL DEFAULT 1 CHECK (activa IN (0,1)),
        UNIQUE (producto_id, nombre)
    );

    CREATE TABLE existencia (
        producto_id INTEGER NOT NULL REFERENCES producto(id),
        ubicacion   TEXT    NOT NULL CHECK (ubicacion IN ('BODEGA','VITRINA')),
        -- Milésimas de la unidad base. Nunca negativa (RF-INV-06).
        cantidad    INTEGER NOT NULL DEFAULT 0 CHECK (cantidad >= 0),
        PRIMARY KEY (producto_id, ubicacion)
    );

    CREATE INDEX idx_producto_nombre    ON producto(nombre);
    CREATE INDEX idx_producto_categoria ON producto(categoria_id);
    CREATE INDEX idx_presentacion_prod  ON presentacion(producto_id);
    "#,
    // 2 — «Se vende en fracciones» deja de ser un dato propio.
    //
    // Se deduce de la unidad base: lo que se cuenta por unidades se vende
    // entero y lo que se pesa o se mide se fracciona (RF-CAT-03). La
    // columna permitía estados que no existen en el mostrador, como «se
    // cuenta en libras pero no se puede vender media libra».
    r#"
    ALTER TABLE producto DROP COLUMN es_granel;
    "#,
];

/// Lleva el esquema a la última versión.
pub fn aplicar(conexion: &Connection) -> ResultadoInfra<()> {
    let version: i64 = conexion.query_row("PRAGMA user_version", [], |fila| fila.get(0))?;
    let version = usize::try_from(version).unwrap_or(0);

    for (indice, migracion) in MIGRACIONES.iter().enumerate().skip(version) {
        conexion.execute_batch(&format!(
            "BEGIN;\n{}\nPRAGMA user_version = {};\nCOMMIT;",
            migracion,
            indice.saturating_add(1)
        ))?;
    }

    Ok(())
}

/// Versión de esquema que esta compilación espera.
///
/// Sirve para rechazar un respaldo creado por una versión posterior
/// (RF-DAT-07).
pub fn version_esperada() -> usize {
    MIGRACIONES.len()
}
