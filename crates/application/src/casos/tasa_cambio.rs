//! Casos de uso de la tasa de cambio (RF-DIV).
//!
//! La tasa es un ajuste del negocio, no un dato de la venta: se fija una
//! vez y vale hasta que el dueño la cambie. Cada venta en dólares se queda
//! con una copia de la que había en ese momento (RF-VTA-10b).

use domain::{Dinero, TasaCambio};

use crate::casos::vender::CLAVE_TASA;
use crate::error::Resultado;
use crate::puertos::RepositorioProducto;

/// Consulta la tasa vigente.
#[derive(Debug)]
pub struct ConsultarTasa<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> ConsultarTasa<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    /// Devuelve la tasa formateada, o nada si todavía no se fijó.
    pub fn ejecutar(&self) -> Resultado<Option<String>> {
        let guardada = self.repositorio.configuracion(CLAVE_TASA)?;

        Ok(match guardada {
            None => None,
            Some(texto) => {
                let valor: Dinero = texto.trim().parse()?;
                Some(valor.formatear(2))
            }
        })
    }
}

/// Fija la tasa de cambio.
#[derive(Debug)]
pub struct FijarTasa<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> FijarTasa<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, cup_por_usd: &str) -> Resultado<()> {
        let valor: Dinero = cup_por_usd.trim().parse()?;

        // Se valida con el tipo del dominio antes de guardarla: una tasa de
        // cero o negativa haría que un dólar no valiera nada.
        TasaCambio::nueva(valor)?;

        self.repositorio
            .guardar_configuracion(CLAVE_TASA, &valor.formatear(2))
    }
}
