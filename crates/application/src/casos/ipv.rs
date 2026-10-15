//! Caso de uso: el IPV de un turno de caja (RF-IPV-01).
//!
//! El IPV —Informe de Productos Vendidos— es la tabla que el puntico llena a
//! mano al cerrar el turno: una fila por mercancía, y en cada fila lo que
//! había, lo que entró, lo que se perdió, lo que se llevaron las dueñas, lo
//! que se vendió y lo que quedó. Se imprime y se cuadra contra el estante.
//!
//! **Es la hoja de la vitrina, no del negocio entero.** Esa es la decisión
//! que explica todo lo demás. Se cuadra con la mercancía delante, así que
//! las cifras tienen que ser las del mostrador: lo que duerme en el almacén
//! no se vende ni se merma ahí, y sumarlo haría que la CANT. FINAL no se
//! pudiera comprobar contra el estante.
//!
//! De ahí sale lo que para Kilo12 es un nombre distinto: en el IPV,
//! **ENTRADA es lo que entró a la vitrina**. Casi siempre es un traspaso que
//! baja del almacén (RF-INV-04); también cuenta una compra que se coloca
//! directamente en el mostrador. Y un traspaso que sube de la vitrina al
//! almacén resta de esa misma columna, porque es exactamente lo contrario.
//!
//! La hoja entera descansa en una sola identidad:
//!
//! ```text
//! CANT. FINAL = CANT. INICIAL + ENTRADA − MERMA − CONSUMO − SALIDA
//! ```
//!
//! Aquí esa identidad no se usa para *calcular* la existencia final: la
//! existencia inicial y la final se leen del kárdex, que las guardó cuando
//! ocurrieron (RF-INV-05). La identidad se usa para **comprobar**. Si una
//! fila no cuadra, el informe lo dice (`cuadra`) en lugar de maquillar la
//! cifra, porque una hoja que siempre cuadra no sirve para detectar nada.
//!
//! Dos decisiones más que explican la forma del resultado:
//!
//! * **Al precio de venta.** El costo no aparece: las dueñas no lo usan en
//!   esta hoja. Lo que se enseña es el importe que entró por cada mercancía.
//! * **Unidades base en la columna SALIDA.** Si un producto se vendió suelto
//!   y en paquete, sumar «3 sueltos + 1 paquete = 4» no significa nada. Se
//!   suman unidades base, y el desglose por presentación va en subfilas
//!   (`formas`) para que se pueda ver a qué precio salió cada forma.
//!
//! El PRECIO de la fila del producto es, por tanto, **derivado**: importe
//! entre unidades base vendidas. Es el precio medio al que salió la
//! mercancía ese turno, y es la única cifra que hace que `IMPORTE =
//! SALIDA × PRECIO` se lea coherente cuando hay varias presentaciones. Si no
//! se vendió nada, se enseña el precio de catálogo de la presentación
//! predeterminada: la hoja sirve también para consultar a cuánto está algo.

use std::collections::{BTreeMap, BTreeSet};

use domain::{Cantidad, Dinero, IdSesion, Producto, TipoMovimiento, Ubicacion};

use crate::casos::agregados::{agregar_por, Movido};
use crate::casos::caja::{como_referencia, SesionDeReferencia};
use crate::casos::consultar_ventas::formatear_cantidad_vendida;
use crate::casos::mantenimiento::CLAVE_MANTENIMIENTO;
use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{
    ConsultaIpv, ConsultaVentasDeSesion, LineaDelPeriodo, MovidoEnSesion, ProductoConInventario,
    RepositorioProducto, SaldoDeSesion,
};

// ==================================================== lo que se devuelve

/// Las cifras de una fila sin formatear, para quien escribe el `.xlsx`.
///
/// La pantalla usa los textos de arriba; la hoja de cálculo necesita
/// números, porque si van como texto no se pueden sumar y la hoja deja de
/// ser una hoja de cálculo. Van juntas y no en otro recorrido aparte para
/// que texto y número no puedan describir cifras distintas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CifrasIpv {
    /// Cuántos decimales admite la mercancía: 3 si se pesa, 0 si se cuenta.
    ///
    /// Lo necesita la hoja de cálculo para dar formato a la celda. En la
    /// pantalla no hace falta porque el texto ya viene escrito, pero en
    /// Excel el número y su formato van por separado, y adivinar el formato
    /// mirando el texto haría que media libra se imprimiera como una.
    pub decimales: u32,
    pub inicial: Cantidad,
    pub entrada: Cantidad,
    pub merma: Cantidad,
    pub consumo: Cantidad,
    pub salida: Cantidad,
    pub precio: Dinero,
    pub importe: Dinero,
    pub cierre: Cantidad,
}

/// Una presentación concreta en la que se vendió el producto.
///
/// Solo aparece cuando hubo más de una: con una sola, la subfila repetiría
/// la fila de arriba.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormaVendida {
    pub presentacion: i64,
    /// Nombre congelado en la venta más reciente que la llevó.
    pub nombre: String,
    /// Lo que salió contado en esta presentación: paquetes, si es el paquete.
    pub salida: String,
    pub precio: String,
    pub importe: String,
    pub salida_cruda: Cantidad,
    pub precio_crudo: Dinero,
    pub importe_crudo: Dinero,
}

/// Una mercancía en la hoja del turno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilaIpv {
    pub producto: i64,
    pub mercancia: String,
    pub inicial: String,
    pub entrada: String,
    pub merma: String,
    /// Lo que se llevaron las dueñas, neto de anulaciones.
    pub consumo: String,
    /// Lo vendido en unidades base, neto de devoluciones.
    pub salida: String,
    /// Precio medio por unidad base, o el de catálogo si no se vendió.
    pub precio: String,
    pub importe: String,
    pub cierre: String,
    /// Si la fila se explica a sí misma: inicial + entrada − merma −
    /// consumo − salida = cierre.
    pub cuadra: bool,
    pub formas: Vec<FormaVendida>,
    pub cifras: CifrasIpv,
}

/// La hoja completa de un turno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ipv {
    /// A qué turno pertenece. Vacío si nunca se abrió una caja.
    pub sesion: Option<SesionDeReferencia>,
    /// En orden alfabético: la hoja se recorre estante en mano.
    pub filas: Vec<FilaIpv>,
    /// Suma de los importes de las filas.
    pub importe_total: String,
    /// Lo que la caja registró como vendido en el turno.
    ///
    /// En la hoja de papel esta casilla se dejaba en blanco para apuntarla a
    /// mano. Kilo12 ya la sabe, así que la imprime: lo que la hoja dice que
    /// se vendió y lo que la caja cobró son dos cifras que conviene mirar
    /// una al lado de la otra.
    pub vendido_real: String,
    /// `importe_total` − `vendido_real`. Con signo, y por eso no se puede
    /// deducir de los otros dos textos.
    pub diferencia: String,
    /// Si las dos cifras coinciden al céntimo.
    pub cuadra: bool,
    pub importe_total_crudo: Dinero,
    pub vendido_real_crudo: Dinero,
    pub diferencia_cruda: Dinero,
}

impl Ipv {
    /// La hoja de una tienda que nunca ha abierto la caja.
    fn vacio() -> Self {
        Self {
            sesion: None,
            filas: Vec::new(),
            importe_total: Dinero::CERO.formatear(2),
            vendido_real: Dinero::CERO.formatear(2),
            diferencia: Dinero::CERO.formatear(2),
            cuadra: true,
            importe_total_crudo: Dinero::CERO,
            vendido_real_crudo: Dinero::CERO,
            diferencia_cruda: Dinero::CERO,
        }
    }
}

