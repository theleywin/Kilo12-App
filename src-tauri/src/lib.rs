//! Punto de entrada de la aplicación de escritorio.

#![forbid(unsafe_code)]

mod comandos;
mod dto;
mod estado;

use tauri::Manager;

use crate::estado::Estado;

/// Arranca la aplicación.
pub fn ejecutar() {
    tauri::Builder::default()
        .setup(|app| {
            // La base de datos vive en la carpeta de datos del sistema, no
            // junto al ejecutable: en Windows el directorio de instalación
            // suele ser de solo lectura para el usuario.
            let carpeta = app.path().app_data_dir()?;
            let estado = Estado::iniciar(carpeta.join("kilo12.db"))?;
            app.manage(estado);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            comandos::registrar_producto,
            comandos::listar_productos,
            comandos::calcular_margen,
            comandos::consultar_almacen,
            comandos::registrar_entrada,
            comandos::registrar_merma,
            comandos::consultar_vitrina,
            comandos::fijar_objetivo_vitrina,
            comandos::traspasar,
            comandos::simular_movimiento,
            comandos::consultar_kardex,
        ])
        .run(tauri::generate_context!())
        .expect("no se pudo iniciar Kilo12");
}
