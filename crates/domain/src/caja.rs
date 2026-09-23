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

/// Acumulado de una sesión: qué se vendió y qué hay en la gaveta.
///
/// **Aquí viven dos cosas que no son la misma**, y mezclarlas es lo que hace
/// que una caja no cuadre nunca:
///
/// - `efectivo_cup`, `transferencia` y `efectivo_usd_en_cup` son el
///   **desglose de la venta** por forma de pago. Suman el total vendido.
/// - `efectivo_cup_neto` son los **billetes de peso** que quedaron en la
///   gaveta: lo que entró en efectivo menos lo que salió como vuelto.
///
/// Se separan porque el vuelto **siempre se devuelve en pesos** (R-11),
/// incluso cuando el cliente pagó en dólares. Una venta de 800 pagada con
/// 200 en efectivo y 2 USD deja 200 imputados a la venta en efectivo, pero
/// la gaveta pierde 40 pesos: entraron 200 y salieron 240 de vuelto. Si el
/// arqueo usara la cifra de venta, marcaría un sobrante de 240 en cada
/// cobro de ese tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TotalesCaja {
    efectivo_cup: Dinero,
    transferencia: Dinero,
    /// Dólares recibidos, en su propia moneda.
    efectivo_usd: Dinero,
    /// Equivalente en pesos de esos dólares, con las tasas de cada cobro.
    efectivo_usd_en_cup: Dinero,
    /// Billetes de peso que quedaron en la gaveta por las ventas: lo
    /// entregado en efectivo menos el vuelto devuelto.
    efectivo_cup_neto: Dinero,
}

impl TotalesCaja {
    pub const VACIOS: Self = Self {
        efectivo_cup: Dinero::CERO,
        transferencia: Dinero::CERO,
        efectivo_usd: Dinero::CERO,
        efectivo_usd_en_cup: Dinero::CERO,
        efectivo_cup_neto: Dinero::CERO,
    };