// ========================================================= el caso de uso

/// Qué hoja se pide y con qué permiso.
///
/// La clave es la misma del sistema (`CLAVE_MANTENIMIENTO`) y se comprueba
/// **aquí y no en la pantalla**, por el mismo motivo que en `mantenimiento`:
/// el comando queda expuesto a la interfaz y una validación que solo viva en
/// JavaScript se salta abriendo las herramientas de desarrollo.
///
/// Tampoco pretende ser seguridad —la clave está escrita en el código—, sino
/// una puerta que hay que empujar a conciencia: el IPV del turno en curso se
/// mira cien veces al día y el de hace tres semanas, nunca por accidente.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PeticionIpv {
    /// Turno pedido. `None` es el que está abierto ahora mismo.
    pub sesion: Option<i64>,
    /// Obligatoria para cualquier turno que no sea el abierto (RF-IPV-14).
    pub clave: Option<String>,
}

impl PeticionIpv {
    /// La hoja del turno en curso, que no necesita permiso.
    pub const fn del_turno_abierto() -> Self {
        Self {
            sesion: None,
            clave: None,
        }
    }
}

/// Arma el IPV de un turno.
///
/// Tres puertos porque hacen falta tres cosas que ninguna otra pantalla
/// pide juntas: el catálogo y la caja (`RepositorioProducto`), los saldos y
/// los asientos del turno (`ConsultaIpv`) y los renglones vendidos con su
/// presentación (`ConsultaVentasDeSesion`). Separados según lo que cada uno
/// sabe, no según quién los implementa (DT-5).
#[derive(Debug)]
pub struct ConsultarIpv<'a, P: RepositorioProducto, K: ConsultaIpv, V: ConsultaVentasDeSesion> {
    productos: &'a P,
    kardex: &'a K,
    ventas: &'a V,
}

impl<'a, P: RepositorioProducto, K: ConsultaIpv, V: ConsultaVentasDeSesion>
    ConsultarIpv<'a, P, K, V>
{
    pub const fn nuevo(productos: &'a P, kardex: &'a K, ventas: &'a V) -> Self {
        Self {
            productos,
            kardex,
            ventas,
        }
    }

    /// Arma la hoja del turno pedido, si se tiene permiso para verla.
    pub fn ejecutar(&self, peticion: &PeticionIpv) -> Resultado<Ipv> {
        let Some(referencia) = self.turno_permitido(peticion)? else {
            return Ok(Ipv::vacio());
        };
        let id = IdSesion(referencia.id);

        let saldos = self.kardex.saldos_de_sesion(id)?;
        let movidos = self.kardex.movimientos_de_sesion(id)?;
        // El IMPORTE sale de los renglones vendidos y la SALIDA del kárdex
        // filtrado a la vitrina. Las dos cifras hablan de lo mismo porque
        // `vender.rs` descuenta SIEMPRE de la vitrina; si algún día se
        // pudiera vender de la bodega, el importe incluiría algo que la
        // columna no, y habría que filtrar también los renglones.
        let lineas = self.ventas.lineas_de_sesion(id)?;
        let vendido_real = self.productos.acumulado_de_sesion(id)?.total_vendido;
        // Con los inactivos: un producto retirado del catálogo a media
        // semana pudo venderse en este turno, y su fila tiene que salir.
        let catalogo = self.productos.listar(true)?;

        armar(
            referencia,
            &catalogo,
            &saldos,
            &movidos,
            &lineas,
            vendido_real,
        )
    }

    /// Resuelve qué turno se va a leer, aplicando RF-IPV-14.
    ///
    /// Devuelve `None` cuando no hay nada que enseñar y eso no es un error:
    /// la tienda no tiene la caja abierta, o se pidió un turno que no
    /// existe. Lo que sí es un error es pedir el pasado sin la clave.
    fn turno_permitido(&self, peticion: &PeticionIpv) -> Resultado<Option<SesionDeReferencia>> {
        let abierta = self
            .productos
            .sesion_abierta()?
            .map(como_referencia)
            .transpose()?;

        let Some(pedida) = peticion.sesion else {
            // Sin turno pedido se enseña el que está abierto. Y si no hay
            // ninguno, nada: la hoja del turno es del turno en curso, y la
            // del último cerrado ya es historia (RF-IPV-14).
            return Ok(abierta);
        };

        if abierta.as_ref().is_some_and(|sesion| sesion.id == pedida) {
            return Ok(abierta);
        }

        // La comprobación va ANTES de leer nada. Si se hiciera después,
        // alguien podría averiguar qué turnos existen probando números y
        // mirando si el error cambia.
        if peticion.clave.as_deref().map(str::trim) != Some(CLAVE_MANTENIMIENTO) {
            return Err(ErrorAplicacion::ClaveIncorrecta);
        }

        self.productos
            .sesion(IdSesion(pedida))?
            .map(como_referencia)
            .transpose()
    }
}

// =========================================================== la aritmética

/// Lo que un producto movió en la vitrina durante el turno, por columna.
#[derive(Debug, Clone, Copy, Default)]
struct Columnas {
    /// Lo que entró a la vitrina: traspasos que bajaron del almacén y
    /// compras colocadas directamente en el mostrador, menos los traspasos
    /// que subieron de vuelta.
    entrada: Cantidad,
    merma: Cantidad,
    /// Consumos menos consumos anulados. Puede quedar en contra si lo que se
    /// anula hoy se consumió en un turno anterior.
    consumo: Cantidad,
    /// Ventas menos devoluciones, con la misma salvedad.
    salida: Cantidad,
    /// Lo que se movió sin tener columna en esta hoja: los ajustes de
    /// conteo.
    ///
    /// No se intenta colocarlos en ninguna parte. El asiento guarda la
    /// cantidad siempre en positivo, así que su dirección no se puede
    /// recuperar de la suma, y meterlos a ciegas en una columna sería
    /// inventar. Lo que hacen es impedir que la fila cuadre, que es justo lo
    /// que tiene que pasar: un ajuste es una diferencia que alguien tiene
    /// que explicar a mano.
    sin_columna: Cantidad,
}

impl Columnas {
    /// Suma un asiento, si tocó la vitrina.
    ///
    /// Se mira a qué estante entró y de cuál salió, no solo el tipo: el
    /// mismo traspaso llena la vitrina o la vacía según la dirección, y una
    /// merma del almacén no pinta nada en esta hoja. Lo que no toca la
    /// vitrina se descarta sin más.
    fn sumar(&mut self, movido: &MovidoEnSesion) -> Resultado<()> {
        let cantidad = movido.cantidad;
        let entro = movido.destino == Some(Ubicacion::Vitrina);
        let salio = movido.origen == Some(Ubicacion::Vitrina);

        match (movido.tipo, entro, salio) {
            // Mercancía que llega al mostrador, venga del almacén o de la
            // calle. Para el IPV las dos son ENTRADA.
            (TipoMovimiento::Entrada | TipoMovimiento::Traspaso, true, _) => {
                self.entrada = self.entrada.sumar(cantidad)?;
            }
            // Lo que se devuelve al almacén es una entrada en contra.
            (TipoMovimiento::Traspaso, false, true) => {
                self.entrada = self.entrada.restar_con_signo(cantidad)?;
            }
            (TipoMovimiento::Merma, _, true) => self.merma = self.merma.sumar(cantidad)?,
            (TipoMovimiento::Consumo, _, true) => self.consumo = self.consumo.sumar(cantidad)?,
            (TipoMovimiento::ConsumoAnulado, true, _) => {
                self.consumo = self.consumo.restar_con_signo(cantidad)?;
            }
            (TipoMovimiento::Venta, _, true) => self.salida = self.salida.sumar(cantidad)?,
            (TipoMovimiento::Devolucion, true, _) => {
                self.salida = self.salida.restar_con_signo(cantidad)?;
            }
            (TipoMovimiento::Ajuste, true, _) | (TipoMovimiento::Ajuste, _, true) => {
                self.sin_columna = self.sin_columna.sumar(cantidad)?;
            }
            // No tocó la vitrina: una compra que fue al almacén, una merma
            // de lo guardado. Real, pero de otra hoja.
            _ => {}
        }
        Ok(())
    }

