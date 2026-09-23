//! Comandos que la interfaz puede invocar.
//!
//! Son el adaptador de entrada: traducen la petición, delegan en el caso de
//! uso y traducen la respuesta. No contienen reglas de negocio — si alguna
//! vez aparece un `if` que decide algo del negocio aquí, está en el sitio
//! equivocado.
//!
//! Los nombres son verbos del negocio (`registrar_producto`), no
//! operaciones de base de datos (DT-7).

use application::casos::consultar_kardex::LIMITE_POR_DEFECTO;
use application::casos::consultar_ventas::LIMITE_POR_DEFECTO as LIMITE_VENTAS;
use application::casos::{
    AgregarPresentacion, CambiarPrecio, ConsultarAlmacen, ConsultarHistorialPrecios,
    ConsultarKardex, ConsultarProducto, ConsultarVitrina, DesactivarPresentacion, EditarProducto,
    FijarObjetivoVitrina, ListarProductos, MarcarPredeterminada, OrdenCatalogo, RegistrarEntrada,
    RegistrarMerma, RegistrarProducto, SimularMovimiento, Traspasar,
};
use application::casos::{
    CalcularCobro, CatalogoDeVenta, ConsultarTasa, ConsultarVenta, ConsultarVentas, FijarTasa,
    PrevisualizarVenta, Vender,
};
use application::margen;
use domain::{Dinero, Porcentaje};
use tauri::State;

use crate::dto::{
    AlmacenDto, CambioPrecioDto, CambioPrecioListadoDto, ConsultaSimulacionDto, EdicionProductoDto,
    ErrorDto, FichaProductoDto, MargenDto, MovimientoDto, NuevaEntradaDto, NuevaMermaDto,
    NuevaPresentacionDto, NuevoProductoDto, NuevoTraspasoDto, ObjetivoVitrinaDto, ProductoDto,
    ReferenciaPresentacionDto, SimulacionDto, VitrinaDto,
};
use crate::dto::{
    CobroCalculadoDto, HistorialVentasDto, NuevaVentaDto, PagoDto, ProductoVendibleDto,
    VentaDetalladaDto, VentaHechaDto, VentaPrevistaDto,
};
use crate::estado::Estado;

/// Da de alta un producto y devuelve su identificador.
#[tauri::command]
pub fn registrar_producto(
    estado: State<'_, Estado>,
    producto: NuevoProductoDto,
) -> Result<i64, ErrorDto> {
    let caso = RegistrarProducto::nuevo(estado.repositorio_producto());
    caso.ejecutar(producto.into())
        .map(|id| id.0)
        .map_err(ErrorDto::from)
}

/// Devuelve el estado del almacén: qué hay y cuánto vale.
#[tauri::command]
pub fn consultar_almacen(estado: State<'_, Estado>) -> Result<AlmacenDto, ErrorDto> {
    let caso = ConsultarAlmacen::nuevo(estado.repositorio_producto());
    caso.ejecutar()
        .map(AlmacenDto::from)
        .map_err(ErrorDto::from)
}

/// Registra la entrada de mercancía de una compra.
#[tauri::command]
pub fn registrar_entrada(
    estado: State<'_, Estado>,
    entrada: NuevaEntradaDto,
) -> Result<(), ErrorDto> {
    let caso = RegistrarEntrada::nuevo(estado.repositorio_producto());
    caso.ejecutar(entrada.into()).map_err(ErrorDto::from)
}

/// Devuelve el estado de la vitrina y qué hace falta reponer.
#[tauri::command]
pub fn consultar_vitrina(estado: State<'_, Estado>) -> Result<VitrinaDto, ErrorDto> {
    let caso = ConsultarVitrina::nuevo(estado.repositorio_producto());
    caso.ejecutar()
        .map(VitrinaDto::from)
        .map_err(ErrorDto::from)
}

/// Fija cuánto se quiere mantener exhibido de un producto.
#[tauri::command]
pub fn fijar_objetivo_vitrina(
    estado: State<'_, Estado>,
    objetivo: ObjetivoVitrinaDto,
) -> Result<(), ErrorDto> {
    let caso = FijarObjetivoVitrina::nuevo(estado.repositorio_producto());
    caso.ejecutar(objetivo.into()).map_err(ErrorDto::from)
}

