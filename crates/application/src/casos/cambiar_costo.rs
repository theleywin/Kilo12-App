//! Caso de uso: fijar a mano el costo de un producto (revaluación).
//!
//! Es la operación más delicada del inventario y conviene entender por qué
//! antes de tocarla.
//!
//! El costo **no se almacena**: se deriva de dividir el valor invertido
//! entre la existencia (RF-COS-02). No hay un campo «costo» que editar. Así
//! que fijar un costo distinto es mover el valor del inventario:
//!
//! ```text
//! valor_total = costo nuevo × existencia actual
//! ```
//!
//! Y eso significa que **el valor del inventario cambia sin que nadie haya
//! comprado ni vendido nada**. Subir el costo de cien libras de 120 a 150
//! añade tres mil pesos al valor del negocio; bajarlo los destruye. No es un
//! efecto colateral evitable: es la misma operación mirada del otro lado.
//!
//! De ahí las tres decisiones de este módulo:
//!
//! - **Pide la clave de mantenimiento**, la misma que vacía la aplicación.
//!   No porque sean igual de destructivas, sino porque las dos cambian
//!   cifras que nadie puede reconstruir después, y las dos merecen que haya
//!   que teclear algo a conciencia. Se comprueba en Rust y no en la
//!   pantalla: el comando queda expuesto a la interfaz, y una validación
//!   que solo vive en JavaScript se salta abriendo las herramientas de
//!   desarrollo.
//! - **Queda anotada** en el historial de costos, con las cifras de las dos
//!   puntas. Un valor de inventario que cambió sin dejar rastro es dinero
//!   que apareció y nadie puede explicar.
//! - **Sin existencia se niega.** Sin nada que dividir no hay costo que
//!   fijar, y guardar un valor con existencia cero dejaría un producto que
//!   no tiene nada y vale algo.
//!
//! Lo que **no** hace: tocar las ventas pasadas. Cada línea vendida se
//! quedó con su costo congelado (RF-VTA-13), así que la ganancia de ayer
//! sigue diciendo lo que dijo ayer. Esto solo cambia el costo de lo que
//! queda en el almacén, y por tanto el de las ventas que vengan.

use domain::{Dinero, IdProducto};

use crate::casos::formatear_cantidad;
use crate::casos::mantenimiento::CLAVE_MANTENIMIENTO;
use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{CambioDeCosto, ProductoConInventario, RepositorioProducto};

/// Lo que llega de la pantalla para fijar un costo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoCambiarCosto {
    pub producto: i64,
    /// Costo nuevo por unidad base.
    pub costo: String,
    /// Clave de mantenimiento. Se comprueba aquí, no en la pantalla.
    pub clave: String,
}

/// Lo que una revaluación haría, o lo que acaba de hacer.
///
/// Todo llega formateado y con las dos puntas a la vista: lo que el cartel
/// de confirmación tiene que enseñar no es el costo nuevo, sino **cuánto se
/// mueve el valor del inventario**. Esa es la cifra que la gente no espera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revaluacion {
    pub producto: i64,
    pub nombre: String,
    /// Símbolo de la unidad base: `u`, `lb`, `kg`…
    pub unidad_base: String,
    /// Existencia total sobre la que se reparte el valor.
    pub existencia: String,
    pub costo_anterior: String,
    pub costo_nuevo: String,
    pub valor_anterior: String,
    pub valor_nuevo: String,
    /// Cuánto se mueve el valor del inventario. Lleva signo.
    pub diferencia: String,
    /// El valor del inventario sube. Con `false` baja o se queda igual.
    pub sube: bool,
}

