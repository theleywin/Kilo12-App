//! Caso de uso: cuánto va la venta hasta ahora (RF-VTA-05).
//!
//! La pantalla necesita enseñar el total mientras se arma la venta, y la
//! pantalla no suma dinero: se lo pregunta al núcleo, que es donde la
//! aritmética es exacta.
//!
//! Aprovecha el viaje para avisar de lo que no alcanzaría en vitrina, de
//! modo que el problema se vea **antes** de que el cliente esté esperando
//! con el dinero en la mano.

use std::collections::BTreeMap;

use domain::{Cantidad, Dinero, IdPresentacion, IdProducto, Ubicacion};

use crate::casos::formatear_cantidad;
use crate::casos::vender::LineaPedida;
use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{ProductoConInventario, RepositorioProducto};

/// Una línea de la venta en curso, ya calculada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaPrevista {
    pub producto: i64,
    pub presentacion: i64,
    pub nombre_producto: String,
    pub nombre_presentacion: String,
    pub cantidad: String,
    pub precio: String,
    /// Precio × cantidad.
    pub importe: String,
    /// Lo que saldría de la vitrina por esta línea.
    pub unidades_base: String,
    /// La vitrina no da para esta línea, contando las anteriores.
    pub sin_existencia: bool,
}

/// La venta en curso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VentaPrevista {
    pub lineas: Vec<LineaPrevista>,
    pub total: String,
    /// Alguna línea no se puede servir: cobrar fallaría.
    pub hay_faltantes: bool,
}

/// Calcula la venta en curso sin tocar nada.
#[derive(Debug)]
pub struct PrevisualizarVenta<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> PrevisualizarVenta<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, pedidas: Vec<LineaPedida>) -> Resultado<VentaPrevista> {
        let mut lineas = Vec::with_capacity(pedidas.len());
        let mut total = Dinero::CERO;
        // Lo que ya se llevaron las líneas anteriores del mismo producto:
        // dos renglones de arroz compiten por la misma vitrina.
        let mut comprometido: BTreeMap<i64, Cantidad> = BTreeMap::new();
        let mut hay_faltantes = false;

        for pedida in pedidas {
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

            let presentacion = producto
                .presentacion(IdPresentacion(pedida.presentacion))
                .ok_or(ErrorAplicacion::Dominio(
                    domain::ErrorDominio::PresentacionNoEncontrada,
                ))?;

            let cantidad: Cantidad = pedida.cantidad.trim().parse()?;
            producto.validar_cantidad(cantidad)?;

            let unidades = presentacion.a_unidades_base(cantidad)?;
            let importe = presentacion.importe(cantidad)?;
            total = total.sumar(importe)?;

            let ya_comprometido = comprometido
                .get(&pedida.producto)
                .copied()
                .unwrap_or(Cantidad::CERO);
            let necesario = ya_comprometido.sumar(unidades)?;
            let disponible = inventario.existencias().en(Ubicacion::Vitrina);
            let falta = necesario > disponible;

            hay_faltantes = hay_faltantes || falta;
            comprometido.insert(pedida.producto, necesario);

            lineas.push(LineaPrevista {
                producto: pedida.producto,
                presentacion: pedida.presentacion,
                nombre_producto: producto.nombre().to_string(),
                nombre_presentacion: presentacion.nombre().to_string(),
                cantidad: formatear_cantidad(cantidad, &producto),
                precio: presentacion.precio().formatear(2),
                importe: importe.formatear(2),
                unidades_base: formatear_cantidad(unidades, &producto),
                sin_existencia: falta,
            });
        }

        Ok(VentaPrevista {
            lineas,
            total: total.formatear(2),
            hay_faltantes,
        })
    }
}
