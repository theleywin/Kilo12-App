//! Ensamblado de las dependencias de la aplicación.
//!
//! Este es el único punto del programa donde se decide **qué** implementa
//! cada puerto. Las capas de arriba reciben la implementación ya elegida y
//! no saben cuál es (DT-5).

use std::path::PathBuf;
use std::sync::Arc;

use infrastructure::{
    BaseDatos, RepositorioConsumoSqlite, RepositorioProductoSqlite, RepositorioVentaEnEsperaSqlite,
    ResultadoInfra,
};

/// Dependencias vivas mientras la aplicación se ejecuta.
#[derive(Debug)]
pub struct Estado {
    repositorio_producto: RepositorioProductoSqlite,
    /// Comparte la conexión con el de productos: cobrar una venta en espera
    /// la borra en la misma transacción que registra la venta (RNF-5).
    repositorio_venta_en_espera: RepositorioVentaEnEsperaSqlite,
    /// También comparte la conexión: un consumo escribe la deuda, la
    /// existencia y los asientos del kárdex en una sola transacción, y el
    /// candado de la conexión no es reentrante (DT-16).
    repositorio_consumo: RepositorioConsumoSqlite,
}

impl Estado {
    /// Abre la base de datos y construye los adaptadores.
    pub fn iniciar(ruta_datos: PathBuf) -> ResultadoInfra<Self> {
        if let Some(carpeta) = ruta_datos.parent() {
            // Falla más adelante con un mensaje claro si no se puede crear.
            let _ = std::fs::create_dir_all(carpeta);
        }

        let base = Arc::new(BaseDatos::abrir(ruta_datos)?);

        Ok(Self {
            repositorio_producto: RepositorioProductoSqlite::nuevo(Arc::clone(&base)),
            repositorio_venta_en_espera: RepositorioVentaEnEsperaSqlite::nuevo(Arc::clone(&base)),
            repositorio_consumo: RepositorioConsumoSqlite::nuevo(base),
        })
    }

    pub const fn repositorio_producto(&self) -> &RepositorioProductoSqlite {
        &self.repositorio_producto
    }

    pub const fn repositorio_venta_en_espera(&self) -> &RepositorioVentaEnEsperaSqlite {
        &self.repositorio_venta_en_espera
    }

    pub const fn repositorio_consumo(&self) -> &RepositorioConsumoSqlite {
        &self.repositorio_consumo
    }
}
