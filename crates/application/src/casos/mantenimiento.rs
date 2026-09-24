//! Caso de uso: dejar la aplicación como recién instalada.
//!
//! Existe para un momento concreto: terminar de probar y empezar a operar
//! de verdad. Borra **todo** —catálogo, existencias, kárdex, ventas, cajas
//! y ajustes— y deja el esquema intacto, listo para volver a empezar.
//!
//! ## Sobre la clave
//!
//! La clave está escrita en el código y cualquiera con el binario delante
//! puede leerla. **No es un mecanismo de seguridad y no pretende serlo.**
//! Lo que evita es el accidente: que una cajera con prisa pulse un botón
//! rojo y se lleve por delante el historial del negocio. Contra eso, cuatro
//! dígitos que hay que teclear a conciencia bastan; contra alguien que
//! quiera hacer daño no serviría ni una clave de veinte caracteres, porque
//! tiene el archivo de la base de datos al alcance de la mano.
//!
//! Se comprueba **en Rust y no en la pantalla** por un motivo distinto: el
//! comando queda expuesto a la interfaz, y una validación que solo vive en
//! JavaScript se salta abriendo las herramientas de desarrollo.

use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::RepositorioProducto;

/// Clave que hay que teclear para vaciar la aplicación.
pub const CLAVE_MANTENIMIENTO: &str = "314159";

/// Borra todos los datos del negocio.
#[derive(Debug)]
pub struct BorrarTodo<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> BorrarTodo<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    /// Vacía la aplicación. No tiene vuelta atrás.
    pub fn ejecutar(&self, clave: &str) -> Resultado<()> {
        if clave.trim() != CLAVE_MANTENIMIENTO {
            return Err(ErrorAplicacion::ClaveIncorrecta);
        }

        self.repositorio.borrar_todos_los_datos()
    }
}
