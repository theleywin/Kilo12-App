//! Dobles en memoria de los puertos, solo para las pruebas.
//!
//! La estrategia de pruebas (§6 del diseño técnico) prueba los casos de uso
//! con los puertos sustituidos por implementaciones en memoria. Aquí viven
//! las que hacen falta, sin SQLite: si un caso de uso depende de algo que
//! aquí no está, la prueba lo dice con un `unreachable!` en vez de fingir.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use domain::{
    Cantidad, Dinero, EstadoSesion, Existencias, IdPresentacion, IdProducto, IdSesion,
    IdVentaEnEspera, Inventario, Movimiento, MovimientoEfectivo, Presentacion, Producto,
    SesionCaja, TipoMovimiento, Ubicacion, UnidadBase, VentaEnEspera,
};

use crate::error::Resultado;
use crate::puertos::{
    AcumuladoSesion, AnulacionConfirmada, AnulacionConsumo, Asiento, CambioDeCosto, CambioDePrecio,
    CambioRegistrado, CierreConfirmado, ConsultaIpv, ConsultaVentasDeSesion, ConsumoConfirmado,
    ConsumoRegistrado, CostoRegistrado, CuentaConsumo, DescuentoConsumo, DetalleVenta,
    EsperaRegistrada, EsperaResumida, LineaConsumoRegistrada, LineaDelPeriodo, MovidoEnSesion,
    MovimientoEfectivoRegistrado, MovimientoRegistrado, PagoConsumoRegistrado,
    ProductoConInventario, RastroProducto, RepositorioConsumo, RepositorioProducto,
    RepositorioVentaEnEspera, SaldoCuenta, SaldoDeSesion, SesionRegistrada, TotalesPeriodo,
    VentaConfirmada, VentaDiaria, VentaHoraria, VentaPorMetodo, VentaRegistrada,
};

fn cantidad(texto: &str) -> Cantidad {
    texto.parse().expect("cantidad válida")
}

fn dinero(texto: &str) -> Dinero {
    texto.parse().expect("importe válido")
}

/// Catálogo en memoria, con lo justo para vender y retomar.
#[derive(Debug, Default)]
pub(crate) struct ProductosEnMemoria {
    productos: RefCell<BTreeMap<i64, ProductoConInventario>>,
    sesion: RefCell<Option<SesionRegistrada>>,
    /// Sesiones ya cerradas, por su identificador.
    cerradas: RefCell<BTreeMap<i64, SesionRegistrada>>,
    /// Lo que la base sumaría para cada sesión.
    acumulados: RefCell<BTreeMap<i64, AcumuladoSesion>>,
    /// Ventas en el orden en que las devolvería la lista.
    ventas: RefCell<Vec<VentaRegistrada>>,
    /// La espera que llevaba la última venta registrada, tal cual llegó.
    pub(crate) espera_cobrada: RefCell<Option<Option<IdVentaEnEspera>>>,
    /// Revaluaciones anotadas, de la más antigua a la más reciente.
    pub(crate) revaluaciones: RefCell<Vec<(IdProducto, CambioDeCosto)>>,
    /// Lo que cada producto ha dejado escrito fuera de su ficha.
    rastros: RefCell<BTreeMap<i64, RastroProducto>>,
    /// Limpiezas pedidas, como `(borrar, retirar)`.
    pub(crate) limpiezas: RefCell<Vec<(Vec<IdProducto>, Vec<IdProducto>)>>,
    folio: Cell<i64>,
}

impl ProductosEnMemoria {
    /// Da de alta un producto con una presentación por cada `(id, nombre,
    /// factor, precio)` y la existencia indicada en vitrina.
    pub(crate) fn con_producto(
        self,
        id: i64,
        nombre: &str,
        unidad: UnidadBase,
        presentaciones: &[(i64, &str, &str, &str)],
        en_vitrina: &str,
    ) -> Self {
        let presentaciones = presentaciones
            .iter()
            .enumerate()
            .map(|(posicion, (id, nombre, factor, precio))| {
                Presentacion::reconstituir(
                    IdPresentacion(*id),
                    (*nombre).to_owned(),
                    cantidad(factor),
                    dinero(precio),
                    posicion == 0,
                    None,
                    true,
                )
            })
            .collect();

        let producto = Producto::reconstituir(
            IdProducto(id),
            format!("SKU-{id}"),
            nombre.to_owned(),
            unidad,
            Cantidad::CERO,
            Cantidad::CERO,
            presentaciones,
            true,
        );
        let inventario = Inventario::nuevo(
            Existencias::nuevas(Cantidad::CERO, cantidad(en_vitrina)).expect("existencias"),
            Dinero::CERO,
        )
        .expect("inventario");

        self.productos.borrow_mut().insert(
            id,
            ProductoConInventario {
                producto,
                inventario,
            },
        );
        self
    }

