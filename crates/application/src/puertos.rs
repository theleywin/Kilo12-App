//! Puertos: lo que la aplicación necesita del mundo exterior.
//!
//! Son interfaces, no implementaciones. La capa de aplicación declara aquí
//! qué le hace falta —guardar un producto, listarlo— y otra capa decide
//! cómo. Esa inversión es lo que mantiene las flechas apuntando hacia
//! adentro (DT-5) y lo que permitirá cambiar SQLite, o añadir un lector de
//! código de barras, sin tocar ninguna regla de negocio.

use domain::{
    Cantidad, Cobro, Comision, Dinero, Existencias, IdPresentacion, IdProducto, IdSesion,
    IdVentaEnEspera, Inventario, MetodoPago, Movimiento, MovimientoEfectivo, Producto,
    ResumenCierre, SesionCaja, Ubicacion, UnidadBase, Venta, VentaEnEspera,
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

/// Una revaluación de inventario, lista para anotarse.
///
/// El costo no se almacena —se deriva del valor entre la existencia
/// (RF-COS-02)—, así que fijarlo a mano mueve el valor del inventario. Eso
/// crea o destruye valor que nadie compró, y por tanto se anota.
///
/// Viajan las cifras de las dos puntas y la existencia del momento porque
/// **ninguna se puede reconstruir después**: la existencia cambia con cada
/// venta, y el valor viejo lo pisa esta misma operación.
#[derive(Debug, Clone)]
pub struct CambioDeCosto {
    pub anterior: Dinero,
    pub nuevo: Dinero,
    /// Existencia total sobre la que se repartió el valor.
    pub existencia: Cantidad,
    pub valor_anterior: Dinero,
    pub valor_nuevo: Dinero,
}

/// Una revaluación ya registrada, con su fecha.
#[derive(Debug, Clone)]
pub struct CostoRegistrado {
    pub id: i64,
    pub anterior: Dinero,
    pub nuevo: Dinero,
    pub existencia: Cantidad,
    pub valor_anterior: Dinero,
    pub valor_nuevo: Dinero,
    pub cambiado_en: String,
}

/// Lo que un producto ha dejado escrito fuera de su propia ficha.
///
/// Es lo que decide si se puede borrar de verdad o solo retirar de la venta
/// (RF-CAT-07). Un producto sin rastro no le debe nada a nadie: borrarlo no
/// deja un hueco en ninguna parte. Uno que vendió tiene renglones de venta
/// que lo apuntan y un kárdex que explica su existencia, y eso no se tira.
///
/// Las tres cifras van juntas porque las tres impiden lo mismo por motivos
/// distintos, y conviene poder decirle a quien mira la pantalla **cuál** de
/// los tres le está frenando.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RastroProducto {
    /// Renglones de ventas ya cobradas que lo llevan.
    pub ventas: i64,
    /// Asientos del kárdex a su nombre.
    pub movimientos: i64,
    /// Renglones de ventas en espera: ventas empezadas y sin cobrar.
    pub esperas: i64,
}

impl RastroProducto {
    /// Nunca se movió: no hay nada que conservar de él.
    pub const fn esta_limpio(&self) -> bool {
        self.ventas == 0 && self.movimientos == 0 && self.esperas == 0
    }
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
    /// Venta en espera que este cobro liquida, si viene de una (RF-VTA-14).
    ///
    /// Se borra en la MISMA operación que registra la venta. Si ya no
    /// existe, no se registra nada: cobrar dos veces la misma espera sería
    /// una venta doble.
    pub espera: Option<IdVentaEnEspera>,
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
    /// Venta a la que pertenece: dos renglones de la misma venta cuentan
    /// como una sola vez vendido.
    pub venta: i64,
    pub producto: IdProducto,
    pub presentacion: IdPresentacion,
    /// Nombres congelados al cobrar, como en el recibo.
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

    /// Fija el valor del inventario de un producto y anota la revaluación.
    ///
    /// Las dos cosas van juntas y son indivisibles, por el mismo motivo que
    /// un saldo va con su asiento: un valor de inventario que cambió sin
    /// dejar rastro es dinero que apareció y nadie puede explicar.
    ///
    /// **No toca la existencia.** Una revaluación no mueve mercancía, así
    /// que tampoco escribe en el kárdex.
    fn revalorar_producto(
        &self,
        id: IdProducto,
        inventario: &Inventario,
        cambio: &CambioDeCosto,
    ) -> Resultado<()>;

