//! Caso de uso: anular una venta (RF-VTA-15).
//!
//! Anular no es borrar. La venta se queda donde está, marcada, con su motivo
//! y su hora: el folio ya se emitió y el historial tiene que poder explicar
//! qué pasó con él.
//!
//! **Solo se anula dentro de la sesión de caja vigente** (D-5). Una sesión
//! cerrada ya se arqueó contra dinero físico: dejar que una venta suya
//! desaparezca después invalidaría ese arqueo y todo informe que salga de
//! él. Para las ventas de sesiones cerradas la vía es la devolución, que se
//! registra como operación propia y afecta a la caja de hoy.

use domain::{Cantidad, Dinero, ErrorDominio, IdProducto, Inventario, Movimiento, Ubicacion};

use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{
    AnulacionConfirmada, DescuentoVenta, ProductoConInventario, RepositorioProducto,
};

/// Lo que llega de la pantalla al anular.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoAnularVenta {
    pub venta: i64,
    /// Por qué se anula. Obligatorio.
    pub motivo: String,
}

/// Anula una venta de la sesión vigente.
#[derive(Debug)]
pub struct AnularVenta<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> AnularVenta<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoAnularVenta) -> Resultado<()> {
        let motivo = comando.motivo.trim();
        if motivo.is_empty() {
            return Err(ErrorAplicacion::Dominio(ErrorDominio::MotivoObligatorio));
        }

        let detalle = self.repositorio.detalle_venta(comando.venta)?.ok_or(
            ErrorAplicacion::NoEncontrado {
                entidad: "venta",
                id: comando.venta,
            },
        )?;

        if detalle.venta.anulada {
            return Err(ErrorAplicacion::Dominio(ErrorDominio::VentaYaAnulada));
        }

        // La venta tiene que ser de la caja que está abierta ahora mismo.
        // Sin sesión abierta no se anula nada, aunque la venta sea de hace
        // cinco minutos: no habría dónde registrar el efecto.
        let abierta = self
            .repositorio
            .sesion_abierta()?
            .ok_or(ErrorAplicacion::Dominio(ErrorDominio::SinSesionAbierta))?;

        let vigente = abierta.sesion.id().map(|id| id.0);
        if detalle.venta.sesion.is_none() || detalle.venta.sesion != vigente {
            return Err(ErrorAplicacion::Dominio(ErrorDominio::SesionCerrada));
        }

        // La mercancía vuelve producto a producto. Dos líneas del mismo
        // producto se reingresan de una vez, igual que salieron de una vez.
        let mut inventarios: std::collections::BTreeMap<i64, Inventario> =
            std::collections::BTreeMap::new();
        let mut devueltas: std::collections::BTreeMap<i64, Cantidad> =
            std::collections::BTreeMap::new();
        let mut costos: std::collections::BTreeMap<i64, Dinero> = std::collections::BTreeMap::new();

        for linea in &detalle.lineas {
            let id = linea.producto.0;

            // Se lee el producto una sola vez aunque aparezca en dos líneas:
            // la segunda tiene que ver lo que devolvió la primera.
            if let std::collections::btree_map::Entry::Vacant(hueco) = inventarios.entry(id) {
                let ProductoConInventario { inventario, .. } = self
                    .repositorio
                    .obtener(linea.producto)?
                    .ok_or(ErrorAplicacion::NoEncontrado {
                        entidad: "producto",
                        id,
                    })?;
                hueco.insert(inventario);
            }

            let unidades = linea.cantidad.multiplicar_por_factor(linea.factor)?;
            let estado = inventarios.get_mut(&id).expect("recién insertado");

            // Vuelve al costo con que se vendió, no al de hoy: esto deshace
            // una operación, no compra mercancía nueva (RF-COS-05). El
            // inventario recibe el IMPORTE de la mercancía devuelta, no su
            // costo unitario.
            let importe = linea.costo_unitario.multiplicar_por(unidades)?;
            *estado = estado.registrar_entrada(unidades, importe, Ubicacion::Vitrina)?;

            devueltas
                .entry(id)
                .and_modify(|acumulado| {
                    *acumulado = acumulado.sumar(unidades).unwrap_or(*acumulado);
                })
                .or_insert(unidades);
            costos.insert(id, linea.costo_unitario);
        }

        let mut reversas = Vec::with_capacity(inventarios.len());
        for (id, inventario) in inventarios {
            let cantidad = devueltas.get(&id).copied().unwrap_or(Cantidad::CERO);
            let costo = costos.get(&id).copied().unwrap_or(Dinero::CERO);

            reversas.push(DescuentoVenta {
                producto: IdProducto(id),
                inventario,
                movimiento: Movimiento::devolucion(
                    cantidad,
                    costo,
                    Ubicacion::Vitrina,
                    motivo.to_owned(),
                )?,
            });
        }

        self.repositorio.anular_venta(&AnulacionConfirmada {
            venta: comando.venta,
            motivo: motivo.to_owned(),
            reversas: &reversas,
        })
    }
}