    /// Fija el valor invertido de un producto ya dado de alta.
    ///
    /// `con_producto` lo deja en cero, y con valor cero el costo también es
    /// cero. Para probar cualquier cosa que mire el costo hace falta que el
    /// inventario valga algo.
    pub(crate) fn con_valor(self, id: i64, valor: &str) -> Self {
        let mut productos = self.productos.borrow_mut();
        let fila = productos.get_mut(&id).expect("producto dado de alta");

        fila.inventario = Inventario::nuevo(fila.inventario.existencias(), dinero(valor))
            .expect("inventario válido");

        drop(productos);
        self
    }

    /// Pone mercancía también en la bodega.
    ///
    /// `con_producto` solo surte la vitrina, y el consumo puede salir de
    /// cualquiera de las dos.
    pub(crate) fn con_bodega(self, id: i64, en_bodega: &str) -> Self {
        let mut productos = self.productos.borrow_mut();
        let fila = productos.get_mut(&id).expect("producto dado de alta");

        let existencias = Existencias::nuevas(
            cantidad(en_bodega),
            fila.inventario.existencias().en(Ubicacion::Vitrina),
        )
        .expect("existencias");

        fila.inventario = Inventario::nuevo(existencias, fila.inventario.valor_total())
            .expect("inventario válido");

        drop(productos);
        self
    }

    /// Deja el inventario de un producto como se lo entregaron.
    ///
    /// Es lo que hace el adaptador de SQLite al escribir la existencia y el
    /// valor dentro de su transacción.
    pub(crate) fn fijar_inventario(&self, id: IdProducto, inventario: Inventario) {
        if let Some(guardado) = self.productos.borrow_mut().get_mut(&id.0) {
            guardado.inventario = inventario;
        }
    }

    /// El inventario que tiene ahora un producto, para comprobar efectos.
    pub(crate) fn inventario_de(&self, id: i64) -> Inventario {
        self.productos
            .borrow()
            .get(&id)
            .expect("producto dado de alta")
            .inventario
    }

    /// Deja una caja abierta, para poder cobrar.
    pub(crate) fn con_caja_abierta(self) -> Self {
        self.con_sesion_abierta(1, "2026-09-27 08:00:00")
    }

    /// Deja abierta la sesión indicada.
    pub(crate) fn con_sesion_abierta(self, id: i64, abierta_en: &str) -> Self {
        *self.sesion.borrow_mut() = Some(SesionRegistrada {
            sesion: SesionCaja::rehidratar(
                IdSesion(id),
                "Ana",
                Dinero::CERO,
                EstadoSesion::Abierta,
            ),
            abierta_en: abierta_en.to_owned(),
            cerrada_en: None,
        });
        self
    }

    /// Añade una sesión ya cerrada al historial.
    pub(crate) fn con_sesion_cerrada(self, id: i64, abierta_en: &str, cerrada_en: &str) -> Self {
        self.cerradas.borrow_mut().insert(
            id,
            SesionRegistrada {
                sesion: SesionCaja::rehidratar(
                    IdSesion(id),
                    "Ana",
                    Dinero::CERO,
                    EstadoSesion::Cerrada,
                ),
                abierta_en: abierta_en.to_owned(),
                cerrada_en: Some(cerrada_en.to_owned()),
            },
        );
        self
    }

    /// Fija lo que la base sumaría para una sesión: ventas no anuladas,
    /// total vendido y costo.
    pub(crate) fn con_acumulado(self, sesion: i64, cuantas: i64, total: &str, costo: &str) -> Self {
        self.acumulados.borrow_mut().insert(
            sesion,
            AcumuladoSesion {
                cuantas_ventas: cuantas,
                total_vendido: dinero(total),
                costo_vendido: dinero(costo),
                ..AcumuladoSesion::default()
            },
        );
        self
    }

    /// Añade una venta a la lista, la más reciente al final.
    pub(crate) fn con_venta(self, venta: VentaRegistrada) -> Self {
        self.ventas.borrow_mut().push(venta);
        self
    }