    /// Si la fila se explica a sí misma.
    ///
    /// Se comprueba la identidad de la hoja contra la existencia **en
    /// vitrina** que el kárdex guardó. Un movimiento sin columna la invalida
    /// de entrada: la fila podría cuadrar de casualidad y seguiría sin decir
    /// por qué.
    fn cuadra(&self, inicial: Cantidad, cierre: Cantidad) -> Resultado<bool> {
        if !self.sin_columna.es_cero() {
            return Ok(false);
        }

        let esperado = inicial
            .sumar(self.entrada)?
            .restar_con_signo(self.merma)?
            .restar_con_signo(self.consumo)?
            .restar_con_signo(self.salida)?;

        Ok(esperado == cierre)
    }
}

/// El nombre de una presentación congelado en la venta más reciente.
#[derive(Debug, Clone, Copy)]
struct Reciente<'l> {
    venta: i64,
    nombre: &'l str,
}

fn armar(
    sesion: SesionDeReferencia,
    catalogo: &[ProductoConInventario],
    saldos: &[SaldoDeSesion],
    movidos: &[MovidoEnSesion],
    lineas: &[LineaDelPeriodo],
    vendido_real: Dinero,
) -> Resultado<Ipv> {
    let productos: BTreeMap<i64, &Producto> = catalogo
        .iter()
        .filter_map(|c| c.producto.id().map(|id| (id.0, &c.producto)))
        .collect();

    let mut columnas: BTreeMap<i64, Columnas> = BTreeMap::new();
    for movido in movidos {
        columnas
            .entry(movido.producto.0)
            .or_default()
            .sumar(movido)?;
    }

    let por_producto = agregar_por(lineas, |l| l.producto.0)?;
    let por_presentacion = agregar_por(lineas, |l| (l.producto.0, l.presentacion.0))?;
    let nombres = nombres_recientes(lineas);

    let saldos: BTreeMap<i64, &SaldoDeSesion> = saldos.iter().map(|s| (s.producto.0, s)).collect();

    // Quién tiene fila: lo que está activo en el catálogo, más todo lo que
    // tuvo existencia o movió algo en el turno aunque ya esté retirado.
    let mut con_fila: BTreeSet<i64> = catalogo
        .iter()
        .filter(|c| c.producto.esta_activo())
        .filter_map(|c| c.producto.id().map(|id| id.0))
        .collect();
    con_fila.extend(columnas.keys().copied());
    con_fila.extend(
        saldos
            .iter()
            .filter(|(_, s)| !s.inicial.es_cero() || !s.cierre.es_cero())
            .map(|(&id, _)| id),
    );

    let mut filas = Vec::with_capacity(con_fila.len());
    let mut importe_total = Dinero::CERO;

    for id in con_fila {
        let Some(producto) = productos.get(&id).copied() else {
            // Sin producto no hay unidad base ni nombre con que escribir la
            // fila. No debería ocurrir: el kárdex apunta al producto con
            // clave ajena, así que si movió, existe.
            continue;
        };

        let (inicial, cierre) = saldos
            .get(&id)
            .map_or((Cantidad::CERO, Cantidad::CERO), |s| (s.inicial, s.cierre));
        let columnas = columnas.get(&id).copied().unwrap_or_default();
        let vendido = por_producto.get(&id).copied().unwrap_or_default();

        let importe = vendido.importe;
        importe_total = importe_total.sumar(importe)?;

        // Una sola vez, y de ella salen el texto y el número: así no pueden
        // acabar diciendo precios distintos.
        let cifras = CifrasIpv {
            decimales: if producto.es_granel() { 3 } else { 0 },
            inicial,
            entrada: columnas.entrada,
            merma: columnas.merma,
            consumo: columnas.consumo,
            salida: columnas.salida,
            precio: precio_de_fila(importe, columnas.salida, producto)?,
            importe,
            cierre,
        };

        filas.push(FilaIpv {
            producto: id,
            mercancia: producto.nombre().to_owned(),
            inicial: escribir(cifras.inicial),
            entrada: escribir(cifras.entrada),
            merma: escribir(cifras.merma),
            consumo: escribir(cifras.consumo),
            salida: escribir(cifras.salida),
            precio: cifras.precio.formatear(2),
            importe: cifras.importe.formatear(2),
            cierre: escribir(cifras.cierre),
            cuadra: columnas.cuadra(inicial, cierre)?,
            formas: formas_de(id, &por_presentacion, &nombres)?,
            cifras,
        });
    }

    filas.sort_by(|a, b| {
        a.mercancia
            .to_lowercase()
            .cmp(&b.mercancia.to_lowercase())
            .then_with(|| a.producto.cmp(&b.producto))
    });

    let diferencia = importe_total.restar(vendido_real)?;

    Ok(Ipv {
        sesion: Some(sesion),
        filas,
        importe_total: importe_total.formatear(2),
        vendido_real: vendido_real.formatear(2),
        diferencia: diferencia.formatear(2),
        cuadra: diferencia.es_cero(),
        importe_total_crudo: importe_total,
        vendido_real_crudo: vendido_real,
        diferencia_cruda: diferencia,
    })
}

/// Escribe una cantidad para una celda de la hoja.
///
/// Entera si no tiene fracción, con sus tres decimales si la tiene, y eso
/// **aunque la mercancía se pese**. Es distinto del criterio del resto de la
/// aplicación, que mira si el producto es a granel y entonces escribe
/// siempre tres decimales, y la diferencia tiene motivo: aquí hay nueve
/// columnas por 150 filas, y casi todas esas celdas son ceros. Una pared de
/// «0.000» no dice nada y, peor, hace que el «2.500» de verdad deje de
/// saltar a la vista justo cuando hay que mirarlo.
fn escribir(cantidad: Cantidad) -> String {
    if cantidad.es_entera() {
        cantidad.formatear(0)
    } else {
        cantidad.formatear(3)
    }
}

/// Precio medio por unidad base, o el de catálogo si no se vendió nada.
///
/// Se reparte el importe entre las unidades que salieron. Con la salida en
/// cero —o en contra, si solo hubo devoluciones de otro turno— no hay nada
/// que repartir y dividir sería un error: se recurre a la presentación
/// predeterminada, que es el precio que la dueña diría de memoria.
fn precio_de_fila(importe: Dinero, salida: Cantidad, producto: &Producto) -> Resultado<Dinero> {
    if salida.es_positiva() {
        return Ok(importe.dividir_entre(salida)?);
    }

    producto
        .presentacion_predeterminada()
        .map_or(Ok(Dinero::CERO), |p| Ok(p.precio_por_unidad_base()?))
}

