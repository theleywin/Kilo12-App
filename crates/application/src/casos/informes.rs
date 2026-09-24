//! Caso de uso: los informes del negocio (RF-EST).
//!
//! Una sola consulta devuelve todo lo que la pantalla necesita. No es
//! pereza: los informes se miran juntos —el total del periodo al lado de su
//! evolución y de qué producto lo sostiene— y partirlos en siete llamadas
//! solo conseguiría que unas llegaran antes que otras y la pantalla
//! enseñara un rato cifras que no se corresponden entre sí.
//!
//! **Los tres niveles no se confunden nunca** (RF-EST-13):
//!
//! - **Venta**: el dinero que entró.
//! - **Ganancia bruta**: venta menos el costo congelado de lo vendido.
//! - **Ganancia neta**: ganancia bruta menos la comisión del operador.
//!
//! Cada cifra viene formateada y, cuando alimenta una gráfica, acompañada
//! de un **peso** de 0 a 1000 relativo al mayor de su serie. Así la pantalla
//! dibuja barras sin dividir importes: la geometría es suya, la aritmética
//! de dinero no.

use std::collections::BTreeMap;

use domain::{Cantidad, Dinero, Porcentaje};

use crate::casos::caja::CLAVE_COMISION;
use crate::error::Resultado;
use crate::puertos::{LineaDelPeriodo, ProductoConInventario, RepositorioProducto};

/// Cuántos productos se enseñan en cada escalafón.
pub const CUANTOS_EN_RANKING: usize = 8;

/// Cuántos días sin reponer se consideran alarmantes.
pub const DIAS_CRITICOS: i64 = 7;

// ==================================================== lo que se devuelve

/// Las cifras que resumen el periodo (RF-EST-01, RF-EST-02, RF-EST-13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumenPeriodo {
    pub cuantas_ventas: i64,
    pub venta: String,
    pub costo: String,
    pub ganancia_bruta: String,
    /// Porcentaje vigente, sin el símbolo.
    pub comision_porcentaje: String,
    pub comision: String,
    pub ganancia_neta: String,
    /// Venta media por operación.
    pub ticket_promedio: String,
    /// Margen bruto del periodo, sin el símbolo.
    pub margen: String,
    /// Vendiendo por debajo del costo: la ganancia salió en negativo.
    pub en_perdida: bool,
}

/// Un día de la serie (RF-EST-08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PuntoDiario {
    /// `YYYY-MM-DD`.
    pub fecha: String,
    /// `dd/mm`, para el eje.
    pub etiqueta: String,
    pub venta: String,
    pub ganancia: String,
    pub cuantas: i64,
    /// Altura relativa, de 0 a 1000, respecto al mejor día.
    pub peso: i64,
    /// Altura de la ganancia, en la misma escala que la venta.
    pub peso_ganancia: i64,
}

/// Una hora del día (RF-EST-11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PuntoHorario {
    pub hora: i64,
    /// `14 h`.
    pub etiqueta: String,
    pub venta: String,
    pub cuantas: i64,
    pub peso: i64,
}

/// Lo cobrado por una forma de pago (RF-EST-11b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorcionMetodo {
    pub metodo: String,
    pub nombre: String,
    /// Lo entregado en su moneda: los dólares se enseñan también así.
    pub entregado: String,
    pub moneda: String,
    /// Su equivalente en pesos.
    pub importe: String,
    /// Parte del total cobrado, sin el símbolo.
    pub porcentaje: String,
    pub peso: i64,
}

/// Un producto en un escalafón (RF-EST-03, RF-EST-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductoEnInforme {
    pub producto: i64,
    pub nombre: String,
    /// Unidades base vendidas: así el suelto y el paquete son comparables.
    pub cantidad: String,
    pub unidad: String,
    pub importe: String,
    pub ganancia: String,
    pub margen: String,
    pub peso: i64,
}