    /// Desactiva un producto, como si se hubiera hecho desde la ficha.
    pub(crate) fn desactivar_producto(&self, id: i64) {
        if let Some(guardado) = self.productos.borrow_mut().get_mut(&id) {
            guardado.producto.desactivar();
        }
    }

    /// Desactiva una presentación sin pasar por la regla de la última.
    pub(crate) fn desactivar_presentacion(&self, producto: i64, presentacion: i64) {
        if let Some(guardado) = self.productos.borrow_mut().get_mut(&producto) {
            let mut presentaciones = guardado.producto.presentaciones().to_vec();
            for actual in &mut presentaciones {
                if actual.id() == Some(IdPresentacion(presentacion)) {
                    actual.desactivar();
                }
            }

            guardado.producto = Producto::reconstituir(
                IdProducto(producto),
                guardado.producto.sku().to_owned(),
                guardado.producto.nombre().to_owned(),
                guardado.producto.unidad_base(),
                guardado.producto.stock_minimo(),
                guardado.producto.objetivo_vitrina(),
                presentaciones,
                guardado.producto.esta_activo(),
            );
        }
    }

    /// Borra un producto del catálogo, como si nunca hubiera existido.
    pub(crate) fn quitar_producto(&self, id: i64) {
        self.productos.borrow_mut().remove(&id);
    }

    /// Fija lo que un producto ha dejado escrito fuera de su ficha.
    ///
    /// En SQLite son tres `COUNT(*)`; aquí se dictan, que es lo que permite
    /// probar la clasificación sin montar ventas enteras.
    pub(crate) fn con_rastro(self, id: i64, ventas: i64, movimientos: i64, esperas: i64) -> Self {
        self.rastros.borrow_mut().insert(
            id,
            RastroProducto {
                ventas,
                movimientos,
                esperas,
            },
        );
        self
    }
}

impl RepositorioProducto for ProductosEnMemoria {
    fn obtener(&self, id: IdProducto) -> Resultado<Option<ProductoConInventario>> {
        Ok(self.productos.borrow().get(&id.0).cloned())
    }

    fn sesion_abierta(&self) -> Resultado<Option<SesionRegistrada>> {
        Ok(self.sesion.borrow().clone())
    }

    fn listar_sesiones(&self, limite: usize) -> Resultado<Vec<SesionRegistrada>> {
        // Como la base: todas, abierta incluida, de la más nueva a la más
        // vieja por identificador.
        let mut todas: Vec<SesionRegistrada> = self.cerradas.borrow().values().cloned().collect();
        todas.extend(self.sesion.borrow().clone());
        todas.sort_by_key(|s| std::cmp::Reverse(s.sesion.id().map_or(0, |id| id.0)));
        todas.truncate(limite);
        Ok(todas)
    }

    fn acumulado_de_sesion(&self, id: IdSesion) -> Resultado<AcumuladoSesion> {
        Ok(self
            .acumulados
            .borrow()
            .get(&id.0)
            .cloned()
            .unwrap_or_default())
    }

    fn listar_ventas(&self, limite: usize) -> Resultado<Vec<VentaRegistrada>> {
        Ok(self
            .ventas
            .borrow()
            .iter()
            .rev()
            .take(limite)
            .cloned()
            .collect())
    }

    fn listar(&self, incluir_inactivos: bool) -> Resultado<Vec<ProductoConInventario>> {
        Ok(self
            .productos
            .borrow()
            .values()
            .filter(|fila| incluir_inactivos || fila.producto.esta_activo())
            .cloned()
            .collect())
    }

    fn rastro_de_producto(&self, id: IdProducto) -> Resultado<RastroProducto> {
        Ok(self
            .rastros
            .borrow()
            .get(&id.0)
            .copied()
            .unwrap_or_default())
    }

    /// Borra y retira de verdad, como haría SQLite.
    ///
    /// Se implementa en lugar de un `unreachable!` porque lo que hay que
    /// poder comprobar de estos casos de uso es justo el efecto: quién
    /// desapareció y quién se quedó escondido.
    fn limpiar_catalogo(&self, borrar: &[IdProducto], retirar: &[IdProducto]) -> Resultado<()> {
        self.limpiezas
            .borrow_mut()
            .push((borrar.to_vec(), retirar.to_vec()));

        for id in retirar {
            self.desactivar_producto(id.0);
        }

        for id in borrar {
            self.quitar_producto(id.0);
        }

        Ok(())
    }

