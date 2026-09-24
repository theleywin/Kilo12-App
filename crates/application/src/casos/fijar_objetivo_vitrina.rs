//! Caso de uso: decidir cuánto se quiere tener exhibido (RF-VIT-03).
//!
//! Es el número del que sale toda la lista de reposición. Sin él, la
//! vitrina no sabe cuándo está vacía: cero y «suficiente» serían lo mismo.

use domain::{Cantidad, IdProducto};

use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{ProductoConInventario, RepositorioProducto};

/// Datos para fijar el objetivo de exhibición.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoFijarObjetivo {
    pub producto: i64,
    /// Cuánto se quiere mantener en vitrina. Cero desactiva la sugerencia.
    pub objetivo: String,
}

/// Fija la cantidad objetivo en vitrina de un producto.
#[derive(Debug)]
pub struct FijarObjetivoVitrina<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> FijarObjetivoVitrina<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoFijarObjetivo) -> Resultado<()> {
        let id = IdProducto(comando.producto);
        let ProductoConInventario { producto, .. } =
            self.repositorio
                .obtener(id)?
                .ok_or(ErrorAplicacion::NoEncontrado {
                    entidad: "producto",
                    id: comando.producto,
                })?;

        // El texto vacío significa «sin objetivo», no un error: es la forma
        // de desactivar la sugerencia para un producto.
        let objetivo: Cantidad = match comando.objetivo.trim() {
            "" => Cantidad::CERO,
            texto => texto.parse()?,
        };

        let producto = producto.con_objetivo_vitrina(objetivo)?;

        self.repositorio.actualizar_producto(&producto, &[])
    }
}
