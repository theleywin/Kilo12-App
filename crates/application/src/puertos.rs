//! Puertos: lo que la aplicación necesita del mundo exterior.
//!
//! Son interfaces, no implementaciones. La capa de aplicación declara aquí
//! qué le hace falta —guardar un producto, listarlo— y otra capa decide
//! cómo. Esa inversión es lo que mantiene las flechas apuntando hacia
//! adentro (DT-5) y lo que permitirá cambiar SQLite, o añadir un lector de
//! código de barras, sin tocar ninguna regla de negocio.

use domain::{
    Cantidad, Cobro, Comision, Dinero, Existencias, IdPresentacion, IdProducto, IdSesion,
    Inventario, MetodoPago, Movimiento, MovimientoEfectivo, Producto, ResumenCierre, SesionCaja,
    Venta,
};

use crate::error::Resultado;

/// Un producto junto con su existencia y su valor.
///
/// Viajan juntos porque separados mienten: la existencia sin el valor no
/// permite saber el costo, y el costo es lo que decide si una venta deja
/// ganancia (RF-COS-02).
#[derive(Debug, Clone)]
pub struct ProductoConInventario {
    pub producto: Producto,
    pub inventario: Inventario,
}

/// Un movimiento tal como quedó registrado, con su fecha y su saldo.
///
/// El saldo resultante se guardó en su momento y no se recalcula: es lo que
/// permite leer el kárdex de arriba abajo sin rehacer la aritmética de
/// todos los movimientos anteriores (RF-INV-05).
#[derive(Debug, Clone)]
pub struct MovimientoRegistrado {
    pub id: i64,
    pub movimiento: Movimiento,
    /// Fecha y hora en que ocurrió, como la guarda la base de datos.
    pub ocurrido_en: String,
    /// Existencia que quedó justo después de este movimiento.
    pub resultante: Existencias,
}

/// Un movimiento junto con el saldo que dejó.
///
/// Van emparejados porque el saldo depende del orden: si un producto nace
/// con mercancía en bodega y en vitrina, cada asiento deja una foto
/// distinta, y esa foto es la que después se lee en el kárdex.
#[derive(Debug, Clone)]
pub struct Asiento {
    pub movimiento: Movimiento,
    pub resultante: Existencias,
}

/// Un cambio de precio, para el historial (RF-PRE-04).
///
/// El precio de ayer no se deduce del de hoy: si no se anota cuando cambia,
/// se pierde, y con él la posibilidad de saber si una venta vieja dejaba
/// ganancia.
#[derive(Debug, Clone)]
pub struct CambioDePrecio {
    pub presentacion: IdPresentacion,
    pub anterior: Dinero,
    pub nuevo: Dinero,
}

/// Un cambio de precio ya registrado, con su fecha.
#[derive(Debug, Clone)]
pub struct CambioRegistrado {
    pub id: i64,
    pub presentacion: IdPresentacion,
    pub anterior: Dinero,
    pub nuevo: Dinero,
    pub cambiado_en: String,
}

/// Lo que una venta le quita a un producto.
///
/// Viajan juntos el saldo que queda y el asiento que lo explica: escribir
/// uno sin el otro deja el inventario sin coartada (RF-INV-03).
#[derive(Debug, Clone)]
pub struct DescuentoVenta {
    pub producto: IdProducto,
    pub inventario: Inventario,
    pub movimiento: Movimiento,
}

/// Una venta lista para guardar, con sus cuentas ya hechas.
///
/// Los totales llegan calculados: la infraestructura traduce entre el
/// dominio y las filas de la base, no hace aritmética de dinero.
#[derive(Debug)]
pub struct VentaConfirmada<'a> {
    pub venta: &'a Venta,
    pub cobro: &'a Cobro,
    pub total: Dinero,
    pub costo_total: Dinero,
    /// Siempre en pesos (R-11).
    pub vuelto: Dinero,
    pub descuentos: &'a [DescuentoVenta],
    /// Sesión de caja a la que pertenece (RF-CAJ-02).
    pub sesion: IdSesion,
}

/// Una venta ya cobrada, tal como quedó guardada.
///
/// Nada de esto se recalcula al leerlo. El total y el costo se congelaron
/// al cobrar, y ahí siguen: si mañana sube el costo del arroz, la venta de
/// hoy tiene que seguir diciendo la ganancia que dejó hoy (RF-VTA-13).
#[derive(Debug, Clone)]
pub struct VentaRegistrada {
    pub id: i64,
    pub folio: i64,
    pub total: Dinero,
    pub costo_total: Dinero,
    /// Siempre en pesos (R-11).
    pub vuelto: Dinero,
    pub ocurrido_en: String,
    /// Sesión de caja a la que pertenece. Vacía en las ventas anteriores a
    /// que existiera la caja.
    pub sesion: Option<i64>,
    pub anulada: bool,
    pub motivo_anulacion: Option<String>,
}