/// Fija el costo de un producto revalorando su inventario.
#[derive(Debug)]
pub struct CambiarCosto<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> CambiarCosto<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    /// Calcula la revaluación **sin guardar nada**.
    ///
    /// Es lo que alimenta el cartel de confirmación. No pide clave a
    /// propósito: enseñar las cifras que están en juego es justo lo que
    /// tiene que pasar ANTES de que alguien la teclee.
    pub fn previsualizar(&self, producto: i64, costo: &str) -> Resultado<Revaluacion> {
        let (ficha, nuevo) = self.preparar(producto, costo)?;
        self.describir(&ficha, nuevo)
    }

    /// Fija el costo y deja la revaluación anotada.
    pub fn ejecutar(&self, comando: ComandoCambiarCosto) -> Resultado<Revaluacion> {
        // La clave, lo primero. Antes de mirar el producto y antes de hacer
        // una sola cuenta: sin ella no hay nada que decidir.
        if comando.clave.trim() != CLAVE_MANTENIMIENTO {
            return Err(ErrorAplicacion::ClaveIncorrecta);
        }

        let (ficha, nuevo) = self.preparar(comando.producto, &comando.costo)?;
        let resumen = self.describir(&ficha, nuevo)?;

        // El costo anterior se lee ANTES de pisarlo: después no existe en
        // ninguna parte, porque se deriva del valor que esta misma
        // operación cambia.
        let anterior = ficha.inventario.costo_unitario()?;
        let existencia = ficha.inventario.cantidad_total()?;
        let revalorado = ficha.inventario.revalorar(nuevo)?;

        self.repositorio.revalorar_producto(
            IdProducto(comando.producto),
            &revalorado,
            &CambioDeCosto {
                anterior,
                nuevo,
                existencia,
                valor_anterior: ficha.inventario.valor_total(),
                valor_nuevo: revalorado.valor_total(),
            },
        )?;

        Ok(resumen)
    }

    /// Carga el producto e interpreta el costo pedido.
    fn preparar(&self, producto: i64, costo: &str) -> Resultado<(ProductoConInventario, Dinero)> {
        let ficha = self.repositorio.obtener(IdProducto(producto))?.ok_or(
            ErrorAplicacion::NoEncontrado {
                entidad: "producto",
                id: producto,
            },
        )?;

        let nuevo: Dinero = costo.trim().parse()?;
        Ok((ficha, nuevo))
    }

    /// Describe la revaluación con las dos puntas ya formateadas.
    fn describir(&self, ficha: &ProductoConInventario, nuevo: Dinero) -> Resultado<Revaluacion> {
        // Las dos llamadas fallan si no hay existencia, y es lo que tiene
        // que pasar: sin nada que dividir no hay costo que fijar.
        let anterior = ficha.inventario.costo_unitario()?;
        let existencia = ficha.inventario.cantidad_total()?;
        let revalorado = ficha.inventario.revalorar(nuevo)?;

        let valor_anterior = ficha.inventario.valor_total();
        let valor_nuevo = revalorado.valor_total();
        let diferencia = valor_nuevo.restar(valor_anterior)?;

        Ok(Revaluacion {
            producto: ficha.producto.id().map_or(0, |id| id.0),
            nombre: ficha.producto.nombre().to_owned(),
            unidad_base: ficha.producto.unidad_base().simbolo().to_owned(),
            existencia: formatear_cantidad(existencia, &ficha.producto),
            costo_anterior: anterior.formatear(2),
            costo_nuevo: nuevo.formatear(2),
            valor_anterior: valor_anterior.formatear(2),
            valor_nuevo: valor_nuevo.formatear(2),
            diferencia: diferencia.formatear(2),
            sube: diferencia.es_positivo(),
        })
    }
}

// ===================================================== el historial

/// Una revaluación ya ocurrida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CambioDeCostoListado {
    pub id: i64,
    pub anterior: String,
    pub nuevo: String,
    /// Existencia sobre la que se aplicó.
    pub existencia: String,
    pub valor_anterior: String,
    pub valor_nuevo: String,
    /// Cuánto se movió el valor del inventario. Lleva signo.
    pub diferencia: String,
    /// `true` si el costo subió.
    pub subio: bool,
    pub cambiado_en: String,
}

/// Consulta las revaluaciones de un producto.
#[derive(Debug)]
pub struct ConsultarHistorialCostos<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> ConsultarHistorialCostos<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, id: i64) -> Resultado<Vec<CambioDeCostoListado>> {
        // El producto se carga para formatear la existencia con los
        // decimales que su unidad admite: «100.000 lb» y «3 latas».
        let ProductoConInventario { producto, .. } = self
            .repositorio
            .obtener(IdProducto(id))?
            .ok_or(ErrorAplicacion::NoEncontrado {
                entidad: "producto",
                id,
            })?;