/// Un producto que no se vendió en el periodo (RF-EST-05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductoParado {
    pub producto: i64,
    pub nombre: String,
    pub existencia: String,
    pub unidad: String,
    /// Dinero detenido en ese producto.
    pub capital: String,
}

/// Un producto a punto de acabarse (RF-EST-06, RF-EST-07).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductoPorAgotarse {
    pub producto: i64,
    pub nombre: String,
    pub existencia: String,
    pub unidad: String,
    /// Lo que se vende al día, de media, en el periodo.
    pub venta_diaria: String,
    /// Días que aguanta al ritmo actual. Vacío si no se vendió nada.
    pub dias_cobertura: Option<i64>,
    /// Se acaba antes de una semana.
    pub critico: bool,
}

/// Cómo fue el último día frente al anterior.
///
/// Se comparan los **dos últimos días con ventas**, no las dos últimas
/// fechas del calendario: si la tienda cerró ayer, compararse contra un
/// cero no dice nada útil. Por eso viajan las dos etiquetas, para que se
/// vea siempre qué se está comparando con qué.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparativaDiaria {
    pub etiqueta_ultimo: String,
    pub etiqueta_anterior: String,
    pub venta_ultimo: String,
    pub venta_anterior: String,
    /// Diferencia con signo: negativa si se vendió menos.
    pub diferencia: String,
    /// Variación porcentual, sin el símbolo. Vacía si el día anterior fue
    /// cero: dividir entre nada no da un porcentaje, da una mentira.
    pub porcentaje: Option<String>,
    pub subio: bool,
}

/// El informe completo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Informe {
    pub desde: String,
    pub hasta: String,
    pub dias: i64,
    pub resumen: ResumenPeriodo,
    pub por_dia: Vec<PuntoDiario>,
    pub por_hora: Vec<PuntoHorario>,
    pub por_metodo: Vec<PorcionMetodo>,
    pub mas_vendidos: Vec<ProductoEnInforme>,
    pub mas_rentables: Vec<ProductoEnInforme>,
    pub sin_movimiento: Vec<ProductoParado>,
    pub por_agotarse: Vec<ProductoPorAgotarse>,
    /// El último día frente al anterior. Vacío con menos de dos días.
    pub comparativa: Option<ComparativaDiaria>,
    /// Capital total detenido en lo que no se movió.
    pub capital_parado: String,
    /// No hubo ni una venta: la pantalla lo dice en vez de enseñar ceros.
    pub sin_datos: bool,
}

// ========================================================= el caso de uso