/// Nombre de cada presentación, el de la venta más reciente que la llevó.
///
/// Con `>=` gana el último renglón leído dentro de la misma venta, así que
/// el nombre elegido no depende del orden en que lleguen las líneas.
fn nombres_recientes(lineas: &[LineaDelPeriodo]) -> BTreeMap<(i64, i64), Reciente<'_>> {
    let mut nombres: BTreeMap<(i64, i64), Reciente<'_>> = BTreeMap::new();
    for linea in lineas {
        let visto = Reciente {
            venta: linea.venta,
            nombre: &linea.nombre_presentacion,
        };
        let reciente = nombres
            .entry((linea.producto.0, linea.presentacion.0))
            .or_insert(visto);
        if visto.venta >= reciente.venta {
            *reciente = visto;
        }
    }
    nombres
}

/// Las subfilas de un producto, solo si se vendió de más de una forma.
fn formas_de(
    producto: i64,
    por_presentacion: &BTreeMap<(i64, i64), Movido>,
    nombres: &BTreeMap<(i64, i64), Reciente<'_>>,
) -> Resultado<Vec<FormaVendida>> {
    let vendidas: Vec<_> = por_presentacion
        .iter()
        .filter(|(&(p, _), _)| p == producto)
        .collect();

    if vendidas.len() < 2 {
        return Ok(Vec::new());
    }

    let mut formas = vendidas
        .into_iter()
        .map(|(&(_, presentacion), movido)| {
            // Aquí el precio no se deriva: una presentación se vendió a su
            // precio, y es el importe entre las presentaciones que salieron.
            let precio = if movido.cantidad.es_positiva() {
                movido.importe.dividir_entre(movido.cantidad)?
            } else {
                Dinero::CERO
            };
            Ok(FormaVendida {
                presentacion,
                nombre: nombres
                    .get(&(producto, presentacion))
                    .map_or_else(String::new, |r| r.nombre.to_owned()),
                salida: formatear_cantidad_vendida(movido.cantidad),
                precio: precio.formatear(2),
                importe: movido.importe.formatear(2),
                salida_cruda: movido.cantidad,
                precio_crudo: precio,
                importe_crudo: movido.importe,
            })
        })
        .collect::<Resultado<Vec<_>>>()?;

    // De la forma que más dejó entrar a la que menos: la subfila que
    // interesa primero es la que mueve el dinero.
    formas.sort_by(|a, b| {
        b.importe_crudo
            .cmp(&a.importe_crudo)
            .then_with(|| a.nombre.cmp(&b.nombre))
            .then_with(|| a.presentacion.cmp(&b.presentacion))
    });

    Ok(formas)
}

#[cfg(test)]
mod pruebas {
    use domain::{IdPresentacion, IdProducto, UnidadBase};

    use super::*;
    use crate::dobles::{IpvEnMemoria, ProductosEnMemoria, VentasDeSesionEnMemoria};

    const TURNO: i64 = 7;

    /// Los pares `(origen, destino)` de cada clase de asiento, con nombre.
    ///
    /// Escribirlos a mano en cada llamada escondería lo único que importa
    /// aquí: qué estante toca el asiento.
    const A_VITRINA: (Option<Ubicacion>, Option<Ubicacion>) = (None, Some(Ubicacion::Vitrina));
    const A_BODEGA: (Option<Ubicacion>, Option<Ubicacion>) = (None, Some(Ubicacion::Bodega));
    const DE_VITRINA: (Option<Ubicacion>, Option<Ubicacion>) = (Some(Ubicacion::Vitrina), None);
    const DE_BODEGA: (Option<Ubicacion>, Option<Ubicacion>) = (Some(Ubicacion::Bodega), None);
    /// Lo que baja del almacén al mostrador: para el IPV, una entrada.
    const BAJA: (Option<Ubicacion>, Option<Ubicacion>) =
        (Some(Ubicacion::Bodega), Some(Ubicacion::Vitrina));
    /// Lo que sube del mostrador al almacén: una entrada en contra.
    const SUBE: (Option<Ubicacion>, Option<Ubicacion>) =
        (Some(Ubicacion::Vitrina), Some(Ubicacion::Bodega));

    /// Un renglón vendido del turno.
    fn linea(
        venta: i64,
        (producto, presentacion): (i64, i64),
        nombre_presentacion: &str,
        cantidad: &str,
        factor: &str,
        precio: &str,
    ) -> LineaDelPeriodo {
        LineaDelPeriodo {
            venta,
            producto: IdProducto(producto),
            presentacion: IdPresentacion(presentacion),
            nombre_producto: "da igual".to_owned(),
            nombre_presentacion: nombre_presentacion.to_owned(),
            cantidad: cantidad.parse().expect("cantidad"),
            factor: factor.parse().expect("factor"),
            precio: precio.parse().expect("precio"),
            costo_unitario: "10.00".parse().expect("costo"),
        }
    }

    /// Salchicha suelta a 100 y en paquete de 6 a 266.64; arroz por libra a
    /// 1000. Turno 7 abierto.
    fn catalogo() -> ProductosEnMemoria {
        ProductosEnMemoria::default()
            .con_producto(
                1,
                "Salchicha",
                UnidadBase::Unidad,
                &[
                    (10, "Unidad", "1", "100.00"),
                    (11, "Paquete", "6", "266.64"),
                ],
                "20",
            )
            .con_producto(
                2,
                "Arroz el rey",
                UnidadBase::Libra,
                &[(20, "Libra", "1", "1000.00")],
                "5",
            )
            .con_sesion_abierta(TURNO, "2026-10-13 08:00:00")
    }

    /// La hoja del turno abierto, que es la que se mira sin permiso.
    fn consultar(
        productos: &ProductosEnMemoria,
        kardex: &IpvEnMemoria,
        ventas: &VentasDeSesionEnMemoria,
    ) -> Ipv {
        ConsultarIpv::nuevo(productos, kardex, ventas)
            .ejecutar(&PeticionIpv::del_turno_abierto())
            .expect("consultar")
    }

    /// La hoja de un turno concreto, con la clave que se le dé.
    fn pedir(
        productos: &ProductosEnMemoria,
        kardex: &IpvEnMemoria,
        ventas: &VentasDeSesionEnMemoria,
        sesion: i64,
        clave: Option<&str>,
    ) -> Resultado<Ipv> {
        ConsultarIpv::nuevo(productos, kardex, ventas).ejecutar(&PeticionIpv {
            sesion: Some(sesion),
            clave: clave.map(str::to_owned),
        })
    }

    fn fila(hoja: &Ipv, producto: i64) -> &FilaIpv {
        hoja.filas
            .iter()
            .find(|f| f.producto == producto)
            .expect("fila del producto")
    }

    /// El turno que el usuario validó: salchicha vendida de dos formas y
    /// arroz de una. Es la hoja de referencia de casi todas las pruebas.
    fn turno_de_muestra() -> (ProductosEnMemoria, IpvEnMemoria, VentasDeSesionEnMemoria) {
        let productos = catalogo().con_acumulado(TURNO, 2, "1466.64", "0.00");
        let kardex = IpvEnMemoria::default()
            // 20 − 8 vendidas = 12.
            .con_saldo(TURNO, 1, "20", "12")
            .con_movido(TURNO, 1, TipoMovimiento::Venta, DE_VITRINA, "8")
            // 5 − 1 = 4.
            .con_saldo(TURNO, 2, "5", "4")
            .con_movido(TURNO, 2, TipoMovimiento::Venta, DE_VITRINA, "1");
        let ventas = VentasDeSesionEnMemoria::default()
            // 2 sueltas a 100 = 200.
            .con_linea(TURNO, linea(1, (1, 10), "Unidad", "2", "1", "100.00"))
            // 1 paquete de 6 a 266.64.
            .con_linea(TURNO, linea(1, (1, 11), "Paquete", "1", "6", "266.64"))
            .con_linea(TURNO, linea(2, (2, 20), "Libra", "1", "1", "1000.00"));

        (productos, kardex, ventas)
    }