    /// Devuelve las revaluaciones de un producto, de la más reciente a la
    /// más antigua.
    fn historial_costos(&self, id: IdProducto) -> Resultado<Vec<CostoRegistrado>>;

    /// Lista los productos del catálogo.
    ///
    /// `incluir_inactivos` decide si aparecen los productos desactivados,
    /// que siguen existiendo por su historial (RF-CAT-06).
    fn listar(&self, incluir_inactivos: bool) -> Resultado<Vec<ProductoConInventario>>;

    /// Cuenta lo que un producto ha dejado escrito fuera de su ficha.
    fn rastro_de_producto(&self, id: IdProducto) -> Resultado<RastroProducto>;

    /// Quita del catálogo: borra unos productos y retira otros.
    ///
    /// Las dos listas entran en la **misma** operación: una limpieza que se
    /// aplica a medias deja el catálogo en un estado que nadie pidió, y
    /// quien pulsó el botón no tiene forma de saber por dónde se quedó
    /// (RNF-5).
    ///
    /// Borrar es definitivo y solo vale para lo que nunca se movió. Que eso
    /// se cumpla **no depende de esta llamada**: las claves foráneas de
    /// `venta_linea`, `movimiento` y `venta_en_espera_linea` apuntan al
    /// producto, así que la base rechaza el borrado de cualquiera que tenga
    /// rastro. Es RF-CAT-07 impuesto por el esquema y no por un `if` que
    /// alguien puede olvidarse de escribir.
    ///
    /// Retirar es `activo = 0`: el producto desaparece de Vender, Vitrina,
    /// Almacén e informes, y conserva entero su historial (RF-CAT-06).
    fn limpiar_catalogo(&self, borrar: &[IdProducto], retirar: &[IdProducto]) -> Resultado<()>;

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

/// Lo vendido en una sesión de caja, renglón a renglón (RF-VTA-16).
///
/// Es un puerto de solo lectura y aparte de [`RepositorioProducto`] por la
/// misma razón que [`RepositorioVentaEnEspera`]: quien agrupa lo vendido no
/// necesita saber dar de alta un producto, y su doble de pruebas no debería
/// tener que fingir cuarenta métodos para usar uno.
pub trait ConsultaVentasDeSesion {
    /// Renglones de las ventas de una sesión, en el orden en que se
    /// cobraron. Las anuladas nunca aparecen: la mercancía volvió y el
    /// dinero se devolvió.
    fn lineas_de_sesion(&self, sesion: IdSesion) -> Resultado<Vec<LineaDelPeriodo>>;
}

// ================================================ el consumo (RF-CON)

/// Una cuenta de consumo tal como está guardada.
///
/// Son los dueños del local. Viven en una tabla y no en un `CHECK` con tres
/// nombres: añadir o retirar una persona no puede exigir una migración,
/// porque las personas no son esquema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuentaConsumo {
    pub id: i64,
    pub nombre: String,
    pub activa: bool,
}

/// Un renglón de consumo listo para guardar, con todo congelado.
///
/// Los nombres viajan copiados y el costo congelado, por el mismo motivo
/// que en una venta (RF-VTA-13): la deuda que se acuerda hoy no puede
/// moverse porque mañana suba el arroz.
///
/// `origen` va por línea: se puede coger el aceite de la vitrina y el arroz
/// de la bodega en el mismo acto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaConsumo {
    pub producto: IdProducto,
    pub presentacion: IdPresentacion,
    pub nombre_producto: String,
    pub nombre_presentacion: String,
    pub origen: Ubicacion,
    /// Cuántas presentaciones, no unidades base.
    pub cantidad: Cantidad,
    /// Unidades base que lleva cada presentación.
    pub factor: Cantidad,
    pub costo_unitario: Dinero,
}

/// Lo que un consumo le quita a un producto en una ubicación.
///
/// Mismo papel que [`DescuentoVenta`]: el saldo que queda y el asiento que
/// lo explica, juntos, porque escribir uno sin el otro deja el inventario
/// sin coartada (RF-INV-03).
#[derive(Debug, Clone)]
pub struct DescuentoConsumo {
    pub producto: IdProducto,
    pub inventario: Inventario,
    pub movimiento: Movimiento,
}