        let historial = self.repositorio.historial_costos(IdProducto(id))?;

        historial
            .into_iter()
            .map(|cambio| {
                Ok(CambioDeCostoListado {
                    id: cambio.id,
                    anterior: cambio.anterior.formatear(2),
                    nuevo: cambio.nuevo.formatear(2),
                    existencia: formatear_cantidad(cambio.existencia, &producto),
                    valor_anterior: cambio.valor_anterior.formatear(2),
                    valor_nuevo: cambio.valor_nuevo.formatear(2),
                    diferencia: cambio
                        .valor_nuevo
                        .restar(cambio.valor_anterior)?
                        .formatear(2),
                    subio: cambio.nuevo > cambio.anterior,
                    cambiado_en: cambio.cambiado_en,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod pruebas {
    use domain::UnidadBase;

    use super::*;
    use crate::dobles::ProductosEnMemoria;

    /// Cien libras de arroz en vitrina que costaron 12 000 en total: 120 la
    /// libra.
    fn catalogo() -> ProductosEnMemoria {
        ProductosEnMemoria::default()
            .con_producto(
                1,
                "Arroz blanco",
                UnidadBase::Libra,
                &[(10, "Libra", "1", "180.00")],
                "100",
            )
            .con_valor(1, "12000.00")
    }

    fn comando(costo: &str, clave: &str) -> ComandoCambiarCosto {
        ComandoCambiarCosto {
            producto: 1,
            costo: costo.to_owned(),
            clave: clave.to_owned(),
        }
    }

    #[test]
    fn fijar_el_costo_mueve_el_valor_del_inventario() {
        let productos = catalogo();

        let hecha = CambiarCosto::nuevo(&productos)
            .ejecutar(comando("150.00", CLAVE_MANTENIMIENTO))
            .expect("revalorar");

        assert_eq!(hecha.costo_anterior, "120.00");
        assert_eq!(hecha.costo_nuevo, "150.00");
        assert_eq!(hecha.valor_anterior, "12000.00");
        assert_eq!(hecha.valor_nuevo, "15000.00");
        assert_eq!(hecha.diferencia, "3000.00");
        assert!(hecha.sube);

        // Y el inventario quedó de verdad con el valor nuevo.
        assert_eq!(
            productos.inventario_de(1).valor_total().formatear(2),
            "15000.00"
        );
    }

    #[test]
    fn la_existencia_no_se_toca() {
        let productos = catalogo();
        let antes = productos.inventario_de(1).existencias();

        CambiarCosto::nuevo(&productos)
            .ejecutar(comando("150.00", CLAVE_MANTENIMIENTO))
            .expect("revalorar");

        assert_eq!(productos.inventario_de(1).existencias(), antes);
    }

    #[test]
    fn bajar_el_costo_destruye_valor_y_se_ve_en_la_diferencia() {
        let hecha = CambiarCosto::nuevo(&catalogo())
            .ejecutar(comando("100.00", CLAVE_MANTENIMIENTO))
            .expect("revalorar");

        assert_eq!(hecha.valor_nuevo, "10000.00");
        assert_eq!(hecha.diferencia, "-2000.00");
        assert!(!hecha.sube);
    }

    #[test]
    fn sin_la_clave_correcta_no_se_cambia_nada() {
        let productos = catalogo();

        let error = CambiarCosto::nuevo(&productos)
            .ejecutar(comando("150.00", "1234"))
            .expect_err("la clave no es esa");

        assert_eq!(error.codigo(), "CLAVE_INCORRECTA");
        // Y el inventario sigue intacto: ni el valor ni la anotación.
        assert_eq!(
            productos.inventario_de(1).valor_total().formatear(2),
            "12000.00"
        );
        assert!(productos.revaluaciones.borrow().is_empty());
    }

    #[test]
    fn la_clave_se_comprueba_antes_que_cualquier_otra_cosa() {
        // Producto que no existe Y clave mala: tiene que quejarse de la
        // clave. Si contestara «no encontrado» estaría diciendo qué
        // productos hay a quien no ha acertado la clave.
        let error = CambiarCosto::nuevo(&ProductosEnMemoria::default())
            .ejecutar(ComandoCambiarCosto {
                producto: 999,
                costo: "150.00".to_owned(),
                clave: "mala".to_owned(),
            })
            .expect_err("la clave manda");

        assert_eq!(error.codigo(), "CLAVE_INCORRECTA");
    }

    #[test]
    fn sin_existencia_no_hay_costo_que_fijar() {
        // Producto agotado: el valor es cero y tiene que seguir en cero.
        let productos = ProductosEnMemoria::default().con_producto(
            1,
            "Arroz blanco",
            UnidadBase::Libra,
            &[(10, "Libra", "1", "180.00")],
            "0",
        );

        let error = CambiarCosto::nuevo(&productos)
            .ejecutar(comando("150.00", CLAVE_MANTENIMIENTO))
            .expect_err("no hay nada que revalorar");

        assert_eq!(error.codigo(), "SIN_EXISTENCIA_PARA_COSTO");
        assert!(productos.revaluaciones.borrow().is_empty());
    }

    #[test]
    fn la_revaluacion_queda_anotada_con_las_dos_puntas() {
        let productos = catalogo();

        CambiarCosto::nuevo(&productos)
            .ejecutar(comando("150.00", CLAVE_MANTENIMIENTO))
            .expect("revalorar");

        let anotadas = productos.revaluaciones.borrow();
        assert_eq!(anotadas.len(), 1);

        let (producto, cambio) = &anotadas[0];
        assert_eq!(producto.0, 1);
        assert_eq!(cambio.anterior.formatear(2), "120.00");
        assert_eq!(cambio.nuevo.formatear(2), "150.00");
        assert_eq!(cambio.existencia.formatear(0), "100");
        assert_eq!(cambio.valor_anterior.formatear(2), "12000.00");
        assert_eq!(cambio.valor_nuevo.formatear(2), "15000.00");
    }

    #[test]
    fn previsualizar_no_guarda_nada() {
        let productos = catalogo();

        let prevista = CambiarCosto::nuevo(&productos)
            .previsualizar(1, "150.00")
            .expect("previsualizar");

        // Enseña lo mismo que haría...
        assert_eq!(prevista.valor_nuevo, "15000.00");
        assert_eq!(prevista.diferencia, "3000.00");
        // La libra se vende en fracciones, así que lleva sus decimales.
        assert_eq!(prevista.existencia, "100.000");
        assert_eq!(prevista.unidad_base, "lb");
        // ...y no ha tocado nada.
        assert_eq!(
            productos.inventario_de(1).valor_total().formatear(2),
            "12000.00"
        );
        assert!(productos.revaluaciones.borrow().is_empty());
    }

    #[test]
    fn un_costo_que_no_es_un_numero_se_rechaza() {
        let error = CambiarCosto::nuevo(&catalogo())
            .ejecutar(comando("ciento cincuenta", CLAVE_MANTENIMIENTO))
            .expect_err("eso no es un importe");

        assert_eq!(error.codigo(), "TEXTO_NUMERICO_INVALIDO");
    }

    #[test]
    fn el_historial_cuenta_lo_que_paso_con_el_valor() {
        let productos = catalogo();
        let caso = CambiarCosto::nuevo(&productos);

        caso.ejecutar(comando("150.00", CLAVE_MANTENIMIENTO))
            .expect("primera");
        caso.ejecutar(comando("90.00", CLAVE_MANTENIMIENTO))
            .expect("segunda");

        let historial = ConsultarHistorialCostos::nuevo(&productos)
            .ejecutar(1)
            .expect("historial");

        // De la más reciente a la más antigua.
        assert_eq!(historial.len(), 2);

        assert_eq!(historial[0].anterior, "150.00");
        assert_eq!(historial[0].nuevo, "90.00");
        assert_eq!(historial[0].diferencia, "-6000.00");
        assert!(!historial[0].subio);

        assert_eq!(historial[1].anterior, "120.00");
        assert_eq!(historial[1].nuevo, "150.00");
        assert_eq!(historial[1].diferencia, "3000.00");
        assert!(historial[1].subio);
    }

    #[test]
    fn un_producto_sin_revaluaciones_tiene_historial_vacio() {
        let historial = ConsultarHistorialCostos::nuevo(&catalogo())
            .ejecutar(1)
            .expect("historial");

        assert!(historial.is_empty());
    }
}
