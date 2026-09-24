//! Errores de la capa de aplicación.
//!
//! Envuelven los del dominio y añaden los que solo existen al coordinar un
//! caso de uso: que la persistencia falle, que no se encuentre lo pedido o
//! que se viole una regla que el dominio no puede comprobar por sí solo,
//! como la unicidad del SKU —que exige mirar todo el catálogo, no solo el
//! producto que se está creando—.

use core::fmt;

use domain::ErrorDominio;

/// Error producido al ejecutar un caso de uso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorAplicacion {
    /// Una regla de negocio rechazó la operación.
    Dominio(ErrorDominio),
    /// El almacenamiento falló. El texto es diagnóstico, no para el usuario.
    Persistencia(String),
    /// No existe la entidad solicitada.
    NoEncontrado { entidad: &'static str, id: i64 },
    /// Ya hay un producto con ese SKU (RF-CAT-02).
    SkuDuplicado(String),
    /// Se agotaron los desempates al derivar un SKU del nombre (RF-CAT-02).
    SkuAgotado(String),
    /// Entra mercancía sin decir cuánto costó (RF-COS-04).
    CostoRequerido,
    /// Se indicó un costo sin mercancía a la que aplicarlo (RF-COS-02).
    CantidadInicialRequerida,
    /// Se intentó cobrar en dólares sin tasa de cambio configurada.
    TasaNoConfigurada,
    /// La clave de mantenimiento no coincide.
    ClaveIncorrecta,
}

impl ErrorAplicacion {
    /// Código estable, pensado para que la interfaz reaccione distinto según
    /// el caso (DT-7).
    pub fn codigo(&self) -> &'static str {
        match self {
            Self::Dominio(error) => error.codigo(),
            Self::Persistencia(_) => "PERSISTENCIA",
            Self::NoEncontrado { .. } => "NO_ENCONTRADO",
            Self::SkuDuplicado(_) => "SKU_DUPLICADO",
            Self::SkuAgotado(_) => "SKU_AGOTADO",
            Self::CostoRequerido => "COSTO_REQUERIDO",
            Self::CantidadInicialRequerida => "CANTIDAD_INICIAL_REQUERIDA",
            Self::TasaNoConfigurada => "TASA_NO_CONFIGURADA",
            Self::ClaveIncorrecta => "CLAVE_INCORRECTA",
        }
    }

    /// Indica si el error se debe a lo que hizo el usuario.
    ///
    /// Un fallo de persistencia no es culpa suya y merece otro tratamiento
    /// en pantalla.
    pub const fn es_del_usuario(&self) -> bool {
        !matches!(self, Self::Persistencia(_))
    }
}

impl fmt::Display for ErrorAplicacion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dominio(error) => write!(f, "{error}"),
            Self::Persistencia(detalle) => {
                write!(f, "No se pudo acceder a los datos: {detalle}")
            }
            Self::NoEncontrado { entidad, id } => {
                write!(f, "No se encontró {entidad} con identificador {id}")
            }
            Self::SkuDuplicado(sku) => {
                write!(f, "Ya existe un producto con el SKU «{sku}»")
            }
            Self::CostoRequerido => f.write_str(
                "Indica cuánto te costó: si entra mercancía sin costo, el inventario \
                 valdría cero y toda venta parecería ganancia pura",
            ),
            Self::TasaNoConfigurada => {
                f.write_str("Falta la tasa de cambio: fíjala antes de cobrar en dólares")
            }
            Self::ClaveIncorrecta => f.write_str("La clave no es correcta"),
            Self::CantidadInicialRequerida => f.write_str(
                "Indica cuánta mercancía entra: el costo se calcula sobre la existencia, \
                 así que sin cantidad no hay dónde guardarlo",
            ),
            Self::SkuAgotado(base) => {
                write!(
                    f,
                    "Hay demasiados productos que generan el código «{base}»: \
                     escribe un SKU a mano"
                )
            }
        }
    }
}

impl core::error::Error for ErrorAplicacion {}

impl From<ErrorDominio> for ErrorAplicacion {
    fn from(error: ErrorDominio) -> Self {
        Self::Dominio(error)
    }
}

/// Resultado de un caso de uso.
pub type Resultado<T> = core::result::Result<T, ErrorAplicacion>;
