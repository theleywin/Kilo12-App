//! Caso de uso: cobrar una venta (RF-VTA).
//!
//! Es la operación por la que existe el negocio, y la que más cosas toca a
//! la vez: descuenta de la vitrina, congela costos y precios, deja asientos
//! en el kárdex y registra cómo pagó el cliente. O pasa todo, o no pasa
//! nada (RNF-5).

use std::collections::BTreeMap;

use domain::{
    Cantidad, Cobro, Dinero, IdPresentacion, IdProducto, IdVentaEnEspera, Inventario, LineaVenta,
    MetodoPago, Movimiento, Pago, TasaCambio, Ubicacion, Venta,
};

use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{DescuentoVenta, ProductoConInventario, RepositorioProducto, VentaConfirmada};

/// Clave con que se guarda la tasa de cambio vigente.
pub const CLAVE_TASA: &str = "tasa_usd";

/// Un renglón de lo que el cliente se lleva.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaPedida {
    pub producto: i64,
    pub presentacion: i64,
    /// Cuántas presentaciones, no unidades base: 2 six-packs son «2».
    pub cantidad: String,
}

/// Una parte del pago. Un cobro puede tener varias (RF-VTA-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagoPedido {
    /// `EFECTIVO_CUP`, `TRANSFERENCIA` o `EFECTIVO_USD`.
    pub metodo: String,
    /// Lo que entrega el cliente, en la moneda del método.
    pub entregado: String,
}

/// Lo que llega de la pantalla al confirmar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoVender {
    pub lineas: Vec<LineaPedida>,
    pub pagos: Vec<PagoPedido>,
    /// Venta en espera de la que sale este cobro, si sale de una
    /// (RF-VTA-14). Se consume al cobrar, en la misma operación.
    ///
    /// Las líneas mandan, no la espera: al retomarla se pudieron corregir
    /// renglones, y lo que se cobra es lo que hay en pantalla.
    pub espera_id: Option<i64>,
}

/// Lo que se devuelve tras cobrar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VentaHecha {
    pub folio: i64,
    pub total: String,
    pub entregado: String,
    /// Siempre en pesos, aunque se haya pagado en dólares (R-11).
    pub vuelto: String,
}

/// Cobra una venta.
#[derive(Debug)]
pub struct Vender<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> Vender<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoVender) -> Resultado<VentaHecha> {
        if comando.lineas.is_empty() {
            return Err(ErrorAplicacion::Dominio(domain::ErrorDominio::VentaVacia));
        }

        // Sin caja abierta no se cobra (RF-CAJ-06). Se comprueba lo primero,
        // antes de tocar existencias: una venta que no pertenece a ninguna
        // sesión no aparece en ningún arqueo, y ese descuadre no se arregla
        // después porque no hay a qué turno imputarla.
        let sesion = self
            .repositorio
            .sesion_abierta()?
            .ok_or(ErrorAplicacion::Dominio(
                domain::ErrorDominio::SinSesionAbierta,
            ))?;
        let sesion = sesion.sesion.id().ok_or(ErrorAplicacion::Dominio(
            domain::ErrorDominio::SinSesionAbierta,
        ))?;

        let mut venta = Venta::nueva();
        // La existencia se va descontando producto a producto: si el mismo
        // producto aparece en dos líneas, la segunda tiene que ver lo que
        // dejó la primera, no el saldo original.
        let mut inventarios: BTreeMap<i64, Inventario> = BTreeMap::new();
        let mut salidas: BTreeMap<i64, Cantidad> = BTreeMap::new();
        let mut costos: BTreeMap<i64, Dinero> = BTreeMap::new();

        for pedida in &comando.lineas {
            let ProductoConInventario {
                producto,
                inventario,
            } = self
                .repositorio
                .obtener(IdProducto(pedida.producto))?
                .ok_or(ErrorAplicacion::NoEncontrado {
                    entidad: "producto",
                    id: pedida.producto,
                })?;

            // Lo desactivado ya no se vende, aunque siga en el carrito.
            let presentacion =
                producto.presentacion_vendible(IdPresentacion(pedida.presentacion))?;

            let cantidad: Cantidad = pedida.cantidad.trim().parse()?;
            // Media lata no existe, tampoco al venderla (RF-CAT-03).
            producto.validar_cantidad(cantidad)?;

            let estado = inventarios.entry(pedida.producto).or_insert(inventario);

            // El costo se congela ANTES de descontar: es el costo con el
            // que se vendió, no el que quede después (RF-VTA-13).
            let costo_unitario = estado.costo_unitario_o_cero();

            let linea = LineaVenta::nueva(
                IdProducto(pedida.producto),
                IdPresentacion(pedida.presentacion),
                producto.nombre(),
                presentacion.nombre(),
                cantidad,
                presentacion.factor(),
                presentacion.precio(),
                costo_unitario,
            )?;

            // Se descuenta de la VITRINA: la bodega no está a la venta.
            let unidades = linea.unidades_base()?;
            let salida = estado.registrar_salida(unidades, Ubicacion::Vitrina)?;
            *estado = salida.inventario;

            salidas
                .entry(pedida.producto)
                .and_modify(|acumulado| {
                    *acumulado = acumulado.sumar(unidades).unwrap_or(*acumulado);
                })
                .or_insert(unidades);
            costos.insert(pedida.producto, costo_unitario);

            venta.agregar(linea);
        }

