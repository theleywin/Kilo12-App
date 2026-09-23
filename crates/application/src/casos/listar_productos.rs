//! Caso de uso: consultar el catálogo.

use domain::{Cantidad, Producto, Ubicacion};

use crate::error::Resultado;
use crate::margen;
use crate::puertos::{ProductoConInventario, RepositorioProducto};

/// Lo que se muestra cuando un producto todavía no tiene costo.
///
/// El costo se deriva del valor invertido entre la existencia (RF-COS-02):
/// sin mercancía no hay costo, y poner «0.00» sería afirmar que la
/// mercancía es gratis.
const SIN_DATO: &str = "—";

/// Producto tal como se muestra en una lista.
///
/// Es un objeto de transferencia, no la entidad: la interfaz recibe texto ya
/// formateado y no necesita —ni debe— hacer aritmética con dinero (DT-7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductoListado {
    pub id: i64,
    pub sku: String,
    pub nombre: String,
    /// Símbolo de la unidad base: `u`, `lb`, `kg`…
    pub unidad_base: String,
    /// Nombre legible de la unidad: «Libra», «Unidad»…
    pub unidad_nombre: String,
    /// Se vende en fracciones. Se deduce de la unidad, no es un dato aparte.
    pub es_granel: bool,
    pub activo: bool,
    /// Precio de la presentación predeterminada, con dos decimales.
    pub precio: String,
    /// Costo promedio ponderado vigente, o `—` si no hay existencia.
    pub costo: String,
    /// Ganancia por unidad base, o `—` si todavía no hay costo.
    pub ganancia: String,
    /// Margen bruto, ya formateado con su signo y su símbolo.
    pub margen: String,
    /// El costo se comió el precio (RF-COM-05).
    pub en_riesgo: bool,
    pub en_almacen: String,
    pub en_vitrina: String,
    pub existencia_total: String,
    /// La existencia total está por debajo del mínimo configurado.
    pub bajo_minimo: bool,
    /// No queda nada, ni en almacén ni en vitrina.
    pub agotado: bool,
    /// Nombre de la presentación predeterminada.
    pub presentacion: String,
    pub total_presentaciones: usize,
}

impl ProductoListado {
    fn desde(fila: &ProductoConInventario) -> Self {
        let ProductoConInventario {
            producto,
            inventario,
        } = fila;

        let predeterminada = producto.presentacion_predeterminada();
        let precio = predeterminada.map(|p| p.precio());
        let existencias = inventario.existencias();
        let total = existencias.total().unwrap_or(Cantidad::CERO);
        let costo = inventario.costo_unitario().ok();

        // El margen solo existe si hay costo y hay precio con que compararlo.
        let calculo = match (costo, precio) {
            (Some(costo), Some(precio)) => margen::calcular(costo, precio).ok(),
            _ => None,
        };

        Self {
            id: producto.id().map_or(0, |id| id.0),
            sku: producto.sku().to_string(),
            nombre: producto.nombre().to_string(),
            unidad_base: producto.unidad_base().simbolo().to_string(),
            unidad_nombre: producto
                .unidad_base()
                .nombre_presentacion_unitaria()
                .to_string(),
            es_granel: producto.es_granel(),
            activo: producto.esta_activo(),
            precio: precio.map_or_else(|| SIN_DATO.to_owned(), |p| p.formatear(2)),
            costo: costo.map_or_else(|| SIN_DATO.to_owned(), |c| c.formatear(2)),
            ganancia: calculo.map_or_else(|| SIN_DATO.to_owned(), |m| m.ganancia.formatear(2)),
            margen: calculo.map_or_else(
                || SIN_DATO.to_owned(),
                |m| format!("{} %", m.porcentaje.formatear(1)),
            ),
            en_riesgo: calculo.is_some_and(|m| m.en_riesgo),
            en_almacen: formatear(existencias.en(Ubicacion::Bodega), producto),
            en_vitrina: formatear(existencias.en(Ubicacion::Vitrina), producto),
            existencia_total: formatear(total, producto),
            bajo_minimo: producto.esta_bajo_minimo(total),
            agotado: total.es_cero(),
            presentacion: predeterminada
                .map(|p| p.nombre().to_string())
                .unwrap_or_default(),
            total_presentaciones: producto.presentaciones().len(),
        }
    }
}

/// Formatea una cantidad con los decimales que su unidad admite.
///
/// «3 latas» y «3.500 lb» son ambas correctas; «3.000 latas» solo consigue
/// que el ojo tropiece.
fn formatear(cantidad: Cantidad, producto: &Producto) -> String {
    let decimales = if producto.es_granel() { 3 } else { 0 };
    cantidad.formatear(decimales)
}

/// Lista los productos del catálogo.
#[derive(Debug)]
pub struct ListarProductos<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> ListarProductos<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, incluir_inactivos: bool) -> Resultado<Vec<ProductoListado>> {
        let productos = self.repositorio.listar(incluir_inactivos)?;
        Ok(productos.iter().map(ProductoListado::desde).collect())
    }
}
