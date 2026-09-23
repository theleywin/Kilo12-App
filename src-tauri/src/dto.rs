//! Objetos de transferencia entre Rust y la interfaz.
//!
//! Viven aquí, y no en la capa de aplicación, porque `serde` es un detalle
//! del transporte: la aplicación no tiene por qué saber en qué formato
//! viajan sus datos (DT-5).
//!
//! **Los importes y las cantidades son cadenas de texto, nunca números.**
//! Un `number` de JSON es un `f64`: serializar `41.67` y deserializarlo
//! puede no devolver `41.67`, y ahí se pierde la exactitud que todo el
//! dominio se toma el trabajo de garantizar (DT-7).

use application::casos::{
    ComandoFijarObjetivo, ComandoRegistrarEntrada, ComandoRegistrarMerma, ComandoRegistrarProducto,
    ComandoSimular, ComandoTraspasar, LineaKardex, LineaVitrina, ProductoListado, ResumenAlmacen,
    ResumenVitrina, Simulacion,
};
use application::{ErrorAplicacion, Margen};
use serde::{Deserialize, Serialize};

/// Datos para dar de alta un producto.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuevoProductoDto {
    /// Vacío significa «derívalo del nombre» (RF-CAT-02).
    #[serde(default)]
    pub sku: String,
    pub nombre: String,
    /// `unidad`, `kg`, `lb`, `g`, `L` o `ml`. Todo lo que no sea `unidad`
    /// se vende en fracciones: no hay una casilla aparte para eso.
    pub unidad_base: String,
    /// Precio de venta de una unidad base, como texto. Ejemplo: `"180.00"`.
    pub precio_unitario: String,
    /// Lo que cuesta una unidad base al dueño.
    #[serde(default)]
    pub costo_unitario: Option<String>,
    /// Cuánto entra al almacén al dar de alta el producto.
    #[serde(default)]
    pub cantidad_almacen: Option<String>,
    /// Cuánto se pone en la vitrina al dar de alta el producto.
    #[serde(default)]
    pub cantidad_vitrina: Option<String>,
    #[serde(default)]
    pub stock_minimo: Option<String>,
    #[serde(default)]
    pub objetivo_vitrina: Option<String>,
}

impl From<NuevoProductoDto> for ComandoRegistrarProducto {
    fn from(dto: NuevoProductoDto) -> Self {
        Self {
            sku: dto.sku,
            nombre: dto.nombre,
            unidad_base: dto.unidad_base,
            precio_unitario: dto.precio_unitario,
            costo_unitario: dto.costo_unitario,
            cantidad_almacen: dto.cantidad_almacen,
            cantidad_vitrina: dto.cantidad_vitrina,
            stock_minimo: dto.stock_minimo,
            objetivo_vitrina: dto.objetivo_vitrina,
        }
    }
}

/// Producto tal como lo muestra la lista del catálogo.
///
/// Todo llega formateado: la pantalla muestra lo que recibe y no hace ni
/// una cuenta con dinero.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductoDto {
    pub id: i64,
    pub sku: String,
    pub nombre: String,
    pub unidad_base: String,
    pub unidad_nombre: String,
    pub es_granel: bool,
    pub activo: bool,
    pub precio: String,
    pub costo: String,
    pub ganancia: String,
    pub margen: String,
    pub en_riesgo: bool,
    pub en_almacen: String,
    pub en_vitrina: String,
    pub existencia_total: String,
    pub bajo_minimo: bool,
    pub agotado: bool,
    pub presentacion: String,
    pub total_presentaciones: usize,
}

impl From<ProductoListado> for ProductoDto {
    fn from(listado: ProductoListado) -> Self {
        Self {
            id: listado.id,
            sku: listado.sku,
            nombre: listado.nombre,
            unidad_base: listado.unidad_base,
            unidad_nombre: listado.unidad_nombre,
            es_granel: listado.es_granel,
            activo: listado.activo,
            precio: listado.precio,
            costo: listado.costo,
            ganancia: listado.ganancia,
            margen: listado.margen,
            en_riesgo: listado.en_riesgo,
            en_almacen: listado.en_almacen,
            en_vitrina: listado.en_vitrina,
            existencia_total: listado.existencia_total,
            bajo_minimo: listado.bajo_minimo,
            agotado: listado.agotado,
            presentacion: listado.presentacion,
            total_presentaciones: listado.total_presentaciones,
        }
    }
}

