//! Porcentajes con aritmética exacta.
//!
//! Se guardan en diezmilésimas dentro de un entero de 64 bits (DT-4): el 2 %
//! es `20_000`. Sirven para la comisión del operador de caja y para los
//! márgenes, y valen las mismas razones que para el dinero: nada de punto
//! flotante.

use core::fmt;
use core::str::FromStr;

use crate::dinero::{analizar_escalado, formatear_escalado, Dinero};
use crate::error::ErrorDominio;

/// Decimales que se guardan internamente.
pub const ESCALA: u32 = 4;

/// Porcentaje expresado en diezmilésimas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Porcentaje(i64);

impl Porcentaje {
    pub const CERO: Self = Self(0);

    /// Construye un porcentaje a partir de sus diezmilésimas.
    ///
    /// Es la puerta de entrada desde la base de datos.
    pub const fn desde_diezmilesimas(diezmilesimas: i64) -> Self {
        Self(diezmilesimas)
    }

    /// Diezmilésimas que representa. Es lo que se persiste.
    pub const fn diezmilesimas(self) -> i64 {
        self.0
    }

    /// Construye un porcentaje a partir de unidades enteras: `2` es el 2 %.
    pub fn desde_enteros(enteros: i64) -> Result<Self, ErrorDominio> {
        enteros
            .checked_mul(10_000)
            .map(Self)
            .ok_or(ErrorDominio::DesbordeAritmetico)
    }

    pub const fn es_cero(self) -> bool {
        self.0 == 0
    }

    pub const fn es_negativo(self) -> bool {
        self.0 < 0
    }

    /// Aplica el porcentaje sobre un importe.
    ///
    /// Es el cálculo de la comisión: el 2 % de una venta de 100 000 da 2 000
    /// (RF-CMS-03).
    pub fn aplicar_a(self, importe: Dinero) -> Result<Dinero, ErrorDominio> {
        importe.aplicar_porcentaje(self.0)
    }

    /// Calcula qué porcentaje representa `parte` sobre `total`.
    ///
    /// Es el margen: ganancia sobre precio de venta.
    pub fn de_razon(parte: Dinero, total: Dinero) -> Result<Self, ErrorDominio> {
        if total.es_cero() {
            return Err(ErrorDominio::DivisionPorCero);
        }

        // Se escala primero para no perder precisión en la división.
        let numerador = (parte.millonesimas() as i128)
            .checked_mul(1_000_000)
            .ok_or(ErrorDominio::DesbordeAritmetico)?;

        let razon = crate::dinero::dividir_redondeando(numerador, total.millonesimas() as i128)?;

        i64::try_from(razon)
            .map(Self)
            .map_err(|_| ErrorDominio::DesbordeAritmetico)
    }

    /// Representación con `decimales` posiciones, para mostrar al usuario.
    pub fn formatear(self, decimales: u32) -> String {
        formatear_escalado(self.0, ESCALA, decimales)
    }
}

impl fmt::Display for Porcentaje {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&formatear_escalado(self.0, ESCALA, ESCALA))
    }
}

impl FromStr for Porcentaje {
    type Err = ErrorDominio;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        analizar_escalado(texto, ESCALA).map(Self)
    }
}