/// Arma el informe de un periodo.
#[derive(Debug)]
pub struct ConsultarInforme<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> ConsultarInforme<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    /// `desde` y `hasta` son fechas locales `YYYY-MM-DD`, ambas incluidas.
    pub fn ejecutar(&self, desde: &str, hasta: &str, dias: i64) -> Resultado<Informe> {
        let totales = self.repositorio.resumen_periodo(desde, hasta)?;
        let lineas = self.repositorio.lineas_del_periodo(desde, hasta)?;
        let catalogo = self.repositorio.listar(false)?;

        let ganancia_bruta = totales.venta.restar(totales.costo)?;
        let porcentaje = self.porcentaje_comision()?;
        let comision = totales.venta.aplicar_porcentaje(porcentaje)?;

        let resumen = ResumenPeriodo {
            cuantas_ventas: totales.cuantas,
            venta: totales.venta.formatear(2),
            costo: totales.costo.formatear(2),
            ganancia_bruta: ganancia_bruta.formatear(2),
            comision_porcentaje: Porcentaje::desde_diezmilesimas(porcentaje).formatear(2),
            comision: comision.formatear(2),
            ganancia_neta: ganancia_bruta.restar(comision)?.formatear(2),
            ticket_promedio: promedio(totales.venta, totales.cuantas)?.formatear(2),
            margen: razon(ganancia_bruta, totales.venta)?.formatear(2),
            en_perdida: ganancia_bruta.es_negativo(),
        };

        let agregados = agregar_por_producto(&lineas)?;
        let por_dia = self.serie_diaria(desde, hasta)?;
        let comparativa = comparar_ultimos_dias(desde, hasta, self.repositorio)?;

        Ok(Informe {
            desde: desde.to_owned(),
            hasta: hasta.to_owned(),
            dias,
            sin_datos: totales.cuantas == 0,
            por_dia,
            comparativa,
            por_hora: self.serie_horaria(desde, hasta)?,
            por_metodo: self.reparto_por_metodo(desde, hasta)?,
            mas_vendidos: ranking(&agregados, &catalogo, Criterio::Cantidad)?,
            mas_rentables: ranking(&agregados, &catalogo, Criterio::Ganancia)?,
            sin_movimiento: sin_movimiento(&agregados, &catalogo)?,
            capital_parado: capital_parado(&agregados, &catalogo)?.formatear(2),
            por_agotarse: por_agotarse(&agregados, &catalogo, dias)?,
            resumen,
        })
    }

    fn serie_diaria(&self, desde: &str, hasta: &str) -> Resultado<Vec<PuntoDiario>> {
        let dias = self.repositorio.ventas_por_dia(desde, hasta)?;
        let techo = dias
            .iter()
            .map(|dia| dia.venta)
            .max()
            .unwrap_or(Dinero::CERO);

        dias.into_iter()
            .map(|dia| {
                let ganancia = dia.venta.restar(dia.costo)?;

                Ok(PuntoDiario {
                    etiqueta: etiqueta_de_fecha(&dia.fecha),
                    venta: dia.venta.formatear(2),
                    ganancia: ganancia.formatear(2),
                    cuantas: dia.cuantas,
                    peso: peso_de(dia.venta, techo),
                    // La ganancia se mide contra el MISMO techo que la
                    // venta: dibujarla en su propia escala haría que un día
                    // malo pareciera bueno.
                    peso_ganancia: peso_de(ganancia, techo),
                    fecha: dia.fecha,
                })
            })
            .collect()
    }

    fn serie_horaria(&self, desde: &str, hasta: &str) -> Resultado<Vec<PuntoHorario>> {
        let horas = self.repositorio.ventas_por_hora(desde, hasta)?;
        let por_hora: BTreeMap<i64, _> = horas.into_iter().map(|h| (h.hora, h)).collect();
        let techo = por_hora
            .values()
            .map(|h| h.venta)
            .max()
            .unwrap_or(Dinero::CERO);

        // Las 24 horas, incluidas las vacías: un hueco a las cinco de la
        // tarde dice tanto como un pico a las nueve de la mañana.
        Ok((0..24)
            .map(|hora| {
                let venta = por_hora.get(&hora).map_or(Dinero::CERO, |h| h.venta);
                let cuantas = por_hora.get(&hora).map_or(0, |h| h.cuantas);

                PuntoHorario {
                    hora,
                    etiqueta: format!("{hora:02} h"),
                    venta: venta.formatear(2),
                    cuantas,
                    peso: peso_de(venta, techo),
                }
            })
            .collect())
    }

    fn reparto_por_metodo(&self, desde: &str, hasta: &str) -> Resultado<Vec<PorcionMetodo>> {
        let metodos = self.repositorio.ventas_por_metodo(desde, hasta)?;
        let total = metodos
            .iter()
            .try_fold(Dinero::CERO, |suma, m| suma.sumar(m.equivalente))?;

        metodos
            .into_iter()
            .map(|m| {
                Ok(PorcionMetodo {
                    metodo: m.metodo.como_texto().to_owned(),
                    nombre: m.metodo.nombre().to_owned(),
                    entregado: m.entregado.formatear(2),
                    moneda: m.metodo.moneda().simbolo().to_owned(),
                    importe: m.equivalente.formatear(2),
                    porcentaje: razon(m.equivalente, total)?.formatear(1),
                    peso: peso_de(m.equivalente, total),
                })
            })
            .collect()
    }

    /// Porcentaje de comisión vigente, o cero si no se ha configurado.
    fn porcentaje_comision(&self) -> Resultado<i64> {
        let Some(guardado) = self.repositorio.configuracion(CLAVE_COMISION)? else {
            return Ok(0);
        };

        let porcentaje: Porcentaje = guardado.trim().parse()?;
        Ok(porcentaje.diezmilesimas())
    }
}

