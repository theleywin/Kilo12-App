//! Caso de uso: mover mercancía entre el almacén y la vitrina (RF-VIT-01).
//!
//! Un traspaso **no es una compra**. No se le compró nada a nadie, así que
//! no hay costo que indicar: la existencia total no cambia y el valor del
//! inventario tampoco (RF-COS-05). Lo único que cambia es dónde está.

use domain::{Cantidad, IdProducto, Movimiento, Ubicacion};

use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{ProductoConInventario, RepositorioProducto};

/// Datos de un traspaso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoTraspasar {
    pub producto: i64,
    pub cantidad: String,
    /// De dónde sale: `BODEGA` o `VITRINA`. El destino es el otro.
    pub origen: String,
}

/// Mueve mercancía de una ubicación a la otra.
#[derive(Debug)]
pub struct Traspasar<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> Traspasar<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoTraspasar) -> Resultado<()> {
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
        let destino = origen.opuesta();

        let inventario = inventario.traspasar(cantidad, origen, destino)?;

        // El costo se anota solo como referencia de lo que valía lo movido:
        // el traspaso no lo altera.
        let movimiento = Movimiento::traspaso(
            cantidad,
            inventario.costo_unitario_o_cero(),
            origen,
            destino,
        )?;

        self.repositorio
            .registrar_movimiento(id, &inventario, &movimiento)
    }
}
