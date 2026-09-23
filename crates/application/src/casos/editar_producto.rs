//! Casos de uso que cambian la ficha comercial de un producto.
//!
//! Ninguno toca la existencia ni el valor del inventario: eso solo cambia
//! con un movimiento. Aquí se decide **cómo se vende**, no **cuánto hay**.

use domain::{Cantidad, Dinero, IdPresentacion, IdProducto, Presentacion};

use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{CambioDePrecio, ProductoConInventario, RepositorioProducto};

/// Datos editables de un producto (RF-CAT-05, RF-CAT-06).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoEditarProducto {
    pub producto: i64,
    pub nombre: String,
    /// Existencia por debajo de la cual avisar. Vacío equivale a cero.
    pub stock_minimo: Option<String>,
    pub activo: bool,
}

/// Cambia los datos de un producto.
#[derive(Debug)]
pub struct EditarProducto<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> EditarProducto<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoEditarProducto) -> Resultado<()> {
        let mut producto = cargar(self.repositorio, comando.producto)?;

        producto.renombrar(comando.nombre.trim())?;
        producto = producto.con_stock_minimo(cantidad(comando.stock_minimo.as_deref())?)?;

        // Desactivar no borra: el producto desaparece de las ventas nuevas
        // pero su historial sigue consultable (RF-CAT-06).
        if comando.activo {
            producto.activar();
        } else {
            producto.desactivar();
        }

        self.repositorio.actualizar_producto(&producto, &[])
    }
}

/// Datos para poner una presentación nueva a la venta (RF-PRS-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoAgregarPresentacion {
    pub producto: i64,
    pub nombre: String,
    /// Cuántas unidades base se llevan al vender una.
    pub factor: String,
    pub precio: String,
    pub codigo_barras: Option<String>,
}

/// Agrega una forma de vender el producto.
#[derive(Debug)]
pub struct AgregarPresentacion<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> AgregarPresentacion<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoAgregarPresentacion) -> Resultado<()> {
        let mut producto = cargar(self.repositorio, comando.producto)?;

        let factor: Cantidad = comando.factor.trim().parse()?;
        let precio: Dinero = comando.precio.trim().parse()?;

        // La unidad base del producto decide si el factor puede llevar
        // decimales: no existe el paquete de dos latas y media (RF-PRS-14).
        let mut presentacion = Presentacion::nueva(
            comando.nombre.trim(),
            factor,
            precio,
            producto.unidad_base(),
        )?;
        if let Some(codigo) = comando.codigo_barras.as_deref() {
            if !codigo.trim().is_empty() {
                presentacion = presentacion.con_codigo_barras(codigo.trim());
            }
        }

        // El dominio valida el factor contra la unidad base: un producto
        // que se cuenta por unidades no admite paquetes de media (RF-PRS-14).
        producto.agregar_presentacion(presentacion)?;

        self.repositorio.actualizar_producto(&producto, &[])
    }
}

/// Datos para cambiar el precio de una presentación (RF-PRE-01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoCambiarPrecio {
    pub producto: i64,
    pub presentacion: i64,
    pub precio: String,
}

/// Cambia el precio de venta de una presentación.
#[derive(Debug)]
pub struct CambiarPrecio<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> CambiarPrecio<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoCambiarPrecio) -> Resultado<()> {
        let mut producto = cargar(self.repositorio, comando.producto)?;
        let id = IdPresentacion(comando.presentacion);

        // El precio anterior se lee ANTES de pisarlo: después ya no existe
        // en ninguna parte, y sin él no hay historial que valga (RF-PRE-04).
        let anterior = producto
            .presentacion(id)
            .ok_or(ErrorAplicacion::Dominio(
                domain::ErrorDominio::PresentacionNoEncontrada,
            ))?
            .precio();

        let nuevo: Dinero = comando.precio.trim().parse()?;
        producto.cambiar_precio(id, nuevo)?;

        let cambio = CambioDePrecio {
            presentacion: id,
            anterior,
            nuevo,
        };

        self.repositorio.actualizar_producto(&producto, &[cambio])
    }
}

/// Qué presentación se retira o se marca como predeterminada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoPresentacion {
    pub producto: i64,
    pub presentacion: i64,
}

/// Retira una presentación de la venta (RF-PRS-15).
#[derive(Debug)]
pub struct DesactivarPresentacion<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> DesactivarPresentacion<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoPresentacion) -> Resultado<()> {
        let mut producto = cargar(self.repositorio, comando.producto)?;
        producto.desactivar_presentacion(IdPresentacion(comando.presentacion))?;
        self.repositorio.actualizar_producto(&producto, &[])
    }
}

/// Elige la presentación que usa la venta rápida (RF-PRS-08).
#[derive(Debug)]
pub struct MarcarPredeterminada<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> MarcarPredeterminada<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoPresentacion) -> Resultado<()> {
        let mut producto = cargar(self.repositorio, comando.producto)?;
        producto.marcar_predeterminada(IdPresentacion(comando.presentacion))?;
        self.repositorio.actualizar_producto(&producto, &[])
    }
}

fn cargar<R: RepositorioProducto>(repositorio: &R, id: i64) -> Resultado<domain::Producto> {
    let ProductoConInventario { producto, .. } =
        repositorio
            .obtener(IdProducto(id))?
            .ok_or(ErrorAplicacion::NoEncontrado {
                entidad: "producto",
                id,
            })?;

    Ok(producto)
}

fn cantidad(texto: Option<&str>) -> Resultado<Cantidad> {
    match texto.map(str::trim) {
        None | Some("") => Ok(Cantidad::CERO),
        Some(valor) => Ok(valor.parse::<Cantidad>()?),
    }
}