// ============================================================= agregados

/// Lo que un producto movió en el periodo.
#[derive(Debug, Clone, Copy, Default)]
struct Movido {
    /// Unidades base, para que suelto y paquete sean comparables.
    unidades: Cantidad,
    importe: Dinero,
    costo: Dinero,
}

/// Junta las líneas por producto.
///
/// Aquí es donde se multiplica precio por cantidad, y por eso esto no se
/// hace en SQL: las escalas las conocen `Dinero` y `Cantidad`, no SQLite.
fn agregar_por_producto(lineas: &[LineaDelPeriodo]) -> Resultado<BTreeMap<i64, Movido>> {
    let mut acumulado: BTreeMap<i64, Movido> = BTreeMap::new();

    for linea in lineas {
        let unidades = linea.cantidad.multiplicar_por_factor(linea.factor)?;
        let importe = linea.precio.multiplicar_por(linea.cantidad)?;
        let costo = linea.costo_unitario.multiplicar_por(unidades)?;

        let movido = acumulado.entry(linea.producto.0).or_default();
        movido.unidades = movido.unidades.sumar(unidades)?;
        movido.importe = movido.importe.sumar(importe)?;
        movido.costo = movido.costo.sumar(costo)?;
    }

    Ok(acumulado)
}

/// Por qué se ordena un escalafón.
#[derive(Debug, Clone, Copy)]
enum Criterio {
    /// Lo que más salió por la puerta.
    Cantidad,
    /// Lo que más dinero dejó.
    Ganancia,
}

fn ranking(
    agregados: &BTreeMap<i64, Movido>,
    catalogo: &[ProductoConInventario],
    criterio: Criterio,
) -> Resultado<Vec<ProductoEnInforme>> {
    let mut filas = Vec::with_capacity(agregados.len());

    for (id, movido) in agregados {
        let Some(fila) = catalogo
            .iter()
            .find(|f| f.producto.id().map(|i| i.0) == Some(*id))
        else {
            continue;
        };

        let ganancia = movido.importe.restar(movido.costo)?;
        filas.push((
            *movido,
            ganancia,
            ProductoEnInforme {
                producto: *id,
                nombre: fila.producto.nombre().to_owned(),
                cantidad: crate::casos::formatear_cantidad(movido.unidades, &fila.producto),
                unidad: fila.producto.unidad_base().simbolo().to_owned(),
                importe: movido.importe.formatear(2),
                ganancia: ganancia.formatear(2),
                margen: razon(ganancia, movido.importe)?.formatear(2),
                peso: 0,
            },
        ));
    }

    match criterio {
        Criterio::Cantidad => filas.sort_by(|a, b| b.0.unidades.cmp(&a.0.unidades)),
        Criterio::Ganancia => filas.sort_by(|a, b| b.1.cmp(&a.1)),
    }
    filas.truncate(CUANTOS_EN_RANKING);

    // El peso se calcula sobre el criterio que ordena, no sobre otro: una
    // barra que no se corresponda con el orden de la lista confunde más
    // que no poner barra.
    let techo = filas
        .first()
        .map_or(Dinero::CERO, |primera| match criterio {
            Criterio::Cantidad => Dinero::desde_millonesimas(primera.0.unidades.milesimas()),
            Criterio::Ganancia => primera.1,
        });

    Ok(filas
        .into_iter()
        .map(|(movido, ganancia, mut vista)| {
            let valor = match criterio {
                Criterio::Cantidad => Dinero::desde_millonesimas(movido.unidades.milesimas()),
                Criterio::Ganancia => ganancia,
            };
            vista.peso = peso_de(valor, techo);
            vista
        })
        .collect())
}

