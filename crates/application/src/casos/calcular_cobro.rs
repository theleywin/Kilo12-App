//! Caso de uso: ¿alcanza lo que está poniendo el cliente? (RF-VTA-10).
//!
//! La pantalla necesita enseñar, mientras se teclea, cuánto falta o cuánto
//! hay que devolver. No lo calcula ella: sumar pesos con dólares convertidos
//! es aritmética de dinero, y encima el resultado es la cifra que se le
//! entrega al cliente en la mano.
//!
//! Usa exactamente las mismas reglas que el cobro de verdad —incluida la de
//! que de una transferencia no sale vuelto—, así que lo que se ve aquí es lo
//! que va a pasar.

use domain::{Cobro, Dinero, ErrorDominio, MetodoPago, Pago, TasaCambio};

use crate::casos::vender::{PagoPedido, CLAVE_TASA};
use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::RepositorioProducto;

/// Lo que el cliente pone sobre el mostrador, frente a lo que debe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CobroCalculado {
    /// Suma de todo lo entregado, medido en pesos.
    pub entregado: String,
    /// Lo que todavía falta. `0.00` si ya alcanza.
    pub falta: String,
    /// Lo que hay que devolver, siempre en pesos (R-11).
    pub vuelto: String,
    pub alcanza: bool,
}

/// Calcula si el pago cubre la venta.
#[derive(Debug)]
pub struct CalcularCobro<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> CalcularCobro<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, total: &str, pedidos: &[PagoPedido]) -> Resultado<CobroCalculado> {
        let total: Dinero = total.trim().parse()?;
        let mut pagos = Vec::with_capacity(pedidos.len());

        for pedido in pedidos {
            let metodo: MetodoPago = pedido.metodo.trim().parse()?;
            let entregado: Dinero = pedido.entregado.trim().parse()?;

            pagos.push(if metodo.requiere_conversion() {
                Pago::en_usd(entregado, self.tasa_vigente()?)?
            } else {
                Pago::en_cup(metodo, entregado)?
            });
        }

        // Sin nada puesto todavía, falta la venta entera. No es un error:
        // es el estado normal antes de que el cliente saque el dinero.
        if pagos.is_empty() {
            return Ok(CobroCalculado {
                entregado: Dinero::CERO.formatear(2),
                falta: total.formatear(2),
                vuelto: Dinero::CERO.formatear(2),
                alcanza: total.es_cero(),
            });
        }

        let cobro = Cobro::con_pagos(pagos)?;
        let entregado = cobro.entregado_en_cup()?;

        Ok(match cobro.vuelto(total) {
            Ok(vuelto) => CobroCalculado {
                entregado: entregado.formatear(2),
                falta: Dinero::CERO.formatear(2),
                vuelto: vuelto.formatear(2),
                alcanza: true,
            },
            // Que no alcance no es un fallo que interrumpa: es lo que pasa
            // mientras el cliente todavía está contando.
            Err(ErrorDominio::PagoInsuficiente { .. }) => CobroCalculado {
                entregado: entregado.formatear(2),
                falta: total.restar(entregado)?.formatear(2),
                vuelto: Dinero::CERO.formatear(2),
                alcanza: false,
            },
            Err(otro) => return Err(ErrorAplicacion::from(otro)),
        })
    }

    fn tasa_vigente(&self) -> Resultado<TasaCambio> {
        let guardada = self
            .repositorio
            .configuracion(CLAVE_TASA)?
            .ok_or(ErrorAplicacion::TasaNoConfigurada)?;

        let cup_por_usd: Dinero = guardada.trim().parse()?;
        TasaCambio::nueva(cup_por_usd).map_err(ErrorAplicacion::from)
    }
}