/// Un consumo listo para guardar, con sus cuentas hechas.
#[derive(Debug)]
pub struct ConsumoConfirmado<'a> {
    pub cuenta: i64,
    /// Quien se llevó la mercancía. Puede no ser la dueña de la cuenta.
    pub tomado_por: &'a str,
    /// Deuda que genera, al costo. Llega calculada: la infraestructura no
    /// hace aritmética de dinero.
    pub total_costo: Dinero,
    pub lineas: &'a [LineaConsumo],
    /// Un asiento por `(producto, origen)`, no uno por línea.
    pub descuentos: &'a [DescuentoConsumo],
}

/// Datos con que se anula un consumo (RF-CON-16).
#[derive(Debug)]
pub struct AnulacionConsumo<'a> {
    pub consumo: i64,
    pub motivo: String,
    /// Reingresos a su ubicación de origen, con su asiento de reversa.
    pub reversas: &'a [DescuentoConsumo],
}

/// Un consumo tal como está guardado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumoRegistrado {
    pub id: i64,
    pub cuenta: i64,
    pub nombre_cuenta: String,
    pub tomado_por: String,
    /// Costo congelado. No se recalcula al leerlo.
    pub total_costo: Dinero,
    pub ocurrido_en: String,
    /// Con fecha, el consumo dejó de contar para el saldo.
    pub anulado_en: Option<String>,
    pub motivo_anulacion: Option<String>,
}

/// Un renglón de un consumo ya guardado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaConsumoRegistrada {
    pub producto: IdProducto,
    pub nombre_producto: String,
    pub nombre_presentacion: String,
    /// Unidad base del producto, para saber con cuántos decimales se
    /// enseña la cantidad. No se deduce del factor: una presentación
    /// «Libra» tiene factor 1 —entero— y aun así se lleva `2.500`.
    pub unidad_base: UnidadBase,
    pub origen: Ubicacion,
    pub cantidad: Cantidad,
    pub factor: Cantidad,
    pub costo_unitario: Dinero,
}

/// Una liquidación tal como está guardada.
///
/// No lleva sesión de caja a propósito: liquidar no es un ingreso. El
/// negocio ya gastó ese dinero al comprar la mercancía, así que el pago
/// recupera un costo y no gana nada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagoConsumoRegistrado {
    pub id: i64,
    pub cuenta: i64,
    pub importe: Dinero,
    pub nota: Option<String>,
    pub ocurrido_en: String,
    pub anulado_en: Option<String>,
    pub motivo_anulacion: Option<String>,
}

/// El saldo de una cuenta, sumado por la base.
///
/// Es una suma de dos columnas enteras ya guardadas, con el filtro de
/// anulación a los **dos** lados. Nada se recalcula de los costos de hoy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaldoCuenta {
    pub cuenta: i64,
    pub nombre: String,
    pub activa: bool,
    /// Lo consumido sin anular.
    pub consumido: Dinero,
    /// Lo pagado sin anular.
    pub pagado: Dinero,
    /// Consumido menos pagado. **Puede ser negativo**: la cuenta tiene
    /// dinero a favor, y eso no se recorta ni se esconde.
    pub saldo: Dinero,
}

/// Acceso al consumo de los dueños (RF-CON).
///
/// Es un puerto aparte y no un puñado más de métodos en
/// [`RepositorioProducto`], que ya arrastra más de treinta: quien liquida
/// una deuda no necesita saber cerrar una caja, y su doble de pruebas no
/// debería tener que fingir cuarenta métodos para usar diez (DT-5).
///
/// **Su adaptador comparte la misma base que el resto, y eso impone algo que
/// no es evidente:** el candado de la conexión no es reentrante, así que un
/// método de este puerto **no puede llamar a otro puerto** dentro de su
/// transacción sin colgar el programa. Por eso `registrar_consumo` escribe
/// él solo la cabecera, los renglones, el valor del inventario, la
/// existencia y los asientos. Es lo mismo que hace `registrar_venta`.
pub trait RepositorioConsumo {
    /// Las cuentas de consumo, por orden de creación.
    fn cuentas(&self, incluir_inactivas: bool) -> Resultado<Vec<CuentaConsumo>>;