fn sin_movimiento(
    agregados: &BTreeMap<i64, Movido>,
    catalogo: &[ProductoConInventario],
) -> Resultado<Vec<ProductoParado>> {
    let mut parados: Vec<(Dinero, ProductoParado)> = catalogo
        .iter()
        .filter(|fila| {
            let id = fila.producto.id().map_or(0, |i| i.0);
            // Sin ventas y con mercancía: un producto agotado y sin vender
            // no tiene capital detenido, así que no es el problema.
            !agregados.contains_key(&id) && fila.inventario.valor_total().es_positivo()
        })
        .map(|fila| {
            let capital = fila.inventario.valor_total();

            (
                capital,
                ProductoParado {
                    producto: fila.producto.id().map_or(0, |i| i.0),
                    nombre: fila.producto.nombre().to_owned(),
                    existencia: crate::casos::formatear_cantidad(
                        fila.inventario.cantidad_total().unwrap_or(Cantidad::CERO),
                        &fila.producto,
                    ),
                    unidad: fila.producto.unidad_base().simbolo().to_owned(),
                    capital: capital.formatear(2),
                },
            )
        })
        .collect();

    // El que más dinero tiene parado va primero: es el que más urge mover.
    // Se ordena por el importe, no por su texto ya formateado: comparar
    // cadenas ordenaría «9.00» por encima de «100.00».
    parados.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.nombre.cmp(&b.1.nombre)));
    Ok(parados.into_iter().map(|(_, parado)| parado).collect())
}

fn capital_parado(
    agregados: &BTreeMap<i64, Movido>,
    catalogo: &[ProductoConInventario],
) -> Resultado<Dinero> {
    catalogo
        .iter()
        .filter(|fila| !agregados.contains_key(&fila.producto.id().map_or(0, |i| i.0)))
        .try_fold(Dinero::CERO, |suma, fila| {
            suma.sumar(fila.inventario.valor_total())
        })
        .map_err(Into::into)
}

fn por_agotarse(
    agregados: &BTreeMap<i64, Movido>,
    catalogo: &[ProductoConInventario],
    dias: i64,
) -> Resultado<Vec<ProductoPorAgotarse>> {
    let mut filas = Vec::new();

    for fila in catalogo {
        let id = fila.producto.id().map_or(0, |i| i.0);
        let existencia = fila.inventario.cantidad_total().unwrap_or(Cantidad::CERO);
        let vendidas = agregados.get(&id).map_or(Cantidad::CERO, |m| m.unidades);

        // Sin ventas no hay ritmo con que estimar nada, y sin existencia ya
        // no hay qué agotar.
        if !vendidas.es_positiva() || !existencia.es_positiva() {
            continue;
        }

        // Días que aguanta: existencia ÷ (vendidas ÷ días). Se calcula
        // sobre milésimas y se redondea hacia abajo, que es el lado
        // prudente: es mejor creer que queda menos y reponer de más.
        let cobertura = existencia
            .milesimas()
            .saturating_mul(dias.max(1))
            .checked_div(vendidas.milesimas())
            .unwrap_or(0);

        let diaria = Cantidad::desde_milesimas(vendidas.milesimas() / dias.max(1));

        filas.push(ProductoPorAgotarse {
            producto: id,
            nombre: fila.producto.nombre().to_owned(),
            existencia: crate::casos::formatear_cantidad(existencia, &fila.producto),
            unidad: fila.producto.unidad_base().simbolo().to_owned(),
            venta_diaria: crate::casos::formatear_cantidad(diaria, &fila.producto),
            dias_cobertura: Some(cobertura),
            critico: cobertura <= DIAS_CRITICOS,
        });
    }

    // Lo que antes se acaba, primero.
    filas.sort_by_key(|f| f.dias_cobertura.unwrap_or(i64::MAX));
    filas.truncate(CUANTOS_EN_RANKING);
    Ok(filas)
}

