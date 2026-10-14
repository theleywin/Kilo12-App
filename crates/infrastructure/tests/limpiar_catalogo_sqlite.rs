//! Quitar productos del catálogo, contra SQLite de verdad (RF-CAT-06,
//! RF-CAT-07).
//!
//! Esto no se puede probar con dobles, y es el único sitio donde se puede
//! probar. La regla de RF-CAT-07 —no borrar un producto que tenga
//! movimientos o ventas— no la impone un `if` de la aplicación: la imponen
//! las claves foráneas de `venta_linea`, `movimiento` y
//! `venta_en_espera_linea` contra `producto`, y esas solo muerden contra la
//! base. Un doble en memoria diría «sí, borrado» a cualquier cosa.
//!
//! Lo que se mide aquí:
//!
//! - que un producto sin rastro desaparece **entero**: su ficha, sus
//!   presentaciones, su existencia y sus dos historiales;
//! - que uno que vendió **no** se puede borrar, y que el intento no deja
//!   el catálogo a medio limpiar;
//! - que la limpieza completa reparte cada producto en su grupo y aplica
//!   las dos cosas en una sola operación;
//! - que una venta apartada sin cobrar frena las dos.

use std::sync::Arc;

use application::casos::{
    AbrirCaja, ComandoAbrirCaja, ComandoDejarEnEspera, ComandoRegistrarMerma,
    ComandoRegistrarProducto, ComandoVender, DejarVentaEnEspera, Destino, LimpiarCatalogo,
    LineaPedida, ListarProductos, PagoPedido, ProductoListado, QuitarDelCatalogo, RegistrarMerma,
    RegistrarProducto, Vender,
};
use application::casos::{CambiarCosto, ComandoCambiarCosto, CLAVE_MANTENIMIENTO};
use application::casos::{CambiarPrecio, ComandoCambiarPrecio, ConsultarProducto};
use infrastructure::{BaseDatos, RepositorioProductoSqlite, RepositorioVentaEnEsperaSqlite};

/// Los dos repositorios sobre la MISMA base, como en la aplicación.
struct Entorno {
    base: Arc<BaseDatos>,
    productos: RepositorioProductoSqlite,
    esperas: RepositorioVentaEnEsperaSqlite,
}

impl Entorno {
    fn en_memoria() -> Self {
        let base = Arc::new(BaseDatos::en_memoria().expect("abrir la base en memoria"));
        Self {
            productos: RepositorioProductoSqlite::nuevo(Arc::clone(&base)),
            esperas: RepositorioVentaEnEsperaSqlite::nuevo(Arc::clone(&base)),
            base,
        }
    }

    /// Cuenta filas de una tabla sin pasar por ningún caso de uso: es la
    /// única forma de comprobar que no quedó basura detrás.
    fn filas(&self, tabla: &str) -> i64 {
        let consulta = format!("SELECT COUNT(*) FROM {tabla}");
        self.base
            .con(|conexion| Ok(conexion.query_row(&consulta, [], |fila| fila.get(0))?))
            .expect("contar filas")
    }

    /// Da de alta un producto. Sin cantidad nace vacío y sin kárdex: es la
    /// etiqueta pura.
    fn alta(&self, nombre: &str, en_vitrina: Option<&str>) -> i64 {
        RegistrarProducto::nuevo(&self.productos)
            .ejecutar(ComandoRegistrarProducto {
                sku: String::new(),
                nombre: nombre.to_owned(),
                unidad_base: "unidad".to_owned(),
                precio_unitario: "80.00".to_owned(),
                costo_unitario: en_vitrina.map(|_| "41.67".to_owned()),
                cantidad_almacen: None,
                cantidad_vitrina: en_vitrina.map(str::to_owned),
                stock_minimo: None,
                objetivo_vitrina: None,
            })
            .expect("registrar")
            .0
    }

    fn presentacion_de(&self, producto: i64) -> i64 {
        ConsultarProducto::nuevo(&self.productos)
            .ejecutar(producto)
            .expect("ficha")
            .presentaciones[0]
            .id
    }

    fn abrir_caja(&self) {
        AbrirCaja::nuevo(&self.productos)
            .ejecutar(ComandoAbrirCaja {
                operador: "Yaneisy".to_owned(),
                fondo_inicial: "500.00".to_owned(),
            })
            .expect("abrir la caja");
    }

