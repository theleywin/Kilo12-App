//! Puertos: lo que la aplicación necesita del mundo exterior.
//!
//! Son interfaces, no implementaciones. La capa de aplicación declara aquí
//! qué le hace falta —guardar un producto, listarlo— y otra capa decide
//! cómo. Esa inversión es lo que mantiene las flechas apuntando hacia
//! adentro (DT-5) y lo que permitirá cambiar SQLite, o añadir un lector de
//! código de barras, sin tocar ninguna regla de negocio.

use domain::{IdProducto, Inventario, Producto};

use crate::error::Resultado;

/// Un producto junto con su existencia y su valor.
///
/// Viajan juntos porque separados mienten: la existencia sin el valor no
/// permite saber el costo, y el costo es lo que decide si una venta deja
/// ganancia (RF-COS-02).
#[derive(Debug, Clone)]
pub struct ProductoConInventario {
    pub producto: Producto,
    pub inventario: Inventario,
}

/// Acceso al catálogo de productos.
pub trait RepositorioProducto {
    /// Guarda un producto nuevo con su existencia de apertura y devuelve el
    /// identificador asignado.
    ///
    /// El inventario entra en la misma operación porque dar de alta un
    /// producto y meter la mercancía que ya tienes en el almacén es, para
    /// quien lo usa, un solo acto. Que por dentro sean dos cosas no es
    /// asunto suyo.
    fn crear(&self, producto: &Producto, inventario: &Inventario) -> Resultado<IdProducto>;

    /// Recupera un producto con sus presentaciones y su existencia.
    fn obtener(&self, id: IdProducto) -> Resultado<Option<ProductoConInventario>>;

    /// Lista los productos del catálogo.
    ///
    /// `incluir_inactivos` decide si aparecen los productos desactivados,
    /// que siguen existiendo por su historial (RF-CAT-06).
    fn listar(&self, incluir_inactivos: bool) -> Resultado<Vec<ProductoConInventario>>;

    /// Indica si ya existe un producto con ese SKU (RF-CAT-02).
    fn existe_sku(&self, sku: &str) -> Resultado<bool>;
}
