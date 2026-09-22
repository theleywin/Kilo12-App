//! Dónde está físicamente la mercancía.
//!
//! El sistema reconoce exactamente dos ubicaciones y no son configurables
//! (R-10). La distinción es el corazón del inventario de Kilo12: la venta
//! descuenta de la vitrina, no del total, porque tener cuarenta unidades en
//! bodega con la vitrina vacía es una venta perdida (RF-VIT-06).
//!
//! Este enum es también el punto que el riesgo RI-1 pide mantener aislado:
//! si algún día hiciera falta más de una vitrina, este es el único lugar que
//! debería crecer.

use core::fmt;
use core::str::FromStr;

use crate::error::ErrorDominio;

/// Ubicación de una existencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ubicacion {
    /// Trastienda, almacén, cajas cerradas. No expuesta al cliente.
    Bodega,
    /// Estantes y mostrador. Es lo único disponible para venta inmediata.
    Vitrina,
}

impl Ubicacion {
    /// Las dos ubicaciones, en el orden en que se muestran.
    pub const TODAS: [Self; 2] = [Self::Bodega, Self::Vitrina];

    /// Texto con que se guarda en la base de datos.
    pub const fn como_texto(self) -> &'static str {
        match self {
            Self::Bodega => "BODEGA",
            Self::Vitrina => "VITRINA",
        }
    }

    /// Nombre para mostrar al usuario.
    pub const fn nombre(self) -> &'static str {
        match self {
            Self::Bodega => "Bodega",
            Self::Vitrina => "Vitrina",
        }
    }

    /// Indica si desde esta ubicación se puede vender.
    pub const fn permite_venta(self) -> bool {
        matches!(self, Self::Vitrina)
    }

    /// La otra ubicación. Útil para plantear un traspaso.
    pub const fn opuesta(self) -> Self {
        match self {
            Self::Bodega => Self::Vitrina,
            Self::Vitrina => Self::Bodega,
        }
    }
}

impl fmt::Display for Ubicacion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.nombre())
    }
}

impl FromStr for Ubicacion {
    type Err = ErrorDominio;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        match texto {
            "BODEGA" => Ok(Self::Bodega),
            "VITRINA" => Ok(Self::Vitrina),
            _ => Err(ErrorDominio::UbicacionDesconocida),
        }
    }
}