    /// Vende `cuantas` unidades pagando de sobra.
    fn vender(&self, producto: i64, cuantas: &str) {
        Vender::nuevo(&self.productos)
            .ejecutar(ComandoVender {
                lineas: vec![LineaPedida {
                    producto,
                    presentacion: self.presentacion_de(producto),
                    cantidad: cuantas.to_owned(),
                }],
                pagos: vec![PagoPedido {
                    metodo: "EFECTIVO_CUP".to_owned(),
                    entregado: "10000.00".to_owned(),
                }],
                espera_id: None,
            })
            .expect("cobrar");
    }

    fn catalogo(&self, incluir_retirados: bool) -> Vec<ProductoListado> {
        ListarProductos::nuevo(&self.productos)
            .ejecutar(incluir_retirados)
            .expect("listar el catálogo")
    }

    fn nombres(&self, incluir_retirados: bool) -> Vec<String> {
        self.catalogo(incluir_retirados)
            .into_iter()
            .map(|producto| producto.nombre)
            .collect()
    }
}

// ================================================= borrar lo que nunca se movió

#[test]
fn una_etiqueta_que_nunca_se_movio_se_borra_entera() {
    let entorno = Entorno::en_memoria();
    let producto = entorno.alta("Mango", None);

    // Se le cambió el precio un par de veces antes de darse por vencido:
    // eso deja historial de precio, que sí se borra con él.
    CambiarPrecio::nuevo(&entorno.productos)
        .ejecutar(ComandoCambiarPrecio {
            producto,
            presentacion: entorno.presentacion_de(producto),
            precio: "95.00".to_owned(),
        })
        .expect("cambiar el precio");

    assert_eq!(entorno.filas("historial_precio"), 1);

    let hecho = QuitarDelCatalogo::nuevo(&entorno.productos)
        .ejecutar(producto)
        .expect("quitar");

    assert_eq!(hecho.destino, Destino::Borrar);

    // Ni la ficha, ni sus presentaciones, ni sus dos filas de existencia,
    // ni su historial de precios.
    assert_eq!(entorno.filas("producto"), 0);
    assert_eq!(entorno.filas("presentacion"), 0);
    assert_eq!(entorno.filas("existencia"), 0);
    assert_eq!(entorno.filas("historial_precio"), 0);
}

#[test]
fn borrar_una_etiqueta_libera_su_sku() {
    // El motivo práctico de borrar en vez de retirar: el SKU es único, y
    // un retirado se lo queda. Si mañana vuelve el mango, su código tiene
    // que poder ser `MANGO` otra vez y no `MANGO-2`.
    let entorno = Entorno::en_memoria();
    let producto = entorno.alta("Mango", None);

    QuitarDelCatalogo::nuevo(&entorno.productos)
        .ejecutar(producto)
        .expect("quitar");

    let vuelve = entorno.alta("Mango", None);
    let ficha = ConsultarProducto::nuevo(&entorno.productos)
        .ejecutar(vuelve)
        .expect("ficha");

    assert_eq!(ficha.sku, "MANGO");
}

#[test]
fn un_producto_con_el_costo_fijado_a_mano_se_retira_y_guarda_su_revaluacion() {
    // Fijar un costo a mano exige existencia, y la existencia exige una
    // entrada, que deja asiento. Así que un producto con revaluaciones
    // tiene kárdex por construcción y nunca cae en el grupo de los
    // borrables: se retira, y su revaluación se queda donde está.
    let entorno = Entorno::en_memoria();
    let producto = entorno.alta("Mango", Some("4"));

    CambiarCosto::nuevo(&entorno.productos)
        .ejecutar(ComandoCambiarCosto {
            producto,
            costo: "50.00".to_owned(),
            clave: CLAVE_MANTENIMIENTO.to_owned(),
        })
        .expect("revalorar");

    // Se va la mercancía por merma, así que queda en cero. El kárdex, en
    // cambio, se queda con dos asientos.
    RegistrarMerma::nuevo(&entorno.productos)
        .ejecutar(ComandoRegistrarMerma {
            producto,
            origen: "VITRINA".to_owned(),
            cantidad: "4".to_owned(),
            motivo: "Se echó a perder".to_owned(),
        })
        .expect("mermar");

    assert_eq!(entorno.filas("historial_costo"), 1);

    let hecho = QuitarDelCatalogo::nuevo(&entorno.productos)
        .ejecutar(producto)
        .expect("quitar");

    assert_eq!(hecho.destino, Destino::Retirar);
    assert_eq!(hecho.movimientos, 2);
    assert_eq!(entorno.filas("historial_costo"), 1);
}

