//! Núcleo de dominio de Kilo12.
//!
//! Aquí viven las reglas del negocio y nada más. Este crate no conoce SQLite,
//! ni Tauri, ni el formato en que sus datos viajan a la interfaz: esa es la
//! regla de dependencia de la arquitectura hexagonal (DT-5), y las flechas
//! apuntan siempre hacia adentro.
//!
//! El vocabulario está en español porque es el del negocio (DT-13): bodega,
//! vitrina, merma y arqueo no tienen traducción limpia, y el código debe
//! poder leerse junto al documento de requerimientos sin traducir nada. Los
//! identificadores van sin tildes ni eñes; los comentarios, con ellas.
//!
//! # Punto de partida
//!
//! Todo el sistema descansa sobre dos tipos: [`Dinero`] y [`Cantidad`].
//! Ambos son enteros escalados, nunca decimales de punto flotante, porque de
//! ahí depende que el inventario cuadre después de miles de movimientos
//! (DT-4).

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod caja;
pub mod cantidad;
pub mod dinero;
pub mod error;
pub mod existencias;
pub mod inventario;
pub mod movimiento;
pub mod pago;
pub mod porcentaje;
pub mod presentacion;
pub mod producto;
pub mod sesion_caja;
pub mod tasa_cambio;
pub mod ubicacion;
pub mod unidad;
pub mod venta;

pub use caja::{
    contar_billetes, ArqueoMoneda, ModoCierre, ResumenCierre, TotalesCaja, DENOMINACIONES_CUP,
};
pub use cantidad::Cantidad;
pub use dinero::Dinero;
pub use error::ErrorDominio;
pub use existencias::Existencias;
pub use inventario::{Inventario, Salida};
pub use movimiento::{Movimiento, TipoMovimiento};
pub use pago::{Cobro, MetodoPago, Moneda, Pago};
pub use porcentaje::Porcentaje;
pub use presentacion::{IdPresentacion, Presentacion};
pub use producto::{IdProducto, Producto};
pub use sesion_caja::{
    Comision, EstadoSesion, IdSesion, MovimientoEfectivo, SesionCaja, TipoMovimientoEfectivo,
};
pub use tasa_cambio::TasaCambio;
pub use ubicacion::Ubicacion;
pub use unidad::UnidadBase;
pub use venta::{IdVenta, LineaVenta, Venta};

/// Resultado de una operación del dominio.
pub type Resultado<T> = core::result::Result<T, ErrorDominio>;
