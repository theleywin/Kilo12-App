//! Dobles en memoria de los puertos, solo para las pruebas.
//!
//! La estrategia de pruebas (§6 del diseño técnico) prueba los casos de uso
//! con los puertos sustituidos por implementaciones en memoria. Aquí viven
//! las que hacen falta, sin SQLite: si un caso de uso depende de algo que
//! aquí no está, la prueba lo dice con un `unreachable!` en vez de fingir.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use domain::{
    Cantidad, Dinero, EstadoSesion, Existencias, IdPresentacion, IdProducto, IdSesion,
    IdVentaEnEspera, Inventario, Movimiento, MovimientoEfectivo, Presentacion, Producto,
    SesionCaja, UnidadBase, VentaEnEspera,
};

use crate::error::Resultado;
use crate::puertos::{
    AcumuladoSesion, AnulacionConfirmada, Asiento, CambioDePrecio, CambioRegistrado,
    CierreConfirmado, ConsultaVentasDeSesion, DetalleVenta, EsperaRegistrada, EsperaResumida,
    LineaDelPeriodo, MovimientoEfectivoRegistrado, MovimientoRegistrado, ProductoConInventario,
    RepositorioProducto, RepositorioVentaEnEspera, SesionRegistrada, TotalesPeriodo,
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

    fn sesion(&self, _: IdSesion) -> Resultado<Option<SesionRegistrada>> {
        unreachable!("sesion no se usa en estas pruebas")
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
