//! La sesión de caja: el turno durante el cual se cobra.
//!
//! Una sesión empieza cuando alguien pone el fondo inicial en la gaveta y
//! termina cuando se cuenta lo que hay dentro. Entre medias, toda venta le
//! pertenece (RF-CAJ-02), y ese vínculo es lo que permite preguntar «¿cuánto
//! debería haber aquí ahora mismo?».
//!
//! Dos reglas mandan sobre todo lo demás:
//!
//! - **Una sesión cerrada es inmutable** (RF-CAJ-08). Ya se contó el dinero
//!   físico contra sus cifras; dejar que cambien después convertiría ese
//!   arqueo en una firma sobre un papel en blanco. Por eso el cierre guarda
//!   sus totales congelados en vez de recalcularlos al leerlos.
//! - **Solo puede haber una sesión abierta a la vez.** Dos turnos abiertos
//!   compartiendo la misma gaveta hacen imposible saber a quién imputar un
//!   faltante.
//!
//! El arqueo en sí —esperado contra contado, por moneda— vive en
//! [`crate::caja`]. Aquí está el turno; allí, la cuenta.

use crate::dinero::Dinero;
use crate::error::ErrorDominio;

/// Identificador de una sesión de caja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdSesion(pub i64);

/// En qué punto de su vida está una sesión.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EstadoSesion {
    /// Se está cobrando contra ella.
    #[default]
    Abierta,
    /// Ya se arqueó. No admite ni una operación más.
    Cerrada,
}

impl EstadoSesion {
    /// Texto con que se guarda en la base de datos.
    pub const fn como_texto(self) -> &'static str {
        match self {
            Self::Abierta => "ABIERTA",
            Self::Cerrada => "CERRADA",
        }
    }

    pub const fn esta_abierta(self) -> bool {
        matches!(self, Self::Abierta)
    }
}

impl core::str::FromStr for EstadoSesion {
    type Err = ErrorDominio;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        match texto {
            "ABIERTA" => Ok(Self::Abierta),
            "CERRADA" => Ok(Self::Cerrada),
            _ => Err(ErrorDominio::EstadoSesionDesconocido),
        }
    }
}

/// En qué dirección se mueve el efectivo por algo que no es una venta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoMovimientoEfectivo {
    /// Entra dinero a la gaveta: ingreso de cambio, aporte del dueño.
    Entrada,
    /// Sale dinero: retiro parcial, pago de un gasto, pago de comisión.
    Salida,
}

impl TipoMovimientoEfectivo {
    pub const fn como_texto(self) -> &'static str {
        match self {
            Self::Entrada => "ENTRADA",
            Self::Salida => "SALIDA",
        }
    }

    pub const fn nombre(self) -> &'static str {
        match self {
            Self::Entrada => "Entrada",
            Self::Salida => "Salida",
        }
    }

    pub const fn suma(self) -> bool {
        matches!(self, Self::Entrada)
    }
}

impl core::str::FromStr for TipoMovimientoEfectivo {
    type Err = ErrorDominio;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        match texto {
            "ENTRADA" => Ok(Self::Entrada),
            "SALIDA" => Ok(Self::Salida),
            _ => Err(ErrorDominio::TipoMovimientoDesconocido),
        }
    }
}

/// Un movimiento de efectivo ajeno a la venta (RF-CAJ-03).
///
/// El motivo es obligatorio y va en el constructor, no en un `setter`: un
/// retiro sin explicación es indistinguible de un faltante, y a la hora de
/// cuadrar la caja esa diferencia lo es todo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovimientoEfectivo {
    tipo: TipoMovimientoEfectivo,
    importe: Dinero,
    motivo: String,
}

impl MovimientoEfectivo {
    pub fn nuevo(
        tipo: TipoMovimientoEfectivo,
        importe: Dinero,
        motivo: impl Into<String>,
    ) -> Result<Self, ErrorDominio> {
        if !importe.es_positivo() {
            return Err(ErrorDominio::ImporteNoPositivo);
        }

        let motivo = motivo.into();
        if motivo.trim().is_empty() {
            return Err(ErrorDominio::MotivoObligatorio);
        }

        Ok(Self {
            tipo,
            importe,
            motivo: motivo.trim().to_owned(),
        })
    }