    #[test]
    fn la_hoja_reproduce_las_columnas_del_ipv_de_papel() {
        let (productos, kardex, ventas) = turno_de_muestra();

        let hoja = consultar(&productos, &kardex, &ventas);

        let salchicha = fila(&hoja, 1);
        assert_eq!(salchicha.mercancia, "Salchicha");
        assert_eq!(salchicha.inicial, "20");
        assert_eq!(salchicha.entrada, "0");
        assert_eq!(salchicha.merma, "0");
        assert_eq!(salchicha.consumo, "0");
        // Unidades base: 2 sueltas + 1 × 6 del paquete.
        assert_eq!(salchicha.salida, "8");
        assert_eq!(salchicha.importe, "466.64");
        assert_eq!(salchicha.cierre, "12");
        assert!(salchicha.cuadra);
    }

    #[test]
    fn el_precio_de_la_fila_es_el_medio_por_unidad_base() {
        let (productos, kardex, ventas) = turno_de_muestra();

        let hoja = consultar(&productos, &kardex, &ventas);

        // 466.64 entre 8 unidades base. Ninguna de las dos presentaciones se
        // vendió a este precio: es el precio al que salió la mercancía.
        assert_eq!(fila(&hoja, 1).precio, "58.33");
        // Con una sola forma, el medio coincide con el de catálogo.
        assert_eq!(fila(&hoja, 2).precio, "1000.00");
    }

    #[test]
    fn las_subfilas_desglosan_cada_presentacion_vendida() {
        let (productos, kardex, ventas) = turno_de_muestra();

        let hoja = consultar(&productos, &kardex, &ventas);

        let formas = &fila(&hoja, 1).formas;
        assert_eq!(formas.len(), 2);
        // De mayor a menor importe: el paquete dejó más dinero.
        assert_eq!(formas[0].nombre, "Paquete");
        assert_eq!(formas[0].salida, "1");
        assert_eq!(formas[0].precio, "266.64");
        assert_eq!(formas[0].importe, "266.64");
        assert_eq!(formas[1].nombre, "Unidad");
        assert_eq!(formas[1].salida, "2");
        assert_eq!(formas[1].precio, "100.00");
        assert_eq!(formas[1].importe, "200.00");
    }

    #[test]
    fn con_una_sola_presentacion_no_hay_subfilas() {
        let (productos, kardex, ventas) = turno_de_muestra();

        let hoja = consultar(&productos, &kardex, &ventas);

        // La subfila repetiría la fila de arriba.
        assert!(fila(&hoja, 2).formas.is_empty());
    }

    /// Los decimales se escriben cuando los hay, no cuando el producto los
    /// admite.
    ///
    /// Es el criterio propio de esta hoja, distinto del de las demás
    /// pantallas: con nueve columnas por 150 filas, una pared de «0.000»
    /// tapa el «1.500» que sí hay que mirar.
    #[test]
    fn solo_se_escriben_los_decimales_que_hay() {
        let productos = catalogo().con_acumulado(TURNO, 1, "1500.00", "0.00");
        let kardex = IpvEnMemoria::default()
            .con_saldo(TURNO, 1, "20", "20")
            .con_saldo(TURNO, 2, "5", "3.5")
            .con_movido(TURNO, 2, TipoMovimiento::Venta, DE_VITRINA, "1.5");
        let ventas = VentasDeSesionEnMemoria::default()
            .con_linea(TURNO, linea(1, (2, 20), "Libra", "1.5", "1", "1000.00"));

        let hoja = consultar(&productos, &kardex, &ventas);

        let arroz = fila(&hoja, 2);
        // Las enteras sin cola, las fraccionarias con sus tres decimales,
        // y todo ello en la misma fila del mismo producto a granel.
        assert_eq!(arroz.inicial, "5");
        assert_eq!(arroz.salida, "1.500");
        assert_eq!(arroz.cierre, "3.500");
        assert_eq!(arroz.entrada, "0");
        assert_eq!(arroz.importe, "1500.00");
        assert!(arroz.cuadra);
        // Pero la cifra sin formatear sigue diciendo que se pesa: es lo que
        // usa el Excel para dar formato a la celda.
        assert_eq!(arroz.cifras.decimales, 3);
        // La salchicha se cuenta por unidades y no admite fracción.
        assert_eq!(fila(&hoja, 1).inicial, "20");
        assert_eq!(fila(&hoja, 1).cifras.decimales, 0);
    }

    #[test]
    fn la_entrada_la_merma_y_el_consumo_tienen_su_columna() {
        let productos = catalogo().con_acumulado(TURNO, 1, "200.00", "0.00");
        let kardex = IpvEnMemoria::default()
            // 20 + 10 − 3 − 1 − 2 = 24.
            .con_saldo(TURNO, 1, "20", "24")
            .con_movido(TURNO, 1, TipoMovimiento::Entrada, A_VITRINA, "10")
            .con_movido(TURNO, 1, TipoMovimiento::Merma, DE_VITRINA, "3")
            .con_movido(TURNO, 1, TipoMovimiento::Consumo, DE_VITRINA, "1")
            .con_movido(TURNO, 1, TipoMovimiento::Venta, DE_VITRINA, "2");
        let ventas = VentasDeSesionEnMemoria::default()
            .con_linea(TURNO, linea(1, (1, 10), "Unidad", "2", "1", "100.00"));

        let hoja = consultar(&productos, &kardex, &ventas);
        let salchicha = fila(&hoja, 1);

        assert_eq!(salchicha.entrada, "10");
        assert_eq!(salchicha.merma, "3");
        assert_eq!(salchicha.consumo, "1");
        assert_eq!(salchicha.salida, "2");
        assert_eq!(salchicha.cierre, "24");
        assert!(salchicha.cuadra);
    }

    /// Lo que baja del almacén al mostrador es la ENTRADA del IPV.
    ///
    /// Es la fuente habitual de la columna: la mercancía se compra una vez y
    /// se va bajando al estante a lo largo de los días. Para Kilo12 es un
    /// traspaso (RF-INV-04) y para la hoja es una entrada, y es el mismo
    /// hecho visto desde los dos lados.
    #[test]
    fn lo_que_baja_del_almacen_es_una_entrada() {
        let productos = catalogo().con_acumulado(TURNO, 0, "0.00", "0.00");
        let kardex = IpvEnMemoria::default()
            // 20 + 5 bajadas del almacén = 25 en vitrina.
            .con_saldo(TURNO, 1, "20", "25")
            .con_movido(TURNO, 1, TipoMovimiento::Traspaso, BAJA, "5");

        let hoja = consultar(&productos, &kardex, &VentasDeSesionEnMemoria::default());
        let salchicha = fila(&hoja, 1);

        assert_eq!(salchicha.entrada, "5");
        assert_eq!(salchicha.salida, "0");
        assert_eq!(salchicha.cierre, "25");
        assert!(salchicha.cuadra);
    }