/// Un renglón de una venta ya cobrada.
///
/// Los nombres viajan copiados, no por referencia al producto: si el
/// producto se renombra o se borra, el recibo de ayer no puede cambiar.
#[derive(Debug, Clone)]
pub struct LineaRegistrada {
    pub producto: IdProducto,
    pub nombre_producto: String,
    pub nombre_presentacion: String,
    pub cantidad: Cantidad,
    /// Unidades base que se lleva cada unidad de la presentación.
    pub factor: Cantidad,
    pub precio: Dinero,
    pub costo_unitario: Dinero,
}

/// Una de las formas en que se pagó una venta.
#[derive(Debug, Clone)]
pub struct PagoRegistrado {
    pub metodo: MetodoPago,
    /// Lo que entregó el cliente, en la moneda del método.
    pub entregado: Dinero,
    /// Tasa aplicada, congelada. Solo en los pagos en dólares.
    pub tasa: Option<Dinero>,
    pub equivalente_cup: Dinero,
}

/// Una venta con todo lo que hizo falta para cobrarla.
#[derive(Debug, Clone)]
pub struct DetalleVenta {
    pub venta: VentaRegistrada,
    pub lineas: Vec<LineaRegistrada>,
    pub pagos: Vec<PagoRegistrado>,
}

/// Lo vendido en una jornada.
///
/// El corte del día lo hace la base con su propio reloj: preguntar «¿qué
/// llevo hoy?» desde Rust obligaría a saber en qué huso está la tienda, y
/// la tienda está donde está la máquina.
#[derive(Debug, Clone, Default)]
pub struct ResumenDia {
    pub cuantas: i64,
    pub total: Dinero,
    pub costo_total: Dinero,
}

/// Sumas de una sesión, tal como las calcula la base.
///
/// Van juntas porque se leen de una vez y porque separadas invitan al error
/// que corrige [`domain::TotalesCaja`]: `efectivo_cup_neto` son billetes y
/// `total_vendido` es venta. No son la misma cifra.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AcumuladoSesion {
    /// Ventas confirmadas y no anuladas de la sesión.
    pub cuantas_ventas: i64,
    pub total_vendido: Dinero,
    /// Costo congelado de lo vendido.
    pub costo_vendido: Dinero,
    pub transferencia: Dinero,
    /// Dólares recibidos, en dólares.
    pub efectivo_usd: Dinero,
    /// Equivalente en pesos de esos dólares, con la tasa de cada venta.
    pub efectivo_usd_en_cup: Dinero,
    /// Billetes de peso que entraron menos el vuelto devuelto.
    pub efectivo_cup_neto: Dinero,
    /// Entradas de efectivo ajenas a la venta.
    pub entradas: Dinero,
    /// Salidas de efectivo ajenas a la venta.
    pub salidas: Dinero,
    /// Costo de la mercancía dada de baja por merma durante la sesión.
    pub merma_costo: Dinero,
}

/// Un movimiento de efectivo ya registrado.
#[derive(Debug, Clone)]
pub struct MovimientoEfectivoRegistrado {
    pub id: i64,
    pub movimiento: MovimientoEfectivo,
    pub ocurrido_en: String,
}

/// Una sesión de caja tal como está guardada.
#[derive(Debug, Clone)]
pub struct SesionRegistrada {
    pub sesion: SesionCaja,
    pub abierta_en: String,
    pub cerrada_en: Option<String>,
}

/// El cierre de una sesión, listo para guardarse congelado.
///
/// Todo llega calculado. La infraestructura no hace aritmética de dinero:
/// escribe lo que el dominio decidió, y a partir de ahí es inmutable
/// (RF-CAJ-08).
#[derive(Debug, Clone)]
pub struct CierreConfirmado {
    pub sesion: IdSesion,
    pub resumen: ResumenCierre,
    pub costo_vendido: Dinero,
    pub merma_costo: Dinero,
    pub comision: Comision,
}

/// Datos con que se anula una venta (RF-VTA-15).
#[derive(Debug, Clone)]
pub struct AnulacionConfirmada<'a> {
    pub venta: i64,
    pub motivo: String,
    /// Devoluciones a vitrina, con su asiento de reversa.
    pub reversas: &'a [DescuentoVenta],
}