    /// Guarda un consumo entero y devuelve su identificador.
    ///
    /// Cabecera, renglones, saldos y asientos entran en la misma operación:
    /// medio consumo registrado deja el inventario mintiendo (RNF-5).
    fn registrar_consumo(&self, confirmado: &ConsumoConfirmado<'_>) -> Resultado<i64>;

    /// Devuelve un consumo con sus renglones.
    fn consumo(
        &self,
        id: i64,
    ) -> Resultado<Option<(ConsumoRegistrado, Vec<LineaConsumoRegistrada>)>>;

    /// Marca un consumo como anulado y devuelve la mercancía.
    ///
    /// Las dos cosas van juntas: un consumo marcado sin devolver la
    /// mercancía deja la existencia mintiendo, y devolverla sin marcarlo
    /// cobraría dos veces la misma deuda.
    fn anular_consumo(&self, anulacion: &AnulacionConsumo<'_>) -> Resultado<()>;

    /// Anota una liquidación y devuelve su identificador.
    fn registrar_pago(&self, cuenta: i64, importe: Dinero, nota: Option<&str>) -> Resultado<i64>;

    /// Devuelve una liquidación por su identificador.
    fn pago(&self, id: i64) -> Resultado<Option<PagoConsumoRegistrado>>;

    /// Marca una liquidación como anulada.
    ///
    /// Sube el saldo otra vez: es la otra dirección del mismo eje que
    /// anular un consumo, que lo baja.
    fn anular_pago(&self, id: i64, motivo: &str) -> Resultado<()>;

    /// El saldo de cada cuenta, incluidas las inactivas que aún deben algo.
    fn saldos(&self) -> Resultado<Vec<SaldoCuenta>>;

    /// Consumos de una cuenta, del más reciente al más antiguo. Los
    /// anulados también: están para verse.
    fn consumos_de_cuenta(&self, cuenta: i64, limite: usize) -> Resultado<Vec<ConsumoRegistrado>>;

    /// Liquidaciones de una cuenta, de la más reciente a la más antigua.
    fn pagos_de_cuenta(&self, cuenta: i64, limite: usize) -> Resultado<Vec<PagoConsumoRegistrado>>;

    /// Renglones consumidos, **sin agregar**.
    ///
    /// Se devuelven en crudo a propósito, igual que `lineas_del_periodo`:
    /// agrupar por producto exige multiplicar cantidad por factor y por
    /// costo, y esa cuenta no se hace en SQL (DT-5). Una división entera
    /// por el camino perdería milésimas, que es justo el redondeo que todo
    /// el dominio se toma el trabajo de evitar.
    ///
    /// `cuenta` en blanco trae las tres. Las fechas, en blanco, no acotan.
    /// Los consumos anulados nunca aparecen.
    fn lineas_consumidas(
        &self,
        cuenta: Option<i64>,
        desde: Option<&str>,
        hasta: Option<&str>,
    ) -> Resultado<Vec<LineaConsumoRegistrada>>;
}

/// Una venta en espera tal como está guardada.
#[derive(Debug, Clone)]
pub struct EsperaRegistrada {
    pub id: IdVentaEnEspera,
    pub espera: VentaEnEspera,
    /// Fecha y hora en que se apartó, como la guarda la base de datos.
    pub creada_en: String,
}

/// Una venta en espera vista desde la lista: lo justo para reconocerla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EsperaResumida {
    pub id: IdVentaEnEspera,
    pub nota: Option<String>,
    pub creada_en: String,
    pub cuantas_lineas: i64,
}

/// Acceso a las ventas en espera (RF-VTA-14).
///
/// Es un puerto aparte y no un puñado más de métodos en
/// [`RepositorioProducto`]: quien lista esperas no necesita saber cerrar
/// una caja, y quien la implemente en memoria para una prueba no debería
/// tener que escribir cuarenta métodos para cuatro.
pub trait RepositorioVentaEnEspera {
    /// Guarda una venta en espera y devuelve su identificador.
    ///
    /// La cabecera y sus renglones entran juntos: una espera sin renglones
    /// es una fila que no se puede retomar.
    fn guardar(&self, espera: &VentaEnEspera) -> Resultado<IdVentaEnEspera>;

    /// Lista las ventas en espera, de la más antigua a la más reciente:
    /// la que lleva más tiempo esperando va primero.
    fn listar(&self) -> Resultado<Vec<EsperaResumida>>;

    /// Recupera una venta en espera con sus renglones en orden.
    fn obtener(&self, id: IdVentaEnEspera) -> Resultado<Option<EsperaRegistrada>>;

    /// Borra una venta en espera con sus renglones.
    ///
    /// Devuelve `false` si no existía, para que el caso de uso decida qué
    /// decirle al usuario.
    fn eliminar(&self, id: IdVentaEnEspera) -> Resultado<bool>;
}