/// Mueve mercancía entre el almacén y la vitrina.
///
/// No lleva costo: no se le compró nada a nadie, solo cambia de sitio.
#[tauri::command]
pub fn traspasar(estado: State<'_, Estado>, traspaso: NuevoTraspasoDto) -> Result<(), ErrorDto> {
    let caso = Traspasar::nuevo(estado.repositorio_producto());
    caso.ejecutar(traspaso.into()).map_err(ErrorDto::from)
}

/// Responde cómo quedaría la existencia si el movimiento se hiciera.
#[tauri::command]
pub fn simular_movimiento(
    estado: State<'_, Estado>,
    consulta: ConsultaSimulacionDto,
) -> Result<SimulacionDto, ErrorDto> {
    let caso = SimularMovimiento::nuevo(estado.repositorio_producto());
    caso.ejecutar(consulta.into())
        .map(SimulacionDto::from)
        .map_err(ErrorDto::from)
}

/// Da de baja mercancía perdida.
#[tauri::command]
pub fn registrar_merma(estado: State<'_, Estado>, merma: NuevaMermaDto) -> Result<(), ErrorDto> {
    let caso = RegistrarMerma::nuevo(estado.repositorio_producto());
    caso.ejecutar(merma.into()).map_err(ErrorDto::from)
}

/// Devuelve el historial de movimientos de un producto (RF-INV-05).
#[tauri::command]
pub fn consultar_kardex(
    estado: State<'_, Estado>,
    producto: i64,
    limite: Option<usize>,
) -> Result<Vec<MovimientoDto>, ErrorDto> {
    let caso = ConsultarKardex::nuevo(estado.repositorio_producto());
    caso.ejecutar(producto, limite.unwrap_or(LIMITE_POR_DEFECTO))
        .map(|lineas| lineas.into_iter().map(MovimientoDto::from).collect())
        .map_err(ErrorDto::from)
}

// ------------------------------------------------------------ vender

/// Devuelve lo que se puede vender ahora mismo.
///
/// Trae la existencia de la VITRINA, no la total: la bodega no está a la
/// venta (RF-VTA-11).
#[tauri::command]
pub fn catalogo_de_venta(estado: State<'_, Estado>) -> Result<Vec<ProductoVendibleDto>, ErrorDto> {
    let caso = CatalogoDeVenta::nuevo(estado.repositorio_producto());
    caso.ejecutar()
        .map(|productos| {
            productos
                .into_iter()
                .map(ProductoVendibleDto::from)
                .collect()
        })
        .map_err(ErrorDto::from)
}

/// Calcula la venta en curso sin tocar nada.
///
/// La pantalla no suma dinero: pregunta. Y de paso se entera de si alguna
/// línea no cabe en la vitrina antes de intentar cobrarla.
#[tauri::command]
pub fn previsualizar_venta(
    estado: State<'_, Estado>,
    lineas: Vec<crate::dto::LineaVentaDto>,
) -> Result<VentaPrevistaDto, ErrorDto> {
    let caso = PrevisualizarVenta::nuevo(estado.repositorio_producto());
    let pedidas = lineas
        .into_iter()
        .map(|l| application::casos::LineaPedida {
            producto: l.producto,
            presentacion: l.presentacion,
            cantidad: l.cantidad,
        })
        .collect();

    caso.ejecutar(pedidas)
        .map(VentaPrevistaDto::from)
        .map_err(ErrorDto::from)
}

/// Dice si lo que pone el cliente cubre la venta, y cuánto se le devuelve.
///
/// Se llama mientras se teclea: el cajero tiene que ver el vuelto mientras
/// cuenta los billetes, no después de confirmar.
#[tauri::command]
pub fn calcular_cobro(
    estado: State<'_, Estado>,
    total: String,
    pagos: Vec<PagoDto>,
) -> Result<CobroCalculadoDto, ErrorDto> {
    let caso = CalcularCobro::nuevo(estado.repositorio_producto());
    let pedidos: Vec<application::casos::PagoPedido> = pagos
        .into_iter()
        .map(|p| application::casos::PagoPedido {
            metodo: p.metodo,
            entregado: p.entregado,
        })
        .collect();

    caso.ejecutar(&total, &pedidos)
        .map(CobroCalculadoDto::from)
        .map_err(ErrorDto::from)
}