// ============================================ RF-CAT-07: lo que vendió no se borra

#[test]
fn un_producto_que_vendio_se_retira_y_no_pierde_ni_una_venta() {
    let entorno = Entorno::en_memoria();
    entorno.abrir_caja();
    let producto = entorno.alta("Refresco 500 ml", Some("3"));

    // Se vende hasta la última lata: queda en cero, con ventas y kárdex.
    entorno.vender(producto, "3");

    let hecho = QuitarDelCatalogo::nuevo(&entorno.productos)
        .ejecutar(producto)
        .expect("quitar");

    assert_eq!(hecho.destino, Destino::Retirar);
    assert_eq!(hecho.ventas, 1);

    // La ficha sigue, fuera de la venta, y la contabilidad intacta.
    assert_eq!(entorno.filas("producto"), 1);
    assert_eq!(entorno.filas("venta_linea"), 1);
    assert_eq!(entorno.filas("movimiento"), 2);
    assert!(entorno.nombres(false).is_empty());
    assert_eq!(entorno.nombres(true), ["Refresco 500 ml"]);
}

#[test]
fn la_base_rechaza_el_borrado_de_un_producto_con_ventas() {
    // La garantía de RF-CAT-07 no es el caso de uso: es el esquema. Esta
    // prueba se salta la aplicación y pide el borrado directo al puerto,
    // que es lo que haría un error de programación mañana.
    use application::puertos::RepositorioProducto;
    use domain::IdProducto;

    let entorno = Entorno::en_memoria();
    entorno.abrir_caja();
    let producto = entorno.alta("Refresco 500 ml", Some("3"));
    entorno.vender(producto, "3");

    let error = entorno
        .productos
        .limpiar_catalogo(&[IdProducto(producto)], &[])
        .expect_err("la clave foránea tiene que morder");

    assert_eq!(error.codigo(), "PERSISTENCIA");

    // Y lo que importa: la transacción se deshizo entera. No quedó un
    // producto sin presentaciones ni una existencia huérfana.
    assert_eq!(entorno.filas("producto"), 1);
    assert_eq!(entorno.filas("presentacion"), 1);
    assert_eq!(entorno.filas("existencia"), 2);
    assert_eq!(entorno.filas("venta_linea"), 1);
}

#[test]
fn con_mercancia_encima_no_se_quita_del_catalogo() {
    let entorno = Entorno::en_memoria();
    let producto = entorno.alta("Refresco 500 ml", Some("20"));

    let error = QuitarDelCatalogo::nuevo(&entorno.productos)
        .ejecutar(producto)
        .expect_err("todavía hay refrescos");

    assert_eq!(error.codigo(), "PRODUCTO_CON_EXISTENCIA");
    assert_eq!(entorno.nombres(false), ["Refresco 500 ml"]);
}

// ======================================== la venta apartada frena las dos cosas

#[test]
fn un_producto_apartado_en_una_venta_sin_cobrar_no_se_toca() {
    let entorno = Entorno::en_memoria();
    let producto = entorno.alta("Refresco 500 ml", Some("3"));
    let presentacion = entorno.presentacion_de(producto);

    // Se aparta la venta con las tres latas y después se van por merma: el
    // producto queda en cero con una venta a medias apuntándole.
    DejarVentaEnEspera::nuevo(&entorno.productos, &entorno.esperas)
        .ejecutar(ComandoDejarEnEspera {
            nota: Some("Para Lázaro".to_owned()),
            lineas: vec![LineaPedida {
                producto,
                presentacion,
                cantidad: "3".to_owned(),
            }],
        })
        .expect("dejar en espera");

    RegistrarMerma::nuevo(&entorno.productos)
        .ejecutar(ComandoRegistrarMerma {
            producto,
            origen: "VITRINA".to_owned(),
            cantidad: "3".to_owned(),
            motivo: "Se rompieron".to_owned(),
        })
        .expect("mermar");

    let error = QuitarDelCatalogo::nuevo(&entorno.productos)
        .ejecutar(producto)
        .expect_err("hay una venta sin cobrar");

    assert_eq!(error.codigo(), "PRODUCTO_EN_VENTA_EN_ESPERA");
    // Sigue a la venta: ni borrado ni retirado.
    assert_eq!(entorno.nombres(false), ["Refresco 500 ml"]);
}

