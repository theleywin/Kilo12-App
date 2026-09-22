//! Tasa de cambio entre el dólar y el peso cubano.
//!
//! El sistema opera sin conexión (R-1), así que no hay servicio que consulte
//! la tasa: la teclea el dueño y la actualiza cuando cambia.
//!
//! De ahí se sigue lo más importante de este módulo: **la tasa aplicada se
//! congela en cada venta**. Si mañana el dueño la sube, el cobro de hoy debe
//! seguir diciendo lo mismo que dijo. Es el mismo principio que con el costo
//! congelado en la línea de venta (RF-VTA-13) y con el porcentaje de comisión
//! del cierre (RF-CMS-06): lo que ya ocurrió es un hecho, no algo que se
//! recalcule.

use core::fmt;

use crate::dinero::{dividir_redondeando, Dinero, ESCALA};
use crate::error::ErrorDominio;

/// Pesos cubanos que equivalen a un dólar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TasaCambio(Dinero);

impl TasaCambio {
    /// Crea una tasa a partir de los pesos que vale un dólar.
    ///
    /// Una tasa de cero o negativa no significa nada y haría desaparecer el
    /// dinero de las conversiones.
    pub fn nueva(cup_por_usd: Dinero) -> Result<Self, ErrorDominio> {
        if !cup_por_usd.es_positivo() {
            return Err(ErrorDominio::TasaCambioInvalida);
        }
        Ok(Self(cup_por_usd))
    }

    /// Pesos que vale un dólar con esta tasa.
    pub const fn cup_por_usd(self) -> Dinero {
        self.0
    }

    /// Convierte un importe en dólares a pesos cubanos.
    ///
    /// Es lo que ocurre al cobrar en dólares: 10 USD con la tasa a 700 son
    /// 7 000 CUP.
    pub fn a_cup(self, usd: Dinero) -> Result<Dinero, ErrorDominio> {
        let factor = 10_i128
            .checked_pow(ESCALA)
            .ok_or(ErrorDominio::DesbordeAritmetico)?;

        let producto = (usd.millonesimas() as i128)
            .checked_mul(self.0.millonesimas() as i128)
            .ok_or(ErrorDominio::DesbordeAritmetico)?;

        let cup = dividir_redondeando(producto, factor)?;

        i64::try_from(cup)
            .map(Dinero::desde_millonesimas)
            .map_err(|_| ErrorDominio::DesbordeAritmetico)
    }

    /// Convierte un importe en pesos cubanos a dólares.
    ///
    /// Sirve para mostrar a cuánto equivale un total cuando el cliente
    /// pregunta, no para cobrar: los precios se fijan en pesos.
    pub fn a_usd(self, cup: Dinero) -> Result<Dinero, ErrorDominio> {
        let factor = 10_i128
            .checked_pow(ESCALA)
            .ok_or(ErrorDominio::DesbordeAritmetico)?;

        let dividendo = (cup.millonesimas() as i128)
            .checked_mul(factor)
            .ok_or(ErrorDominio::DesbordeAritmetico)?;

        let usd = dividir_redondeando(dividendo, self.0.millonesimas() as i128)?;

        i64::try_from(usd)
            .map(Dinero::desde_millonesimas)
            .map_err(|_| ErrorDominio::DesbordeAritmetico)
    }
}

impl fmt::Display for TasaCambio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "1 USD = {} CUP", self.0.formatear(2))
    }
}