    fn configuracion(&self, _clave: &str) -> Resultado<Option<String>> {
        Ok(None)
    }

    fn registrar_venta(&self, confirmada: &VentaConfirmada<'_>) -> Resultado<i64> {
        *self.espera_cobrada.borrow_mut() = Some(confirmada.espera);
        let folio = self.folio.get().saturating_add(1);
        self.folio.set(folio);
        Ok(folio)
    }

    fn crear(&self, _: &Producto, _: &Inventario, _: &[Asiento]) -> Resultado<IdProducto> {
        unreachable!("crear no se usa en estas pruebas")
    }

    fn actualizar_producto(&self, _: &Producto, _: &[CambioDePrecio]) -> Resultado<()> {
        unreachable!("actualizar_producto no se usa en estas pruebas")
    }

    fn historial_precios(&self, _: IdProducto) -> Resultado<Vec<CambioRegistrado>> {
        unreachable!("historial_precios no se usa en estas pruebas")
    }

    /// Guarda el valor nuevo y anota la revaluación, como haría SQLite.
    ///
    /// Aquí sí se implementa de verdad en lugar de con `unreachable!`: lo
    /// que hay que poder comprobar de este caso de uso es justo que el
    /// valor del inventario quedó donde debía y que la anotación salió.
    fn revalorar_producto(
        &self,
        id: IdProducto,
        inventario: &Inventario,
        cambio: &CambioDeCosto,
    ) -> Resultado<()> {
        let mut productos = self.productos.borrow_mut();
        let fila = productos.get_mut(&id.0).expect("producto dado de alta");
        fila.inventario = *inventario;
        drop(productos);

        self.revaluaciones.borrow_mut().push((id, cambio.clone()));
        Ok(())
    }

    fn historial_costos(&self, id: IdProducto) -> Resultado<Vec<CostoRegistrado>> {
        // Las anotadas, de la más reciente a la más antigua, como la base.
        Ok(self
            .revaluaciones
            .borrow()
            .iter()
            .rev()
            .filter(|(producto, _)| *producto == id)
            .enumerate()
            .map(|(posicion, (_, cambio))| CostoRegistrado {
                id: posicion as i64 + 1,
                anterior: cambio.anterior,
                nuevo: cambio.nuevo,
                existencia: cambio.existencia,
                valor_anterior: cambio.valor_anterior,
                valor_nuevo: cambio.valor_nuevo,
                cambiado_en: "2026-10-04 12:00:00".to_owned(),
            })
            .collect())
    }

    fn registrar_movimiento(&self, _: IdProducto, _: &Inventario, _: &Movimiento) -> Resultado<()> {
        unreachable!("registrar_movimiento no se usa en estas pruebas")
    }

    fn kardex(&self, _: IdProducto, _: usize) -> Resultado<Vec<MovimientoRegistrado>> {
        unreachable!("kardex no se usa en estas pruebas")
    }

    fn existe_sku(&self, _: &str) -> Resultado<bool> {
        unreachable!("existe_sku no se usa en estas pruebas")
    }

    fn detalle_venta(&self, _: i64) -> Resultado<Option<DetalleVenta>> {
        unreachable!("detalle_venta no se usa en estas pruebas")
    }

    fn anular_venta(&self, _: &AnulacionConfirmada<'_>) -> Resultado<()> {
        unreachable!("anular_venta no se usa en estas pruebas")
    }

    fn abrir_sesion(&self, _: &SesionCaja) -> Resultado<IdSesion> {
        unreachable!("abrir_sesion no se usa en estas pruebas")
    }

    fn sesion(&self, id: IdSesion) -> Resultado<Option<SesionRegistrada>> {
        let abierta = self
            .sesion
            .borrow()
            .as_ref()
            .filter(|s| s.sesion.id() == Some(id))
            .cloned();

        Ok(abierta.or_else(|| self.cerradas.borrow().get(&id.0).cloned()))
    }

    fn registrar_movimiento_efectivo(&self, _: IdSesion, _: &MovimientoEfectivo) -> Resultado<()> {
        unreachable!("registrar_movimiento_efectivo no se usa en estas pruebas")
    }

    fn movimientos_efectivo(&self, _: IdSesion) -> Resultado<Vec<MovimientoEfectivoRegistrado>> {
        unreachable!("movimientos_efectivo no se usa en estas pruebas")
    }