/// Cobra una venta: descuenta de la vitrina y la deja registrada.
///
/// Todo ocurre en una sola operación. Si algo falla —falta existencia, el
/// pago no alcanza—, no se guarda nada (RNF-5).
#[tauri::command]
pub fn vender(estado: State<'_, Estado>, venta: NuevaVentaDto) -> Result<VentaHechaDto, ErrorDto> {
    let caso = Vender::nuevo(estado.repositorio_producto());
    caso.ejecutar(venta.into())
        .map(VentaHechaDto::from)
        .map_err(ErrorDto::from)
}

/// Devuelve el historial de ventas con el corte del día (RF-VTA-16).
#[tauri::command]
pub fn consultar_ventas(
    estado: State<'_, Estado>,
    limite: Option<usize>,
) -> Result<HistorialVentasDto, ErrorDto> {
    let caso = ConsultarVentas::nuevo(estado.repositorio_producto());
    caso.ejecutar(limite.unwrap_or(LIMITE_VENTAS))
        .map(HistorialVentasDto::from)
        .map_err(ErrorDto::from)
}

/// Devuelve una venta concreta con sus líneas y sus pagos.
#[tauri::command]
pub fn consultar_venta(estado: State<'_, Estado>, id: i64) -> Result<VentaDetalladaDto, ErrorDto> {
    let caso = ConsultarVenta::nuevo(estado.repositorio_producto());
    caso.ejecutar(id)
        .map(VentaDetalladaDto::from)
        .map_err(ErrorDto::from)
}

/// Devuelve la tasa de cambio vigente, si está fijada.
#[tauri::command]
pub fn consultar_tasa(estado: State<'_, Estado>) -> Result<Option<String>, ErrorDto> {
    let caso = ConsultarTasa::nuevo(estado.repositorio_producto());
    caso.ejecutar().map_err(ErrorDto::from)
}

/// Fija la tasa con la que se convierten los dólares (RF-DIV).
#[tauri::command]
pub fn fijar_tasa(estado: State<'_, Estado>, tasa: String) -> Result<(), ErrorDto> {
    let caso = FijarTasa::nuevo(estado.repositorio_producto());
    caso.ejecutar(&tasa).map_err(ErrorDto::from)
}

/// Calcula la ganancia y el margen de un precio frente a su costo.
///
/// La pantalla lo llama mientras se escribe el precio, para que el dueño
/// vea lo que gana ANTES de guardar. No hay estado de por medio: es una
/// cuenta, y se hace donde la aritmética es exacta.
#[tauri::command]
pub fn calcular_margen(costo: String, precio: String) -> Result<MargenDto, ErrorDto> {
    let costo: Dinero = costo.trim().parse()?;
    let precio: Dinero = precio.trim().parse()?;

    margen::calcular(costo, precio)
        .map(MargenDto::from)
        .map_err(ErrorDto::from)
}

/// Lista el catálogo, ordenado por la columna que se pida.
///
/// El orden se resuelve en Rust: ordenar por margen es comparar dinero, y
/// esa comparación tiene que ser exacta.
#[tauri::command]
pub fn listar_productos(
    estado: State<'_, Estado>,
    incluir_inactivos: Option<bool>,
    orden: Option<String>,
    descendente: Option<bool>,
) -> Result<Vec<ProductoDto>, ErrorDto> {
    let caso = ListarProductos::nuevo(estado.repositorio_producto());

    let orden: OrdenCatalogo = match orden.as_deref() {
        None | Some("") => OrdenCatalogo::default(),
        Some(texto) => texto.parse()?,
    };

    caso.ordenado(
        incluir_inactivos.unwrap_or(false),
        orden,
        descendente.unwrap_or(false),
    )
    .map(|productos| productos.into_iter().map(ProductoDto::from).collect())
    .map_err(ErrorDto::from)
}

