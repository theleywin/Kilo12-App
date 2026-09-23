//! Caso de uso: «¿qué pasaría si hago esto?».
//!
//! Responde con la existencia que quedaría **usando las mismas reglas que
//! ejecutarían el movimiento de verdad**. Esa es la razón de que viva aquí
//! y no en la pantalla: una vista previa calculada por otro camino puede
//! discrepar del resultado real, y entonces es peor que no tenerla.

use domain::{Cantidad, Dinero, ErrorDominio, IdProducto, Producto, TipoMovimiento, Ubicacion};

use crate::casos::formatear_cantidad;
use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{ProductoConInventario, RepositorioProducto};

/// Qué movimiento se está planteando.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoSimular {
    pub producto: i64,
    /// `ENTRADA`, `TRASPASO` o `MERMA`.
    pub tipo: String,
    pub cantidad: String,
    /// En una entrada, a dónde llega. En las demás, de dónde sale.
    pub ubicacion: String,
}

/// Cómo quedaría la existencia si el movimiento se hiciera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Simulacion {
    /// Si es `false`, el movimiento se rechazaría.
    pub posible: bool,
    /// Por qué no se puede, redactado para quien lo lee.
    pub problema: Option<String>,
    /// Cuánto hay ahora mismo en la ubicación implicada.
    pub disponible: String,
    pub bodega_resultante: String,
    pub vitrina_resultante: String,
}

/// Calcula cómo quedaría la existencia tras un movimiento.
#[derive(Debug)]
pub struct SimularMovimiento<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> SimularMovimiento<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoSimular) -> Resultado<Simulacion> {
        let id = IdProducto(comando.producto);
        let ProductoConInventario {
            producto,
            inventario,
        } = self
            .repositorio
            .obtener(id)?
            .ok_or(ErrorAplicacion::NoEncontrado {
                entidad: "producto",
                id: comando.producto,
            })?;

        let cantidad: Cantidad = comando.cantidad.trim().parse()?;
        let ubicacion: Ubicacion = comando.ubicacion.trim().parse()?;
        let tipo: TipoMovimiento = comando.tipo.trim().parse()?;
        let disponible = inventario.existencias().en(ubicacion);

        let resultado = match tipo {
            // El importe no influye en las cantidades y esto no se guarda,
            // así que simular con cero es exacto para lo que se pregunta.
            TipoMovimiento::Entrada => {
                inventario.registrar_entrada(cantidad, Dinero::CERO, ubicacion)
            }
            TipoMovimiento::Traspaso => {
                inventario.traspasar(cantidad, ubicacion, ubicacion.opuesta())
            }
            TipoMovimiento::Merma => inventario
                .registrar_salida(cantidad, ubicacion)
                .map(|salida| salida.inventario),
            _ => {
                return Err(ErrorAplicacion::Dominio(
                    ErrorDominio::TipoMovimientoDesconocido,
                ))
            }
        };

        Ok(match resultado {
            Ok(despues) => Simulacion {
                posible: true,
                problema: None,
                disponible: formatear_cantidad(disponible, &producto),
                bodega_resultante: formatear_cantidad(
                    despues.existencias().en(Ubicacion::Bodega),
                    &producto,
                ),
                vitrina_resultante: formatear_cantidad(
                    despues.existencias().en(Ubicacion::Vitrina),
                    &producto,
                ),
            },
            // El dominio ya redacta sus mensajes para que los lea una
            // persona, así que se muestran tal cual.
            Err(error) => Simulacion {
                posible: false,
                problema: Some(mensaje(error, disponible, ubicacion, &producto)),
                disponible: formatear_cantidad(disponible, &producto),
                bodega_resultante: formatear_cantidad(
                    inventario.existencias().en(Ubicacion::Bodega),
                    &producto,
                ),
                vitrina_resultante: formatear_cantidad(
                    inventario.existencias().en(Ubicacion::Vitrina),
                    &producto,
                ),
            },
        })
    }
}

/// Traduce el rechazo a algo accionable.
///
/// «No hay existencia suficiente» es correcto pero inútil; decir cuánto hay
/// y dónde ahorra el viaje de ir a mirarlo.
fn mensaje(
    error: ErrorDominio,
    disponible: Cantidad,
    ubicacion: Ubicacion,
    producto: &Producto,
) -> String {
    match error {
        ErrorDominio::ExistenciaInsuficiente { .. } | ErrorDominio::SinExistenciaParaCosto => {
            format!(
                "No alcanza: en {} solo hay {} {}",
                ubicacion.nombre().to_lowercase(),
                formatear_cantidad(disponible, producto),
                producto.unidad_base().simbolo(),
            )
        }
        otro => otro.to_string(),
    }
}
