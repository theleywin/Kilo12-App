//! Comandos que la interfaz puede invocar.
//!
//! Son el adaptador de entrada: traducen la petición, delegan en el caso de
//! uso y traducen la respuesta. No contienen reglas de negocio — si alguna
//! vez aparece un `if` que decide algo del negocio aquí, está en el sitio
//! equivocado.
//!
//! Los nombres son verbos del negocio (`registrar_producto`), no
//! operaciones de base de datos (DT-7).

use application::casos::{ListarProductos, RegistrarProducto};
use application::margen;
use domain::Dinero;
use tauri::State;

use crate::dto::{ErrorDto, MargenDto, NuevoProductoDto, ProductoDto};
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
