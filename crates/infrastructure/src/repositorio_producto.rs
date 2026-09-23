//! Adaptador SQLite del repositorio de productos.
//!
//! Traduce entre las entidades del dominio y las filas de la base de datos.
//! Es el único sitio del sistema que sabe a la vez cómo son unas y otras.

use std::sync::Arc;

use application::error::Resultado;
use application::puertos::{
    Asiento, CambioDePrecio, CambioRegistrado, MovimientoRegistrado, ProductoConInventario,
    RepositorioProducto,
};
use domain::{
    Cantidad, Dinero, Existencias, IdPresentacion, IdProducto, Inventario, Movimiento,
    Presentacion, Producto, TipoMovimiento, Ubicacion, UnidadBase,
};

use rusqlite::{params, Connection, Transaction};

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
    fn crear(
        &self,
        producto: &Producto,
        inventario: &Inventario,
        asientos: &[Asiento],
    ) -> Resultado<IdProducto> {
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

            // El asiento de apertura: la mercancía con la que nace el
            // producto también tiene que dejar rastro (RF-INV-03).
            for asiento in asientos {
                escribir_movimiento(tx, id_producto, &asiento.movimiento, asiento.resultante)?;
            }

            Ok(id_producto)
        })?;

        Ok(IdProducto(id))
    }

    fn registrar_movimiento(
        &self,
        id: IdProducto,
        inventario: &Inventario,
        movimiento: &Movimiento,
    ) -> Resultado<()> {
        self.base.en_transaccion(|tx| {
            // El saldo y su explicación se escriben juntos o no se escribe
            // ninguno de los dos (RNF-5).
            tx.execute(
                "UPDATE producto SET valor_total = ?2 WHERE id = ?1",
                params![id.0, inventario.valor_total().millonesimas()],
            )?;

            for ubicacion in Ubicacion::TODAS {
                tx.execute(
                    "UPDATE existencia SET cantidad = ?3
                     WHERE producto_id = ?1 AND ubicacion = ?2",
                    params![
                        id.0,
                        ubicacion.como_texto(),
                        inventario.existencias().en(ubicacion).milesimas(),
                    ],
                )?;
            }

            escribir_movimiento(tx, id.0, movimiento, inventario.existencias())?;

            Ok(())
        })?;

        Ok(())
    }

    fn kardex(&self, id: IdProducto, limite: usize) -> Resultado<Vec<MovimientoRegistrado>> {
        let movimientos = self.base.con(|conexion| {
            let mut consulta = conexion.prepare(
                "SELECT id, tipo, origen, destino, cantidad, costo_unitario,
                        bodega_resultante, vitrina_resultante, motivo, ocurrido_en
                 FROM movimiento
                 WHERE producto_id = ?1
                 ORDER BY id DESC
                 LIMIT ?2",
            )?;

            let mut filas = consulta.query(params![id.0, limite as i64])?;
            let mut movimientos = Vec::new();

            while let Some(fila) = filas.next()? {
                movimientos.push(leer_movimiento(fila)?);
            }

            Ok(movimientos)
        })?;

        Ok(movimientos)
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

    fn actualizar_producto(
        &self,
        producto: &Producto,
        cambios: &[CambioDePrecio],
    ) -> Resultado<()> {
        let id = producto.id().ok_or_else(|| {
            ErrorInfra::DatoCorrupto("se intentó actualizar un producto sin id".to_owned())
        })?;

        self.base.en_transaccion(|tx| {
            // Solo lo editable. La existencia y el valor no se tocan aquí:
            // eso únicamente cambia con un movimiento.
            tx.execute(
                "UPDATE producto
                    SET nombre = ?2, stock_minimo = ?3, objetivo_vitrina = ?4, activo = ?5
                  WHERE id = ?1",
                params![
                    id.0,
                    producto.nombre(),
                    producto.stock_minimo().milesimas(),
                    producto.objetivo_vitrina().milesimas(),
                    i64::from(producto.esta_activo()),
                ],
            )?;

            for presentacion in producto.presentaciones() {
                match presentacion.id() {
                    Some(presentacion_id) => {
                        tx.execute(
                            "UPDATE presentacion
                                SET nombre = ?2, factor = ?3, precio = ?4,
                                    es_predeterminada = ?5, codigo_barras = ?6, activa = ?7
                              WHERE id = ?1",
                            params![
                                presentacion_id.0,
                                presentacion.nombre(),
                                presentacion.factor().milesimas(),
                                presentacion.precio().millonesimas(),
                                i64::from(presentacion.es_predeterminada()),
                                presentacion.codigo_barras(),
                                i64::from(presentacion.esta_activa()),
                            ],
                        )?;
                    }
                    // Sin identificador: es una presentación recién añadida.
                    None => {
                        tx.execute(
                            "INSERT INTO presentacion
                               (producto_id, nombre, factor, precio,
                                es_predeterminada, codigo_barras, activa)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                            params![
                                id.0,
                                presentacion.nombre(),
                                presentacion.factor().milesimas(),
                                presentacion.precio().millonesimas(),
                                i64::from(presentacion.es_predeterminada()),
                                presentacion.codigo_barras(),
                                i64::from(presentacion.esta_activa()),
                            ],
                        )?;
                    }
                }
            }

            // El historial se escribe en la misma transacción: un precio
            // cambiado sin su anotación es un precio que nadie puede
            // explicar (RF-PRE-04).
            for cambio in cambios {
                tx.execute(
                    "INSERT INTO historial_precio
                       (producto_id, presentacion_id, anterior, nuevo, cambiado_en)
                     VALUES (?1, ?2, ?3, ?4, datetime('now'))",
                    params![
                        id.0,
                        cambio.presentacion.0,
                        cambio.anterior.millonesimas(),
                        cambio.nuevo.millonesimas(),
                    ],
                )?;
            }

            Ok(())
        })?;

        Ok(())
    }

    fn historial_precios(&self, id: IdProducto) -> Resultado<Vec<CambioRegistrado>> {
        let historial = self.base.con(|conexion| {
            let mut consulta = conexion.prepare(
                "SELECT id, presentacion_id, anterior, nuevo, cambiado_en
                   FROM historial_precio
                  WHERE producto_id = ?1
                  ORDER BY id DESC",
            )?;

            let mut filas = consulta.query(params![id.0])?;
            let mut historial = Vec::new();

            while let Some(fila) = filas.next()? {
                historial.push(CambioRegistrado {
                    id: fila.get(0)?,
                    presentacion: IdPresentacion(fila.get(1)?),
                    anterior: Dinero::desde_millonesimas(fila.get(2)?),
                    nuevo: Dinero::desde_millonesimas(fila.get(3)?),
                    cambiado_en: fila.get(4)?,
                });
            }

            Ok(historial)
        })?;

        Ok(historial)
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

/// Escribe un asiento del kárdex.
///
/// Guarda el saldo resultante junto al movimiento, en lugar de derivarlo al
/// leer: así el historial se lee de arriba abajo sin rehacer la aritmética
/// de todos los movimientos anteriores (RF-INV-05).
fn escribir_movimiento(
    tx: &Transaction<'_>,
    producto_id: i64,
    movimiento: &Movimiento,
    resultante: Existencias,
) -> ResultadoInfra<()> {
    tx.execute(
        "INSERT INTO movimiento
           (producto_id, tipo, origen, destino, cantidad, costo_unitario,
            bodega_resultante, vitrina_resultante, motivo, ocurrido_en)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))",
        params![
            producto_id,
            movimiento.tipo().como_texto(),
            movimiento.origen().map(Ubicacion::como_texto),
            movimiento.destino().map(Ubicacion::como_texto),
            movimiento.cantidad().milesimas(),
            movimiento.costo_unitario().millonesimas(),
            resultante.bodega().milesimas(),
            resultante.vitrina().milesimas(),
            movimiento.motivo(),
        ],
    )?;

    Ok(())
}

