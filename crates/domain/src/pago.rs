//! Formas en que el cliente paga y monedas que intervienen.
//!
//! El mercadito acepta tres cosas distintas, y conviene no confundirlas:
//!
//! - **Efectivo en moneda cubana**: billetes que entran a la caja.
//! - **Transferencia**: dinero que llega a una cuenta. No pasa por la caja,
//!   así que no cuenta para el arqueo de efectivo (RF-CAJ-04).
//! - **Efectivo en dólares**: billetes que entran a la caja, pero de otra
//!   moneda, y por tanto se cuentan y se arquean por separado.
//!
//! El arqueo no admite mezclas: al cerrar, el dueño cuenta pesos por un lado
//! y dólares por otro. Sumarlos en un único saldo haría imposible detectar un
//! faltante.

use core::fmt;
use core::str::FromStr;

use crate::dinero::Dinero;
use crate::error::ErrorDominio;
use crate::tasa_cambio::TasaCambio;

/// Moneda en que se expresa un importe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Moneda {
    /// Peso cubano. Es la moneda en que opera el negocio.
    Cup,
    /// Dólar estadounidense.
    Usd,
}

impl Moneda {
    pub const TODAS: [Self; 2] = [Self::Cup, Self::Usd];

    /// Texto con que se guarda en la base de datos.
    pub const fn como_texto(self) -> &'static str {
        match self {
            Self::Cup => "CUP",
            Self::Usd => "USD",
        }
    }

    /// Símbolo para mostrar junto a un importe.
    pub const fn simbolo(self) -> &'static str {
        match self {
            Self::Cup => "$",
            Self::Usd => "USD",
        }
    }

    /// Indica si es la moneda en que se lleva la contabilidad del negocio.
    ///
    /// Todo importe termina expresado en ella: precios, costos, ganancias e
    /// informes.
    pub const fn es_moneda_del_negocio(self) -> bool {
        matches!(self, Self::Cup)
    }
}

impl fmt::Display for Moneda {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.como_texto())
    }
}

impl FromStr for Moneda {
    type Err = ErrorDominio;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        match texto {
            "CUP" => Ok(Self::Cup),
            "USD" => Ok(Self::Usd),
            _ => Err(ErrorDominio::MonedaDesconocida),
        }
    }
}

/// Forma en que el cliente paga una venta (RF-VTA-09).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MetodoPago {
    /// Billetes en pesos cubanos.
    EfectivoCup,
    /// Transferencia bancaria en pesos cubanos.
    Transferencia,
    /// Billetes en dólares estadounidenses.
    EfectivoUsd,
}

impl MetodoPago {
    pub const TODOS: [Self; 3] = [Self::EfectivoCup, Self::Transferencia, Self::EfectivoUsd];

    /// Texto con que se guarda en la base de datos.
    pub const fn como_texto(self) -> &'static str {
        match self {
            Self::EfectivoCup => "EFECTIVO_CUP",
            Self::Transferencia => "TRANSFERENCIA",
            Self::EfectivoUsd => "EFECTIVO_USD",
        }
    }

    /// Nombre para mostrar al usuario.
    pub const fn nombre(self) -> &'static str {
        match self {
            Self::EfectivoCup => "Efectivo",
            Self::Transferencia => "Transferencia",
            Self::EfectivoUsd => "Dólares",
        }
    }

    /// Moneda en que el cliente entrega el dinero.
    pub const fn moneda(self) -> Moneda {
        match self {
            Self::EfectivoCup | Self::Transferencia => Moneda::Cup,
            Self::EfectivoUsd => Moneda::Usd,
        }
    }

    /// Indica si el pago hace entrar billetes a la caja.
    ///
    /// La transferencia no: el dinero llega a una cuenta y no debe alterar el
    /// efectivo esperado al arquear (RF-CAJ-04).
    pub const fn entra_a_la_caja(self) -> bool {
        matches!(self, Self::EfectivoCup | Self::EfectivoUsd)
    }

    /// Indica si admite devolver cambio al cliente.
    ///
    /// Solo el efectivo: de una transferencia no se da vuelto.
    pub const fn admite_cambio(self) -> bool {
        self.entra_a_la_caja()
    }

    /// Indica si el importe necesita convertirse a la moneda del negocio.
    pub const fn requiere_conversion(self) -> bool {
        !self.moneda().es_moneda_del_negocio()
    }
}

impl fmt::Display for MetodoPago {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.nombre())
    }
}