/// Datos de una entrada de mercancía.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuevaEntradaDto {
    pub producto: i64,
    pub cantidad: String,
    pub costo_unitario: String,
    /// `BODEGA` o `VITRINA`.
    pub destino: String,
}

impl From<NuevaEntradaDto> for ComandoRegistrarEntrada {
    fn from(dto: NuevaEntradaDto) -> Self {
        Self {
            producto: dto.producto,
            cantidad: dto.cantidad,
            costo_unitario: dto.costo_unitario,
            destino: dto.destino,
        }
    }
}

/// Datos de una baja por merma.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuevaMermaDto {
    pub producto: i64,
    pub cantidad: String,
    /// `BODEGA` o `VITRINA`.
    pub origen: String,
    pub motivo: String,
}

impl From<NuevaMermaDto> for ComandoRegistrarMerma {
    fn from(dto: NuevaMermaDto) -> Self {
        Self {
            producto: dto.producto,
            cantidad: dto.cantidad,
            origen: dto.origen,
            motivo: dto.motivo,
        }
    }
}

/// Datos de un traspaso entre almacén y vitrina.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuevoTraspasoDto {
    pub producto: i64,
    pub cantidad: String,
    /// De dónde sale. El destino es la otra ubicación.
    pub origen: String,
}

impl From<NuevoTraspasoDto> for ComandoTraspasar {
    fn from(dto: NuevoTraspasoDto) -> Self {
        Self {
            producto: dto.producto,
            cantidad: dto.cantidad,
            origen: dto.origen,
        }
    }
}

/// Movimiento que se está planteando, para la vista previa.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsultaSimulacionDto {
    pub producto: i64,
    /// `ENTRADA`, `TRASPASO` o `MERMA`.
    pub tipo: String,
    pub cantidad: String,
    pub ubicacion: String,
}

impl From<ConsultaSimulacionDto> for ComandoSimular {
    fn from(dto: ConsultaSimulacionDto) -> Self {
        Self {
            producto: dto.producto,
            tipo: dto.tipo,
            cantidad: dto.cantidad,
            ubicacion: dto.ubicacion,
        }
    }
}

/// Cómo quedaría la existencia si el movimiento se hiciera.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulacionDto {
    pub posible: bool,
    pub problema: Option<String>,
    pub disponible: String,
    pub bodega_resultante: String,
    pub vitrina_resultante: String,
}

impl From<Simulacion> for SimulacionDto {
    fn from(simulacion: Simulacion) -> Self {
        Self {
            posible: simulacion.posible,
            problema: simulacion.problema,
            disponible: simulacion.disponible,
            bodega_resultante: simulacion.bodega_resultante,
            vitrina_resultante: simulacion.vitrina_resultante,
        }
    }
}

/// Estado del almacén tal como lo muestra la pantalla.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlmacenDto {
    pub productos: Vec<ProductoDto>,
    pub valor_total: String,
    pub con_existencia: usize,
    pub bajo_minimo: usize,
    pub agotados: usize,
}

impl From<ResumenAlmacen> for AlmacenDto {
    fn from(resumen: ResumenAlmacen) -> Self {
        Self {
            productos: resumen
                .productos
                .into_iter()
                .map(ProductoDto::from)
                .collect(),
            valor_total: resumen.valor_total,
            con_existencia: resumen.con_existencia,
            bajo_minimo: resumen.bajo_minimo,
            agotados: resumen.agotados,
        }
    }
}

/// Un producto visto desde la vitrina.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineaVitrinaDto {
    pub id: i64,
    pub sku: String,
    pub nombre: String,
    pub unidad_base: String,
    pub unidad_nombre: String,
    pub en_vitrina: String,
    pub objetivo: String,
    pub en_almacen: String,
    pub sugerido: String,
    pub hay_que_reponer: bool,
    pub esta_exhibido: bool,
    pub falta_comprar: bool,
    pub disponible_sin_exhibir: bool,
    pub agotado: bool,
}

impl From<LineaVitrina> for LineaVitrinaDto {
    fn from(linea: LineaVitrina) -> Self {
        Self {
            id: linea.id,
            sku: linea.sku,
            nombre: linea.nombre,
            unidad_base: linea.unidad_base,
            unidad_nombre: linea.unidad_nombre,
            en_vitrina: linea.en_vitrina,
            objetivo: linea.objetivo,
            en_almacen: linea.en_almacen,
            sugerido: linea.sugerido,
            hay_que_reponer: linea.hay_que_reponer,
            esta_exhibido: linea.esta_exhibido,
            falta_comprar: linea.falta_comprar,
            disponible_sin_exhibir: linea.disponible_sin_exhibir,
            agotado: linea.agotado,
        }
    }
}

