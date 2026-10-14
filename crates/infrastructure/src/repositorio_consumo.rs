//! Adaptador SQLite del consumo de los dueños (RF-CON).
//!
//! Comparte la base con el repositorio de productos, pero es otro puerto:
//! quien liquida una deuda no necesita saber cerrar una caja (DT-5).
//!
//! ## Por qué `registrar_consumo` escribe tanto
//!
//! Escribe la cabecera, los renglones, el valor del inventario, la
//! existencia y los asientos del kárdex. Parece demasiado para un método, y
//! no se puede repartir: `BaseDatos::con` y `BaseDatos::en_transaccion`
//! toman el **mismo candado**, y `std::sync::Mutex` no es reentrante. Un
//! método de este adaptador que llamara al repositorio de productos dentro
//! de su transacción **colgaría el programa**, y no como error de
//! compilación sino de madrugada, con la caja abierta.
//!
//! Es el mismo motivo por el que `registrar_venta` escribe venta, renglones,
//! pagos, saldos, asientos y la espera que consume, todo en un sitio.
//!
//! ## El saldo es una suma, no un cálculo
//!
//! `saldos` suma dos columnas enteras ya guardadas —`consumo.total_costo` y
//! `pago_consumo.importe`— con el filtro de anulación a los **dos** lados.
//! Nada se recalcula de los costos de hoy, y por eso una deuda acordada no
//! se mueve cuando sube el arroz.
//!
//! Lo que **no** se hace aquí es multiplicar: `lineas_consumidas` devuelve
//! los renglones en crudo y la agregación por producto sube a Rust, igual
//! que en `lineas_del_periodo`. Multiplicar cantidad por factor por costo
//! con enteros escalados pierde milésimas en la división intermedia.

use std::sync::Arc;

use application::error::Resultado;
use application::puertos::{
    AnulacionConsumo, ConsumoConfirmado, ConsumoRegistrado, CuentaConsumo, DescuentoConsumo,
    LineaConsumoRegistrada, PagoConsumoRegistrado, RepositorioConsumo, SaldoCuenta,
};
use domain::{Cantidad, Dinero, IdProducto, Ubicacion, UnidadBase};

use rusqlite::{params, Connection, Row, Transaction};

use crate::conexion::BaseDatos;
use crate::error::{ErrorInfra, ResultadoInfra};
use crate::repositorio_producto::escribir_movimiento;

/// Repositorio del consumo sobre SQLite.
#[derive(Debug, Clone)]
pub struct RepositorioConsumoSqlite {
    base: Arc<BaseDatos>,
}

impl RepositorioConsumoSqlite {
    pub const fn nuevo(base: Arc<BaseDatos>) -> Self {
        Self { base }
    }
}

impl RepositorioConsumo for RepositorioConsumoSqlite {
    fn cuentas(&self, incluir_inactivas: bool) -> Resultado<Vec<CuentaConsumo>> {
        let cuentas = self.base.con(|conexion| {
            let filtro = if incluir_inactivas {
                ""
            } else {
                "WHERE activa = 1"
            };

            let mut consulta = conexion.prepare(&format!(
                "SELECT id, nombre, activa FROM cuenta_consumo {filtro} ORDER BY id"
            ))?;

            let mut filas = consulta.query([])?;
            let mut cuentas = Vec::new();

            while let Some(fila) = filas.next()? {
                cuentas.push(CuentaConsumo {
                    id: fila.get(0)?,
                    nombre: fila.get(1)?,
                    activa: fila.get::<_, i64>(2)? == 1,
                });
            }

            Ok(cuentas)
        })?;

        Ok(cuentas)
    }

    fn registrar_consumo(&self, confirmado: &ConsumoConfirmado<'_>) -> Resultado<i64> {
        let id = self.base.en_transaccion(|tx| {
            // La cuenta se comprueba dentro de la transacción: que exista y
            // esté activa lo decidió el caso de uso, pero la clave foránea
            // es la que impide de verdad cargar una deuda a una cuenta que
            // no está.
            tx.execute(
                "INSERT INTO consumo
                   (cuenta_id, tomado_por, total_costo, ocurrido_en)
                 VALUES (?1, ?2, ?3, datetime('now', 'localtime'))",
                params![
                    confirmado.cuenta,
                    confirmado.tomado_por,
                    confirmado.total_costo.millonesimas(),
                ],
            )?;

            let id_consumo = tx.last_insert_rowid();

            for linea in confirmado.lineas {
                tx.execute(
                    "INSERT INTO consumo_linea
                       (consumo_id, producto_id, presentacion_id, nombre_producto,
                        nombre_presentacion, origen, cantidad, factor, costo_unitario)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        id_consumo,
                        linea.producto.0,
                        linea.presentacion.0,
                        linea.nombre_producto,
                        linea.nombre_presentacion,
                        linea.origen.como_texto(),
                        linea.cantidad.milesimas(),
                        linea.factor.milesimas(),
                        linea.costo_unitario.millonesimas(),
                    ],
                )?;
            }

            // La mercancía sale y cada salida deja su asiento.
            aplicar(tx, confirmado.descuentos)?;

            Ok(id_consumo)
        })?;

