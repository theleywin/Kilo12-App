//! Puertos: lo que la aplicación necesita del mundo exterior.
//!
//! Son interfaces, no implementaciones. La capa de aplicación declara aquí
//! qué le hace falta —guardar un producto, listarlo— y otra capa decide
//! cómo. Esa inversión es lo que mantiene las flechas apuntando hacia
//! adentro (DT-5) y lo que permitirá cambiar SQLite, o añadir un lector de
//! código de barras, sin tocar ninguna regla de negocio.

use domain::{Dinero, Existencias, IdPresentacion, IdProducto, Inventario, Movimiento, Producto};

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

/// Un movimiento tal como quedó registrado, con su fecha y su saldo.
///
/// El saldo resultante se guardó en su momento y no se recalcula: es lo que
/// permite leer el kárdex de arriba abajo sin rehacer la aritmética de
/// todos los movimientos anteriores (RF-INV-05).
#[derive(Debug, Clone)]
pub struct MovimientoRegistrado {
    pub id: i64,
    pub movimiento: Movimiento,
    /// Fecha y hora en que ocurrió, como la guarda la base de datos.
    pub ocurrido_en: String,
    /// Existencia que quedó justo después de este movimiento.
    pub resultante: Existencias,
}

/// Un movimiento junto con el saldo que dejó.
///
/// Van emparejados porque el saldo depende del orden: si un producto nace
/// con mercancía en bodega y en vitrina, cada asiento deja una foto
/// distinta, y esa foto es la que después se lee en el kárdex.
#[derive(Debug, Clone)]
pub struct Asiento {
    pub movimiento: Movimiento,
    pub resultante: Existencias,
}

/// Un cambio de precio, para el historial (RF-PRE-04).
///
/// El precio de ayer no se deduce del de hoy: si no se anota cuando cambia,
/// se pierde, y con él la posibilidad de saber si una venta vieja dejaba
/// ganancia.
#[derive(Debug, Clone)]
pub struct CambioDePrecio {
    pub presentacion: IdPresentacion,
    pub anterior: Dinero,
    pub nuevo: Dinero,
}

/// Un cambio de precio ya registrado, con su fecha.
#[derive(Debug, Clone)]
pub struct CambioRegistrado {
    pub id: i64,
    pub presentacion: IdPresentacion,
    pub anterior: Dinero,
    pub nuevo: Dinero,
    pub cambiado_en: String,
}

/// Acceso al catálogo de productos y a su inventario.
pub trait RepositorioProducto {
    /// Guarda un producto nuevo con su existencia de apertura y devuelve el
    /// identificador asignado.
    ///
    /// El inventario y sus movimientos entran en la misma operación porque
    /// dar de alta un producto y meter la mercancía que ya tienes es, para
    /// quien lo usa, un solo acto. Que por dentro sean varias cosas no es
    /// asunto suyo.
    fn crear(
        &self,
        producto: &Producto,
        inventario: &Inventario,
        asientos: &[Asiento],
    ) -> Resultado<IdProducto>;

    /// Recupera un producto con sus presentaciones y su existencia.
    fn obtener(&self, id: IdProducto) -> Resultado<Option<ProductoConInventario>>;

    /// Guarda los datos editables de un producto y sus presentaciones.
    ///
    /// No toca la existencia ni el valor: eso solo cambia con un
    /// movimiento, nunca editando una ficha.
    ///
    /// Los cambios de precio viajan aparte porque el historial necesita
    /// saber de dónde venía cada uno, y eso el producto ya no lo recuerda:
    /// dentro de él solo está el precio nuevo.
    fn actualizar_producto(&self, producto: &Producto, cambios: &[CambioDePrecio])
        -> Resultado<()>;

    /// Devuelve el historial de precios de un producto (RF-PRE-04).
    fn historial_precios(&self, id: IdProducto) -> Resultado<Vec<CambioRegistrado>>;

    /// Lista los productos del catálogo.
    ///
    /// `incluir_inactivos` decide si aparecen los productos desactivados,
    /// que siguen existiendo por su historial (RF-CAT-06).
    fn listar(&self, incluir_inactivos: bool) -> Resultado<Vec<ProductoConInventario>>;

    /// Asienta un movimiento y deja la existencia como quedó después.
    ///
    /// Las dos cosas van juntas y son indivisibles: un saldo sin su asiento
    /// es un número que nadie puede explicar, y un asiento sin su saldo es
    /// un historial que no cuadra con la realidad (RNF-5).
    fn registrar_movimiento(
        &self,
        id: IdProducto,
        inventario: &Inventario,
        movimiento: &Movimiento,
    ) -> Resultado<()>;

    /// Devuelve el historial de un producto, del más reciente al más
    /// antiguo (RF-INV-05).
    fn kardex(&self, id: IdProducto, limite: usize) -> Resultado<Vec<MovimientoRegistrado>>;

    /// Indica si ya existe un producto con ese SKU (RF-CAT-02).
    fn existe_sku(&self, sku: &str) -> Resultado<bool>;
}
