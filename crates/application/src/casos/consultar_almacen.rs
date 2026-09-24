//! Caso de uso: la foto del almacén (RF-INV-09, RF-INV-10).

use domain::Dinero;

use crate::casos::listar_productos::ProductoListado;
use crate::error::Resultado;
use crate::puertos::RepositorioProducto;

/// Estado del almacén: qué hay y cuánto vale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumenAlmacen {
    pub productos: Vec<ProductoListado>,
    /// Suma del valor de todo el inventario, a costo promedio ponderado.
    pub valor_total: String,
    /// Cuántos productos tienen algo que vender.
    pub con_existencia: usize,
    /// Cuántos están por debajo de su mínimo (RF-EST-06).
    pub bajo_minimo: usize,
    pub agotados: usize,
}

/// Consulta el estado del almacén.
#[derive(Debug)]
pub struct ConsultarAlmacen<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> ConsultarAlmacen<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self) -> Resultado<ResumenAlmacen> {
        let filas = self.repositorio.listar(false)?;

        // El valor se suma con la aritmética del dominio, no con un `f64`:
        // sumar mil importes redondeados descuadra el inventario.
        let mut valor_total = Dinero::CERO;
        for fila in &filas {
            valor_total = valor_total.sumar(fila.inventario.valor_total())?;
        }

        let productos: Vec<ProductoListado> = filas.iter().map(ProductoListado::desde).collect();

        Ok(ResumenAlmacen {
            valor_total: valor_total.formatear(2),
            con_existencia: productos.iter().filter(|p| !p.agotado).count(),
            bajo_minimo: productos.iter().filter(|p| p.bajo_minimo).count(),
            agotados: productos.iter().filter(|p| p.agotado).count(),
            productos,
        })
    }
}
