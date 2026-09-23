//! Errores de infraestructura y su traducción a la capa de aplicación.

use application::ErrorAplicacion;

/// Error al hablar con el almacenamiento.
#[derive(Debug)]
pub enum ErrorInfra {
    /// Falla de SQLite.
    Sqlite(rusqlite::Error),
    /// Un dato guardado no se pudo interpretar.
    ///
    /// Indica que la base de datos contiene algo que el dominio no reconoce:
    /// una unidad inválida, un texto donde se esperaba un número. No debería
    /// ocurrir, y si ocurre conviene que se note.
    DatoCorrupto(String),
}

pub type ResultadoInfra<T> = core::result::Result<T, ErrorInfra>;

impl core::fmt::Display for ErrorInfra {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Sqlite(error) => write!(f, "{error}"),
            Self::DatoCorrupto(detalle) => {
                write!(f, "dato ilegible en la base de datos: {detalle}")
            }
        }
    }
}

impl core::error::Error for ErrorInfra {}

impl From<rusqlite::Error> for ErrorInfra {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

/// Traduce el error hacia la capa de aplicación.
///
/// La aplicación no debe conocer SQLite, así que el detalle técnico se
/// conserva como texto de diagnóstico y no como tipo.
impl From<ErrorInfra> for ErrorAplicacion {
    fn from(error: ErrorInfra) -> Self {
        Self::Persistencia(error.to_string())
    }
}