impl FromStr for MetodoPago {
    type Err = ErrorDominio;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        match texto {
            "EFECTIVO_CUP" => Ok(Self::EfectivoCup),
            "TRANSFERENCIA" => Ok(Self::Transferencia),
            "EFECTIVO_USD" => Ok(Self::EfectivoUsd),
            _ => Err(ErrorDominio::MetodoPagoDesconocido),
        }
    }
}

/// Cobro de una venta, con lo que entregó el cliente y su equivalente en
/// pesos.
///
/// Los precios se fijan siempre en pesos; el dólar es solo una forma de
/// pagar. Por eso todo pago termina expresado en pesos, que es la moneda en
/// que el negocio lleva sus cuentas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pago {
    metodo: MetodoPago,
    /// Lo que entregó el cliente, en la moneda del método.
    entregado: Dinero,
    /// Tasa aplicada, presente solo cuando hubo conversión.
    ///
    /// Se guarda con el pago: si el dueño cambia la tasa mañana, este cobro
    /// debe seguir diciendo lo mismo.
    tasa: Option<TasaCambio>,
    /// Equivalente en pesos de lo entregado.
    equivalente_cup: Dinero,
}

impl Pago {
    /// Registra un pago en pesos, ya sea en efectivo o por transferencia.
    pub fn en_cup(metodo: MetodoPago, entregado: Dinero) -> Result<Self, ErrorDominio> {
        if metodo.requiere_conversion() {
            return Err(ErrorDominio::TasaCambioRequerida);
        }
        if entregado.es_negativo() {
            return Err(ErrorDominio::DineroNegativo);
        }

        Ok(Self {
            metodo,
            entregado,
            tasa: None,
            equivalente_cup: entregado,
        })
    }

    /// Registra un pago en dólares, convertido con la tasa vigente.
    pub fn en_usd(entregado: Dinero, tasa: TasaCambio) -> Result<Self, ErrorDominio> {
        if entregado.es_negativo() {
            return Err(ErrorDominio::DineroNegativo);
        }

        Ok(Self {
            metodo: MetodoPago::EfectivoUsd,
            entregado,
            tasa: Some(tasa),
            equivalente_cup: tasa.a_cup(entregado)?,
        })
    }

    pub const fn metodo(&self) -> MetodoPago {
        self.metodo
    }

    /// Lo que entregó el cliente, en la moneda del método.
    pub const fn entregado(&self) -> Dinero {
        self.entregado
    }

    pub const fn tasa(&self) -> Option<TasaCambio> {
        self.tasa
    }

    /// Equivalente en pesos de lo entregado.
    pub const fn equivalente_cup(&self) -> Dinero {
        self.equivalente_cup
    }

    /// Vuelto que corresponde devolver, dado el total de la venta.
    ///
    /// **Siempre en pesos**, aunque el cliente haya pagado en dólares: el
    /// mercadito no devuelve divisa.
    pub fn vuelto(&self, total_venta: Dinero) -> Result<Dinero, ErrorDominio> {
        if self.equivalente_cup < total_venta {
            return Err(ErrorDominio::PagoInsuficiente {
                entregado: self.equivalente_cup.millonesimas(),
                total: total_venta.millonesimas(),
            });
        }

        if !self.metodo.admite_cambio() {
            return Ok(Dinero::CERO);
        }

        self.equivalente_cup.restar(total_venta)
    }
}

/// Todo lo que el cliente entrega para pagar una venta.
///
/// Un cobro puede ser **mixto**: quinientos en efectivo, mil por
/// transferencia y diez dólares. Cada parte conserva su método y su tasa;
/// lo único común es que todas se miden en pesos para compararlas con el
/// total (R-11).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cobro {
    pagos: Vec<Pago>,
}

impl Cobro {
    pub const fn nuevo() -> Self {
        Self { pagos: Vec::new() }
    }

    pub fn con_pagos(pagos: Vec<Pago>) -> Result<Self, ErrorDominio> {
        if pagos.is_empty() {
            return Err(ErrorDominio::CobroSinPagos);
        }
        Ok(Self { pagos })
    }

    pub fn agregar(&mut self, pago: Pago) {
        self.pagos.push(pago);
    }

    pub fn pagos(&self) -> &[Pago] {
        &self.pagos
    }

    /// Suma de todo lo entregado, medido en pesos.
    pub fn entregado_en_cup(&self) -> Result<Dinero, ErrorDominio> {
        self.pagos.iter().try_fold(Dinero::CERO, |suma, pago| {
            suma.sumar(pago.equivalente_cup())
        })
    }

