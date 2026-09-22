//! Importes monetarios con aritmética exacta.
//!
//! Un `Dinero` guarda millonésimas de la moneda en un entero de 64 bits
//! (DT-4). No existe ninguna conversión desde `f32` ni `f64`, y es a
//! propósito: en punto flotante binario `0.1 + 0.2 != 0.3`, y ese error se
//! acumula movimiento a movimiento hasta descuadrar el inventario.
//!
//! La escala de seis decimales no es capricho. El costo promedio ponderado
//! divide —`$250 / 6 = $41,6666…`— y RF-COS-09 exige que ese resto no se
//! acumule. Con seis decimales el error por operación queda por debajo de una
//! millonésima.

use core::fmt;
use core::str::FromStr;

use crate::cantidad::Cantidad;
use crate::error::ErrorDominio;

/// Decimales que se guardan internamente.
pub const ESCALA: u32 = 6;

/// Millonésimas que forman una unidad monetaria.
const FACTOR: i64 = 1_000_000;

/// Importe monetario exacto.
///
/// Se almacena en millonésimas de la moneda: `$41,67` es `41_670_000`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Dinero(i64);

impl Dinero {
    pub const CERO: Self = Self(0);

    /// Construye un importe a partir de sus millonésimas.
    ///
    /// Es la puerta de entrada desde la base de datos, que guarda justamente
    /// este entero.
    pub const fn desde_millonesimas(millonesimas: i64) -> Self {
        Self(millonesimas)
    }

    /// Millonésimas que representa. Es lo que se persiste.
    pub const fn millonesimas(self) -> i64 {
        self.0
    }

    /// Construye un importe a partir de unidades enteras de la moneda.
    pub fn desde_unidades(unidades: i64) -> Result<Self, ErrorDominio> {
        unidades
            .checked_mul(FACTOR)
            .map(Self)
            .ok_or(ErrorDominio::DesbordeAritmetico)
    }

    pub const fn es_cero(self) -> bool {
        self.0 == 0
    }

    pub const fn es_negativo(self) -> bool {
        self.0 < 0
    }

    pub const fn es_positivo(self) -> bool {
        self.0 > 0
    }

    pub fn sumar(self, otro: Self) -> Result<Self, ErrorDominio> {
        self.0
            .checked_add(otro.0)
            .map(Self)
            .ok_or(ErrorDominio::DesbordeAritmetico)
    }

    pub fn restar(self, otro: Self) -> Result<Self, ErrorDominio> {
        self.0
            .checked_sub(otro.0)
            .map(Self)
            .ok_or(ErrorDominio::DesbordeAritmetico)
    }

    /// Importe total de vender `cantidad` a este precio por unidad.
    ///
    /// El producto intermedio se calcula en 128 bits: multiplicar dos enteros
    /// escalados desborda 64 bits con cifras perfectamente normales.
    pub fn multiplicar_por(self, cantidad: Cantidad) -> Result<Self, ErrorDominio> {
        let producto = (self.0 as i128)
            .checked_mul(cantidad.milesimas() as i128)
            .ok_or(ErrorDominio::DesbordeAritmetico)?;

        // El producto queda con la escala de dinero más la de cantidad;
        // se devuelve a la escala de dinero.
        Self::desde_i128(dividir_redondeando(producto, Cantidad::FACTOR as i128)?)
    }

    /// Reparte este importe entre `cantidad` unidades.
    ///
    /// Es el cálculo del costo unitario: valor total del inventario dividido
    /// entre la existencia (RF-COS-02).
    pub fn dividir_entre(self, cantidad: Cantidad) -> Result<Self, ErrorDominio> {
        if cantidad.es_cero() {
            return Err(ErrorDominio::DivisionPorCero);
        }

        let dividendo = (self.0 as i128)
            .checked_mul(Cantidad::FACTOR as i128)
            .ok_or(ErrorDominio::DesbordeAritmetico)?;

        Self::desde_i128(dividir_redondeando(dividendo, cantidad.milesimas() as i128)?)
    }

