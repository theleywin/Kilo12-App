//! Apertura y configuración de la base de datos.
//!
//! Los PRAGMA de esta función no son opcionales ni cosméticos. En especial
//! `foreign_keys`: SQLite las declara pero **no las aplica** salvo que se
//! activen en cada conexión, así que sin esta línea las relaciones del
//! esquema serían decorativas.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::{ErrorInfra, ResultadoInfra};
use crate::migraciones;

/// Deja una copia del estado anterior antes de migrar (RF-DAT-02).
///
/// Una migración que reconstruye una tabla reescribe datos que no se pueden
/// recomponer: el kárdex explica por qué la existencia es la que es, y no
/// hay forma de deducirlo de nada. Hasta esta función, el riesgo RT-4
/// declaraba como mitigación un «respaldo automático previo a migrar» que
/// **no existía en el código**.
///
/// ## Por qué `VACUUM INTO` y no copiar el archivo
///
/// La conexión trabaja en modo WAL, así que la base vive en tres archivos:
/// el `.db`, el `-wal` y el `-shm`. Copiar solo el `.db` mientras hay
/// escrituras sin fusionar produce **un respaldo que existe y no sirve**, y
/// eso es peor que no tener ninguno: invita a seguir adelante con confianza
/// falsa. `VACUUM INTO` escribe una base consistente y completa en un solo
/// archivo, sin tocar la original.
///
/// ## Cuándo corre
///
/// Solo si hay migraciones pendientes, y solo si ya había algo que perder:
/// con `user_version = 0` no hay ni tablas, porque las crea la migración 1.
/// Resguardar en cada arranque sería coste sin motivo.
///
/// Se escribe primero con un nombre parcial y se renombra al final. Si el
/// proceso muere a mitad, lo que queda es un `.parcial` evidente y no un
/// respaldo truncado con el nombre del bueno.
fn resguardar(conexion: &Connection, ruta: &Path) -> ResultadoInfra<()> {
    let version: i64 = conexion.query_row("PRAGMA user_version", [], |fila| fila.get(0))?;
    let esperada = i64::try_from(migraciones::version_esperada()).unwrap_or(i64::MAX);

    // Base nueva: la migración 1 crea las tablas, así que en la versión 0
    // no hay nada que resguardar. Y al día: nada que migrar.
    if version == 0 || version >= esperada {
        return Ok(());
    }

    let destino = con_sufijo(ruta, &format!("previo-v{version}"));
    let parcial = con_sufijo(ruta, &format!("previo-v{version}.parcial"));

    // `VACUUM INTO` se niega a escribir sobre un archivo que ya existe, y
    // un intento anterior pudo dejar restos.
    for sobrante in [&parcial, &destino] {
        if sobrante.exists() {
            std::fs::remove_file(sobrante).map_err(|fallo| {
                ErrorInfra::DatoCorrupto(format!(
                    "no se pudo retirar el respaldo anterior {}: {fallo}",
                    sobrante.display()
                ))
            })?;
        }
    }

    let texto = parcial.to_str().ok_or_else(|| {
        ErrorInfra::DatoCorrupto(format!(
            "la ruta del respaldo no es texto válido: {}",
            parcial.display()
        ))
    })?;

    conexion.execute("VACUUM INTO ?1", [texto])?;

    std::fs::rename(&parcial, &destino).map_err(|fallo| {
        ErrorInfra::DatoCorrupto(format!(
            "no se pudo dejar el respaldo en {}: {fallo}",
            destino.display()
        ))
    })?;

    Ok(())
}

/// `kilo12.db` + `previo-v8` → `kilo12.db.previo-v8`.
///
/// Se añade al nombre completo en lugar de cambiar la extensión: así el
/// respaldo queda al lado de su base y ordenado junto a ella.
fn con_sufijo(ruta: &Path, sufijo: &str) -> PathBuf {
    let mut nombre = ruta.as_os_str().to_owned();
    nombre.push(".");
    nombre.push(sufijo);
    PathBuf::from(nombre)
}

/// Base de datos de la aplicación.
///
/// La conexión va protegida por exclusión mutua porque Kilo12 tiene un solo
/// operador (R-3): no hay contención real, y un pool solo añadiría
/// complejidad sin resolver nada (DT-3).
#[derive(Debug)]
pub struct BaseDatos {
    conexion: Mutex<Connection>,
}

impl BaseDatos {
    /// Abre la base de datos en disco, creándola si no existe, y deja el
    /// esquema al día.
    ///
    /// Antes de migrar deja una copia del estado anterior. Ver
    /// [`resguardar`] para el por qué.
    pub fn abrir(ruta: impl AsRef<Path>) -> ResultadoInfra<Self> {
        let ruta = ruta.as_ref();
        let conexion = Connection::open(ruta)?;

        // Antes de `preparar`, que es quien migra. Si esto falla, no se
        // migra: la aplicación se niega a arrancar en lugar de reescribir
        // el esquema sin red debajo.
        resguardar(&conexion, ruta)?;

        Self::preparar(conexion)
    }

    /// Abre una base de datos en memoria.
    ///
    /// Pensada para verificar el comportamiento sin tocar el disco.
    pub fn en_memoria() -> ResultadoInfra<Self> {
        let conexion = Connection::open_in_memory()?;
        Self::preparar(conexion)
    }

    fn preparar(conexion: Connection) -> ResultadoInfra<Self> {
        // Las claves foráneas se aplican por conexión, no por esquema.
        conexion.pragma_update(None, "foreign_keys", "ON")?;
        // Mejor comportamiento ante cortes y lecturas concurrentes.
        conexion.pragma_update(None, "journal_mode", "WAL")?;
        // Más lento a propósito: perder la última venta por un corte de luz
        // es peor que tardar unos milisegundos más (DT-3).
        conexion.pragma_update(None, "synchronous", "FULL")?;
        conexion.busy_timeout(std::time::Duration::from_secs(5))?;

        migraciones::aplicar(&conexion)?;

        Ok(Self {
            conexion: Mutex::new(conexion),
        })
    }

    /// Ejecuta una operación con la conexión.
    ///
    /// Si otro hilo entró en pánico sosteniendo el candado, se recupera el
    /// acceso: la base de datos no quedó corrupta, y negar el servicio a
    /// partir de ahí sería peor que continuar.
    pub fn con<T>(
        &self,
        operacion: impl FnOnce(&Connection) -> ResultadoInfra<T>,
    ) -> ResultadoInfra<T> {
        let guarda = self
            .conexion
            .lock()
            .unwrap_or_else(|envenenado| envenenado.into_inner());
        operacion(&guarda)
    }

    /// Ejecuta una operación dentro de una transacción.
    ///
    /// O se aplica entera o no se aplica nada (RNF-5). Es lo que impide que
    /// una venta deje el inventario a medio actualizar.
    pub fn en_transaccion<T>(
        &self,
        operacion: impl FnOnce(&rusqlite::Transaction<'_>) -> ResultadoInfra<T>,
    ) -> ResultadoInfra<T> {
        let mut guarda = self
            .conexion
            .lock()
            .unwrap_or_else(|envenenado| envenenado.into_inner());

        let transaccion = guarda.transaction()?;
        let resultado = operacion(&transaccion)?;
        transaccion.commit()?;
        Ok(resultado)
    }
}
