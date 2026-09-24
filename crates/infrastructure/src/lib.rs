//! Adaptadores de persistencia de Kilo12.
//!
//! Implementa sobre SQLite los puertos que declara la capa de aplicación.
//! Es la única capa que conoce el motor de base de datos; si algún día se
//! cambiara, este crate es lo único que habría que reescribir (DT-5).

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod conexion;
pub mod error;
pub mod migraciones;
pub mod repositorio_producto;

pub use conexion::BaseDatos;
pub use error::{ErrorInfra, ResultadoInfra};
pub use repositorio_producto::RepositorioProductoSqlite;
