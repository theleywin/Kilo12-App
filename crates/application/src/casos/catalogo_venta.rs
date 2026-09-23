//! Caso de uso: lo que se puede vender ahora mismo.
//!
//! Es una vista distinta del catálogo, hecha para el mostrador: solo trae
//! lo que hace falta para cobrar —cómo se llama, cuánto cuesta cada
//! presentación y cuánto queda **en vitrina**— y nada de lo que hace falta
//! para administrar.
//!
//! La existencia que se muestra es la de la vitrina, no la total: la
//! bodega no está a la venta (RF-VTA-11).

use domain::Ubicacion;

use crate::casos::formatear_cantidad;
use crate::error::Resultado;
use crate::puertos::RepositorioProducto;

/// Una forma de vender un producto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentacionVendible {
    pub id: i64,
    pub nombre: String,
    /// Precio ya formateado: la pantalla no hace cuentas con dinero.
    pub precio: String,
    /// Unidades base que se lleva cada una.
    pub factor: String,
    pub es_predeterminada: bool,
}

/// Un producto tal como se ve desde la pantalla de vender.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductoVendible {
    pub id: i64,
    pub sku: String,
    pub nombre: String,
    pub unidad_base: String,
    /// Admite cantidades con decimales (RF-VTA-03).
    pub es_granel: bool,
    pub en_vitrina: String,
    /// No queda nada exhibido: se puede buscar, pero no vender.
    pub agotado: bool,
    /// Solo las activas: lo retirado no se ofrece al cobrar (RF-PRS-15).
    pub presentaciones: Vec<PresentacionVendible>,
}

/// Consulta lo que hay para vender.
#[derive(Debug)]
pub struct CatalogoDeVenta<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> CatalogoDeVenta<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self) -> Resultado<Vec<ProductoVendible>> {
        let filas = self.repositorio.listar(false)?;

        Ok(filas
            .iter()
            .map(|fila| {
                let producto = &fila.producto;
                let en_vitrina = fila.inventario.existencias().en(Ubicacion::Vitrina);

                ProductoVendible {
                    id: producto.id().map_or(0, |id| id.0),
                    sku: producto.sku().to_string(),
                    nombre: producto.nombre().to_string(),
                    unidad_base: producto.unidad_base().simbolo().to_string(),
                    es_granel: producto.es_granel(),
                    en_vitrina: formatear_cantidad(en_vitrina, producto),
                    agotado: !en_vitrina.es_positiva(),
                    presentaciones: producto
                        .presentaciones_activas()
                        .map(|presentacion| PresentacionVendible {
                            id: presentacion.id().map_or(0, |id| id.0),
                            nombre: presentacion.nombre().to_string(),
                            precio: presentacion.precio().formatear(2),
                            factor: formatear_cantidad(presentacion.factor(), producto),
                            es_predeterminada: presentacion.es_predeterminada(),
                        })
                        .collect(),
                }
            })
            .collect())
    }
}
