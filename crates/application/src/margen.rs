//! Ganancia y margen de una venta (RF-PRE-01).
//!
//! Se calcula aquí, en Rust, y no en la pantalla. No es purismo: el margen
//! sale de restar y dividir importes, y JavaScript no tiene aritmética
//! decimal exacta. Todo el dominio se toma el trabajo de no perder ni una
//! millonésima; regalarla en el último metro, solo para ahorrarse una
//! llamada, sería absurdo.

use domain::{Dinero, ErrorDominio, Porcentaje};

/// Lo que deja una venta sobre su costo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Margen {
    /// Precio menos costo. Puede ser negativo: eso es vender perdiendo.
    pub ganancia: Dinero,
    /// Ganancia sobre el precio de venta, que es el margen bruto.
    pub porcentaje: Porcentaje,
    /// El costo se comió el precio (RF-COM-05).
    pub en_riesgo: bool,
}

/// Calcula la ganancia y el margen de un precio frente a su costo.
pub fn calcular(costo: Dinero, precio: Dinero) -> Result<Margen, ErrorDominio> {
    let ganancia = precio.restar(costo)?;

    // Sin precio no hay margen que calcular, y dividir entre cero sería
    // inventarse un número.
    let porcentaje = if precio.es_cero() {
        Porcentaje::desde_diezmilesimas(0)
    } else {
        Porcentaje::de_razon(ganancia, precio)?
    };

    Ok(Margen {
        ganancia,
        porcentaje,
        en_riesgo: !ganancia.es_positivo(),
    })
}
