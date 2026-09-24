//! Caso de uso: la ficha comercial de un producto.
//!
//! Responde la pregunta que hace el dueño cuando mira su catálogo: *¿a
//! cuánto lo vendo y cuánto me deja?* Cada presentación trae su propia
//! economía, porque sus precios son independientes entre sí: el six-pack no
//! vale seis veces la lata suelta (RF-PRS-04).

use domain::{Dinero, IdProducto, Presentacion};

use crate::casos::formatear_cantidad;
use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{ProductoConInventario, RepositorioProducto};

/// Lo que se muestra cuando el dato no se puede calcular todavía.
const SIN_DATO: &str = "—";

/// Una forma de vender el producto, con su economía (RF-PRS-11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentacionDetallada {
    pub id: i64,
    pub nombre: String,
    /// Cuántas unidades base se llevan al vender una (RF-PRS-02).
    pub factor: String,
    pub precio: String,
    /// `factor × costo unitario base`, o `—` si todavía no hay costo.
    pub costo: String,
    pub ganancia: String,
    pub margen: String,
    /// Precio equivalente por unidad base, para comparar entre sí las
    /// presentaciones (RF-PRS-10).
    pub precio_por_unidad_base: String,
    /// El costo se comió el precio (RF-COM-05).
    pub en_riesgo: bool,
    /// Sale más cara por unidad que una presentación menor (RF-PRS-12).
    pub precio_anomalo: bool,
    pub es_predeterminada: bool,
    pub activa: bool,
    pub codigo_barras: Option<String>,
}

/// La ficha completa de un producto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FichaProducto {
    pub id: i64,
    pub sku: String,
    pub nombre: String,
    pub unidad_base: String,
    pub unidad_nombre: String,
    pub es_granel: bool,
    pub activo: bool,
    /// Costo promedio ponderado vigente por unidad base.
    pub costo: String,
    pub stock_minimo: String,
    pub objetivo_vitrina: String,
    pub existencia_total: String,
    pub presentaciones: Vec<PresentacionDetallada>,
}

/// Consulta la ficha comercial de un producto.
#[derive(Debug)]
pub struct ConsultarProducto<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> ConsultarProducto<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, id: i64) -> Resultado<FichaProducto> {
        let ProductoConInventario {
            producto,
            inventario,
        } = self
            .repositorio
            .obtener(IdProducto(id))?
            .ok_or(ErrorAplicacion::NoEncontrado {
                entidad: "producto",
                id,
            })?;

        let costo = inventario.costo_unitario().ok();

        // Las anómalas las detecta el dominio comparando precios por unidad
        // base entre presentaciones (RF-PRS-12).
        let anomalas: Vec<i64> = producto
            .presentaciones_con_precio_anomalo()
            .unwrap_or_default()
            .iter()
            .filter_map(|p| p.id().map(|id| id.0))
            .collect();

        let presentaciones = producto
            .presentaciones()
            .iter()
            .map(|presentacion| detallar(presentacion, costo, &anomalas, &producto))
            .collect();

        Ok(FichaProducto {
            id,
            sku: producto.sku().to_string(),
            nombre: producto.nombre().to_string(),
            unidad_base: producto.unidad_base().simbolo().to_string(),
            unidad_nombre: producto
                .unidad_base()
                .nombre_presentacion_unitaria()
                .to_string(),
            es_granel: producto.es_granel(),
            activo: producto.esta_activo(),
            costo: costo.map_or_else(|| SIN_DATO.to_owned(), |c| c.formatear(2)),
            stock_minimo: formatear_cantidad(producto.stock_minimo(), &producto),
            objetivo_vitrina: formatear_cantidad(producto.objetivo_vitrina(), &producto),
            existencia_total: formatear_cantidad(
                inventario
                    .cantidad_total()
                    .unwrap_or(domain::Cantidad::CERO),
                &producto,
            ),
            presentaciones,
        })
    }
}

fn detallar(
    presentacion: &Presentacion,
    costo: Option<Dinero>,
    anomalas: &[i64],
    producto: &domain::Producto,
) -> PresentacionDetallada {
    let id = presentacion.id().map_or(0, |id| id.0);

    // Sin existencia no hay costo, y sin costo no hay margen que enseñar.
    // Poner cero sería afirmar que la mercancía es gratis.
    let economia = costo.map(|costo| {
        (
            presentacion.costo(costo),
            presentacion.ganancia(costo),
            presentacion.margen(costo),
        )
    });

    PresentacionDetallada {
        id,
        nombre: presentacion.nombre().to_string(),
        factor: formatear_cantidad(presentacion.factor(), producto),
        precio: presentacion.precio().formatear(2),
        costo: economia
            .as_ref()
            .and_then(|(costo, _, _)| costo.as_ref().ok())
            .map_or_else(|| SIN_DATO.to_owned(), |costo| costo.formatear(2)),
        ganancia: economia
            .as_ref()
            .and_then(|(_, ganancia, _)| ganancia.as_ref().ok())
            .map_or_else(|| SIN_DATO.to_owned(), |valor| valor.formatear(2)),
        margen: economia
            .as_ref()
            .and_then(|(_, _, margen)| margen.as_ref().ok())
            .map_or_else(
                || SIN_DATO.to_owned(),
                |margen| format!("{} %", margen.formatear(1)),
            ),
        en_riesgo: economia
            .as_ref()
            .and_then(|(_, ganancia, _)| ganancia.as_ref().ok())
            .is_some_and(|ganancia| !ganancia.es_positivo()),
        precio_por_unidad_base: presentacion
            .precio_por_unidad_base()
            .map_or_else(|_| SIN_DATO.to_owned(), |precio| precio.formatear(2)),
        precio_anomalo: anomalas.contains(&id),
        es_predeterminada: presentacion.es_predeterminada(),
        activa: presentacion.esta_activa(),
        codigo_barras: presentacion.codigo_barras().map(str::to_owned),
    }
}
