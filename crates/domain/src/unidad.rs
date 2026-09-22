//! Unidad base en que se cuenta un producto.
//!
//! Cada producto tiene exactamente una, y no cambia una vez que hay
//! movimientos registrados (RF-PRS-01, RF-CAT-04). Toda existencia y todo
//! costo del producto se expresan en ella.

use core::fmt;
use core::str::FromStr;

use crate::error::ErrorDominio;

/// Unidad en que se registran la existencia y el costo de un producto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnidadBase {
    /// Artículos que se cuentan de uno en uno.
    Unidad,
    Kilogramo,
    /// Unidad habitual de venta a granel en el mercadito.
    ///
    /// Es una unidad base por derecho propio, no una presentación sobre el
    /// kilogramo: una libra son 0,45359237 kg y la escala de cantidad guarda
    /// tres decimales, así que convertir en cada operación iría perdiendo
    /// fracciones. Un producto que se vende por libra se cuenta en libras.
    Libra,
    Gramo,
    Litro,
    Mililitro,
}

impl UnidadBase {
    /// Indica si la unidad admite cantidades fraccionarias.
    ///
    /// Medio kilo de arroz tiene sentido; media lata de refresco, no
    /// (RF-CAT-03, RF-PRS-14).
    pub const fn admite_fracciones(self) -> bool {
        !matches!(self, Self::Unidad)
    }

    /// Texto con que se guarda en la base de datos.
    ///
    /// Coincide con los valores admitidos por la restricción de la columna
    /// `unidad_base`.
    pub const fn como_texto(self) -> &'static str {
        match self {
            Self::Unidad => "unidad",
            Self::Kilogramo => "kg",
            Self::Libra => "lb",
            Self::Gramo => "g",
            Self::Litro => "L",
            Self::Mililitro => "ml",
        }
    }

    /// Nombre de la presentación que se crea junto con el producto.
    ///
    /// Todo producto nace con una presentación de factor 1 (RF-PRS-03), y
    /// llamarla «Unidad» para un producto que se vende por kilo confundiría.
    pub const fn nombre_presentacion_unitaria(self) -> &'static str {
        match self {
            Self::Unidad => "Unidad",
            Self::Kilogramo => "Kilogramo",
            Self::Libra => "Libra",
            Self::Gramo => "Gramo",
            Self::Litro => "Litro",
            Self::Mililitro => "Mililitro",
        }
    }

    /// Abreviatura para mostrar junto a una cantidad.
    pub const fn simbolo(self) -> &'static str {
        match self {
            Self::Unidad => "u",
            Self::Kilogramo => "kg",
            Self::Libra => "lb",
            Self::Gramo => "g",
            Self::Litro => "L",
            Self::Mililitro => "ml",
        }
    }
}

impl fmt::Display for UnidadBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.como_texto())
    }
}

impl FromStr for UnidadBase {
    type Err = ErrorDominio;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        match texto {
            "unidad" => Ok(Self::Unidad),
            "kg" => Ok(Self::Kilogramo),
            "lb" => Ok(Self::Libra),
            "g" => Ok(Self::Gramo),
            "L" => Ok(Self::Litro),
            "ml" => Ok(Self::Mililitro),
            _ => Err(ErrorDominio::UnidadDesconocida),
        }
    }
}