    /// Lo que sube de vuelta al almacén resta de la misma columna.
    ///
    /// No hay una columna para eso en la hoja de papel y no se va a
    /// inventar: subir mercancía es lo contrario de bajarla, así que va a
    /// ENTRADA con el signo cambiado y la identidad sigue cerrando.
    #[test]
    fn lo_que_sube_al_almacen_resta_de_la_entrada() {
        let productos = catalogo().con_acumulado(TURNO, 0, "0.00", "0.00");
        let kardex = IpvEnMemoria::default()
            // Bajaron 8 y subieron 3: entraron 5.
            .con_saldo(TURNO, 1, "20", "25")
            .con_movido(TURNO, 1, TipoMovimiento::Traspaso, BAJA, "8")
            .con_movido(TURNO, 1, TipoMovimiento::Traspaso, SUBE, "3");

        let hoja = consultar(&productos, &kardex, &VentasDeSesionEnMemoria::default());
        let salchicha = fila(&hoja, 1);

        assert_eq!(salchicha.entrada, "5");
        assert_eq!(salchicha.cierre, "25");
        assert!(salchicha.cuadra);
    }

    #[test]
    fn lo_que_solo_sube_deja_la_entrada_en_contra() {
        let productos = catalogo().con_acumulado(TURNO, 0, "0.00", "0.00");
        let kardex = IpvEnMemoria::default()
            // Se retiraron 4 del mostrador y no bajó nada.
            .con_saldo(TURNO, 1, "20", "16")
            .con_movido(TURNO, 1, TipoMovimiento::Traspaso, SUBE, "4");

        let hoja = consultar(&productos, &kardex, &VentasDeSesionEnMemoria::default());
        let salchicha = fila(&hoja, 1);

        assert_eq!(salchicha.entrada, "-4");
        assert_eq!(salchicha.cierre, "16");
        assert!(salchicha.cuadra);
    }

    /// Lo que pasa en el almacén no sale en esta hoja.
    ///
    /// Una compra que se guarda sin colocar y una merma de lo guardado son
    /// hechos reales, pero no del mostrador: si entraran, la CANT. FINAL ya
    /// no se podría comprobar contra el estante y la hoja perdería su única
    /// utilidad.
    #[test]
    fn lo_que_solo_toca_el_almacen_no_aparece() {
        let productos = catalogo().con_acumulado(TURNO, 0, "0.00", "0.00");
        let kardex = IpvEnMemoria::default()
            .con_saldo(TURNO, 1, "20", "20")
            // Llegaron 100 del proveedor y se guardaron.
            .con_movido(TURNO, 1, TipoMovimiento::Entrada, A_BODEGA, "100")
            // Y se echaron a perder 7 de lo guardado.
            .con_movido(TURNO, 1, TipoMovimiento::Merma, DE_BODEGA, "7");

        let hoja = consultar(&productos, &kardex, &VentasDeSesionEnMemoria::default());
        let salchicha = fila(&hoja, 1);

        assert_eq!(salchicha.entrada, "0");
        assert_eq!(salchicha.merma, "0");
        assert_eq!(salchicha.cierre, "20");
        // Y la fila cuadra: la vitrina, en efecto, no se movió.
        assert!(salchicha.cuadra);
    }

    /// Una compra colocada directamente en el mostrador también es ENTRADA.
    #[test]
    fn una_compra_puesta_en_la_vitrina_es_una_entrada() {
        let productos = catalogo().con_acumulado(TURNO, 0, "0.00", "0.00");
        let kardex = IpvEnMemoria::default()
            .con_saldo(TURNO, 1, "20", "32")
            // 12 que llegaron y se pusieron en el estante sin pasar por el
            // almacén, más 0 traspasos.
            .con_movido(TURNO, 1, TipoMovimiento::Entrada, A_VITRINA, "12");

        let hoja = consultar(&productos, &kardex, &VentasDeSesionEnMemoria::default());
        let salchicha = fila(&hoja, 1);

        assert_eq!(salchicha.entrada, "12");
        assert_eq!(salchicha.cierre, "32");
        assert!(salchicha.cuadra);
    }

    /// Las dos vías de entrada se suman en la misma columna.
    #[test]
    fn la_compra_directa_y_lo_bajado_del_almacen_comparten_columna() {
        let productos = catalogo().con_acumulado(TURNO, 0, "0.00", "0.00");
        let kardex = IpvEnMemoria::default()
            // 20 + 6 del almacén + 4 de la calle = 30.
            .con_saldo(TURNO, 1, "20", "30")
            .con_movido(TURNO, 1, TipoMovimiento::Traspaso, BAJA, "6")
            .con_movido(TURNO, 1, TipoMovimiento::Entrada, A_VITRINA, "4");

        let hoja = consultar(&productos, &kardex, &VentasDeSesionEnMemoria::default());

        assert_eq!(fila(&hoja, 1).entrada, "10");
        assert!(fila(&hoja, 1).cuadra);
    }

    /// Un consumo que salió del almacén no es de esta hoja.
    #[test]
    fn un_consumo_sacado_del_almacen_no_aparece() {
        let productos = catalogo().con_acumulado(TURNO, 0, "0.00", "0.00");
        let kardex = IpvEnMemoria::default()
            .con_saldo(TURNO, 1, "20", "20")
            .con_movido(TURNO, 1, TipoMovimiento::Consumo, DE_BODEGA, "3");

        let hoja = consultar(&productos, &kardex, &VentasDeSesionEnMemoria::default());

        assert_eq!(fila(&hoja, 1).consumo, "0");
        assert!(fila(&hoja, 1).cuadra);
    }

    #[test]
    fn la_devolucion_resta_de_la_salida() {
        let productos = catalogo().con_acumulado(TURNO, 2, "300.00", "0.00");
        let kardex = IpvEnMemoria::default()
            // Se vendieron 5 y se devolvieron 2: salieron 3.
            .con_saldo(TURNO, 1, "20", "17")
            .con_movido(TURNO, 1, TipoMovimiento::Venta, DE_VITRINA, "5")
            .con_movido(TURNO, 1, TipoMovimiento::Devolucion, A_VITRINA, "2");
        let ventas = VentasDeSesionEnMemoria::default()
            .con_linea(TURNO, linea(1, (1, 10), "Unidad", "3", "1", "100.00"));

        let hoja = consultar(&productos, &kardex, &ventas);
        let salchicha = fila(&hoja, 1);

        assert_eq!(salchicha.salida, "3");
        assert!(salchicha.cuadra);
    }

    #[test]
    fn la_anulacion_de_consumo_resta_del_consumo() {
        let productos = catalogo().con_acumulado(TURNO, 0, "0.00", "0.00");
        let kardex = IpvEnMemoria::default()
            // Se cargaron 4 y se anularon 3: se llevaron 1.
            .con_saldo(TURNO, 1, "20", "19")
            .con_movido(TURNO, 1, TipoMovimiento::Consumo, DE_VITRINA, "4")
            .con_movido(TURNO, 1, TipoMovimiento::ConsumoAnulado, A_VITRINA, "3");

        let hoja = consultar(&productos, &kardex, &VentasDeSesionEnMemoria::default());
        let salchicha = fila(&hoja, 1);

        assert_eq!(salchicha.consumo, "1");
        assert!(salchicha.cuadra);
    }

    #[test]
    fn lo_devuelto_de_un_turno_anterior_deja_la_columna_en_contra() {
        let productos = catalogo().con_acumulado(TURNO, 0, "0.00", "0.00");
        // Hoy solo se devolvieron 2 que se habían vendido ayer.
        let kardex = IpvEnMemoria::default()
            .con_saldo(TURNO, 1, "20", "22")
            .con_movido(TURNO, 1, TipoMovimiento::Devolucion, A_VITRINA, "2");

        let hoja = consultar(&productos, &kardex, &VentasDeSesionEnMemoria::default());
        let salchicha = fila(&hoja, 1);

        // La cifra sale en contra y la fila sigue cuadrando: la existencia
        // subió justo eso.
        assert_eq!(salchicha.salida, "-2");
        assert_eq!(salchicha.cierre, "22");
        assert!(salchicha.cuadra);
        // Sin salida no hay nada que repartir: manda el precio de catálogo.
        assert_eq!(salchicha.precio, "100.00");
        assert_eq!(salchicha.importe, "0.00");
    }