/// Totales de un periodo, sumados por la base.
///
/// Solo suma columnas ya guardadas: `total` y `costo_total` son enteros
/// exactos y sumarlos no pierde nada. En cuanto haga falta multiplicar
/// —precio por cantidad— la cuenta sube a Rust, donde están los tipos que
/// saben de escalas.
#[derive(Debug, Clone, Default)]
pub struct TotalesPeriodo {
    pub cuantas: i64,
    pub venta: Dinero,
    pub costo: Dinero,
}

/// Lo vendido en un día.
#[derive(Debug, Clone)]
pub struct VentaDiaria {
    /// `YYYY-MM-DD`.
    pub fecha: String,
    pub venta: Dinero,
    pub costo: Dinero,
    pub cuantas: i64,
}

/// Lo vendido en una hora del día, acumulado en todo el periodo.
#[derive(Debug, Clone)]
pub struct VentaHoraria {
    /// Hora en formato 24 h, de 0 a 23.
    pub hora: i64,
    pub venta: Dinero,
    pub cuantas: i64,
}

/// Lo cobrado por una forma de pago.
#[derive(Debug, Clone)]
pub struct VentaPorMetodo {
    pub metodo: MetodoPago,
    /// Lo entregado en la moneda del método.
    pub entregado: Dinero,
    /// Su equivalente en pesos.
    pub equivalente: Dinero,
}

/// Un renglón vendido, sin agregar.
///
/// Se devuelven en crudo a propósito: agrupar por producto exige
/// multiplicar precio por cantidad, y esa cuenta no se hace en SQL.
#[derive(Debug, Clone)]
pub struct LineaDelPeriodo {
    pub producto: IdProducto,
    pub nombre_producto: String,
    pub nombre_presentacion: String,
    pub cantidad: Cantidad,
    pub factor: Cantidad,
    pub precio: Dinero,
    pub costo_unitario: Dinero,
}

/// Acceso al catálogo de productos y a su inventario.
pub trait RepositorioProducto {
    /// Guarda un producto nuevo con su existencia de apertura y devuelve el
    /// identificador asignado.
    ///
    /// El inventario y sus movimientos entran en la misma operación porque
    /// dar de alta un producto y meter la mercancía que ya tienes es, para
    /// quien lo usa, un solo acto. Que por dentro sean varias cosas no es
    /// asunto suyo.
    fn crear(
        &self,
        producto: &Producto,
        inventario: &Inventario,
        asientos: &[Asiento],
    ) -> Resultado<IdProducto>;

    /// Recupera un producto con sus presentaciones y su existencia.
    fn obtener(&self, id: IdProducto) -> Resultado<Option<ProductoConInventario>>;

    /// Guarda los datos editables de un producto y sus presentaciones.
    ///
    /// No toca la existencia ni el valor: eso solo cambia con un
    /// movimiento, nunca editando una ficha.
    ///
    /// Los cambios de precio viajan aparte porque el historial necesita
    /// saber de dónde venía cada uno, y eso el producto ya no lo recuerda:
    /// dentro de él solo está el precio nuevo.
    fn actualizar_producto(&self, producto: &Producto, cambios: &[CambioDePrecio])
        -> Resultado<()>;

    /// Devuelve el historial de precios de un producto (RF-PRE-04).
    fn historial_precios(&self, id: IdProducto) -> Resultado<Vec<CambioRegistrado>>;

    /// Lista los productos del catálogo.
    ///
    /// `incluir_inactivos` decide si aparecen los productos desactivados,
    /// que siguen existiendo por su historial (RF-CAT-06).
    fn listar(&self, incluir_inactivos: bool) -> Resultado<Vec<ProductoConInventario>>;

    /// Asienta un movimiento y deja la existencia como quedó después.
    ///
    /// Las dos cosas van juntas y son indivisibles: un saldo sin su asiento
    /// es un número que nadie puede explicar, y un asiento sin su saldo es
    /// un historial que no cuadra con la realidad (RNF-5).
    fn registrar_movimiento(
        &self,
        id: IdProducto,
        inventario: &Inventario,
        movimiento: &Movimiento,
    ) -> Resultado<()>;

    /// Devuelve el historial de un producto, del más reciente al más
    /// antiguo (RF-INV-05).
    fn kardex(&self, id: IdProducto, limite: usize) -> Resultado<Vec<MovimientoRegistrado>>;

    /// Indica si ya existe un producto con ese SKU (RF-CAT-02).
    fn existe_sku(&self, sku: &str) -> Resultado<bool>;

