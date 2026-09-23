//! Adaptador SQLite del repositorio de productos.
//!
//! Traduce entre las entidades del dominio y las filas de la base de datos.
//! Es el único sitio del sistema que sabe a la vez cómo son unas y otras.

use std::sync::Arc;

use application::error::Resultado;
use application::puertos::{ProductoConInventario, RepositorioProducto};
use domain::{
    Cantidad, Dinero, Existencias, IdPresentacion, IdProducto, Inventario, Presentacion,
    Producto, Ubicacion, UnidadBase,
};
use rusqlite::{params, Connection};

use crate::conexion::BaseDatos;
use crate::error::{ErrorInfra, ResultadoInfra};

/// Columnas del producto, en el orden en que las leen las consultas.
const COLUMNAS: &str = "id, sku, nombre, unidad_base, valor_total,
                        stock_minimo, objetivo_vitrina, activo";

/// Repositorio de productos sobre SQLite.
#[derive(Debug, Clone)]
pub struct RepositorioProductoSqlite {
    base: Arc<BaseDatos>,
}

impl RepositorioProductoSqlite {
    pub const fn nuevo(base: Arc<BaseDatos>) -> Self {
        Self { base }
    }
}

impl RepositorioProducto for RepositorioProductoSqlite {
    fn crear(&self, producto: &Producto, inventario: &Inventario) -> Resultado<IdProducto> {
        let id = self.base.en_transaccion(|tx| {
            tx.execute(
                "INSERT INTO producto
                   (sku, nombre, unidad_base, valor_total,
                    stock_minimo, objetivo_vitrina, activo, creado_en)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
                params![
                    producto.sku(),
                    producto.nombre(),
                    producto.unidad_base().como_texto(),
                    inventario.valor_total().millonesimas(),
                    producto.stock_minimo().milesimas(),
                    producto.objetivo_vitrina().milesimas(),
                    i64::from(producto.esta_activo()),
                ],
            )?;

            let id_producto = tx.last_insert_rowid();

            for presentacion in producto.presentaciones() {
                tx.execute(
                    "INSERT INTO presentacion
                       (producto_id, nombre, factor, precio,
                        es_predeterminada, codigo_barras, activa)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        id_producto,
                        presentacion.nombre(),
                        presentacion.factor().milesimas(),
                        presentacion.precio().millonesimas(),
                        i64::from(presentacion.es_predeterminada()),
                        presentacion.codigo_barras(),
                        i64::from(presentacion.esta_activa()),
                    ],
                )?;
            }

            // Las dos ubicaciones existen siempre, aunque estén a cero: así
            // no hay que distinguir entre «cero» y «no hay fila».
            for ubicacion in Ubicacion::TODAS {
                tx.execute(
                    "INSERT INTO existencia (producto_id, ubicacion, cantidad)
                     VALUES (?1, ?2, ?3)",
                    params![
                        id_producto,
                        ubicacion.como_texto(),
                        inventario.existencias().en(ubicacion).milesimas(),
                    ],
                )?;
            }

            Ok(id_producto)
        })?;

        Ok(IdProducto(id))
    }

    fn obtener(&self, id: IdProducto) -> Resultado<Option<ProductoConInventario>> {
        let producto = self.base.con(|conexion| {
            let mut consulta =
                conexion.prepare(&format!("SELECT {COLUMNAS} FROM producto WHERE id = ?1"))?;

            let mut filas = consulta.query(params![id.0])?;
            match filas.next()? {
                Some(fila) => Ok(Some(leer_fila(conexion, fila)?)),
                None => Ok(None),
            }
        })?;

        Ok(producto)
    }

    fn listar(&self, incluir_inactivos: bool) -> Resultado<Vec<ProductoConInventario>> {
        let productos = self.base.con(|conexion| {
            let filtro = if incluir_inactivos {
                ""
            } else {
                "WHERE activo = 1"
            };

            let mut consulta = conexion.prepare(&format!(
                "SELECT {COLUMNAS} FROM producto {filtro} ORDER BY nombre"
            ))?;

            let mut filas = consulta.query([])?;
            let mut productos = Vec::new();

            while let Some(fila) = filas.next()? {
                productos.push(leer_fila(conexion, fila)?);
            }

            Ok(productos)
        })?;

        Ok(productos)
    }

    fn existe_sku(&self, sku: &str) -> Resultado<bool> {
        let existe = self.base.con(|conexion| {
            let total: i64 = conexion.query_row(
                "SELECT COUNT(*) FROM producto WHERE sku = ?1",
                params![sku],
                |fila| fila.get(0),
            )?;
            Ok(total > 0)
        })?;

        Ok(existe)
    }
}