    #[test]
    fn una_fila_que_no_se_explica_se_marca() {
        let productos = catalogo().con_acumulado(TURNO, 0, "0.00", "0.00");
        // 20 − 2 debería dejar 18, y el kárdex dice 15.
        let kardex = IpvEnMemoria::default()
            .con_saldo(TURNO, 1, "20", "15")
            .con_movido(TURNO, 1, TipoMovimiento::Merma, DE_VITRINA, "2");

        let hoja = consultar(&productos, &kardex, &VentasDeSesionEnMemoria::default());
        let salchicha = fila(&hoja, 1);

        // Las cifras se enseñan tal cual: la hoja avisa, no corrige.
        assert_eq!(salchicha.merma, "2");
        assert_eq!(salchicha.cierre, "15");
        assert!(!salchicha.cuadra);
    }

    #[test]
    fn un_ajuste_impide_que_la_fila_cuadre_aunque_las_cifras_encajen() {
        let productos = catalogo().con_acumulado(TURNO, 0, "0.00", "0.00");
        let kardex = IpvEnMemoria::default()
            .con_saldo(TURNO, 1, "20", "20")
            .con_movido(TURNO, 1, TipoMovimiento::Ajuste, DE_VITRINA, "5");

        let hoja = consultar(&productos, &kardex, &VentasDeSesionEnMemoria::default());
        let salchicha = fila(&hoja, 1);

        // Sin columna donde ponerlo, cuadrar sería casualidad.
        assert!(!salchicha.cuadra);
    }

    #[test]
    fn las_filas_van_en_orden_alfabetico() {
        let (productos, kardex, ventas) = turno_de_muestra();

        let hoja = consultar(&productos, &kardex, &ventas);

        let nombres: Vec<&str> = hoja.filas.iter().map(|f| f.mercancia.as_str()).collect();
        // El arroz primero, aunque la salchicha dejó más dinero: esta hoja
        // se recorre estante en mano, no por importe.
        assert_eq!(nombres, ["Arroz el rey", "Salchicha"]);
    }

    #[test]
    fn un_producto_sin_ventas_sale_con_su_precio_de_catalogo() {
        let productos = catalogo().con_acumulado(TURNO, 0, "0.00", "0.00");

        let hoja = consultar(
            &productos,
            &IpvEnMemoria::default(),
            &VentasDeSesionEnMemoria::default(),
        );

        // Las dos filas están, con todo en cero y el precio puesto: la hoja
        // sirve también para consultar a cuánto está algo.
        assert_eq!(hoja.filas.len(), 2);
        let salchicha = fila(&hoja, 1);
        assert_eq!(salchicha.salida, "0");
        assert_eq!(salchicha.importe, "0.00");
        // El de la presentación predeterminada, por unidad base.
        assert_eq!(salchicha.precio, "100.00");
        assert!(salchicha.cuadra);
    }

    #[test]
    fn el_precio_de_catalogo_se_reparte_entre_el_factor() {
        let productos = ProductosEnMemoria::default()
            .con_producto(
                3,
                "Cerveza",
                UnidadBase::Unidad,
                // La predeterminada es el six-pack a 600: 100 la lata.
                &[(30, "Six-pack", "6", "600.00"), (31, "Lata", "1", "120.00")],
                "12",
            )
            .con_sesion_abierta(TURNO, "2026-10-13 08:00:00")
            .con_acumulado(TURNO, 0, "0.00", "0.00");

        let hoja = consultar(
            &productos,
            &IpvEnMemoria::default(),
            &VentasDeSesionEnMemoria::default(),
        );

        assert_eq!(fila(&hoja, 3).precio, "100.00");
    }

    #[test]
    fn un_producto_retirado_que_movio_sigue_teniendo_fila() {
        let productos = catalogo().con_acumulado(TURNO, 1, "1000.00", "0.00");
        productos.desactivar_producto(2);
        let kardex = IpvEnMemoria::default()
            .con_saldo(TURNO, 2, "5", "4")
            .con_movido(TURNO, 2, TipoMovimiento::Venta, DE_VITRINA, "1");
        let ventas = VentasDeSesionEnMemoria::default()
            .con_linea(TURNO, linea(1, (2, 20), "Libra", "1", "1", "1000.00"));

        let hoja = consultar(&productos, &kardex, &ventas);

        // Lo que se vendió, se vendió: tiene que aparecer en la hoja.
        assert_eq!(hoja.filas.len(), 2);
        assert_eq!(fila(&hoja, 2).importe, "1000.00");
    }

    #[test]
    fn un_producto_retirado_con_existencia_tambien_tiene_fila() {
        let productos = catalogo().con_acumulado(TURNO, 0, "0.00", "0.00");
        productos.desactivar_producto(2);
        // No movió nada, pero quedan 5 libras en el estante que hay que
        // contar.
        let kardex = IpvEnMemoria::default().con_saldo(TURNO, 2, "5", "5");

        let hoja = consultar(&productos, &kardex, &VentasDeSesionEnMemoria::default());

        assert_eq!(hoja.filas.len(), 2);
        assert_eq!(fila(&hoja, 2).cierre, "5");
    }

    #[test]
    fn un_producto_retirado_y_vacio_no_ensucia_la_hoja() {
        let productos = catalogo().con_acumulado(TURNO, 0, "0.00", "0.00");
        productos.desactivar_producto(2);
        let kardex = IpvEnMemoria::default().con_saldo(TURNO, 2, "0", "0");

        let hoja = consultar(&productos, &kardex, &VentasDeSesionEnMemoria::default());

        assert_eq!(hoja.filas.len(), 1);
        assert_eq!(hoja.filas[0].producto, 1);
    }

    #[test]
    fn el_total_de_la_hoja_se_compara_con_lo_que_cobro_la_caja() {
        let (productos, kardex, ventas) = turno_de_muestra();

        let hoja = consultar(&productos, &kardex, &ventas);

        // 466.64 de salchicha + 1000 de arroz.
        assert_eq!(hoja.importe_total, "1466.64");
        assert_eq!(hoja.vendido_real, "1466.64");
        assert_eq!(hoja.diferencia, "0.00");
        assert!(hoja.cuadra);
    }

    #[test]
    fn un_descuento_separa_la_hoja_de_lo_cobrado_y_se_dice() {
        // La hoja suma precio por cantidad; la caja cobró 100 menos.
        let productos = catalogo().con_acumulado(TURNO, 2, "1366.64", "0.00");
        let (_, kardex, ventas) = turno_de_muestra();

        let hoja = consultar(&productos, &kardex, &ventas);

        assert_eq!(hoja.importe_total, "1466.64");
        assert_eq!(hoja.vendido_real, "1366.64");
        assert_eq!(hoja.diferencia, "100.00");
        assert!(!hoja.cuadra);
    }

    #[test]
    fn la_diferencia_sale_con_signo_cuando_la_caja_cobro_mas() {
        let productos = catalogo().con_acumulado(TURNO, 2, "1500.00", "0.00");
        let (_, kardex, ventas) = turno_de_muestra();

        let hoja = consultar(&productos, &kardex, &ventas);

        assert_eq!(hoja.diferencia, "-33.36");
        assert!(!hoja.cuadra);
    }