        Ok(id)
    }

    fn consumo(
        &self,
        id: i64,
    ) -> Resultado<Option<(ConsumoRegistrado, Vec<LineaConsumoRegistrada>)>> {
        let consumo = self.base.con(|conexion| {
            let mut consulta = conexion.prepare(
                "SELECT c.id, c.cuenta_id, k.nombre, c.tomado_por, c.total_costo,
                        c.ocurrido_en, c.anulado_en, c.motivo_anulacion
                   FROM consumo c
                   JOIN cuenta_consumo k ON k.id = c.cuenta_id
                  WHERE c.id = ?1",
            )?;

            let mut filas = consulta.query(params![id])?;
            let Some(fila) = filas.next()? else {
                return Ok(None);
            };

            let cabecera = leer_consumo(fila)?;
            drop(filas);

            Ok(Some((cabecera, renglones_de(conexion, id)?)))
        })?;

        Ok(consumo)
    }

    fn anular_consumo(&self, anulacion: &AnulacionConsumo<'_>) -> Resultado<()> {
        self.base.en_transaccion(|tx| {
            // La marca y la devolución van juntas. Y la marca solo prende
            // si no estaba ya puesta: anular dos veces devolvería la
            // mercancía dos veces, y eso inventaría existencia.
            let marcadas = tx.execute(
                "UPDATE consumo
                    SET anulado_en = datetime('now', 'localtime'), motivo_anulacion = ?2
                  WHERE id = ?1 AND anulado_en IS NULL",
                params![anulacion.consumo, anulacion.motivo],
            )?;

            if marcadas == 0 {
                return Err(ErrorInfra::DatoCorrupto(format!(
                    "el consumo {} no existe o ya estaba anulado",
                    anulacion.consumo
                )));
            }

            aplicar(tx, anulacion.reversas)?;

            Ok(())
        })?;

        Ok(())
    }

    fn registrar_pago(&self, cuenta: i64, importe: Dinero, nota: Option<&str>) -> Resultado<i64> {
        let id = self.base.con(|conexion| {
            conexion.execute(
                "INSERT INTO pago_consumo (cuenta_id, importe, nota, ocurrido_en)
                 VALUES (?1, ?2, ?3, datetime('now', 'localtime'))",
                params![cuenta, importe.millonesimas(), nota],
            )?;

            Ok(conexion.last_insert_rowid())
        })?;

        Ok(id)
    }

    fn pago(&self, id: i64) -> Resultado<Option<PagoConsumoRegistrado>> {
        let pago = self.base.con(|conexion| {
            let mut consulta = conexion.prepare(
                "SELECT id, cuenta_id, importe, nota, ocurrido_en,
                        anulado_en, motivo_anulacion
                   FROM pago_consumo WHERE id = ?1",
            )?;

            let mut filas = consulta.query(params![id])?;
            match filas.next()? {
                Some(fila) => Ok(Some(leer_pago(fila)?)),
                None => Ok(None),
            }
        })?;

        Ok(pago)
    }

    fn anular_pago(&self, id: i64, motivo: &str) -> Resultado<()> {
        self.base.con(|conexion| {
            let marcados = conexion.execute(
                "UPDATE pago_consumo
                    SET anulado_en = datetime('now', 'localtime'), motivo_anulacion = ?2
                  WHERE id = ?1 AND anulado_en IS NULL",
                params![id, motivo],
            )?;

            if marcados == 0 {
                return Err(ErrorInfra::DatoCorrupto(format!(
                    "la liquidación {id} no existe o ya estaba anulada"
                )));
            }

            Ok(())
        })?;

        Ok(())
    }

    fn saldos(&self) -> Resultado<Vec<SaldoCuenta>> {
        let saldos = self.base.con(|conexion| {
            // Dos subconsultas y no dos `JOIN`: unir las dos tablas a la vez
            // multiplicaría las filas —tres consumos por dos pagos son seis
            // combinaciones— y cada importe se sumaría varias veces. Es el
            // error clásico de este tipo de informe.
            let mut consulta = conexion.prepare(
                "SELECT k.id, k.nombre, k.activa,
                        COALESCE((SELECT SUM(c.total_costo) FROM consumo c
                                   WHERE c.cuenta_id = k.id
                                     AND c.anulado_en IS NULL), 0),
                        COALESCE((SELECT SUM(p.importe) FROM pago_consumo p
                                   WHERE p.cuenta_id = k.id
                                     AND p.anulado_en IS NULL), 0)
                   FROM cuenta_consumo k
                  ORDER BY k.id",
            )?;

            let mut filas = consulta.query([])?;
            let mut saldos = Vec::new();

            while let Some(fila) = filas.next()? {
                let consumido = Dinero::desde_millonesimas(fila.get(3)?);
                let pagado = Dinero::desde_millonesimas(fila.get(4)?);

                saldos.push(SaldoCuenta {
                    cuenta: fila.get(0)?,
                    nombre: fila.get(1)?,
                    activa: fila.get::<_, i64>(2)? == 1,
                    consumido,
                    pagado,
                    // La resta se hace en Rust y **sin recortar**: un saldo
                    // negativo es una cuenta con dinero a favor, no un
                    // error que haya que tapar con un cero.
                    saldo: consumido.restar(pagado).map_err(ErrorInfra::Dominio)?,
                });
            }

            Ok(saldos)
        })?;

        Ok(saldos)
    }

    fn consumos_de_cuenta(&self, cuenta: i64, limite: usize) -> Resultado<Vec<ConsumoRegistrado>> {
        let consumos = self.base.con(|conexion| {
            let mut consulta = conexion.prepare(
                "SELECT c.id, c.cuenta_id, k.nombre, c.tomado_por, c.total_costo,
                        c.ocurrido_en, c.anulado_en, c.motivo_anulacion
                   FROM consumo c
                   JOIN cuenta_consumo k ON k.id = c.cuenta_id
                  WHERE c.cuenta_id = ?1
                  ORDER BY c.id DESC
                  LIMIT ?2",
            )?;

            let mut filas = consulta.query(params![cuenta, limite as i64])?;
            let mut consumos = Vec::new();

            while let Some(fila) = filas.next()? {
                consumos.push(leer_consumo(fila)?);
            }

            Ok(consumos)
        })?;

        Ok(consumos)
    }

    fn pagos_de_cuenta(&self, cuenta: i64, limite: usize) -> Resultado<Vec<PagoConsumoRegistrado>> {
        let pagos = self.base.con(|conexion| {
            let mut consulta = conexion.prepare(
                "SELECT id, cuenta_id, importe, nota, ocurrido_en,
                        anulado_en, motivo_anulacion
                   FROM pago_consumo
                  WHERE cuenta_id = ?1
                  ORDER BY id DESC
                  LIMIT ?2",
            )?;

            let mut filas = consulta.query(params![cuenta, limite as i64])?;
            let mut pagos = Vec::new();

            while let Some(fila) = filas.next()? {
                pagos.push(leer_pago(fila)?);
            }

            Ok(pagos)
        })?;

        Ok(pagos)
    }

    fn lineas_consumidas(
        &self,
        cuenta: Option<i64>,
        desde: Option<&str>,
        hasta: Option<&str>,
    ) -> Resultado<Vec<LineaConsumoRegistrada>> {
        let lineas = self.base.con(|conexion| {
            // Sin agregar: la multiplicación sube a Rust. Las fechas se
            // comparan contra la fecha local del consumo, como en los
            // informes.
            let mut consulta = conexion.prepare(
                "SELECT l.producto_id, l.nombre_producto, l.nombre_presentacion,
                        p.unidad_base, l.origen, l.cantidad, l.factor, l.costo_unitario
                   FROM consumo_linea l
                   JOIN consumo c  ON c.id = l.consumo_id
                   JOIN producto p ON p.id = l.producto_id
                  WHERE c.anulado_en IS NULL
                    AND (?1 IS NULL OR c.cuenta_id = ?1)
                    AND (?2 IS NULL OR date(c.ocurrido_en) >= date(?2))
                    AND (?3 IS NULL OR date(c.ocurrido_en) <= date(?3))
                  ORDER BY l.id",
            )?;

            let mut filas = consulta.query(params![cuenta, desde, hasta])?;
            let mut lineas = Vec::new();

            while let Some(fila) = filas.next()? {
                lineas.push(leer_renglon(fila)?);
            }

            Ok(lineas)
        })?;

        Ok(lineas)
    }
}

