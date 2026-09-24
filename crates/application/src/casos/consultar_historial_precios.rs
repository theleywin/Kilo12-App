//! Caso de uso: cómo ha ido cambiando el precio (RF-PRE-04).
//!
//! Sirve para responder «¿cuándo subí esto y de cuánto venía?», que es la
//! pregunta que aparece cuando un cliente se queja del precio o cuando hay
//! que revisar si una subida de costo se trasladó a tiempo.

use domain::IdProducto;

use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{ProductoConInventario, RepositorioProducto};

/// Un cambio de precio ya ocurrido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CambioDePrecioListado {
    pub id: i64,
    /// Nombre de la presentación a la que se le cambió el precio.
    pub presentacion: String,
    pub anterior: String,
    pub nuevo: String,
    /// `true` si el precio subió.
    pub subio: bool,
    pub cambiado_en: String,
}

/// Consulta el historial de precios de un producto.
#[derive(Debug)]
pub struct ConsultarHistorialPrecios<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> ConsultarHistorialPrecios<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, id: i64) -> Resultado<Vec<CambioDePrecioListado>> {
        let ProductoConInventario { producto, .. } = self
            .repositorio
            .obtener(IdProducto(id))?
            .ok_or(ErrorAplicacion::NoEncontrado {
                entidad: "producto",
                id,
            })?;

        let historial = self.repositorio.historial_precios(IdProducto(id))?;

        Ok(historial
            .into_iter()
            .map(|cambio| CambioDePrecioListado {
                id: cambio.id,
                // El nombre se resuelve aquí y no se guarda en el historial:
                // si la presentación se renombra, el historial debe seguir
                // hablando de la presentación, no de un nombre congelado.
                presentacion: producto
                    .presentacion(cambio.presentacion)
                    .map_or_else(|| "—".to_owned(), |p| p.nombre().to_string()),
                anterior: cambio.anterior.formatear(2),
                nuevo: cambio.nuevo.formatear(2),
                subio: cambio.nuevo > cambio.anterior,
                cambiado_en: cambio.cambiado_en,
            })
            .collect())
    }
}
