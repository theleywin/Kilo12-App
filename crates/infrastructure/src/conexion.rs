//! Apertura y configuración de la base de datos.
//!
//! Los PRAGMA de esta función no son opcionales ni cosméticos. En especial
//! `foreign_keys`: SQLite las declara pero **no las aplica** salvo que se
//! activen en cada conexión, así que sin esta línea las relaciones del
//! esquema serían decorativas.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::ResultadoInfra;
use crate::migraciones;

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
    pub fn abrir(ruta: impl AsRef<Path>) -> ResultadoInfra<Self> {
        let conexion = Connection::open(ruta)?;
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