    fn cerrar_sesion(&self, _: &CierreConfirmado) -> Resultado<()> {
        unreachable!("cerrar_sesion no se usa en estas pruebas")
    }

    fn cierre_de_sesion(&self, _: IdSesion) -> Resultado<Option<CierreConfirmado>> {
        unreachable!("cierre_de_sesion no se usa en estas pruebas")
    }

    fn resumen_periodo(&self, _: &str, _: &str) -> Resultado<TotalesPeriodo> {
        unreachable!("resumen_periodo no se usa en estas pruebas")
    }

    fn ventas_por_dia(&self, _: &str, _: &str) -> Resultado<Vec<VentaDiaria>> {
        unreachable!("ventas_por_dia no se usa en estas pruebas")
    }

    fn ventas_por_hora(&self, _: &str, _: &str) -> Resultado<Vec<VentaHoraria>> {
        unreachable!("ventas_por_hora no se usa en estas pruebas")
    }

    fn ventas_por_metodo(&self, _: &str, _: &str) -> Resultado<Vec<VentaPorMetodo>> {
        unreachable!("ventas_por_metodo no se usa en estas pruebas")
    }

    fn lineas_del_periodo(&self, _: &str, _: &str) -> Resultado<Vec<LineaDelPeriodo>> {
        unreachable!("lineas_del_periodo no se usa en estas pruebas")
    }

    fn borrar_todos_los_datos(&self) -> Resultado<()> {
        unreachable!("borrar_todos_los_datos no se usa en estas pruebas")
    }

    fn guardar_configuracion(&self, _: &str, _: &str) -> Resultado<()> {
        unreachable!("guardar_configuracion no se usa en estas pruebas")
    }
}

/// Renglones vendidos por sesión, en memoria.
///
/// Guarda lo que devolvería la base: las ventas anuladas ya no están. Que
/// la base las excluya se prueba contra SQLite, no aquí.
#[derive(Debug, Default)]
pub(crate) struct VentasDeSesionEnMemoria {
    lineas: RefCell<BTreeMap<i64, Vec<LineaDelPeriodo>>>,
}

impl VentasDeSesionEnMemoria {
    /// Añade un renglón vendido en la sesión indicada.
    pub(crate) fn con_linea(self, sesion: i64, linea: LineaDelPeriodo) -> Self {
        self.lineas
            .borrow_mut()
            .entry(sesion)
            .or_default()
            .push(linea);
        self
    }
}

impl ConsultaVentasDeSesion for VentasDeSesionEnMemoria {
    fn lineas_de_sesion(&self, sesion: IdSesion) -> Resultado<Vec<LineaDelPeriodo>> {
        Ok(self
            .lineas
            .borrow()
            .get(&sesion.0)
            .cloned()
            .unwrap_or_default())
    }
}

/// Saldos y asientos de un turno, en memoria.
///
/// Se dictan los dos lados a mano, y eso es deliberado: aquí se prueba la
/// aritmética del IPV, no la lectura del kárdex. Por eso se puede fabricar
/// un turno **incoherente** —un cierre que no se explica por sus asientos—,
/// que es la única forma de comprobar que la hoja lo denuncia en vez de
/// maquillarlo. Que SQLite lea bien la existencia anterior a la apertura se
/// prueba contra SQLite.
#[derive(Debug, Default)]
pub(crate) struct IpvEnMemoria {
    saldos: RefCell<BTreeMap<i64, Vec<SaldoDeSesion>>>,
    movidos: RefCell<BTreeMap<i64, Vec<MovidoEnSesion>>>,
}

impl IpvEnMemoria {
    /// Lo que un producto tenía al abrir y al cerrar el turno.
    pub(crate) fn con_saldo(self, sesion: i64, producto: i64, inicial: &str, cierre: &str) -> Self {
        self.saldos
            .borrow_mut()
            .entry(sesion)
            .or_default()
            .push(SaldoDeSesion {
                producto: IdProducto(producto),
                inicial: cantidad(inicial),
                cierre: cantidad(cierre),
            });
        self
    }

