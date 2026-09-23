//! Casos de uso de Kilo12.
//!
//! Esta capa coordina el dominio y declara, en forma de puertos, lo que
//! necesita del exterior. No sabe que existe SQLite ni Tauri: recibe
//! implementaciones de sus puertos y las usa (DT-5).
//!
//! Los datos entran y salen como texto en los importes y las cantidades. No
//! es descuido: un número de JSON es `f64` y perdería la exactitud que todo
//! el dominio se toma el trabajo de garantizar (DT-7).

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod casos;
pub mod error;
pub mod margen;
pub mod puertos;
pub mod sku;

pub use error::{ErrorAplicacion, Resultado};
pub use margen::Margen;
pub use puertos::{
    Asiento, CambioDePrecio, CambioRegistrado, DescuentoVenta, DetalleVenta, LineaRegistrada,
    MovimientoRegistrado, PagoRegistrado, ProductoConInventario, RepositorioProducto, ResumenDia,
    VentaConfirmada, VentaRegistrada,
};