    /// Lo entregado con métodos de los que se puede sacar cambio.
    ///
    /// De una transferencia no se devuelve nada: el dinero ya está en la
    /// cuenta y la gaveta no lo tiene.
    fn entregado_devolvible(&self) -> Result<Dinero, ErrorDominio> {
        self.pagos
            .iter()
            .filter(|pago| pago.metodo().admite_cambio())
            .try_fold(Dinero::CERO, |suma, pago| {
                suma.sumar(pago.equivalente_cup())
            })
    }

    /// Vuelto que corresponde devolver, **siempre en pesos** (R-11).
    ///
    /// Se limita a lo que se pagó en efectivo: si alguien transfiere de más,
    /// ese excedente no se puede devolver en billetes que no entraron.
    pub fn vuelto(&self, total_venta: Dinero) -> Result<Dinero, ErrorDominio> {
        let entregado = self.entregado_en_cup()?;

        if entregado < total_venta {
            return Err(ErrorDominio::PagoInsuficiente {
                entregado: entregado.millonesimas(),
                total: total_venta.millonesimas(),
            });
        }

        let excedente = entregado.restar(total_venta)?;
        let devolvible = self.entregado_devolvible()?;

        Ok(if excedente > devolvible {
            devolvible
        } else {
            excedente
        })
    }
}

#[cfg(test)]
mod pruebas_cobro {
    use super::*;

    fn dinero(texto: &str) -> Dinero {
        texto.parse().expect("importe válido")
    }

    fn tasa() -> TasaCambio {
        TasaCambio::nueva(dinero("420.00")).expect("tasa válida")
    }

    #[test]
    fn un_cobro_mixto_suma_todo_en_pesos() {
        let mut cobro = Cobro::nuevo();
        cobro.agregar(Pago::en_cup(MetodoPago::EfectivoCup, dinero("500.00")).unwrap());
        cobro.agregar(Pago::en_cup(MetodoPago::Transferencia, dinero("1000.00")).unwrap());
        cobro.agregar(Pago::en_usd(dinero("10.00"), tasa()).unwrap());

        // 500 + 1000 + (10 × 420) = 5 700
        assert_eq!(cobro.entregado_en_cup().unwrap().formatear(2), "5700.00");
    }

    #[test]
    fn el_vuelto_sale_en_pesos_aunque_se_pague_en_dolares() {
        let cobro = Cobro::con_pagos(vec![Pago::en_usd(dinero("10.00"), tasa()).unwrap()]).unwrap();

        // Entregó 4 200 en pesos por una venta de 4 000.
        assert_eq!(
            cobro.vuelto(dinero("4000.00")).unwrap().formatear(2),
            "200.00"
        );
    }

    #[test]
    fn de_una_transferencia_no_sale_cambio() {
        let cobro = Cobro::con_pagos(vec![Pago::en_cup(
            MetodoPago::Transferencia,
            dinero("1000.00"),
        )
        .unwrap()])
        .unwrap();

        // Transfirió 1 000 por una venta de 900: el excedente no se devuelve
        // en billetes que nunca entraron en la gaveta.
        assert_eq!(cobro.vuelto(dinero("900.00")).unwrap().formatear(2), "0.00");
    }

    #[test]
    fn el_cambio_se_limita_al_efectivo_recibido() {
        let mut cobro = Cobro::nuevo();
        cobro.agregar(Pago::en_cup(MetodoPago::Transferencia, dinero("1000.00")).unwrap());
        cobro.agregar(Pago::en_cup(MetodoPago::EfectivoCup, dinero("100.00")).unwrap());

        // Entregó 1 100 por una venta de 900: sobran 200, pero solo entraron
        // 100 en efectivo, así que solo eso se puede devolver.
        assert_eq!(
            cobro.vuelto(dinero("900.00")).unwrap().formatear(2),
            "100.00"
        );
    }

    #[test]
    fn no_se_confirma_una_venta_que_no_se_cubre() {
        let cobro = Cobro::con_pagos(vec![
            Pago::en_cup(MetodoPago::EfectivoCup, dinero("50.00")).unwrap()
        ])
        .unwrap();

        assert!(matches!(
            cobro.vuelto(dinero("900.00")),
            Err(ErrorDominio::PagoInsuficiente { .. })
        ));
    }

    #[test]
    fn un_cobro_sin_pagos_no_es_un_cobro() {
        assert_eq!(
            Cobro::con_pagos(Vec::new()).expect_err("nadie pagó nada"),
            ErrorDominio::CobroSinPagos
        );
    }
}
