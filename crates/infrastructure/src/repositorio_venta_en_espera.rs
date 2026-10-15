//! Adaptador SQLite de las ventas en espera (RF-VTA-14).
//!
//! Comparte la base con el repositorio de productos, pero es otro puerto:
//! quien aparta una venta no necesita saber cerrar una caja. El borrado de
//! la espera al cobrarla **no** vive aquí sino en `registrar_venta`, porque
//! tiene que ir dentro de la misma transacción que la venta.

use std::sync::Arc;

use application::error::Resultado;
use application::puertos::{EsperaRegistrada, EsperaResumida, RepositorioVentaEnEspera};
use domain::{Cantidad, IdPresentacion, IdProducto, IdVentaEnEspera, LineaEnEspera, VentaEnEspera};

use rusqlite::{params, Connection};

use crate::conexion::BaseDatos;
use crate::error::{ErrorInfra, ResultadoInfra};

/// Repositorio de ventas en espera sobre SQLite.
#[derive(Debug, Clone)]
pub struct RepositorioVentaEnEsperaSqlite {
    base: Arc<BaseDatos>,
}

impl RepositorioVentaEnEsperaSqlite {
    pub const fn nuevo(base: Arc<BaseDatos>) -> Self {
        Self { base }
    }
}

impl RepositorioVentaEnEspera for RepositorioVentaEnEsperaSqlite {
    fn guardar(&self, espera: &VentaEnEspera) -> Resultado<IdVentaEnEspera> {
        let id = self.base.en_transaccion(|tx| {
            tx.execute(
                "INSERT INTO venta_en_espera (nota, creada_en)
                 VALUES (?1, datetime('now', 'localtime'))",
                params![espera.nota()],
            )?;

            let id_espera = tx.last_insert_rowid();

            for (orden, linea) in espera.lineas().iter().enumerate() {
                let orden = i64::try_from(orden).map_err(|_| {
                    ErrorInfra::DatoCorrupto(format!("demasiados renglones en la espera: {orden}"))
                })?;

                tx.execute(
                    "INSERT INTO venta_en_espera_linea
                       (espera_id, producto_id, presentacion_id, cantidad, orden)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        id_espera,
                        linea.producto().0,
                        linea.presentacion().0,
                        linea.cantidad().milesimas(),
                        orden,
                    ],
                )?;
            }

            Ok(id_espera)
        })?;

        Ok(IdVentaEnEspera(id))
    }

    fn listar(&self) -> Resultado<Vec<EsperaResumida>> {
        let esperas = self.base.con(|conexion| {
            let mut consulta = conexion.prepare(
                "SELECT e.id, e.nota, e.creada_en, COUNT(l.id)
                 FROM venta_en_espera e
                 LEFT JOIN venta_en_espera_linea l ON l.espera_id = e.id
                 GROUP BY e.id
                 ORDER BY e.id",
            )?;

            let mut filas = consulta.query([])?;
            let mut esperas = Vec::new();

            while let Some(fila) = filas.next()? {
                esperas.push(EsperaResumida {
                    id: IdVentaEnEspera(fila.get(0)?),
                    nota: fila.get(1)?,
                    creada_en: fila.get(2)?,
                    cuantas_lineas: fila.get(3)?,
                });
            }

            Ok(esperas)
        })?;

        Ok(esperas)
    }

    fn obtener(&self, id: IdVentaEnEspera) -> Resultado<Option<EsperaRegistrada>> {
        let registrada = self.base.con(|conexion| {
            let mut cabecera =
                conexion.prepare("SELECT nota, creada_en FROM venta_en_espera WHERE id = ?1")?;
            let mut filas = cabecera.query(params![id.0])?;

            let Some(fila) = filas.next()? else {
                return Ok(None);
            };
            let nota: Option<String> = fila.get(0)?;
            let creada_en: String = fila.get(1)?;

            let lineas = leer_lineas(conexion, id.0)?;
            let espera = VentaEnEspera::nueva(nota.as_deref(), lineas).map_err(|error| {
                ErrorInfra::DatoCorrupto(format!("venta en espera {}: {error}", id.0))
            })?;

            Ok(Some(EsperaRegistrada {
                id,
                espera,
                creada_en,
            }))
        })?;

        Ok(registrada)
    }

    fn eliminar(&self, id: IdVentaEnEspera) -> Resultado<bool> {
        // Los renglones caen solos por la clave foránea en cascada.
        let borradas = self.base.con(|conexion| {
            conexion
                .execute("DELETE FROM venta_en_espera WHERE id = ?1", params![id.0])
                .map_err(ErrorInfra::from)
        })?;

        Ok(borradas > 0)
    }
}

/// Lee los renglones de una espera, en el orden en que se armó.
fn leer_lineas(conexion: &Connection, espera_id: i64) -> ResultadoInfra<Vec<LineaEnEspera>> {
    let mut consulta = conexion.prepare(
        "SELECT producto_id, presentacion_id, cantidad
         FROM venta_en_espera_linea
         WHERE espera_id = ?1
         ORDER BY orden",
    )?;

    let mut filas = consulta.query(params![espera_id])?;
    let mut lineas = Vec::new();

    while let Some(fila) = filas.next()? {
        let linea = LineaEnEspera::nueva(
            IdProducto(fila.get(0)?),
            IdPresentacion(fila.get(1)?),
            Cantidad::desde_milesimas(fila.get(2)?),
        )
        .map_err(|error| {
            ErrorInfra::DatoCorrupto(format!(
                "renglón de la venta en espera {espera_id}: {error}"
            ))
        })?;
        lineas.push(linea);
    }

    Ok(lineas)
}
