//! Errores de infraestructura y su traducción a la capa de aplicación.

use application::ErrorAplicacion;
use domain::ErrorDominio;

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
    /// Una regla del negocio que solo la base puede comprobar.
    ///
    /// «Ya hay una caja abierta» o «esa sesión ya se cerró» no son fallos
    /// técnicos: son reglas, y su unicidad la garantiza un índice, no un
    /// `if` previo que dos procesos podrían saltarse a la vez. Viajan como
    /// error de dominio para que la pantalla reciba su código de siempre y
    /// no un «error de persistencia» que no dice nada.
    Dominio(ErrorDominio),
}

pub type ResultadoInfra<T> = core::result::Result<T, ErrorInfra>;

impl core::fmt::Display for ErrorInfra {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Sqlite(error) => write!(f, "{error}"),
            Self::DatoCorrupto(detalle) => {
                write!(f, "dato ilegible en la base de datos: {detalle}")
            }
            Self::Dominio(error) => write!(f, "{error}"),
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
        match error {
            // La regla de negocio conserva su identidad hasta la pantalla.
            ErrorInfra::Dominio(dominio) => Self::Dominio(dominio),
            otro => Self::Persistencia(otro.to_string()),
        }
    }
}
