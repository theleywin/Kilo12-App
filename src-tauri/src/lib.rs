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
            comandos::consultar_producto,
            comandos::editar_producto,
            comandos::agregar_presentacion,
            comandos::cambiar_precio,
            comandos::desactivar_presentacion,
            comandos::marcar_predeterminada,
            comandos::consultar_historial_precios,
            comandos::previsualizar_costo,
            comandos::cambiar_costo,
            comandos::consultar_historial_costos,
            comandos::previsualizar_quitar_producto,
            comandos::quitar_producto,
            comandos::previsualizar_limpieza,
            comandos::limpiar_catalogo,
            comandos::previsualizar_consumo,
            comandos::registrar_consumo,
            comandos::liquidar_deuda,
            comandos::anular_consumo,
            comandos::anular_pago_consumo,
            comandos::consultar_panel_consumo,
            comandos::consultar_cuenta_consumo,
            comandos::consultar_renglones_consumo,
            comandos::consultar_consumo_por_producto,
            comandos::calcular_precio_para_margen,
            comandos::catalogo_de_venta,
            comandos::previsualizar_venta,
            comandos::calcular_cobro,
            comandos::vender,
            comandos::dejar_venta_en_espera,
            comandos::listar_ventas_en_espera,
            comandos::retomar_venta_en_espera,
            comandos::eliminar_venta_en_espera,
            comandos::consultar_ventas,
            comandos::consultar_ventas_por_producto,
            comandos::consultar_venta,
            comandos::consultar_tasa,
            comandos::fijar_tasa,
            comandos::abrir_caja,
            comandos::consultar_caja,
            comandos::mover_efectivo,
            comandos::previsualizar_cierre,
            comandos::cerrar_caja,
            comandos::listar_cajas,
            comandos::consultar_cierre,
            comandos::anular_venta,
            comandos::consultar_comision,
            comandos::fijar_comision,
            comandos::denominaciones_efectivo,
            comandos::contar_efectivo,
            comandos::consultar_informe,
            comandos::borrar_todos_los_datos,
        ])
        .run(tauri::generate_context!())
        .expect("no se pudo iniciar Kilo12");
}
