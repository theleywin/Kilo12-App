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
    /// Ya existe una presentación con ese nombre en el producto.
    PresentacionDuplicada,
    /// La presentación indicada no pertenece a este producto.
    PresentacionNoEncontrada,
    /// Es la única forma de venta que le queda al producto (RF-PRS-03).
    UltimaPresentacion,
    /// Se pidió un margen del 100 % o más, que exigiría precio infinito.
    MargenImposible,
    /// Se intentó cobrar una venta sin ninguna línea.
    VentaVacia,
    /// Se intentó confirmar un cobro sin ningún pago.
    CobroSinPagos,
    /// La unidad de medida recibida no es una de las admitidas.
    UnidadDesconocida,
    /// La ubicación recibida no es bodega ni vitrina.
    UbicacionDesconocida,
    /// El tipo de movimiento recibido no es uno de los admitidos.
    TipoMovimientoDesconocido,
    /// El movimiento exige explicar por qué se hizo (RF-INV-08).
    MotivoObligatorio,
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
    /// El estado de sesión recibido no es uno de los admitidos.
    EstadoSesionDesconocido,
    /// Se operó sobre una sesión de caja ya cerrada (RF-CAJ-08).
    SesionCerrada,
    /// Se intentó abrir una sesión habiendo otra sin cerrar.
    SesionYaAbierta,
    /// La operación exige una sesión de caja abierta (RF-CAJ-06).
    SinSesionAbierta,
    /// Falta el nombre de quien atiende la caja (RF-CAJ-09).
    OperadorRequerido,
    /// Se movió un importe de cero o negativo.
    ImporteNoPositivo,
    /// El porcentaje recibido no es válido.
    PorcentajeInvalido,
    /// La venta ya se había anulado.
    VentaYaAnulada,
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
            Self::PresentacionDuplicada => "PRESENTACION_DUPLICADA",
            Self::PresentacionNoEncontrada => "PRESENTACION_NO_ENCONTRADA",
            Self::UltimaPresentacion => "ULTIMA_PRESENTACION",
            Self::MargenImposible => "MARGEN_IMPOSIBLE",
            Self::VentaVacia => "VENTA_VACIA",
            Self::CobroSinPagos => "COBRO_SIN_PAGOS",
            Self::UnidadDesconocida => "UNIDAD_DESCONOCIDA",
            Self::UbicacionDesconocida => "UBICACION_DESCONOCIDA",
            Self::TipoMovimientoDesconocido => "TIPO_MOVIMIENTO_DESCONOCIDO",
            Self::MotivoObligatorio => "MOTIVO_OBLIGATORIO",
            Self::MonedaDesconocida => "MONEDA_DESCONOCIDA",
            Self::MetodoPagoDesconocido => "METODO_PAGO_DESCONOCIDO",
            Self::TasaCambioInvalida => "TASA_CAMBIO_INVALIDA",
            Self::TasaCambioRequerida => "TASA_CAMBIO_REQUERIDA",
            Self::PagoInsuficiente { .. } => "PAGO_INSUFICIENTE",
            Self::TextoObligatorio(_) => "TEXTO_OBLIGATORIO",
            Self::EstadoSesionDesconocido => "ESTADO_SESION_DESCONOCIDO",
            Self::SesionCerrada => "SESION_CERRADA",
            Self::SesionYaAbierta => "SESION_YA_ABIERTA",
            Self::SinSesionAbierta => "SIN_SESION_ABIERTA",
            Self::OperadorRequerido => "OPERADOR_REQUERIDO",
            Self::ImporteNoPositivo => "IMPORTE_NO_POSITIVO",
            Self::PorcentajeInvalido => "PORCENTAJE_INVALIDO",
            Self::VentaYaAnulada => "VENTA_YA_ANULADA",
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
            Self::CantidadNegativa => f.write_str("La existencia no puede quedar en negativo"),
            Self::DineroNegativo => f.write_str("El importe no puede ser negativo"),
            Self::CantidadNoPositiva => f.write_str("La cantidad debe ser mayor que cero"),
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
            Self::PrecisionExcedida => f.write_str("El valor trae más decimales de los admitidos"),
            Self::CantidadFraccionariaNoPermitida => {
                f.write_str("Este producto no se vende a granel: la cantidad debe ser entera")
            }
            Self::FactorInvalido => f.write_str("El factor de conversión debe ser mayor que cero"),
            Self::PresentacionDuplicada => f.write_str("Ya existe una presentación con ese nombre"),
            Self::PresentacionNoEncontrada => {
                f.write_str("Esa presentación no pertenece a este producto")
            }
            Self::UltimaPresentacion => f.write_str(
                "Es la única forma de venderlo que queda: el producto se quedaría invendible",
            ),
            Self::MargenImposible => {
                f.write_str("Un margen del 100 % o más exigiría un precio infinito")
            }
            Self::VentaVacia => f.write_str("No hay nada que cobrar"),
            Self::CobroSinPagos => f.write_str("Falta indicar cómo paga el cliente"),
            Self::UnidadDesconocida => f.write_str("La unidad de medida no es válida"),
            Self::UbicacionDesconocida => f.write_str("La ubicación no es válida"),
            Self::TipoMovimientoDesconocido => f.write_str("El tipo de movimiento no es válido"),
            Self::MotivoObligatorio => {
                f.write_str("Explica el motivo: la mercancía no puede desaparecer sin razón")
            }
            Self::MonedaDesconocida => f.write_str("La moneda no es válida"),
            Self::MetodoPagoDesconocido => f.write_str("El método de pago no es válido"),
            Self::TasaCambioInvalida => f.write_str("La tasa de cambio debe ser mayor que cero"),
            Self::TasaCambioRequerida => {
                f.write_str("Falta la tasa de cambio para convertir el pago a pesos")
            }
            Self::PagoInsuficiente { entregado, total } => write!(
                f,
                "El pago no alcanza: se entregaron {} y la venta es de {}",
                crate::Dinero::desde_millonesimas(*entregado).formatear(2),
                crate::Dinero::desde_millonesimas(*total).formatear(2),
            ),
            Self::EstadoSesionDesconocido => f.write_str("El estado de sesión no es válido"),
            Self::SesionCerrada => f.write_str(
                "La caja de esa sesión ya se cerró y arqueó: no admite cambios. \
                 Registra la corrección en la sesión de hoy",
            ),
            Self::SesionYaAbierta => {
                f.write_str("Ya hay una caja abierta: ciérrala antes de abrir otra")
            }
            Self::SinSesionAbierta => {
                f.write_str("Abre la caja antes de cobrar: sin sesión, la venta no cuadraría")
            }
            Self::OperadorRequerido => f.write_str("Indica quién atiende la caja"),
            Self::ImporteNoPositivo => f.write_str("El importe debe ser mayor que cero"),
            Self::PorcentajeInvalido => f.write_str("El porcentaje no es válido"),
            Self::VentaYaAnulada => f.write_str("Esa venta ya estaba anulada"),
            Self::TextoObligatorio(campo) => {
                write!(f, "El campo «{campo}» no puede quedar vacío")
            }
        }
    }
}

impl core::error::Error for ErrorDominio {}
