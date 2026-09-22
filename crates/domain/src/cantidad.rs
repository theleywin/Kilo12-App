//! Cantidades de inventario expresadas en la unidad base del producto.
//!
//! Una `Cantidad` guarda milésimas de la unidad base en un entero de 64 bits
//! (DT-4): si la unidad base es el kilogramo, la unidad almacenada es el
//! gramo. Toda existencia del sistema está expresada así (R-9).
//!
//! La cantidad no lleva consigo su unidad. La unidad pertenece al producto, y
//! es el producto quien garantiza que no se mezclen kilogramos con litros.

use core::fmt;
use core::str::FromStr;

use crate::dinero::{analizar_escalado, dividir_redondeando, formatear_escalado};
use crate::error::ErrorDominio;

/// Decimales que se guardan internamente.
pub const ESCALA: u32 = 3;

/// Cantidad de producto en milésimas de su unidad base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Cantidad(i64);

impl Cantidad {
    pub const CERO: Self = Self(0);

    /// Milésimas que forman una unidad base.
    pub(crate) const FACTOR: i64 = 1_000;

    /// Construye una cantidad a partir de sus milésimas.
    ///
    /// Es la puerta de entrada desde la base de datos.
    pub const fn desde_milesimas(milesimas: i64) -> Self {
        Self(milesimas)
    }

    /// Milésimas que representa. Es lo que se persiste.
    pub const fn milesimas(self) -> i64 {
        self.0
    }

    /// Construye una cantidad a partir de unidades enteras.
    pub fn desde_unidades(unidades: i64) -> Result<Self, ErrorDominio> {
        unidades
            .checked_mul(Self::FACTOR)
            .map(Self)
            .ok_or(ErrorDominio::DesbordeAritmetico)
    }

    pub const fn es_cero(self) -> bool {
        self.0 == 0
    }

    pub const fn es_negativa(self) -> bool {
        self.0 < 0
    }

    pub const fn es_positiva(self) -> bool {
        self.0 > 0
    }

    /// Indica si la cantidad representa un número entero de unidades.
    ///
    /// Un producto que no se vende a granel no admite fracciones
    /// (RF-CAT-03).
    pub const fn es_entera(self) -> bool {
        self.0 % Self::FACTOR == 0
    }

    pub fn sumar(self, otra: Self) -> Result<Self, ErrorDominio> {
        self.0
            .checked_add(otra.0)
            .map(Self)
            .ok_or(ErrorDominio::DesbordeAritmetico)
    }

    /// Resta otra cantidad, rechazando el resultado negativo.
    ///
    /// Ninguna ubicación puede quedar con existencia negativa (RF-INV-06), y
    /// el sitio donde eso se impide es este.
    pub fn restar(self, otra: Self) -> Result<Self, ErrorDominio> {
        let resultado = self
            .0
            .checked_sub(otra.0)
            .ok_or(ErrorDominio::DesbordeAritmetico)?;

        if resultado < 0 {
            return Err(ErrorDominio::CantidadNegativa);
        }

        Ok(Self(resultado))
    }

    /// Resta permitiendo un resultado negativo.
    ///
    /// Reservado para los ajustes de inventario, que sí necesitan expresar
    /// una diferencia en contra tras un conteo físico (RF-INV-07).
    pub fn restar_con_signo(self, otra: Self) -> Result<Self, ErrorDominio> {
        self.0
            .checked_sub(otra.0)
            .map(Self)
            .ok_or(ErrorDominio::DesbordeAritmetico)
    }

    /// Convierte una cantidad de presentaciones a unidades base.
    ///
    /// Vender un six-pack descuenta seis unidades: el factor llega en
    /// milésimas, igual que la cantidad (RF-PRS-06).
    pub fn multiplicar_por_factor(self, factor: Self) -> Result<Self, ErrorDominio> {
        let producto = (self.0 as i128)
            .checked_mul(factor.0 as i128)
            .ok_or(ErrorDominio::DesbordeAritmetico)?;

        let escalado = dividir_redondeando(producto, Self::FACTOR as i128)?;

        i64::try_from(escalado)
            .map(Self)
            .map_err(|_| ErrorDominio::DesbordeAritmetico)
    }

    /// Representación con `decimales` posiciones, para mostrar al usuario.
    pub fn formatear(self, decimales: u32) -> String {
        formatear_escalado(self.0, ESCALA, decimales)
    }
}

/// Texto exacto de la cantidad, con sus tres decimales.
impl fmt::Display for Cantidad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&formatear_escalado(self.0, ESCALA, ESCALA))
    }
}

impl FromStr for Cantidad {
    type Err = ErrorDominio;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        analizar_escalado(texto, ESCALA).map(Self)
    }
}