/// Estado de la vitrina tal como lo muestra la pantalla.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VitrinaDto {
    pub productos: Vec<LineaVitrinaDto>,
    pub exhibidos: usize,
    pub por_reponer: usize,
    pub sin_exhibir: usize,
    pub falta_comprar: usize,
}

impl From<ResumenVitrina> for VitrinaDto {
    fn from(resumen: ResumenVitrina) -> Self {
        Self {
            productos: resumen
                .productos
                .into_iter()
                .map(LineaVitrinaDto::from)
                .collect(),
            exhibidos: resumen.exhibidos,
            por_reponer: resumen.por_reponer,
            sin_exhibir: resumen.sin_exhibir,
            falta_comprar: resumen.falta_comprar,
        }
    }
}

/// Cuánto se quiere mantener exhibido de un producto.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjetivoVitrinaDto {
    pub producto: i64,
    pub objetivo: String,
}

impl From<ObjetivoVitrinaDto> for ComandoFijarObjetivo {
    fn from(dto: ObjetivoVitrinaDto) -> Self {
        Self {
            producto: dto.producto,
            objetivo: dto.objetivo,
        }
    }
}

/// Una línea del historial de un producto (RF-INV-05).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovimientoDto {
    pub id: i64,
    pub tipo: String,
    pub tipo_nombre: String,
    pub es_entrada: bool,
    pub cantidad: String,
    pub costo_unitario: String,
    pub importe: String,
    pub origen: Option<String>,
    pub destino: Option<String>,
    pub bodega_resultante: String,
    pub vitrina_resultante: String,
    pub motivo: Option<String>,
    pub ocurrido_en: String,
}

impl From<LineaKardex> for MovimientoDto {
    fn from(linea: LineaKardex) -> Self {
        Self {
            id: linea.id,
            tipo: linea.tipo,
            tipo_nombre: linea.tipo_nombre,
            es_entrada: linea.es_entrada,
            cantidad: linea.cantidad,
            costo_unitario: linea.costo_unitario,
            importe: linea.importe,
            origen: linea.origen,
            destino: linea.destino,
            bodega_resultante: linea.bodega_resultante,
            vitrina_resultante: linea.vitrina_resultante,
            motivo: linea.motivo,
            ocurrido_en: linea.ocurrido_en,
        }
    }
}

/// Ganancia y margen de un precio frente a su costo (RF-PRE-01).
///
/// La pantalla lo pide mientras el usuario escribe, para que vea lo que
/// gana antes de guardar. La cuenta la hace Rust: JavaScript no tiene
/// aritmética decimal exacta.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MargenDto {
    /// Precio menos costo, con dos decimales. Puede venir en negativo.
    pub ganancia: String,
    /// Margen bruto ya formateado, sin el símbolo de porcentaje.
    pub porcentaje: String,
    /// El costo se comió el precio (RF-COM-05).
    pub en_riesgo: bool,
}

impl From<Margen> for MargenDto {
    fn from(margen: Margen) -> Self {
        Self {
            ganancia: margen.ganancia.formatear(2),
            porcentaje: margen.porcentaje.formatear(1),
            en_riesgo: margen.en_riesgo,
        }
    }
}

/// Error tal como lo recibe la interfaz.
///
/// Lleva un código estable para que la pantalla pueda distinguir «SKU
/// duplicado» de «falló la base de datos» y reaccionar distinto, en lugar de
/// mostrar un texto suelto y encogerse de hombros.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDto {
    pub codigo: String,
    pub mensaje: String,
    /// Si es `true`, el usuario puede corregirlo; si no, es un fallo técnico.
    pub del_usuario: bool,
}

impl From<ErrorAplicacion> for ErrorDto {
    fn from(error: ErrorAplicacion) -> Self {
        Self {
            codigo: error.codigo().to_string(),
            mensaje: error.to_string(),
            del_usuario: error.es_del_usuario(),
        }
    }
}

impl From<domain::ErrorDominio> for ErrorDto {
    fn from(error: domain::ErrorDominio) -> Self {
        Self::from(ErrorAplicacion::Dominio(error))
    }
}
