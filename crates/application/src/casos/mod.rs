//! Casos de uso, uno por operación del negocio.

pub mod listar_productos;
pub mod registrar_producto;

pub use listar_productos::{ListarProductos, ProductoListado};
pub use registrar_producto::{ComandoRegistrarProducto, RegistrarProducto};
