//! Totales y arqueo de una sesión de caja.
//!
//! Aquí conviven dos cifras que se parecen y **no son lo mismo**, y
//! confundirlas es el error que hace que una caja nunca cuadre:
//!
//! - **Total vendido**: todo lo que entró por ventas, sin importar cómo se
//!   pagó. Incluye las transferencias.
//! - **Efectivo esperado en caja**: cuántos billetes debe haber físicamente.
//!   **No** incluye las transferencias, porque ese dinero fue a una cuenta y
//!   nunca pasó por la gaveta. Sumarlas marcaría un faltante en cada cierre.
//!
//! Los dólares se cuentan aparte de los pesos, por la misma razón: son fajos
//! distintos y un faltante en uno no puede taparse con el otro.

use crate::dinero::Dinero;
use crate::error::ErrorDominio;
use crate::pago::{MetodoPago, Pago};
use crate::tasa_cambio::TasaCambio;

/// Cómo se presentan los dólares en el cierre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModoCierre {
    /// Los dólares se muestran aparte, en su moneda.
    ///
    /// El cierre dice «10 000 CUP y 10 USD».
    #[default]
    Separado,
    /// Los dólares se convierten a pesos y se suman al total.
    ///
    /// El cierre dice «10 000 CUP + 10 USD = 17 000 CUP».
    Consolidado,
}

/// Acumulado de cobros de una sesión, desglosado por método de pago.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TotalesCaja {
    efectivo_cup: Dinero,
    transferencia: Dinero,
    /// Dólares recibidos, en su propia moneda.
    efectivo_usd: Dinero,
    /// Equivalente en pesos de esos dólares, con las tasas de cada cobro.
    efectivo_usd_en_cup: Dinero,
}

impl TotalesCaja {
    pub const VACIOS: Self = Self {
        efectivo_cup: Dinero::CERO,
        transferencia: Dinero::CERO,
        efectivo_usd: Dinero::CERO,
        efectivo_usd_en_cup: Dinero::CERO,
    };

    /// Suma un cobro al acumulado.
    ///
    /// El importe que se acumula es el de la venta, no lo que el cliente
    /// entregó: el vuelto sale de la caja y no es ingreso.
    pub fn agregar(&self, pago: &Pago, importe_venta: Dinero) -> Result<Self, ErrorDominio> {
        if importe_venta.es_negativo() {
            return Err(ErrorDominio::DineroNegativo);
        }

        let mut totales = *self;
        match pago.metodo() {
            MetodoPago::EfectivoCup => {
                totales.efectivo_cup = self.efectivo_cup.sumar(importe_venta)?;
            }
            MetodoPago::Transferencia => {
                totales.transferencia = self.transferencia.sumar(importe_venta)?;
            }
            MetodoPago::EfectivoUsd => {
                let tasa = pago.tasa().ok_or(ErrorDominio::TasaCambioRequerida)?;
                totales.efectivo_usd = self.efectivo_usd.sumar(tasa.a_usd(importe_venta)?)?;
                totales.efectivo_usd_en_cup = self.efectivo_usd_en_cup.sumar(importe_venta)?;
            }
        }
        Ok(totales)
    }

    pub const fn efectivo_cup(&self) -> Dinero {
        self.efectivo_cup
    }

    pub const fn transferencia(&self) -> Dinero {
        self.transferencia
    }

    /// Dólares recibidos, en dólares.
    pub const fn efectivo_usd(&self) -> Dinero {
        self.efectivo_usd
    }

    /// Equivalente en pesos de los dólares recibidos.
    pub const fn efectivo_usd_en_cup(&self) -> Dinero {
        self.efectivo_usd_en_cup
    }

    /// Ventas cobradas en pesos: efectivo más transferencia.
    ///
    /// Las transferencias se suman aquí porque son venta, aunque no sean
    /// billetes.
    pub fn total_en_cup(&self) -> Result<Dinero, ErrorDominio> {
        self.efectivo_cup.sumar(self.transferencia)
    }

    /// Total vendido con los dólares ya convertidos a pesos.
    ///
    /// Es la cifra del cierre consolidado.
    pub fn total_consolidado(&self) -> Result<Dinero, ErrorDominio> {
        self.total_en_cup()?.sumar(self.efectivo_usd_en_cup)
    }

    /// Efectivo en pesos que debe haber en la caja por ventas.
    ///
    /// Excluye la transferencia, que nunca entró a la gaveta.
    pub const fn efectivo_cup_en_caja(&self) -> Dinero {
        self.efectivo_cup
    }
}