/// Reconstruye un asiento del kárdex a partir de su fila.
fn leer_movimiento(fila: &rusqlite::Row<'_>) -> ResultadoInfra<MovimientoRegistrado> {
    let id: i64 = fila.get(0)?;
    let tipo_texto: String = fila.get(1)?;
    let origen_texto: Option<String> = fila.get(2)?;
    let destino_texto: Option<String> = fila.get(3)?;
    let cantidad: i64 = fila.get(4)?;
    let costo_unitario: i64 = fila.get(5)?;
    let bodega: i64 = fila.get(6)?;
    let vitrina: i64 = fila.get(7)?;
    let motivo: Option<String> = fila.get(8)?;
    let ocurrido_en: String = fila.get(9)?;

    let tipo: TipoMovimiento = tipo_texto.parse().map_err(|_| {
        ErrorInfra::DatoCorrupto(format!("tipo «{tipo_texto}» en el movimiento {id}"))
    })?;

    let resultante = Existencias::nuevas(
        Cantidad::desde_milesimas(bodega),
        Cantidad::desde_milesimas(vitrina),
    )
    .map_err(|error| ErrorInfra::DatoCorrupto(format!("saldo del movimiento {id}: {error}")))?;

    Ok(MovimientoRegistrado {
        id,
        movimiento: Movimiento::reconstituir(
            tipo,
            ubicacion_opcional(origen_texto.as_deref(), id)?,
            ubicacion_opcional(destino_texto.as_deref(), id)?,
            Cantidad::desde_milesimas(cantidad),
            Dinero::desde_millonesimas(costo_unitario),
            motivo,
        ),
        ocurrido_en,
        resultante,
    })
}

fn ubicacion_opcional(texto: Option<&str>, id: i64) -> ResultadoInfra<Option<Ubicacion>> {
    texto
        .map(|valor| {
            valor.parse::<Ubicacion>().map_err(|_| {
                ErrorInfra::DatoCorrupto(format!("ubicación «{valor}» en el movimiento {id}"))
            })
        })
        .transpose()
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
    let mut consulta =
        conexion.prepare("SELECT ubicacion, cantidad FROM existencia WHERE producto_id = ?1")?;

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