/// Compara los dos últimos días con ventas del periodo.
fn comparar_ultimos_dias<R: RepositorioProducto>(
    desde: &str,
    hasta: &str,
    repositorio: &R,
) -> Resultado<Option<ComparativaDiaria>> {
    let dias = repositorio.ventas_por_dia(desde, hasta)?;
    let [.., anterior, ultimo] = dias.as_slice() else {
        return Ok(None);
    };

    let diferencia = ultimo.venta.restar(anterior.venta)?;

    Ok(Some(ComparativaDiaria {
        etiqueta_ultimo: etiqueta_de_fecha(&ultimo.fecha),
        etiqueta_anterior: etiqueta_de_fecha(&anterior.fecha),
        venta_ultimo: ultimo.venta.formatear(2),
        venta_anterior: anterior.venta.formatear(2),
        diferencia: diferencia.formatear(2),
        // Sin base con que comparar no hay porcentaje que valga.
        porcentaje: if anterior.venta.es_positivo() {
            Some(razon(diferencia, anterior.venta)?.formatear(1))
        } else {
            None
        },
        subio: !diferencia.es_negativo(),
    }))
}

// ============================================================= ayudantes

/// Reparte un importe entre un número de operaciones.
fn promedio(total: Dinero, cuantas: i64) -> Resultado<Dinero> {
    if cuantas <= 0 {
        return Ok(Dinero::CERO);
    }

    total
        .dividir_entre(Cantidad::desde_unidades(cuantas)?)
        .map_err(Into::into)
}

/// Qué parte representa un importe de otro, en porcentaje.
fn razon(parte: Dinero, total: Dinero) -> Resultado<Porcentaje> {
    if total.es_cero() {
        return Ok(Porcentaje::desde_diezmilesimas(0));
    }

    Porcentaje::de_razon(parte, total).map_err(Into::into)
}

/// Altura relativa de una barra, de 0 a 1000.
///
/// Es geometría, no dinero: la pantalla la usa para dibujar y nunca para
/// enseñar una cifra. Por eso puede ser un entero corriente.
fn peso_de(valor: Dinero, techo: Dinero) -> i64 {
    if !techo.es_positivo() || !valor.es_positivo() {
        return 0;
    }

    let peso = i128::from(valor.millonesimas()) * 1000 / i128::from(techo.millonesimas());
    peso.clamp(0, 1000) as i64
}

/// `2026-09-23` se convierte en `23/09`.
fn etiqueta_de_fecha(fecha: &str) -> String {
    let partes: Vec<&str> = fecha.split('-').collect();
    match partes.as_slice() {
        [_, mes, dia] => format!("{dia}/{mes}"),
        _ => fecha.to_owned(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_etiqueta_del_eje_es_dia_y_mes() {
        assert_eq!(etiqueta_de_fecha("2026-09-23"), "23/09");
        assert_eq!(etiqueta_de_fecha("raro"), "raro");
    }

    #[test]
    fn el_peso_es_relativo_al_techo() {
        let techo = "1000.00".parse::<Dinero>().expect("importe");
        let mitad = "500.00".parse::<Dinero>().expect("importe");

        assert_eq!(peso_de(techo, techo), 1000);
        assert_eq!(peso_de(mitad, techo), 500);
        assert_eq!(peso_de(Dinero::CERO, techo), 0);
    }

    #[test]
    fn sin_techo_no_hay_barra_que_dibujar() {
        // Un periodo sin ventas no puede dividir por cero.
        let algo = "10.00".parse::<Dinero>().expect("importe");
        assert_eq!(peso_de(algo, Dinero::CERO), 0);
    }

    #[test]
    fn el_ticket_promedio_de_cero_ventas_es_cero() {
        let total = "500.00".parse::<Dinero>().expect("importe");
        assert_eq!(promedio(total, 0).unwrap().formatear(2), "0.00");
        assert_eq!(promedio(total, 4).unwrap().formatear(2), "125.00");
    }
}