/// Arqueo de una moneda: lo que el sistema espera frente a lo que se contó.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArqueoMoneda {
    pub esperado: Dinero,
    pub contado: Dinero,
}

impl ArqueoMoneda {
    pub const fn nuevo(esperado: Dinero, contado: Dinero) -> Self {
        Self { esperado, contado }
    }

    /// Diferencia entre lo contado y lo esperado.
    ///
    /// Positiva es sobrante; negativa, faltante.
    pub fn diferencia(&self) -> Result<Dinero, ErrorDominio> {
        self.contado.restar(self.esperado)
    }

    pub fn cuadra(&self) -> Result<bool, ErrorDominio> {
        Ok(self.diferencia()?.es_cero())
    }
}

/// Resultado del cierre de una sesión de caja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResumenCierre {
    pub totales: TotalesCaja,
    pub modo: ModoCierre,
    /// Arqueo del efectivo en pesos.
    pub arqueo_cup: ArqueoMoneda,
    /// Arqueo de los dólares, en dólares. Se cuentan siempre aparte.
    pub arqueo_usd: ArqueoMoneda,
}

impl ResumenCierre {
    /// Calcula el cierre de la sesión.
    ///
    /// `fondo_inicial`, `entradas` y `salidas` son movimientos de efectivo en
    /// pesos ajenos a la venta (RF-CAJ-03).
    #[allow(clippy::too_many_arguments)]
    pub fn calcular(
        totales: TotalesCaja,
        modo: ModoCierre,
        fondo_inicial: Dinero,
        entradas: Dinero,
        salidas: Dinero,
        contado_cup: Dinero,
        contado_usd: Dinero,
    ) -> Result<Self, ErrorDominio> {
        // El efectivo esperado NO incluye transferencias ni dólares:
        // son billetes de peso los que se cuentan aquí.
        let esperado_cup = fondo_inicial
            .sumar(totales.efectivo_cup_en_caja())?
            .sumar(entradas)?
            .restar(salidas)?;

        Ok(Self {
            totales,
            modo,
            arqueo_cup: ArqueoMoneda::nuevo(esperado_cup, contado_cup),
            arqueo_usd: ArqueoMoneda::nuevo(totales.efectivo_usd(), contado_usd),
        })
    }

    /// Total vendido según el modo elegido.
    ///
    /// En modo separado devuelve solo los pesos; los dólares se consultan
    /// aparte con [`TotalesCaja::efectivo_usd`].
    pub fn total_vendido(&self) -> Result<Dinero, ErrorDominio> {
        match self.modo {
            ModoCierre::Separado => self.totales.total_en_cup(),
            ModoCierre::Consolidado => self.totales.total_consolidado(),
        }
    }

    /// Cambia la forma de presentar los dólares sin recalcular nada.
    ///
    /// El dueño puede alternar entre las dos vistas en la misma pantalla de
    /// cierre.
    pub const fn con_modo(mut self, modo: ModoCierre) -> Self {
        self.modo = modo;
        self
    }

    /// Indica si ambas monedas cuadran.
    pub fn cuadra(&self) -> Result<bool, ErrorDominio> {
        Ok(self.arqueo_cup.cuadra()? && self.arqueo_usd.cuadra()?)
    }

    /// Líneas del cierre, listas para mostrar.
    ///
    /// Es la diferencia entre «10 000 CUP y 10 USD» y «10 000 CUP + 10 USD =
    /// 17 000 CUP».
    pub fn lineas(&self, tasa: TasaCambio) -> Result<Vec<(String, String)>, ErrorDominio> {
        let mut lineas = vec![
            (
                "Efectivo en pesos".to_string(),
                self.totales.efectivo_cup().formatear(2),
            ),
            (
                "Transferencias".to_string(),
                self.totales.transferencia().formatear(2),
            ),
        ];

        match self.modo {
            ModoCierre::Separado => {
                lineas.push((
                    "Total en pesos".to_string(),
                    self.totales.total_en_cup()?.formatear(2),
                ));
                lineas.push((
                    "Dólares".to_string(),
                    format!("{} USD", self.totales.efectivo_usd().formatear(2)),
                ));
            }
            ModoCierre::Consolidado => {
                lineas.push((
                    format!(
                        "Dólares convertidos ({} USD a {})",
                        self.totales.efectivo_usd().formatear(2),
                        tasa
                    ),
                    self.totales.efectivo_usd_en_cup().formatear(2),
                ));
                lineas.push((
                    "Total en pesos".to_string(),
                    self.totales.total_consolidado()?.formatear(2),
                ));
            }
        }

        Ok(lineas)
    }
}