// ================================================= piezas compartidas

/// Lleva a la base los saldos que el caso de uso calculó y escribe sus
/// asientos.
///
/// Sirve para las dos direcciones: los descuentos de un consumo y las
/// reversas de su anulación. Lo que distingue a una de otra es el tipo del
/// movimiento, que el dominio ya decidió.
fn aplicar(tx: &Transaction<'_>, movidos: &[DescuentoConsumo]) -> ResultadoInfra<()> {
    for movido in movidos {
        tx.execute(
            "UPDATE producto SET valor_total = ?2 WHERE id = ?1",
            params![
                movido.producto.0,
                movido.inventario.valor_total().millonesimas()
            ],
        )?;

        for ubicacion in Ubicacion::TODAS {
            tx.execute(
                "UPDATE existencia SET cantidad = ?3
                  WHERE producto_id = ?1 AND ubicacion = ?2",
                params![
                    movido.producto.0,
                    ubicacion.como_texto(),
                    movido.inventario.existencias().en(ubicacion).milesimas(),
                ],
            )?;
        }

        escribir_movimiento(
            tx,
            movido.producto.0,
            &movido.movimiento,
            movido.inventario.existencias(),
        )?;
    }

    Ok(())
}

/// Reconstruye la cabecera de un consumo a partir de su fila.
fn leer_consumo(fila: &Row<'_>) -> ResultadoInfra<ConsumoRegistrado> {
    Ok(ConsumoRegistrado {
        id: fila.get(0)?,
        cuenta: fila.get(1)?,
        nombre_cuenta: fila.get(2)?,
        tomado_por: fila.get(3)?,
        total_costo: Dinero::desde_millonesimas(fila.get(4)?),
        ocurrido_en: fila.get(5)?,
        anulado_en: fila.get(6)?,
        motivo_anulacion: fila.get(7)?,
    })
}