/// Construye un producto con su inventario a partir de su fila.
fn leer_fila(
    conexion: &Connection,
    fila: &rusqlite::Row<'_>,
) -> ResultadoInfra<ProductoConInventario> {
    let id: i64 = fila.get(0)?;
    let sku: String = fila.get(1)?;
    let nombre: String = fila.get(2)?;
    let unidad_texto: String = fila.get(3)?;
    let valor_total: i64 = fila.get(4)?;
    let stock_minimo: i64 = fila.get(5)?;
    let objetivo_vitrina: i64 = fila.get(6)?;
    let activo: i64 = fila.get(7)?;

    let unidad_base: UnidadBase = unidad_texto.parse().map_err(|_| {
        ErrorInfra::DatoCorrupto(format!("unidad «{unidad_texto}» en el producto {id}"))
    })?;

    let producto = Producto::reconstituir(
        IdProducto(id),
        sku,
        nombre,
        unidad_base,
        Cantidad::desde_milesimas(stock_minimo),
        Cantidad::desde_milesimas(objetivo_vitrina),
        leer_presentaciones(conexion, id)?,
        activo != 0,
    );

    let inventario = Inventario::nuevo(
        leer_existencias(conexion, id)?,
        Dinero::desde_millonesimas(valor_total),
    )
    .map_err(|error| ErrorInfra::DatoCorrupto(format!("inventario del producto {id}: {error}")))?;

    Ok(ProductoConInventario {
        producto,
        inventario,
    })
}

/// Lee la existencia de las dos ubicaciones.
///
/// Una ubicación sin fila se lee como cero: es lo que ocurre con los
/// productos escritos antes de que existiera la fila, y negarse a leerlos
/// sería peor que asumir que no hay nada.
fn leer_existencias(conexion: &Connection, producto_id: i64) -> ResultadoInfra<Existencias> {
    let mut consulta = conexion
        .prepare("SELECT ubicacion, cantidad FROM existencia WHERE producto_id = ?1")?;

    let mut filas = consulta.query(params![producto_id])?;
    let mut bodega = Cantidad::CERO;
    let mut vitrina = Cantidad::CERO;

    while let Some(fila) = filas.next()? {
        let ubicacion_texto: String = fila.get(0)?;
        let cantidad: i64 = fila.get(1)?;

        let ubicacion: Ubicacion = ubicacion_texto.parse().map_err(|_| {
            ErrorInfra::DatoCorrupto(format!(
                "ubicación «{ubicacion_texto}» en el producto {producto_id}"
            ))
        })?;

        match ubicacion {
            Ubicacion::Bodega => bodega = Cantidad::desde_milesimas(cantidad),
            Ubicacion::Vitrina => vitrina = Cantidad::desde_milesimas(cantidad),
        }
    }

    Existencias::nuevas(bodega, vitrina).map_err(|error| {
        ErrorInfra::DatoCorrupto(format!("existencias del producto {producto_id}: {error}"))
    })
}

fn leer_presentaciones(
    conexion: &Connection,
    producto_id: i64,
) -> ResultadoInfra<Vec<Presentacion>> {
    let mut consulta = conexion.prepare(
        "SELECT id, nombre, factor, precio, es_predeterminada, codigo_barras, activa
         FROM presentacion WHERE producto_id = ?1 ORDER BY factor",
    )?;

    let mut filas = consulta.query(params![producto_id])?;
    let mut presentaciones = Vec::new();

    while let Some(fila) = filas.next()? {
        let id: i64 = fila.get(0)?;
        let nombre: String = fila.get(1)?;
        let factor: i64 = fila.get(2)?;
        let precio: i64 = fila.get(3)?;
        let es_predeterminada: i64 = fila.get(4)?;
        let codigo_barras: Option<String> = fila.get(5)?;
        let activa: i64 = fila.get(6)?;

        presentaciones.push(Presentacion::reconstituir(
            IdPresentacion(id),
            nombre,
            Cantidad::desde_milesimas(factor),
            Dinero::desde_millonesimas(precio),
            es_predeterminada != 0,
            codigo_barras,
            activa != 0,
        ));
    }

    Ok(presentaciones)
}