    /// Un asiento del turno, ya sumado por tipo y por par de estantes.
    ///
    /// `origen` y `destino` se dictan a mano porque son lo que decide a qué
    /// columna va: el mismo traspaso llena o vacía la vitrina según la
    /// dirección.
    pub(crate) fn con_movido(
        self,
        sesion: i64,
        producto: i64,
        tipo: TipoMovimiento,
        (origen, destino): (Option<Ubicacion>, Option<Ubicacion>),
        cantidad_movida: &str,
    ) -> Self {
        self.movidos
            .borrow_mut()
            .entry(sesion)
            .or_default()
            .push(MovidoEnSesion {
                producto: IdProducto(producto),
                tipo,
                origen,
                destino,
                cantidad: cantidad(cantidad_movida),
            });
        self
    }
}

impl ConsultaIpv for IpvEnMemoria {
    fn saldos_de_sesion(&self, sesion: IdSesion) -> Resultado<Vec<SaldoDeSesion>> {
        Ok(self
            .saldos
            .borrow()
            .get(&sesion.0)
            .cloned()
            .unwrap_or_default())
    }

    fn movimientos_de_sesion(&self, sesion: IdSesion) -> Resultado<Vec<MovidoEnSesion>> {
        Ok(self
            .movidos
            .borrow()
            .get(&sesion.0)
            .cloned()
            .unwrap_or_default())
    }
}

/// Los asientos que un consumo entregó al puerto: su identificador y, por
/// cada estante tocado, el producto y el movimiento.
type AsientosDeConsumo = (i64, Vec<(IdProducto, Movimiento)>);

/// El consumo de los dueños, en memoria.
///
/// Guarda lo mismo que guardaría SQLite y calcula el saldo igual: suma de
/// lo congelado menos lo pagado, con el filtro de anulación a los **dos**
/// lados. Si esta cuenta se hiciera de otra forma aquí, las pruebas de los
/// casos de uso dejarían de decir nada sobre lo que pasa de verdad.
#[derive(Debug)]
pub(crate) struct ConsumosEnMemoria {
    cuentas: RefCell<Vec<CuentaConsumo>>,
    /// Cabeceras y renglones, por identificador.
    consumos: RefCell<BTreeMap<i64, (ConsumoRegistrado, Vec<LineaConsumoRegistrada>)>>,
    pagos: RefCell<BTreeMap<i64, PagoConsumoRegistrado>>,
    /// Los descuentos que recibió cada consumo, para comprobar los asientos.
    pub(crate) asientos: RefCell<Vec<AsientosDeConsumo>>,
    /// El catálogo sobre el que escribe, si se le dio uno.
    ///
    /// En SQLite, `registrar_consumo` escribe **él solo** la existencia y el
    /// valor del inventario, porque el candado de la conexión no es
    /// reentrante y no puede delegar en otro puerto. Aquí hace lo mismo, y
    /// no es un detalle: sin esto, una prueba de anulación vería la
    /// existencia intacta y pasaría en verde sin haber comprobado nada.
    catalogo: Option<Rc<ProductosEnMemoria>>,
    siguiente_consumo: Cell<i64>,
    siguiente_pago: Cell<i64>,
}

impl Default for ConsumosEnMemoria {
    fn default() -> Self {
        Self {
            cuentas: RefCell::new(
                ["Ivett", "Claudia", "Inés"]
                    .iter()
                    .enumerate()
                    .map(|(posicion, nombre)| CuentaConsumo {
                        id: posicion as i64 + 1,
                        nombre: (*nombre).to_owned(),
                        activa: true,
                    })
                    .collect(),
            ),
            consumos: RefCell::new(BTreeMap::new()),
            pagos: RefCell::new(BTreeMap::new()),
            asientos: RefCell::new(Vec::new()),
            catalogo: None,
            siguiente_consumo: Cell::new(0),
            siguiente_pago: Cell::new(0),
        }
    }
}

impl ConsumosEnMemoria {
    /// Escribe sobre ese catálogo, como hace el adaptador de SQLite.
    pub(crate) fn sobre(catalogo: Rc<ProductosEnMemoria>) -> Self {
        Self {
            catalogo: Some(catalogo),
            ..Self::default()
        }
    }

    /// Lleva al catálogo los saldos que el caso de uso calculó.
    fn aplicar(&self, descuentos: &[DescuentoConsumo]) {
        if let Some(catalogo) = &self.catalogo {
            for descuento in descuentos {
                catalogo.fijar_inventario(descuento.producto, descuento.inventario);
            }
        }
    }

    /// Retira una cuenta, como si se hubiera desactivado desde la pantalla.
    pub(crate) fn desactivar_cuenta(&self, id: i64) {
        for cuenta in self.cuentas.borrow_mut().iter_mut() {
            if cuenta.id == id {
                cuenta.activa = false;
            }
        }
    }