    /// Construye los totales a partir de las sumas de la sesión.
    ///
    /// `efectivo_cup` se deduce por diferencia —total vendido menos lo
    /// cobrado por transferencia y en dólares— para que el desglose sume
    /// exactamente el total y no quede un descuadre de redondeo entre
    /// ambas cifras (RF-CAJ-05c).
    pub fn nuevos(
        total_vendido: Dinero,
        transferencia: Dinero,
        efectivo_usd: Dinero,
        efectivo_usd_en_cup: Dinero,
        efectivo_cup_neto: Dinero,
    ) -> Result<Self, ErrorDominio> {
        let efectivo_cup = total_vendido
            .restar(transferencia)?
            .restar(efectivo_usd_en_cup)?;

        Ok(Self {
            efectivo_cup,
            transferencia,
            efectivo_usd,
            efectivo_usd_en_cup,
            efectivo_cup_neto,
        })
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

    /// Billetes de peso que las ventas dejaron en la gaveta.
    ///
    /// **No es la venta cobrada en efectivo**: es lo entregado en billetes
    /// menos el vuelto devuelto. Excluye la transferencia, que nunca pasó
    /// por la caja, y descuenta el vuelto de los pagos en dólares, que sale
    /// de esta misma gaveta (RF-CAJ-04).
    pub const fn efectivo_cup_en_caja(&self) -> Dinero {
        self.efectivo_cup_neto
    }
}

/// Billetes de peso cubano con que se cuenta la caja, de menor a mayor.
///
/// El orden importa: es el que se recorre al contar, y contar de menor a
/// mayor es como se hace con los fajos en la mano.
pub const DENOMINACIONES_CUP: [i64; 12] = [
    5, 10, 20, 50, 100, 200, 500, 1_000, 2_000, 5_000, 10_000, 20_000,
];

/// Suma un recuento de billetes.
///
/// Recibe pares de `(denominación, cuántos)` y devuelve lo que suman. La
/// cuenta se hace aquí y no en la pantalla por la misma razón que todas las
/// demás: es la cifra contra la que se arquea la caja, y un céntimo de
/// diferencia por redondeo son treinta minutos buscando un descuadre que no
/// existe.
pub fn contar_billetes(recuento: &[(i64, i64)]) -> Result<Dinero, ErrorDominio> {
    recuento
        .iter()
        .try_fold(Dinero::CERO, |suma, (valor, cuantos)| {
            if *valor <= 0 || *cuantos < 0 {
                return Err(ErrorDominio::ImporteNoPositivo);
            }

            let parcial = valor
                .checked_mul(*cuantos)
                .ok_or(ErrorDominio::DesbordeAritmetico)?;

            suma.sumar(Dinero::desde_unidades(parcial)?)
        })
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

#[cfg(test)]
mod pruebas {
    use super::*;

    fn dinero(texto: &str) -> Dinero {
        texto.parse().expect("importe válido")
    }

    /// Venta de 800 pagada con 200 en efectivo y 2 USD a 420, vuelto 240.
    ///
    /// Es el caso que rompe el modelo ingenuo: la gaveta pierde pesos en una
    /// venta que sí fue un ingreso.
    fn totales_con_vuelto_en_pesos() -> TotalesCaja {
        TotalesCaja::nuevos(
            dinero("800.00"), // total vendido
            Dinero::CERO,     // nada por transferencia
            dinero("2.00"),   // 2 USD recibidos
            dinero("840.00"), // que valen 840
            dinero("-40.00"), // entraron 200, salieron 240 de vuelto
        )
        .expect("totales")
    }

    #[test]
    fn el_desglose_de_la_venta_suma_el_total() {
        let totales = totales_con_vuelto_en_pesos();

        // 800 − 0 de transferencia − 840 en dólares = −40 imputados a
        // efectivo: el cliente pagó de más en divisa y se le devolvió.
        assert_eq!(totales.efectivo_cup().formatear(2), "-40.00");
        assert_eq!(totales.total_consolidado().unwrap().formatear(2), "800.00");
    }

    #[test]
    fn la_gaveta_no_cuenta_la_venta_sino_los_billetes() {
        let totales = totales_con_vuelto_en_pesos();

        // Lo que hay que contar al arquear son los billetes que quedaron,
        // no lo que se vendió.
        assert_eq!(totales.efectivo_cup_en_caja().formatear(2), "-40.00");
        assert_eq!(totales.efectivo_usd().formatear(2), "2.00");
    }

    #[test]
    fn la_transferencia_es_venta_pero_no_es_caja() {
        // Venta de 1 000 por transferencia: nada entró a la gaveta.
        let totales = TotalesCaja::nuevos(
            dinero("1000.00"),
            dinero("1000.00"),
            Dinero::CERO,
            Dinero::CERO,
            Dinero::CERO,
        )
        .expect("totales");

        assert_eq!(totales.total_en_cup().unwrap().formatear(2), "1000.00");
        // RF-CAJ-04: sumarla marcaría un faltante de 1 000 al arquear.
        assert_eq!(totales.efectivo_cup_en_caja().formatear(2), "0.00");
    }

    #[test]
    fn el_efectivo_esperado_parte_del_fondo_y_los_movimientos() {
        // Fondo 500, ventas en efectivo 800 netos, entra 100, sale 200.
        let totales = TotalesCaja::nuevos(
            dinero("800.00"),
            Dinero::CERO,
            Dinero::CERO,
            Dinero::CERO,
            dinero("800.00"),
        )
        .expect("totales");

        let cierre = ResumenCierre::calcular(
            totales,
            ModoCierre::Separado,
            dinero("500.00"),
            dinero("100.00"),
            dinero("200.00"),
            dinero("1200.00"),
            Dinero::CERO,
        )
        .expect("cierre");

        // 500 + 800 + 100 − 200 = 1 200
        assert_eq!(cierre.arqueo_cup.esperado.formatear(2), "1200.00");
        assert!(cierre.cuadra().unwrap());
    }

    #[test]
    fn un_faltante_se_ve_en_negativo_y_no_cuadra() {
        let totales = TotalesCaja::nuevos(
            dinero("800.00"),
            Dinero::CERO,
            Dinero::CERO,
            Dinero::CERO,
            dinero("800.00"),
        )
        .expect("totales");

        let cierre = ResumenCierre::calcular(
            totales,
            ModoCierre::Separado,
            dinero("500.00"),
            Dinero::CERO,
            Dinero::CERO,
            dinero("1250.00"),
            Dinero::CERO,
        )
        .expect("cierre");

        // Esperaba 1 300 y hay 1 250: faltan 50.
        assert_eq!(
            cierre.arqueo_cup.diferencia().unwrap().formatear(2),
            "-50.00"
        );
        assert!(!cierre.cuadra().unwrap());
    }

    #[test]
    fn los_dolares_se_arquean_aparte_y_en_su_moneda() {
        let totales = TotalesCaja::nuevos(
            dinero("840.00"),
            Dinero::CERO,
            dinero("2.00"),
            dinero("840.00"),
            Dinero::CERO,
        )
        .expect("totales");

        let cierre = ResumenCierre::calcular(
            totales,
            ModoCierre::Separado,
            Dinero::CERO,
            Dinero::CERO,
            Dinero::CERO,
            Dinero::CERO,
            dinero("2.00"),
        )
        .expect("cierre");

        // Se cuentan 2 dólares, no su equivalente: son otro fajo.
        assert_eq!(cierre.arqueo_usd.esperado.formatear(2), "2.00");
        assert!(cierre.cuadra().unwrap());
    }

    #[test]
    fn el_modo_decide_si_los_dolares_se_suman_o_se_muestran_aparte() {
        let totales = TotalesCaja::nuevos(
            dinero("1840.00"),
            Dinero::CERO,
            dinero("2.00"),
            dinero("840.00"),
            dinero("1000.00"),
        )
        .expect("totales");

        let separado = ResumenCierre::calcular(
            totales,
            ModoCierre::Separado,
            Dinero::CERO,
            Dinero::CERO,
            Dinero::CERO,
            Dinero::CERO,
            Dinero::CERO,
        )
        .expect("cierre");

        // Separado: solo los pesos. Consolidado: todo junto.
        assert_eq!(separado.total_vendido().unwrap().formatear(2), "1000.00");
        assert_eq!(
            separado
                .con_modo(ModoCierre::Consolidado)
                .total_vendido()
                .unwrap()
                .formatear(2),
            "1840.00"
        );
    }
}

#[cfg(test)]
mod pruebas_conteo {
    use super::*;

    #[test]
    fn suma_los_billetes_contados() {
        // 3 de 1 000, 5 de 200 y 4 de 50 = 3 000 + 1 000 + 200
        let total = contar_billetes(&[(1_000, 3), (200, 5), (50, 4)]).expect("contar");
        assert_eq!(total.formatear(2), "4200.00");
    }

    #[test]
    fn contar_cero_billetes_no_suma_nada() {
        let total = contar_billetes(&[(1_000, 0), (500, 0)]).expect("contar");
        assert_eq!(total.formatear(2), "0.00");
    }

    #[test]
    fn no_se_cuenta_un_numero_negativo_de_billetes() {
        assert_eq!(
            contar_billetes(&[(1_000, -1)]).expect_err("no existen billetes negativos"),
            ErrorDominio::ImporteNoPositivo
        );
    }

    #[test]
    fn las_denominaciones_van_de_menor_a_mayor() {
        // Se cuentan los fajos en ese orden; si la lista se desordena, la
        // pantalla deja de parecerse a la mesa.
        assert!(DENOMINACIONES_CUP.windows(2).all(|par| par[0] < par[1]));
    }
}