    pub const fn tipo(&self) -> TipoMovimientoEfectivo {
        self.tipo
    }

    pub const fn importe(&self) -> Dinero {
        self.importe
    }

    pub fn motivo(&self) -> &str {
        &self.motivo
    }

    /// Lo que este movimiento suma o resta al efectivo de la gaveta.
    pub fn efecto(&self) -> Result<Dinero, ErrorDominio> {
        if self.tipo.suma() {
            Ok(self.importe)
        } else {
            Dinero::CERO.restar(self.importe)
        }
    }
}

/// Una sesión de caja.
///
/// Guarda lo que la define —quién la atiende y con cuánto empezó— y su
/// estado. Los totales de venta no viven aquí: se derivan de las ventas que
/// le pertenecen mientras está abierta, y se congelan en el cierre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SesionCaja {
    id: Option<IdSesion>,
    /// Quién atiende la caja. No es una cuenta de usuario: es un dato de la
    /// sesión (R-3, RF-CAJ-09).
    operador: String,
    fondo_inicial: Dinero,
    estado: EstadoSesion,
}

impl SesionCaja {
    /// Abre una sesión con su fondo inicial (RF-CAJ-01).
    pub fn abrir(operador: impl Into<String>, fondo_inicial: Dinero) -> Result<Self, ErrorDominio> {
        if fondo_inicial.es_negativo() {
            return Err(ErrorDominio::DineroNegativo);
        }

        let operador = operador.into();
        if operador.trim().is_empty() {
            return Err(ErrorDominio::OperadorRequerido);
        }

        Ok(Self {
            id: None,
            operador: operador.trim().to_owned(),
            fondo_inicial,
            estado: EstadoSesion::Abierta,
        })
    }

    /// Reconstruye una sesión ya guardada.
    pub fn rehidratar(
        id: IdSesion,
        operador: impl Into<String>,
        fondo_inicial: Dinero,
        estado: EstadoSesion,
    ) -> Self {
        Self {
            id: Some(id),
            operador: operador.into(),
            fondo_inicial,
            estado,
        }
    }

    pub const fn id(&self) -> Option<IdSesion> {
        self.id
    }

    pub fn operador(&self) -> &str {
        &self.operador
    }

    pub const fn fondo_inicial(&self) -> Dinero {
        self.fondo_inicial
    }

    pub const fn estado(&self) -> EstadoSesion {
        self.estado
    }

    pub const fn esta_abierta(&self) -> bool {
        self.estado.esta_abierta()
    }

    /// Comprueba que la sesión admite operaciones.
    ///
    /// Es el guardián de RF-CAJ-08: cualquier cosa que pretenda tocar una
    /// sesión pasa por aquí antes, y una cerrada no deja pasar nada.
    pub const fn exigir_abierta(&self) -> Result<(), ErrorDominio> {
        if self.estado.esta_abierta() {
            Ok(())
        } else {
            Err(ErrorDominio::SesionCerrada)
        }
    }

    /// Marca la sesión como cerrada.
    pub fn cerrar(&mut self) -> Result<(), ErrorDominio> {
        self.exigir_abierta()?;
        self.estado = EstadoSesion::Cerrada;
        Ok(())
    }
}

/// Comisión que le corresponde al operador por la sesión (RF-CMS, D-6).
///
/// La base es la **venta total** de la sesión, no la ganancia: así el
/// operador puede verificar su propio importe sabiendo solo cuánto se
/// vendió, sin que haga falta enseñarle los costos de compra (RI-3).
///
/// El porcentaje se congela al cerrar. Si mañana el dueño lo cambia, lo ya
/// liquidado no se mueve: es un hecho auditable, igual que el costo
/// congelado en una línea de venta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comision {
    /// Venta total de la sesión.
    base: Dinero,
    /// Porcentaje aplicado, en la escala de [`crate::Porcentaje`].
    porcentaje: i64,
    importe: Dinero,
}

impl Comision {
    /// Calcula la comisión de una sesión.
    pub fn calcular(base: Dinero, porcentaje: i64) -> Result<Self, ErrorDominio> {
        if base.es_negativo() {
            return Err(ErrorDominio::DineroNegativo);
        }
        if porcentaje < 0 {
            return Err(ErrorDominio::PorcentajeInvalido);
        }

        Ok(Self {
            base,
            porcentaje,
            importe: base.aplicar_porcentaje(porcentaje)?,
        })
    }