    /// El saldo de una cuenta, ya calculado.
    pub(crate) fn saldo_de(&self, cuenta: i64) -> Dinero {
        self.saldos()
            .expect("saldos")
            .into_iter()
            .find(|saldo| saldo.cuenta == cuenta)
            .map_or(Dinero::CERO, |saldo| saldo.saldo)
    }
}

impl RepositorioConsumo for ConsumosEnMemoria {
    fn cuentas(&self, incluir_inactivas: bool) -> Resultado<Vec<CuentaConsumo>> {
        Ok(self
            .cuentas
            .borrow()
            .iter()
            .filter(|cuenta| incluir_inactivas || cuenta.activa)
            .cloned()
            .collect())
    }

    fn registrar_consumo(&self, confirmado: &ConsumoConfirmado<'_>) -> Resultado<i64> {
        let id = self.siguiente_consumo.get().saturating_add(1);
        self.siguiente_consumo.set(id);

        let nombre_cuenta = self
            .cuentas
            .borrow()
            .iter()
            .find(|cuenta| cuenta.id == confirmado.cuenta)
            .map_or_else(String::new, |cuenta| cuenta.nombre.clone());

        let lineas = confirmado
            .lineas
            .iter()
            .map(|linea| LineaConsumoRegistrada {
                producto: linea.producto,
                nombre_producto: linea.nombre_producto.clone(),
                nombre_presentacion: linea.nombre_presentacion.clone(),
                unidad_base: UnidadBase::Unidad,
                origen: linea.origen,
                cantidad: linea.cantidad,
                factor: linea.factor,
                costo_unitario: linea.costo_unitario,
            })
            .collect();

        self.consumos.borrow_mut().insert(
            id,
            (
                ConsumoRegistrado {
                    id,
                    cuenta: confirmado.cuenta,
                    nombre_cuenta,
                    tomado_por: confirmado.tomado_por.to_owned(),
                    total_costo: confirmado.total_costo,
                    ocurrido_en: "2026-10-14 12:00:00".to_owned(),
                    anulado_en: None,
                    motivo_anulacion: None,
                },
                lineas,
            ),
        );

        self.aplicar(confirmado.descuentos);

        self.asientos.borrow_mut().push((
            id,
            confirmado
                .descuentos
                .iter()
                .map(|descuento| (descuento.producto, descuento.movimiento.clone()))
                .collect(),
        ));

        Ok(id)
    }

    fn consumo(
        &self,
        id: i64,
    ) -> Resultado<Option<(ConsumoRegistrado, Vec<LineaConsumoRegistrada>)>> {
        Ok(self.consumos.borrow().get(&id).cloned())
    }

    fn anular_consumo(&self, anulacion: &AnulacionConsumo<'_>) -> Resultado<()> {
        if let Some((consumo, _)) = self.consumos.borrow_mut().get_mut(&anulacion.consumo) {
            consumo.anulado_en = Some("2026-10-14 13:00:00".to_owned());
            consumo.motivo_anulacion = Some(anulacion.motivo.clone());
        }

        self.aplicar(anulacion.reversas);

        self.asientos.borrow_mut().push((
            anulacion.consumo,
            anulacion
                .reversas
                .iter()
                .map(|reversa| (reversa.producto, reversa.movimiento.clone()))
                .collect(),
        ));

        Ok(())
    }

    fn registrar_pago(&self, cuenta: i64, importe: Dinero, nota: Option<&str>) -> Resultado<i64> {
        let id = self.siguiente_pago.get().saturating_add(1);
        self.siguiente_pago.set(id);

        self.pagos.borrow_mut().insert(
            id,
            PagoConsumoRegistrado {
                id,
                cuenta,
                importe,
                nota: nota.map(str::to_owned),
                ocurrido_en: "2026-10-14 12:30:00".to_owned(),
                anulado_en: None,
                motivo_anulacion: None,
            },
        );

        Ok(id)
    }

    fn pago(&self, id: i64) -> Resultado<Option<PagoConsumoRegistrado>> {
        Ok(self.pagos.borrow().get(&id).cloned())
    }

    fn anular_pago(&self, id: i64, motivo: &str) -> Resultado<()> {
        if let Some(pago) = self.pagos.borrow_mut().get_mut(&id) {
            pago.anulado_en = Some("2026-10-14 13:30:00".to_owned());
            pago.motivo_anulacion = Some(motivo.to_owned());
        }
        Ok(())
    }