    /// Guarda una venta entera y devuelve su folio (RF-VTA-17).
    ///
    /// Líneas, pagos, saldos y asientos entran en la misma operación: media
    /// venta registrada es peor que ninguna (RNF-5).
    fn registrar_venta(&self, confirmada: &VentaConfirmada<'_>) -> Resultado<i64>;

    /// Lista las ventas cobradas, de la más reciente a la más antigua.
    fn listar_ventas(&self, limite: usize) -> Resultado<Vec<VentaRegistrada>>;

    /// Devuelve una venta con sus líneas y sus pagos.
    fn detalle_venta(&self, id: i64) -> Resultado<Option<DetalleVenta>>;

    /// Cuánto se lleva vendido hoy, según el reloj de la máquina.
    fn resumen_de_hoy(&self) -> Resultado<ResumenDia>;

    /// Anula una venta y devuelve la mercancía a vitrina (RF-VTA-15).
    ///
    /// La reversa del inventario y la marca de anulada entran en la misma
    /// operación: una venta marcada sin devolver la mercancía deja la
    /// vitrina mintiendo.
    fn anular_venta(&self, anulacion: &AnulacionConfirmada<'_>) -> Resultado<()>;

    // ----------------------------------------------------- caja (RF-CAJ)

    /// Abre una sesión de caja y devuelve su identificador.
    ///
    /// Falla si ya hay otra abierta: la unicidad la impone un índice de la
    /// base, no una comprobación previa que dos procesos podrían saltarse a
    /// la vez.
    fn abrir_sesion(&self, sesion: &SesionCaja) -> Resultado<IdSesion>;

    /// Devuelve la sesión abierta, si la hay.
    fn sesion_abierta(&self) -> Resultado<Option<SesionRegistrada>>;

    /// Devuelve una sesión por su identificador.
    fn sesion(&self, id: IdSesion) -> Resultado<Option<SesionRegistrada>>;

    /// Suma todo lo que lleva una sesión.
    fn acumulado_de_sesion(&self, id: IdSesion) -> Resultado<AcumuladoSesion>;

    /// Anota una entrada o salida de efectivo (RF-CAJ-03).
    fn registrar_movimiento_efectivo(
        &self,
        sesion: IdSesion,
        movimiento: &MovimientoEfectivo,
    ) -> Resultado<()>;

    /// Movimientos de efectivo de una sesión, del más reciente al más viejo.
    fn movimientos_efectivo(
        &self,
        sesion: IdSesion,
    ) -> Resultado<Vec<MovimientoEfectivoRegistrado>>;

    /// Cierra la sesión guardando sus cifras congeladas (RF-CAJ-08).
    fn cerrar_sesion(&self, cierre: &CierreConfirmado) -> Resultado<()>;

    /// Lista las sesiones cerradas, de la más reciente a la más antigua.
    fn listar_sesiones(&self, limite: usize) -> Resultado<Vec<SesionRegistrada>>;

    /// Devuelve el cierre congelado de una sesión ya cerrada.
    fn cierre_de_sesion(&self, id: IdSesion) -> Resultado<Option<CierreConfirmado>>;

    // -------------------------------------------- informes (RF-EST)

    /// Totales de las ventas de un periodo, ambos extremos incluidos.
    ///
    /// Las fechas llegan como `YYYY-MM-DD` y se comparan contra la fecha
    /// local de la venta. Las anuladas nunca cuentan.
    fn resumen_periodo(&self, desde: &str, hasta: &str) -> Resultado<TotalesPeriodo>;

    /// Venta y costo agrupados por día.
    fn ventas_por_dia(&self, desde: &str, hasta: &str) -> Resultado<Vec<VentaDiaria>>;

    /// Venta agrupada por hora del día, acumulando todo el periodo.
    fn ventas_por_hora(&self, desde: &str, hasta: &str) -> Resultado<Vec<VentaHoraria>>;

    /// Lo cobrado por cada forma de pago en el periodo.
    fn ventas_por_metodo(&self, desde: &str, hasta: &str) -> Resultado<Vec<VentaPorMetodo>>;

    /// Renglones vendidos en el periodo, sin agrupar.
    fn lineas_del_periodo(&self, desde: &str, hasta: &str) -> Resultado<Vec<LineaDelPeriodo>>;

    /// Vacía todas las tablas de datos y deja el esquema en pie.
    ///
    /// Es un borrón completo: catálogo, existencias, kárdex, ventas, cajas
    /// y ajustes. No hay vuelta atrás y no la debe haber: una papelera
    /// daría la sensación de que esto se puede deshacer.
    fn borrar_todos_los_datos(&self) -> Resultado<()>;

    /// Lee un ajuste del negocio, como la tasa de cambio vigente.
    fn configuracion(&self, clave: &str) -> Resultado<Option<String>>;

    /// Guarda un ajuste del negocio.
    fn guardar_configuracion(&self, clave: &str, valor: &str) -> Resultado<()>;
}