    /// Reconstruye una comisión ya liquidada, sin recalcularla.
    ///
    /// Se usa al leer una sesión cerrada: sus cifras son las que se pactaron
    /// entonces, no las que saldrían hoy.
    pub const fn liquidada(base: Dinero, porcentaje: i64, importe: Dinero) -> Self {
        Self {
            base,
            porcentaje,
            importe,
        }
    }

    pub const fn base(&self) -> Dinero {
        self.base
    }

    pub const fn porcentaje(&self) -> i64 {
        self.porcentaje
    }

    pub const fn importe(&self) -> Dinero {
        self.importe
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn dinero(texto: &str) -> Dinero {
        texto.parse().expect("importe válido")
    }

    #[test]
    fn una_sesion_cerrada_no_admite_nada_mas() {
        let mut sesion = SesionCaja::abrir("Yaneisy", dinero("500.00")).expect("abrir");
        assert!(sesion.exigir_abierta().is_ok());

        sesion.cerrar().expect("cerrar");

        // RF-CAJ-08: el arqueo ya se hizo contra dinero físico.
        assert_eq!(
            sesion.exigir_abierta().expect_err("ya está cerrada"),
            ErrorDominio::SesionCerrada
        );
        assert_eq!(
            sesion.cerrar().expect_err("no se cierra dos veces"),
            ErrorDominio::SesionCerrada
        );
    }

    #[test]
    fn la_sesion_necesita_saber_quien_la_atiende() {
        assert_eq!(
            SesionCaja::abrir("   ", dinero("500.00")).expect_err("sin operador"),
            ErrorDominio::OperadorRequerido
        );
    }

    #[test]
    fn el_fondo_inicial_no_puede_ser_negativo() {
        assert_eq!(
            SesionCaja::abrir("Yaneisy", dinero("-1.00")).expect_err("fondo negativo"),
            ErrorDominio::DineroNegativo
        );
    }

    #[test]
    fn un_movimiento_de_efectivo_exige_motivo() {
        assert_eq!(
            MovimientoEfectivo::nuevo(TipoMovimientoEfectivo::Salida, dinero("200.00"), "  ")
                .expect_err("sin motivo"),
            ErrorDominio::MotivoObligatorio
        );
    }

    #[test]
    fn la_salida_resta_y_la_entrada_suma() {
        let salida = MovimientoEfectivo::nuevo(
            TipoMovimientoEfectivo::Salida,
            dinero("200.00"),
            "pago de la luz",
        )
        .expect("salida");
        let entrada = MovimientoEfectivo::nuevo(
            TipoMovimientoEfectivo::Entrada,
            dinero("50.00"),
            "ingreso de cambio",
        )
        .expect("entrada");

        assert_eq!(salida.efecto().unwrap().formatear(2), "-200.00");
        assert_eq!(entrada.efecto().unwrap().formatear(2), "50.00");
    }

    #[test]
    fn no_se_mueve_efectivo_de_cero() {
        assert_eq!(
            MovimientoEfectivo::nuevo(TipoMovimientoEfectivo::Entrada, Dinero::CERO, "nada")
                .expect_err("cero no es un movimiento"),
            ErrorDominio::ImporteNoPositivo
        );
    }

    #[test]
    fn la_comision_sale_de_la_venta_total_no_de_la_ganancia() {
        // 10 000 vendidos al 5 %: 500, sin mirar costos ni márgenes.
        let comision = Comision::calcular(dinero("10000.00"), 50_000).expect("comisión");

        assert_eq!(comision.importe().formatear(2), "500.00");
        assert_eq!(comision.base().formatear(2), "10000.00");
    }

    #[test]
    fn una_comision_liquidada_no_se_recalcula() {
        // La sesión se cerró al 5 % y el dueño lo subió al 10 % después.
        // Lo pagado sigue siendo lo pagado.
        let liquidada = Comision::liquidada(dinero("10000.00"), 50_000, dinero("500.00"));

        assert_eq!(liquidada.porcentaje(), 50_000);
        assert_eq!(liquidada.importe().formatear(2), "500.00");
    }
}
