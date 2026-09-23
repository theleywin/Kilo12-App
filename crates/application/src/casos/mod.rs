//! Casos de uso, uno por operación del negocio.

pub mod consultar_almacen;
pub mod consultar_historial_precios;
pub mod consultar_kardex;
pub mod consultar_producto;
pub mod consultar_vitrina;
pub mod editar_producto;
pub mod fijar_objetivo_vitrina;
pub mod listar_productos;
pub mod registrar_entrada;
pub mod registrar_merma;
pub mod registrar_producto;
pub mod simular_movimiento;
pub mod traspasar;

pub use consultar_almacen::{ConsultarAlmacen, ResumenAlmacen};
pub use consultar_historial_precios::{CambioDePrecioListado, ConsultarHistorialPrecios};
pub use consultar_kardex::{ConsultarKardex, LineaKardex};
pub use consultar_producto::{ConsultarProducto, FichaProducto, PresentacionDetallada};
pub use consultar_vitrina::{ConsultarVitrina, LineaVitrina, ResumenVitrina};
pub use editar_producto::{
    AgregarPresentacion, CambiarPrecio, ComandoAgregarPresentacion, ComandoCambiarPrecio,
    ComandoEditarProducto, ComandoPresentacion, DesactivarPresentacion, EditarProducto,
    MarcarPredeterminada,
};
pub use fijar_objetivo_vitrina::{ComandoFijarObjetivo, FijarObjetivoVitrina};
pub use listar_productos::{ListarProductos, OrdenCatalogo, ProductoListado};
pub use registrar_entrada::{ComandoRegistrarEntrada, RegistrarEntrada};
pub use registrar_merma::{ComandoRegistrarMerma, RegistrarMerma};
pub use registrar_producto::{ComandoRegistrarProducto, RegistrarProducto};
pub use simular_movimiento::{ComandoSimular, Simulacion, SimularMovimiento};
pub use traspasar::{ComandoTraspasar, Traspasar};

use domain::{Cantidad, Producto};

/// Formatea una cantidad con los decimales que su unidad admite.
///
/// «3 latas» y «3.500 lb» son ambas correctas; «3.000 latas» solo consigue
/// que el ojo tropiece.
pub(crate) fn formatear_cantidad(cantidad: Cantidad, producto: &Producto) -> String {
    let decimales = if producto.es_granel() { 3 } else { 0 };
    cantidad.formatear(decimales)
}