    /// Aplica un porcentaje expresado en diezmilésimas.
    ///
    /// Es el cálculo de la comisión del operador de caja: el 2 % llega como
    /// `20_000` (DT-4).
    pub fn aplicar_porcentaje(self, diezmilesimas: i64) -> Result<Self, ErrorDominio> {
        const CIEN_POR_CIENTO: i128 = 1_000_000;

        let producto = (self.0 as i128)
            .checked_mul(diezmilesimas as i128)
            .ok_or(ErrorDominio::DesbordeAritmetico)?;

        Self::desde_i128(dividir_redondeando(producto, CIEN_POR_CIENTO)?)
    }

    /// Devuelve `CERO` cuando el importe es negativo.
    ///
    /// La comisión nunca puede ser negativa (RF-CMS-08).
    pub const fn o_cero_si_negativo(self) -> Self {
        if self.0 < 0 {
            Self::CERO
        } else {
            self
        }
    }

    /// Redondea a `decimales` para presentarlo o para cobrarlo.
    ///
    /// El valor acumulado del inventario nunca se redondea: esto es solo para
    /// el importe que se le cobra al cliente y para lo que se muestra en
    /// pantalla (DT-4).
    pub fn redondear_a(self, decimales: u32) -> Result<Self, ErrorDominio> {
        if decimales >= ESCALA {
            return Ok(self);
        }

        let posiciones = ESCALA
            .checked_sub(decimales)
            .ok_or(ErrorDominio::DesbordeAritmetico)?;
        let paso = 10_i64
            .checked_pow(posiciones)
            .ok_or(ErrorDominio::DesbordeAritmetico)?;

        let redondeado = dividir_redondeando(self.0 as i128, paso as i128)?
            .checked_mul(paso as i128)
            .ok_or(ErrorDominio::DesbordeAritmetico)?;

        Self::desde_i128(redondeado)
    }

    /// Representación con `decimales` posiciones, para mostrar al usuario.
    pub fn formatear(self, decimales: u32) -> String {
        formatear_escalado(self.0, ESCALA, decimales)
    }

    fn desde_i128(valor: i128) -> Result<Self, ErrorDominio> {
        i64::try_from(valor)
            .map(Self)
            .map_err(|_| ErrorDominio::DesbordeAritmetico)
    }
}

/// Texto exacto del importe, con los seis decimales sin recortar.
///
/// Es la forma en que el dinero cruza hacia la interfaz (DT-7): como texto,
/// jamás como número de JSON, que es `f64` y perdería el valor.
impl fmt::Display for Dinero {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&formatear_escalado(self.0, ESCALA, ESCALA))
    }
}

impl FromStr for Dinero {
    type Err = ErrorDominio;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        analizar_escalado(texto, ESCALA).map(Self)
    }
}

/// División entera con redondeo al más cercano, alejándose del cero en el
/// empate.
///
/// La división que trunca pierde hasta una unidad en cada operación, siempre
/// en la misma dirección, y ese sesgo es justo lo que RF-COS-09 prohíbe
/// acumular.
pub(crate) fn dividir_redondeando(dividendo: i128, divisor: i128) -> Result<i128, ErrorDominio> {
    if divisor == 0 {
        return Err(ErrorDominio::DivisionPorCero);
    }

    let cociente = dividendo
        .checked_div(divisor)
        .ok_or(ErrorDominio::DesbordeAritmetico)?;
    let resto = dividendo
        .checked_rem(divisor)
        .ok_or(ErrorDominio::DesbordeAritmetico)?;

    if resto == 0 {
        return Ok(cociente);
    }

    let doble_resto = resto
        .checked_mul(2)
        .ok_or(ErrorDominio::DesbordeAritmetico)?
        .unsigned_abs();

    if doble_resto < divisor.unsigned_abs() {
        return Ok(cociente);
    }

    let ajuste = if (dividendo < 0) == (divisor < 0) { 1 } else { -1 };
    cociente
        .checked_add(ajuste)
        .ok_or(ErrorDominio::DesbordeAritmetico)
}

