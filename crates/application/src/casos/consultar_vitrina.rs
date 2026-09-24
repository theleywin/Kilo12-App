//! Caso de uso: el estado de la vitrina y qué hace falta reponer.
//!
//! Este módulo existe por una razón de negocio concreta, escrita en el
//! documento: *existencia total alta con vitrina vacía es venta perdida*.
//! Tener cuarenta libras en la bodega no sirve de nada si el cliente que
//! entra no las ve (RF-VIT-04, RF-VIT-06).

use domain::{Cantidad, Ubicacion};

use crate::casos::formatear_cantidad;
use crate::error::Resultado;
use crate::puertos::RepositorioProducto;

/// Un producto tal como se ve desde la vitrina.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaVitrina {
    pub id: i64,
    pub sku: String,
    pub nombre: String,
    pub unidad_base: String,
    pub unidad_nombre: String,
    pub en_vitrina: String,
    /// Cuánto se quiere mantener exhibido (RF-VIT-03).
    pub objetivo: String,
    pub en_almacen: String,
    /// Cuánto habría que bajar del almacén para alcanzar el objetivo,
    /// limitado por lo que realmente hay guardado (RF-VIT-04).
    pub sugerido: String,
    /// Hay algo que bajar y con qué hacerlo.
    pub hay_que_reponer: bool,
    /// Hay algo exhibido ahora mismo.
    pub esta_exhibido: bool,
    /// Falta para el objetivo y NO hay nada guardado con que cubrirlo:
    /// esto no se arregla bajando mercancía, se arregla comprándola.
    pub falta_comprar: bool,
    /// Hay mercancía en el almacén pero la vitrina está vacía: el producto
    /// existe y el cliente no lo ve (RF-VIT-06).
    pub disponible_sin_exhibir: bool,
    /// No queda nada en ninguna parte.
    pub agotado: bool,
}

/// Estado de la vitrina.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumenVitrina {
    pub productos: Vec<LineaVitrina>,
    /// Cuántos productos tienen algo exhibido.
    pub exhibidos: usize,
    /// Cuántos están por debajo de su objetivo y se pueden reponer.
    pub por_reponer: usize,
    /// Cuántos están guardados pero no exhibidos.
    pub sin_exhibir: usize,
    /// Cuántos no llegan a su objetivo y tampoco tienen repuesto guardado.
    pub falta_comprar: usize,
}

/// Consulta el estado de la vitrina.
#[derive(Debug)]
pub struct ConsultarVitrina<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> ConsultarVitrina<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self) -> Resultado<ResumenVitrina> {
        let filas = self.repositorio.listar(false)?;

        let productos: Vec<LineaVitrina> = filas
            .iter()
            .map(|fila| {
                let producto = &fila.producto;
                let existencias = fila.inventario.existencias();
                let objetivo = producto.objetivo_vitrina();

                // La sugerencia la calcula el dominio: nunca propone bajar
                // más de lo que hay guardado.
                let sugerido = existencias.faltante_para_exhibir(objetivo);
                let total = existencias.total().unwrap_or(Cantidad::CERO);
                let en_vitrina = existencias.en(Ubicacion::Vitrina);

                // Por debajo del objetivo pero sin nada que bajar. Decir
                // «no hay que reponer» sería técnicamente cierto e inútil:
                // lo que hace falta es comprar.
                let falta_comprar =
                    objetivo.es_positiva() && en_vitrina < objetivo && !sugerido.es_positiva();

                LineaVitrina {
                    id: producto.id().map_or(0, |id| id.0),
                    sku: producto.sku().to_string(),
                    nombre: producto.nombre().to_string(),
                    unidad_base: producto.unidad_base().simbolo().to_string(),
                    unidad_nombre: producto
                        .unidad_base()
                        .nombre_presentacion_unitaria()
                        .to_string(),
                    en_vitrina: formatear_cantidad(existencias.en(Ubicacion::Vitrina), producto),
                    objetivo: formatear_cantidad(objetivo, producto),
                    en_almacen: formatear_cantidad(existencias.en(Ubicacion::Bodega), producto),
                    sugerido: formatear_cantidad(sugerido, producto),
                    hay_que_reponer: sugerido.es_positiva(),
                    esta_exhibido: en_vitrina.es_positiva(),
                    falta_comprar,
                    disponible_sin_exhibir: existencias.hay_en_bodega_sin_exhibir(),
                    agotado: total.es_cero(),
                }
            })
            .collect();

        Ok(ResumenVitrina {
            exhibidos: productos.iter().filter(|p| p.esta_exhibido).count(),
            por_reponer: productos.iter().filter(|p| p.hay_que_reponer).count(),
            sin_exhibir: productos
                .iter()
                .filter(|p| p.disponible_sin_exhibir)
                .count(),
            falta_comprar: productos.iter().filter(|p| p.falta_comprar).count(),
            productos,
        })
    }
}
