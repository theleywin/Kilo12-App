// La ventana de consola de Windows se oculta en las compilaciones de
// entrega; en depuración se conserva para poder ver los mensajes.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    kilo12_lib::ejecutar();
}