/// Convierte un entero escalado en texto con los decimales pedidos.
pub(crate) fn formatear_escalado(valor: i64, escala: u32, decimales: u32) -> String {
    // Esta función se usa para mostrar, así que no falla: ante cualquier
    // desborde improbable cae en un valor neutro en lugar de interrumpir.
    let posiciones_sobrantes = escala.checked_sub(decimales);

    let mut valor = valor as i128;
    if let Some(sobrantes) = posiciones_sobrantes.filter(|s| *s > 0) {
        if let Some(paso) = 10_i128.checked_pow(sobrantes) {
            valor = dividir_redondeando(valor, paso)
                .unwrap_or(0)
                .saturating_mul(paso);
        }
    }

    let negativo = valor < 0;
    let absoluto = valor.unsigned_abs();
    let factor = 10_u128.checked_pow(escala).unwrap_or(1);

    let entera = absoluto.checked_div(factor).unwrap_or(0);
    let signo = if negativo { "-" } else { "" };

    if decimales == 0 {
        return format!("{signo}{entera}");
    }

    let divisor = posiciones_sobrantes
        .and_then(|sobrantes| 10_u128.checked_pow(sobrantes))
        .unwrap_or(1);
    let fraccion = absoluto
        .checked_rem(factor)
        .and_then(|resto| resto.checked_div(divisor))
        .unwrap_or(0);
    let ancho = decimales as usize;

    format!("{signo}{entera}.{fraccion:0ancho$}")
}

/// Lee un entero escalado desde su texto decimal.
///
/// Acepta signo, separador decimal y hasta `escala` decimales. Rechaza
/// cualquier cosa que no sea eso: es mejor fallar al leer que arrastrar un
/// importe inventado.
pub(crate) fn analizar_escalado(texto: &str, escala: u32) -> Result<i64, ErrorDominio> {
    let texto = texto.trim();
    if texto.is_empty() {
        return Err(ErrorDominio::TextoNumericoInvalido);
    }

    let (negativo, cuerpo) = match texto.strip_prefix('-') {
        Some(resto) => (true, resto),
        None => (false, texto.strip_prefix('+').unwrap_or(texto)),
    };

    let (entera, fraccion) = match cuerpo.split_once('.') {
        Some((entera, fraccion)) => (entera, fraccion),
        None => (cuerpo, ""),
    };

    if entera.is_empty() && fraccion.is_empty() {
        return Err(ErrorDominio::TextoNumericoInvalido);
    }
    if !entera.bytes().all(|b| b.is_ascii_digit())
        || !fraccion.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(ErrorDominio::TextoNumericoInvalido);
    }
    if fraccion.len() > escala as usize {
        return Err(ErrorDominio::PrecisionExcedida);
    }

    let entera: i64 = if entera.is_empty() {
        0
    } else {
        entera
            .parse()
            .map_err(|_| ErrorDominio::DesbordeAritmetico)?
    };

    // Los decimales escritos se completan hasta la escala interna:
    // «41.67» con escala 6 son 670000 millonésimas, no 67.
    let escritos = u32::try_from(fraccion.len()).map_err(|_| ErrorDominio::PrecisionExcedida)?;
    let faltantes = escala
        .checked_sub(escritos)
        .ok_or(ErrorDominio::PrecisionExcedida)?;

    let fraccion: i64 = if fraccion.is_empty() {
        0
    } else {
        fraccion
            .parse()
            .map_err(|_| ErrorDominio::DesbordeAritmetico)?
    };

    let relleno = 10_i64
        .checked_pow(faltantes)
        .ok_or(ErrorDominio::DesbordeAritmetico)?;
    let fraccion = fraccion
        .checked_mul(relleno)
        .ok_or(ErrorDominio::DesbordeAritmetico)?;

    let factor = 10_i64
        .checked_pow(escala)
        .ok_or(ErrorDominio::DesbordeAritmetico)?;
    let total = entera
        .checked_mul(factor)
        .and_then(|v| v.checked_add(fraccion))
        .ok_or(ErrorDominio::DesbordeAritmetico)?;

    if negativo {
        total.checked_neg().ok_or(ErrorDominio::DesbordeAritmetico)
    } else {
        Ok(total)
    }
}
