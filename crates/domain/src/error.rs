//! Errores del dominio.
//!
//! Son errores de regla de negocio, no de infraestructura: aquí no aparece
//! nada sobre la base de datos ni sobre el transporte. Cada variante lleva un
//! código estable que la interfaz puede usar para decidir qué hacer, además
//! del mensaje para el usuario (DT-7).

use core::fmt;

/// Error producido por una regla del dominio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorDominio {
    /// La operación aritmética no cabe en el rango representable.
    DesbordeAritmetico,
    /// Se intentó repartir un importe entre cero unidades.
    DivisionPorCero,
    /// El resultado dejaría una existencia negativa (RF-INV-06).
    CantidadNegativa,
    /// Se recibió un importe negativo donde el negocio no lo admite.
    DineroNegativo,
    /// La operación exige una cantidad mayor que cero.
    CantidadNoPositiva,
    /// Se pidió el costo unitario de un producto sin existencia.
    SinExistenciaParaCosto,
    /// No hay existencia suficiente en la ubicación para la operación.
    ExistenciaInsuficiente {
        /// Unidades base disponibles.
        disponible: i64,
        /// Unidades base solicitadas.
        solicitado: i64,
    },
    /// El texto recibido no es un número decimal válido.
    TextoNumericoInvalido,
    /// El texto trae más decimales de los que la escala admite.
    PrecisionExcedida,
    /// Se usó una cantidad fraccionaria en un producto que no es a granel.
    CantidadFraccionariaNoPermitida,
    /// El factor de conversión de una presentación debe ser mayor que cero
    /// (RF-PRS-14).
    FactorInvalido,
    /// Se intentó marcar como granel un producto cuya unidad no se fracciona.
    GranelNoAplicable,
    /// Ya existe una presentación con ese nombre en el producto.
    PresentacionDuplicada,
    /// La unidad de medida recibida no es una de las admitidas.
    UnidadDesconocida,
    /// La ubicación recibida no es bodega ni vitrina.
    UbicacionDesconocida,
    /// La moneda recibida no es una de las admitidas.
    MonedaDesconocida,
    /// El método de pago recibido no es uno de los admitidos.
    MetodoPagoDesconocido,
    /// La tasa de cambio debe ser mayor que cero.
    TasaCambioInvalida,
    /// El pago es en otra moneda y no se indicó la tasa a aplicar.
    TasaCambioRequerida,
    /// El cliente entregó menos de lo que cuesta la venta.
    PagoInsuficiente {
        /// Equivalente en pesos de lo entregado.
        entregado: i64,
        /// Total de la venta, en pesos.
        total: i64,
    },
    /// Un texto obligatorio llegó vacío.
    TextoObligatorio(&'static str),
}

impl ErrorDominio {
    /// Código estable del error, pensado para que la interfaz lo interprete.
    ///
    /// Es parte del contrato: cambiar uno de estos valores rompe a quien los
    /// esté distinguiendo.
    pub const fn codigo(&self) -> &'static str {
        match self {
            Self::DesbordeAritmetico => "DESBORDE_ARITMETICO",
            Self::DivisionPorCero => "DIVISION_POR_CERO",
            Self::CantidadNegativa => "CANTIDAD_NEGATIVA",
            Self::DineroNegativo => "DINERO_NEGATIVO",
            Self::CantidadNoPositiva => "CANTIDAD_NO_POSITIVA",
            Self::SinExistenciaParaCosto => "SIN_EXISTENCIA_PARA_COSTO",
            Self::ExistenciaInsuficiente { .. } => "EXISTENCIA_INSUFICIENTE",
            Self::TextoNumericoInvalido => "TEXTO_NUMERICO_INVALIDO",
            Self::PrecisionExcedida => "PRECISION_EXCEDIDA",
            Self::CantidadFraccionariaNoPermitida => "CANTIDAD_FRACCIONARIA_NO_PERMITIDA",
            Self::FactorInvalido => "FACTOR_INVALIDO",
            Self::GranelNoAplicable => "GRANEL_NO_APLICABLE",
            Self::PresentacionDuplicada => "PRESENTACION_DUPLICADA",
            Self::UnidadDesconocida => "UNIDAD_DESCONOCIDA",
            Self::UbicacionDesconocida => "UBICACION_DESCONOCIDA",
            Self::MonedaDesconocida => "MONEDA_DESCONOCIDA",
            Self::MetodoPagoDesconocido => "METODO_PAGO_DESCONOCIDO",
            Self::TasaCambioInvalida => "TASA_CAMBIO_INVALIDA",
            Self::TasaCambioRequerida => "TASA_CAMBIO_REQUERIDA",
            Self::PagoInsuficiente { .. } => "PAGO_INSUFICIENTE",
            Self::TextoObligatorio(_) => "TEXTO_OBLIGATORIO",
        }
    }
}

impl fmt::Display for ErrorDominio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DesbordeAritmetico => {
                f.write_str("La operación produce un número fuera del rango admitido")
            }
            Self::DivisionPorCero => {
                f.write_str("No se puede repartir un importe entre cero unidades")
            }
            Self::CantidadNegativa => {
                f.write_str("La existencia no puede quedar en negativo")
            }
            Self::DineroNegativo => {
                f.write_str("El importe no puede ser negativo")
            }
            Self::CantidadNoPositiva => {
                f.write_str("La cantidad debe ser mayor que cero")
            }
            Self::SinExistenciaParaCosto => {
                f.write_str("No se puede calcular el costo de un producto sin existencia")
            }
            Self::ExistenciaInsuficiente {
                disponible,
                solicitado,
            } => write!(
                f,
                "No hay existencia suficiente: se solicitan {} y hay {}",
                crate::Cantidad::desde_milesimas(*solicitado).formatear(3),
                crate::Cantidad::desde_milesimas(*disponible).formatear(3),
            ),
            Self::TextoNumericoInvalido => {
                f.write_str("El valor recibido no es un número decimal válido")
            }
            Self::PrecisionExcedida => {
                f.write_str("El valor trae más decimales de los admitidos")
            }
            Self::CantidadFraccionariaNoPermitida => {
                f.write_str("Este producto no se vende a granel: la cantidad debe ser entera")
            }
            Self::FactorInvalido => {
                f.write_str("El factor de conversión debe ser mayor que cero")
            }
            Self::GranelNoAplicable => {
                f.write_str("Este producto se cuenta por unidades y no puede venderse a granel")
            }
            Self::PresentacionDuplicada => {
                f.write_str("Ya existe una presentación con ese nombre")
            }
            Self::UnidadDesconocida => f.write_str("La unidad de medida no es válida"),
            Self::UbicacionDesconocida => f.write_str("La ubicación no es válida"),
            Self::MonedaDesconocida => f.write_str("La moneda no es válida"),
            Self::MetodoPagoDesconocido => f.write_str("El método de pago no es válido"),
            Self::TasaCambioInvalida => {
                f.write_str("La tasa de cambio debe ser mayor que cero")
            }
            Self::TasaCambioRequerida => {
                f.write_str("Falta la tasa de cambio para convertir el pago a pesos")
            }
            Self::PagoInsuficiente { entregado, total } => write!(
                f,
                "El pago no alcanza: se entregaron {} y la venta es de {}",
                crate::Dinero::desde_millonesimas(*entregado).formatear(2),
                crate::Dinero::desde_millonesimas(*total).formatear(2),
            ),
            Self::TextoObligatorio(campo) => {
                write!(f, "El campo «{campo}» no puede quedar vacío")
            }
        }
    }
}

impl core::error::Error for ErrorDominio {}
