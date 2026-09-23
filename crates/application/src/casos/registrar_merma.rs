//! Caso de uso: se pierde mercancía (RF-MER).
//!
//! Una merma es mercancía que ya no se va a vender: se echó a perder, se
//! rompió, se derramó. Sale del inventario descontando su valor al costo
//! vigente, y **exige un motivo** (RF-INV-08): mercancía que desaparece sin
//! que nadie explique por qué es, con el tiempo, mercancía que se va por la
//! puerta de atrás.

use domain::{Cantidad, IdProducto, Movimiento, Ubicacion};

use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{ProductoConInventario, RepositorioProducto};

/// Datos de una baja por merma.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoRegistrarMerma {
    pub producto: i64,
    pub cantidad: String,
    /// De dónde sale: `BODEGA` o `VITRINA`.
    pub origen: String,
    /// Por qué se perdió. Obligatorio.
    pub motivo: String,
}

/// Da de baja mercancía perdida.
#[derive(Debug)]
pub struct RegistrarMerma<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> RegistrarMerma<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoRegistrarMerma) -> Resultado<()> {
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
        producto.validar_cantidad(cantidad)?;

        let origen: Ubicacion = comando.origen.trim().parse()?;

        // La salida descuenta al costo vigente y lo devuelve, que es lo que
        // se congela en el asiento (RF-COS-06, RF-COS-07). Si no hay
        // existencia suficiente, el dominio lo rechaza aquí.
        let salida = inventario.registrar_salida(cantidad, origen)?;

        let movimiento = Movimiento::merma(
            cantidad,
            salida.costo_unitario,
            origen,
            comando.motivo.trim(),
        )?;

        self.repositorio
            .registrar_movimiento(id, &salida.inventario, &movimiento)
    }
}