    #[test]
    fn las_cifras_crudas_dicen_lo_mismo_que_los_textos() {
        let (productos, kardex, ventas) = turno_de_muestra();

        let hoja = consultar(&productos, &kardex, &ventas);

        // Lo que escribe el `.xlsx` y lo que lee la pantalla salen del mismo
        // cálculo; si se separaran, la hoja impresa y la exportada dirían
        // cifras distintas.
        let salchicha = fila(&hoja, 1);
        assert_eq!(salchicha.cifras.inicial.formatear(0), "20");
        assert_eq!(salchicha.cifras.salida.formatear(0), "8");
        assert_eq!(salchicha.cifras.cierre.formatear(0), "12");
        assert_eq!(salchicha.cifras.precio.formatear(2), salchicha.precio);
        assert_eq!(salchicha.cifras.importe.formatear(2), salchicha.importe);
        assert_eq!(hoja.importe_total_crudo.formatear(2), hoja.importe_total);
        assert_eq!(hoja.vendido_real_crudo.formatear(2), hoja.vendido_real);
        assert_eq!(hoja.diferencia_cruda.formatear(2), hoja.diferencia);

        let paquete = &salchicha.formas[0];
        assert_eq!(paquete.salida_cruda.formatear(0), paquete.salida);
        assert_eq!(paquete.precio_crudo.formatear(2), paquete.precio);
        assert_eq!(paquete.importe_crudo.formatear(2), paquete.importe);
    }

    // ============================== quién puede ver qué (RF-IPV-14)

    /// Un turno anterior armado para pedirlo: el 5, cerrado, con una venta.
    fn con_turno_cerrado() -> (ProductosEnMemoria, IpvEnMemoria, VentasDeSesionEnMemoria) {
        let productos = catalogo()
            .con_sesion_cerrada(5, "2026-10-12 08:00:00", "2026-10-12 20:00:00")
            .con_acumulado(5, 1, "100.00", "0.00");
        let kardex = IpvEnMemoria::default()
            // El 7 sigue abierto; el que se pide es el 5.
            .con_saldo(TURNO, 1, "20", "20")
            .con_saldo(5, 1, "30", "29")
            .con_movido(5, 1, TipoMovimiento::Venta, DE_VITRINA, "1");
        let ventas = VentasDeSesionEnMemoria::default()
            .con_linea(5, linea(1, (1, 10), "Unidad", "1", "1", "100.00"));

        (productos, kardex, ventas)
    }

    #[test]
    fn con_la_clave_se_puede_ver_un_turno_cerrado() {
        let (productos, kardex, ventas) = con_turno_cerrado();

        let hoja =
            pedir(&productos, &kardex, &ventas, 5, Some(CLAVE_MANTENIMIENTO)).expect("consultar");

        let sesion = hoja.sesion.as_ref().expect("hay turno");
        assert_eq!(sesion.id, 5);
        assert!(!sesion.abierta);
        assert_eq!(sesion.cerrada_en.as_deref(), Some("2026-10-12 20:00:00"));
        let salchicha = fila(&hoja, 1);
        assert_eq!(salchicha.inicial, "30");
        assert_eq!(salchicha.cierre, "29");
        assert_eq!(hoja.importe_total, "100.00");
    }

    #[test]
    fn sin_clave_el_pasado_no_se_ve() {
        let (productos, kardex, ventas) = con_turno_cerrado();

        let fallo = pedir(&productos, &kardex, &ventas, 5, None).expect_err("sin clave");

        assert!(matches!(fallo, ErrorAplicacion::ClaveIncorrecta));
    }

    #[test]
    fn con_la_clave_mala_tampoco() {
        let (productos, kardex, ventas) = con_turno_cerrado();

        let fallo = pedir(&productos, &kardex, &ventas, 5, Some("000000")).expect_err("clave mala");

        assert!(matches!(fallo, ErrorAplicacion::ClaveIncorrecta));
    }

    #[test]
    fn la_clave_admite_espacios_alrededor() {
        let (productos, kardex, ventas) = con_turno_cerrado();

        // Se teclea con prisa y a veces se pega desde otro sitio.
        let hoja = pedir(&productos, &kardex, &ventas, 5, Some("  314159 ")).expect("consultar");

        assert_eq!(hoja.sesion.expect("hay turno").id, 5);
    }

    /// Pedir por su número el turno que está abierto no exige nada.
    ///
    /// Es el mismo que se ve al entrar en la pantalla, así que pedir la
    /// clave sería una puerta en mitad del campo.
    #[test]
    fn el_turno_abierto_no_pide_clave_aunque_se_pida_por_su_numero() {
        let (productos, kardex, ventas) = turno_de_muestra();

        let hoja = pedir(&productos, &kardex, &ventas, TURNO, None).expect("consultar");

        let sesion = hoja.sesion.expect("hay turno");
        assert_eq!(sesion.id, TURNO);
        assert!(sesion.abierta);
    }

    #[test]
    fn el_turno_abierto_es_el_que_se_enseña_sin_pedir_ninguno() {
        let (productos, kardex, ventas) = turno_de_muestra();

        let hoja = consultar(&productos, &kardex, &ventas);

        let sesion = hoja.sesion.expect("hay turno");
        assert_eq!(sesion.id, TURNO);
        assert!(sesion.abierta);
        assert_eq!(sesion.cerrada_en, None);
    }

    /// Con la caja cerrada no hay hoja que mirar.
    ///
    /// Antes se enseñaba la del último turno cerrado, como hace Ventas. Se
    /// quitó a propósito: el IPV se llena y se cuadra mientras el turno está
    /// vivo, y la hoja de ayer es historia, que se consulta desde Caja y con
    /// la clave delante (RF-IPV-14).
    #[test]
    fn con_la_caja_cerrada_no_hay_hoja_aunque_haya_turnos_anteriores() {
        let (_, kardex, ventas) = con_turno_cerrado();
        let productos = ProductosEnMemoria::default()
            .con_sesion_cerrada(5, "2026-10-12 08:00:00", "2026-10-12 20:00:00")
            .con_acumulado(5, 1, "100.00", "0.00");

        let hoja = consultar(&productos, &kardex, &ventas);

        assert_eq!(hoja.sesion, None);
        assert!(hoja.filas.is_empty());
    }

    /// Un turno que no existe se rechaza por la clave, no por no existir.
    ///
    /// Si la clave se comprobara después de buscarlo, probar números y mirar
    /// si cambia el error diría cuántos turnos hay.
    #[test]
    fn un_turno_inexistente_pide_la_clave_igual() {
        let (productos, kardex, ventas) = turno_de_muestra();

        let fallo = pedir(&productos, &kardex, &ventas, 99, None).expect_err("sin clave");

        assert!(matches!(fallo, ErrorAplicacion::ClaveIncorrecta));
    }

    #[test]
    fn con_la_clave_un_turno_que_no_existe_no_es_un_error() {
        let (productos, kardex, ventas) = turno_de_muestra();

        let hoja =
            pedir(&productos, &kardex, &ventas, 99, Some(CLAVE_MANTENIMIENTO)).expect("consultar");

        assert_eq!(hoja.sesion, None);
        assert!(hoja.filas.is_empty());
    }

    #[test]
    fn sin_ninguna_sesion_la_hoja_esta_vacia_y_no_es_un_error() {
        let productos = ProductosEnMemoria::default();

        let hoja = consultar(
            &productos,
            &IpvEnMemoria::default(),
            &VentasDeSesionEnMemoria::default(),
        );

        assert_eq!(hoja.sesion, None);
        assert!(hoja.filas.is_empty());
        assert_eq!(hoja.importe_total, "0.00");
    }
}