// ================================================== la limpieza de golpe

#[test]
fn la_limpieza_borra_unos_retira_otros_y_deja_el_resto() {
    let entorno = Entorno::en_memoria();
    entorno.abrir_caja();

    // Cuatro productos, uno de cada clase.
    let mango = entorno.alta("Mango", None); // etiqueta pura
    let refresco = entorno.alta("Refresco 500 ml", Some("3")); // se agota vendiendo
    let cafe = entorno.alta("Café", Some("10")); // sigue habiendo
    let aceite = entorno.alta("Aceite", Some("2")); // apartado sin cobrar

    entorno.vender(refresco, "3");

    DejarVentaEnEspera::nuevo(&entorno.productos, &entorno.esperas)
        .ejecutar(ComandoDejarEnEspera {
            nota: None,
            lineas: vec![LineaPedida {
                producto: aceite,
                presentacion: entorno.presentacion_de(aceite),
                cantidad: "2".to_owned(),
            }],
        })
        .expect("dejar en espera");

    RegistrarMerma::nuevo(&entorno.productos)
        .ejecutar(ComandoRegistrarMerma {
            producto: aceite,
            origen: "VITRINA".to_owned(),
            cantidad: "2".to_owned(),
            motivo: "Derrame".to_owned(),
        })
        .expect("mermar");

    let prevista = LimpiarCatalogo::nuevo(&entorno.productos)
        .previsualizar()
        .expect("previsualizar");

    assert_eq!(prevista.se_borran.len(), 1, "solo el mango");
    assert_eq!(prevista.se_borran[0].id, mango);
    assert_eq!(prevista.se_retiran.len(), 1, "solo el refresco");
    assert_eq!(prevista.se_retiran[0].id, refresco);
    assert_eq!(prevista.bloqueados.len(), 1, "solo el aceite");
    assert_eq!(prevista.bloqueados[0].id, aceite);

    let hecha = LimpiarCatalogo::nuevo(&entorno.productos)
        .ejecutar()
        .expect("limpiar");

    assert_eq!(hecha.se_borran.len(), 1);
    assert_eq!(hecha.se_retiran.len(), 1);

    // El mango se fue del todo; el refresco se esconde; el café y el
    // aceite siguen a la venta.
    let mut quedan = entorno.nombres(false);
    quedan.sort();
    assert_eq!(quedan, ["Aceite", "Café"]);

    let mut todos = entorno.nombres(true);
    todos.sort();
    assert_eq!(todos, ["Aceite", "Café", "Refresco 500 ml"]);
    assert_eq!(entorno.filas("venta_linea"), 1, "la venta del refresco");

    // Lo del café no se tocó.
    assert!(entorno
        .catalogo(false)
        .iter()
        .any(|p| p.id == cafe && p.existencia_total == "10"));
}

#[test]
fn una_segunda_limpieza_no_vuelve_a_proponer_lo_ya_retirado() {
    // Lo retirado con historial está donde debe estar. Ofrecerlo otra vez
    // sería pedir confirmación de algo que ya se hizo.
    let entorno = Entorno::en_memoria();
    entorno.abrir_caja();
    let refresco = entorno.alta("Refresco 500 ml", Some("3"));
    entorno.vender(refresco, "3");

    LimpiarCatalogo::nuevo(&entorno.productos)
        .ejecutar()
        .expect("primera limpieza");

    let segunda = LimpiarCatalogo::nuevo(&entorno.productos)
        .previsualizar()
        .expect("previsualizar");

    assert!(segunda.no_hay_nada());
}

#[test]
fn un_catalogo_lleno_no_propone_ninguna_limpieza() {
    let entorno = Entorno::en_memoria();
    entorno.alta("Café", Some("10"));

    let prevista = LimpiarCatalogo::nuevo(&entorno.productos)
        .ejecutar()
        .expect("limpiar");

    assert!(prevista.no_hay_nada());
    assert_eq!(entorno.nombres(false), ["Café"]);
}
