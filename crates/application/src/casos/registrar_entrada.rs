//! Caso de uso: entra mercancía comprada (RF-COM-03).
//!
//! Es la única operación que recalcula el costo del producto (RF-COS-04).
//! Vender, traspasar o mermar mueven existencia, pero no cambian lo que la
//! mercancía costó.

use domain::{Cantidad, Dinero, IdProducto, Movimiento, Ubicacion};

use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{ProductoConInventario, RepositorioProducto};

/// Datos de una entrada de mercancía.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoRegistrarEntrada {
    pub producto: i64,
    /// Cuánto entra, en la unidad base del producto.
    pub cantidad: String,
    /// Lo que costó cada unidad base en ESTA compra, no el costo histórico.
    pub costo_unitario: String,
    /// `BODEGA` o `VITRINA`.
    pub destino: String,
}

/// Registra la entrada de mercancía de una compra.
#[derive(Debug)]
pub struct RegistrarEntrada<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> RegistrarEntrada<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoRegistrarEntrada) -> Resultado<()> {
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
        // Un producto que se cuenta por unidades no admite media unidad, ni
        // siquiera comprándola.
        producto.validar_cantidad(cantidad)?;

        let costo: Dinero = comando.costo_unitario.trim().parse()?;
        let destino: Ubicacion = comando.destino.trim().parse()?;

        // Se pasa el importe total de la línea, no el costo unitario: es lo
        // que evita el redondeo intermedio que RF-COS-13 prohíbe.
        let importe = costo.multiplicar_por(cantidad)?;
        let inventario = inventario.registrar_entrada(cantidad, importe, destino)?;
        let movimiento = Movimiento::entrada(cantidad, costo, destino)?;

        self.repositorio
            .registrar_movimiento(id, &inventario, &movimiento)
    }
}
