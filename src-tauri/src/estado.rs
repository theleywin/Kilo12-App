//! Ensamblado de las dependencias de la aplicación.
//!
//! Este es el único punto del programa donde se decide **qué** implementa
//! cada puerto. Las capas de arriba reciben la implementación ya elegida y
//! no saben cuál es (DT-5).

use std::path::PathBuf;
use std::sync::Arc;

use infrastructure::{BaseDatos, RepositorioProductoSqlite, ResultadoInfra};

/// Dependencias vivas mientras la aplicación se ejecuta.
#[derive(Debug)]
pub struct Estado {
    repositorio_producto: RepositorioProductoSqlite,
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
            repositorio_producto: RepositorioProductoSqlite::nuevo(base),
        })
    }

    pub const fn repositorio_producto(&self) -> &RepositorioProductoSqlite {
        &self.repositorio_producto
    }
}