/// Devuelve la ficha comercial de un producto con sus presentaciones.
#[tauri::command]
pub fn consultar_producto(
    estado: State<'_, Estado>,
    producto: i64,
) -> Result<FichaProductoDto, ErrorDto> {
    let caso = ConsultarProducto::nuevo(estado.repositorio_producto());
    caso.ejecutar(producto)
        .map(FichaProductoDto::from)
        .map_err(ErrorDto::from)
}

/// Cambia el nombre, el mínimo o el estado de un producto (RF-CAT-05).
#[tauri::command]
pub fn editar_producto(
    estado: State<'_, Estado>,
    edicion: EdicionProductoDto,
) -> Result<(), ErrorDto> {
    let caso = EditarProducto::nuevo(estado.repositorio_producto());
    caso.ejecutar(edicion.into()).map_err(ErrorDto::from)
}

/// Agrega una forma de vender el producto (RF-PRS-02).
#[tauri::command]
pub fn agregar_presentacion(
    estado: State<'_, Estado>,
    presentacion: NuevaPresentacionDto,
) -> Result<(), ErrorDto> {
    let caso = AgregarPresentacion::nuevo(estado.repositorio_producto());
    caso.ejecutar(presentacion.into()).map_err(ErrorDto::from)
}

/// Cambia el precio de una presentación y lo anota en el historial.
#[tauri::command]
pub fn cambiar_precio(estado: State<'_, Estado>, cambio: CambioPrecioDto) -> Result<(), ErrorDto> {
    let caso = CambiarPrecio::nuevo(estado.repositorio_producto());
    caso.ejecutar(cambio.into()).map_err(ErrorDto::from)
}

/// Retira una presentación de la venta sin borrarla (RF-PRS-15).
#[tauri::command]
pub fn desactivar_presentacion(
    estado: State<'_, Estado>,
    referencia: ReferenciaPresentacionDto,
) -> Result<(), ErrorDto> {
    let caso = DesactivarPresentacion::nuevo(estado.repositorio_producto());
    caso.ejecutar(referencia.into()).map_err(ErrorDto::from)
}

/// Elige la presentación que usa la venta rápida (RF-PRS-08).
#[tauri::command]
pub fn marcar_predeterminada(
    estado: State<'_, Estado>,
    referencia: ReferenciaPresentacionDto,
) -> Result<(), ErrorDto> {
    let caso = MarcarPredeterminada::nuevo(estado.repositorio_producto());
    caso.ejecutar(referencia.into()).map_err(ErrorDto::from)
}

/// Devuelve cómo ha ido cambiando el precio de un producto (RF-PRE-04).
#[tauri::command]
pub fn consultar_historial_precios(
    estado: State<'_, Estado>,
    producto: i64,
) -> Result<Vec<CambioPrecioListadoDto>, ErrorDto> {
    let caso = ConsultarHistorialPrecios::nuevo(estado.repositorio_producto());
    caso.ejecutar(producto)
        .map(|cambios| {
            cambios
                .into_iter()
                .map(CambioPrecioListadoDto::from)
                .collect()
        })
        .map_err(ErrorDto::from)
}

/// Calcula el precio que hay que cobrar para dejar el margen pedido.
///
/// Es el camino inverso del margen: en vez de preguntar cuánto deja un
/// precio, se dice cuánto se quiere dejar y sale el precio (RF-PRE-02).
#[tauri::command]
pub fn calcular_precio_para_margen(costo: String, margen: String) -> Result<String, ErrorDto> {
    let costo: Dinero = costo.trim().parse()?;
    // Se llama `objetivo` y no `margen` para no tapar al módulo del mismo
    // nombre: compila igual, pero leerlo cuesta el doble.
    let objetivo: Porcentaje = margen.trim().parse()?;

    margen::precio_para(costo, objetivo)
        .map(|precio| precio.formatear(2))
        .map_err(ErrorDto::from)
}