    fn saldos(&self) -> Resultado<Vec<SaldoCuenta>> {
        let consumos = self.consumos.borrow();
        let pagos = self.pagos.borrow();

        self.cuentas
            .borrow()
            .iter()
            .map(|cuenta| {
                // Los anulados no cuentan, ni de un lado ni del otro.
                let mut consumido = Dinero::CERO;
                for (registrado, _) in consumos.values() {
                    if registrado.cuenta == cuenta.id && registrado.anulado_en.is_none() {
                        consumido = consumido.sumar(registrado.total_costo)?;
                    }
                }

                let mut pagado = Dinero::CERO;
                for pago in pagos.values() {
                    if pago.cuenta == cuenta.id && pago.anulado_en.is_none() {
                        pagado = pagado.sumar(pago.importe)?;
                    }
                }

                Ok(SaldoCuenta {
                    cuenta: cuenta.id,
                    nombre: cuenta.nombre.clone(),
                    activa: cuenta.activa,
                    consumido,
                    pagado,
                    // Sin recortar: si queda negativo, la cuenta tiene
                    // dinero a favor y eso es un estado legítimo.
                    saldo: consumido.restar(pagado)?,
                })
            })
            .collect()
    }

    fn consumos_de_cuenta(&self, cuenta: i64, limite: usize) -> Resultado<Vec<ConsumoRegistrado>> {
        Ok(self
            .consumos
            .borrow()
            .values()
            .rev()
            .filter(|(registrado, _)| registrado.cuenta == cuenta)
            .take(limite)
            .map(|(registrado, _)| registrado.clone())
            .collect())
    }

    fn pagos_de_cuenta(&self, cuenta: i64, limite: usize) -> Resultado<Vec<PagoConsumoRegistrado>> {
        Ok(self
            .pagos
            .borrow()
            .values()
            .rev()
            .filter(|pago| pago.cuenta == cuenta)
            .take(limite)
            .cloned()
            .collect())
    }

    fn lineas_consumidas(
        &self,
        cuenta: Option<i64>,
        _desde: Option<&str>,
        _hasta: Option<&str>,
    ) -> Resultado<Vec<LineaConsumoRegistrada>> {
        let mut lineas = Vec::new();

        for (registrado, renglones) in self.consumos.borrow().values() {
            // Lo anulado no cuenta: la mercancía volvió.
            if registrado.anulado_en.is_some() {
                continue;
            }
            if cuenta.is_some_and(|pedida| pedida != registrado.cuenta) {
                continue;
            }

            lineas.extend(renglones.iter().cloned());
        }

        Ok(lineas)
    }
}

/// Ventas en espera en memoria.
#[derive(Debug, Default)]
pub(crate) struct EsperasEnMemoria {
    esperas: RefCell<BTreeMap<i64, EsperaRegistrada>>,
    siguiente: Cell<i64>,
}

impl RepositorioVentaEnEspera for EsperasEnMemoria {
    fn guardar(&self, espera: &VentaEnEspera) -> Resultado<IdVentaEnEspera> {
        let id = IdVentaEnEspera(self.siguiente.get().saturating_add(1));
        self.siguiente.set(id.0);
        self.esperas.borrow_mut().insert(
            id.0,
            EsperaRegistrada {
                id,
                espera: espera.clone(),
                creada_en: "2026-09-27 10:15:00".to_owned(),
            },
        );
        Ok(id)
    }

    fn listar(&self) -> Resultado<Vec<EsperaResumida>> {
        Ok(self
            .esperas
            .borrow()
            .values()
            .map(|registrada| EsperaResumida {
                id: registrada.id,
                nota: registrada.espera.nota().map(str::to_owned),
                creada_en: registrada.creada_en.clone(),
                cuantas_lineas: i64::try_from(registrada.espera.lineas().len()).unwrap_or(i64::MAX),
            })
            .collect())
    }

    fn obtener(&self, id: IdVentaEnEspera) -> Resultado<Option<EsperaRegistrada>> {
        Ok(self.esperas.borrow().get(&id.0).cloned())
    }

    fn eliminar(&self, id: IdVentaEnEspera) -> Resultado<bool> {
        Ok(self.esperas.borrow_mut().remove(&id.0).is_some())
    }
}