fn leer_pago(fila: &Row<'_>) -> ResultadoInfra<PagoConsumoRegistrado> {
    Ok(PagoConsumoRegistrado {
        id: fila.get(0)?,
        cuenta: fila.get(1)?,
        importe: Dinero::desde_millonesimas(fila.get(2)?),
        nota: fila.get(3)?,
        ocurrido_en: fila.get(4)?,
        anulado_en: fila.get(5)?,
        motivo_anulacion: fila.get(6)?,
    })
}

/// Reconstruye un renglón. El orden de las columnas es el de las dos
/// consultas que lo usan.
fn leer_renglon(fila: &Row<'_>) -> ResultadoInfra<LineaConsumoRegistrada> {
    let producto: i64 = fila.get(0)?;
    let unidad_texto: String = fila.get(3)?;
    let origen_texto: String = fila.get(4)?;

    let unidad_base: UnidadBase = unidad_texto.parse().map_err(|_| {
        ErrorInfra::DatoCorrupto(format!(
            "unidad «{unidad_texto}» en un renglón del producto {producto}"
        ))
    })?;

    let origen: Ubicacion = origen_texto.parse().map_err(|_| {
        ErrorInfra::DatoCorrupto(format!(
            "ubicación «{origen_texto}» en un renglón del producto {producto}"
        ))
    })?;

    Ok(LineaConsumoRegistrada {
        producto: IdProducto(producto),
        nombre_producto: fila.get(1)?,
        nombre_presentacion: fila.get(2)?,
        unidad_base,
        origen,
        cantidad: Cantidad::desde_milesimas(fila.get(5)?),
        factor: Cantidad::desde_milesimas(fila.get(6)?),
        costo_unitario: Dinero::desde_millonesimas(fila.get(7)?),
    })
}

/// Los renglones de un consumo, en el orden en que se escribieron.
fn renglones_de(
    conexion: &Connection,
    consumo: i64,
) -> ResultadoInfra<Vec<LineaConsumoRegistrada>> {
    let mut consulta = conexion.prepare(
        "SELECT l.producto_id, l.nombre_producto, l.nombre_presentacion,
                p.unidad_base, l.origen, l.cantidad, l.factor, l.costo_unitario
           FROM consumo_linea l
           JOIN producto p ON p.id = l.producto_id
          WHERE l.consumo_id = ?1
          ORDER BY l.id",
    )?;

    let mut filas = consulta.query(params![consumo])?;
    let mut renglones = Vec::new();

    while let Some(fila) = filas.next()? {
        renglones.push(leer_renglon(fila)?);
    }

    Ok(renglones)
}
