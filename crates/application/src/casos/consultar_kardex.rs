//! Caso de uso: el historial de un producto (RF-INV-05).
//!
//! Devuelve los movimientos del más reciente al más antiguo, ya formateados
//! y con el saldo que dejó cada uno. La pantalla no calcula nada: lee.

use domain::IdProducto;

use crate::casos::formatear_cantidad;
use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{ProductoConInventario, RepositorioProducto};

/// Cuántos movimientos se devuelven si no se pide otra cosa.
pub const LIMITE_POR_DEFECTO: usize = 50;

/// Una línea del kárdex, lista para mostrar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaKardex {
    pub id: i64,
    /// Código estable del tipo: `ENTRADA`, `MERMA`, `VENTA`…
    pub tipo: String,
    /// Nombre legible: «Entrada», «Merma»…
    pub tipo_nombre: String,
    /// Suma existencia en lugar de restarla.
    pub es_entrada: bool,
    pub cantidad: String,
    pub costo_unitario: String,
    /// Lo que valía la mercancía que se movió.
    pub importe: String,
    pub origen: Option<String>,
    pub destino: Option<String>,
    /// Existencia que quedó después de este movimiento.
    pub bodega_resultante: String,
    pub vitrina_resultante: String,
    pub motivo: Option<String>,
    pub ocurrido_en: String,
}

/// Consulta el historial de un producto.
#[derive(Debug)]
pub struct ConsultarKardex<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> ConsultarKardex<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, producto: i64, limite: usize) -> Resultado<Vec<LineaKardex>> {
        let id = IdProducto(producto);

        // Hace falta el producto para saber con cuántos decimales se
        // escriben sus cantidades: «3 latas» y «3.500 lb».
        let ProductoConInventario {
            producto: entidad, ..
        } = self
            .repositorio
            .obtener(id)?
            .ok_or(ErrorAplicacion::NoEncontrado {
                entidad: "producto",
                id: producto,
            })?;

        let movimientos = self.repositorio.kardex(id, limite)?;

        Ok(movimientos
            .into_iter()
            .map(|registro| {
                let movimiento = &registro.movimiento;

                LineaKardex {
                    id: registro.id,
                    tipo: movimiento.tipo().como_texto().to_owned(),
                    tipo_nombre: movimiento.tipo().nombre().to_owned(),
                    es_entrada: movimiento.tipo().es_entrada(),
                    cantidad: formatear_cantidad(movimiento.cantidad(), &entidad),
                    costo_unitario: movimiento.costo_unitario().formatear(2),
                    importe: movimiento
                        .importe()
                        .map(|importe| importe.formatear(2))
                        .unwrap_or_default(),
                    origen: movimiento.origen().map(|u| u.nombre().to_owned()),
                    destino: movimiento.destino().map(|u| u.nombre().to_owned()),
                    bodega_resultante: formatear_cantidad(registro.resultante.bodega(), &entidad),
                    vitrina_resultante: formatear_cantidad(registro.resultante.vitrina(), &entidad),
                    motivo: movimiento.motivo().map(str::to_owned),
                    ocurrido_en: registro.ocurrido_en,
                }
            })
            .collect())
    }
}