        let total = venta.total()?;
        let cobro = self.armar_cobro(&comando.pagos)?;
        let vuelto = cobro.vuelto(total)?;

        // Un asiento por producto, no por línea: el kárdex cuenta lo que
        // salió del estante, y del estante salió una sola vez.
        let mut descuentos = Vec::with_capacity(inventarios.len());
        for (id, inventario) in inventarios {
            let cantidad = salidas.get(&id).copied().unwrap_or(Cantidad::CERO);
            let costo = costos.get(&id).copied().unwrap_or(Dinero::CERO);

            descuentos.push(DescuentoVenta {
                producto: IdProducto(id),
                inventario,
                movimiento: Movimiento::venta(cantidad, costo, Ubicacion::Vitrina)?,
            });
        }

        let costo_total = venta.costo_total()?;
        let folio = self.repositorio.registrar_venta(&VentaConfirmada {
            venta: &venta,
            cobro: &cobro,
            total,
            costo_total,
            vuelto,
            descuentos: &descuentos,
            sesion,
            espera: comando.espera_id.map(IdVentaEnEspera),
        })?;

        Ok(VentaHecha {
            folio,
            total: total.formatear(2),
            entregado: cobro.entregado_en_cup()?.formatear(2),
            vuelto: vuelto.formatear(2),
        })
    }

    /// Construye el cobro, buscando la tasa vigente solo si hace falta.
    fn armar_cobro(&self, pedidos: &[PagoPedido]) -> Resultado<Cobro> {
        let mut pagos = Vec::with_capacity(pedidos.len());

        for pedido in pedidos {
            let metodo: MetodoPago = pedido.metodo.trim().parse()?;
            let entregado: Dinero = pedido.entregado.trim().parse()?;

            let pago = if metodo.requiere_conversion() {
                Pago::en_usd(entregado, self.tasa_vigente()?)?
            } else {
                Pago::en_cup(metodo, entregado)?
            };

            pagos.push(pago);
        }

        Cobro::con_pagos(pagos).map_err(ErrorAplicacion::from)
    }

    /// Tasa con la que se convierten los dólares de hoy (RF-VTA-10b).
    ///
    /// Se lee al cobrar y se congela en el pago: si el dueño la cambia
    /// mañana, esta venta debe seguir diciendo lo mismo.
    fn tasa_vigente(&self) -> Resultado<TasaCambio> {
        let guardada = self
            .repositorio
            .configuracion(CLAVE_TASA)?
            .ok_or(ErrorAplicacion::TasaNoConfigurada)?;

        let cup_por_usd: Dinero = guardada.trim().parse()?;
        TasaCambio::nueva(cup_por_usd).map_err(ErrorAplicacion::from)
    }
}

#[cfg(test)]
mod pruebas {
    use domain::UnidadBase;

    use super::*;
    use crate::dobles::ProductosEnMemoria;

    /// Refresco suelto a 80 y en six-pack a 300, con 20 latas en vitrina,
    /// y una caja abierta para poder cobrar.
    fn catalogo() -> ProductosEnMemoria {
        ProductosEnMemoria::default()
            .con_producto(
                1,
                "Refresco 500 ml",
                UnidadBase::Unidad,
                &[
                    (10, "Unidad", "1", "80.00"),
                    (11, "Six-pack", "6", "300.00"),
                ],
                "20",
            )
            .con_caja_abierta()
    }

    fn cobrar(productos: &ProductosEnMemoria, presentacion: i64) -> Resultado<VentaHecha> {
        Vender::nuevo(productos).ejecutar(ComandoVender {
            lineas: vec![LineaPedida {
                producto: 1,
                presentacion,
                cantidad: "1".to_owned(),
            }],
            pagos: vec![PagoPedido {
                metodo: "EFECTIVO_CUP".to_owned(),
                entregado: "500.00".to_owned(),
            }],
            espera_id: None,
        })
    }

    #[test]
    fn se_cobra_lo_que_esta_activo() {
        let hecha = cobrar(&catalogo(), 11).expect("cobrar");

        assert_eq!(hecha.total, "300.00");
        assert_eq!(hecha.vuelto, "200.00");
    }

    #[test]
    fn no_se_cobra_un_producto_desactivado() {
        let productos = catalogo();
        productos.desactivar_producto(1);

        let error = cobrar(&productos, 10).expect_err("ya no se vende");

        assert_eq!(error.codigo(), "PRODUCTO_INACTIVO");
        // No llegó a registrarse nada.
        assert_eq!(*productos.espera_cobrada.borrow(), None);
    }

    #[test]
    fn no_se_cobra_una_presentacion_desactivada() {
        let productos = catalogo();
        productos.desactivar_presentacion(1, 11);

        let error = cobrar(&productos, 11).expect_err("ya no se vende");
        assert_eq!(error.codigo(), "PRESENTACION_INACTIVA");
        assert_eq!(*productos.espera_cobrada.borrow(), None);

        // La lata suelta sigue a la venta.
        assert!(cobrar(&productos, 10).is_ok());
    }
}
