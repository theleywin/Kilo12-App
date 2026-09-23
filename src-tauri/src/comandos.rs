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
use application::casos::{
    ConsultarAlmacen, ConsultarKardex, ConsultarVitrina, FijarObjetivoVitrina, ListarProductos,
    RegistrarEntrada, RegistrarMerma, RegistrarProducto, SimularMovimiento, Traspasar,
};
use application::margen;
use domain::Dinero;
use tauri::State;

use crate::dto::{
    AlmacenDto, ConsultaSimulacionDto, ErrorDto, MargenDto, MovimientoDto, NuevaEntradaDto,
    NuevaMermaDto, NuevoProductoDto, NuevoTraspasoDto, ObjetivoVitrinaDto, ProductoDto,
    SimulacionDto, VitrinaDto,
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

/// Lista el catálogo.
#[tauri::command]
pub fn listar_productos(
    estado: State<'_, Estado>,
    incluir_inactivos: Option<bool>,
) -> Result<Vec<ProductoDto>, ErrorDto> {
    let caso = ListarProductos::nuevo(estado.repositorio_producto());
    caso.ejecutar(incluir_inactivos.unwrap_or(false))
        .map(|productos| productos.into_iter().map(ProductoDto::from).collect())
        .map_err(ErrorDto::from)
}
