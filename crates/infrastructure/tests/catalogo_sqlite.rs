//! Recorrido completo del catálogo contra SQLite de verdad.
//!
//! Estas pruebas atraviesan caso de uso, dominio y base de datos. No usan
//! dobles: si el SKU derivado choca, el desempate se comprueba contra la
//! tabla real, que es donde el `UNIQUE` puede morder.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use application::casos::{
    ComandoRegistrarProducto, ListarProductos, ProductoListado, RegistrarProducto,
};
use infrastructure::{BaseDatos, RepositorioProductoSqlite};

/// Alta mínima: solo nombre y precio, con el SKU en blanco para que lo
/// derive el sistema (RF-CAT-02) y sin mercancía de apertura.
fn alta(nombre: &str) -> ComandoRegistrarProducto {
    ComandoRegistrarProducto {
        sku: String::new(),
        nombre: nombre.to_owned(),
        unidad_base: "unidad".to_owned(),
        precio_unitario: "180.00".to_owned(),
        costo_unitario: None,
        cantidad_almacen: None,
        cantidad_vitrina: None,
        stock_minimo: None,
        objetivo_vitrina: None,
    }
}

fn repositorio_en_memoria() -> RepositorioProductoSqlite {
    let base = Arc::new(BaseDatos::en_memoria().expect("abrir la base en memoria"));
    RepositorioProductoSqlite::nuevo(base)
}

/// Devuelve los SKU del catálogo, en el orden en que los lista la aplicación.
fn skus(repositorio: &RepositorioProductoSqlite) -> Vec<String> {
    catalogo(repositorio)
        .into_iter()
        .map(|producto| producto.sku)
        .collect()
}

fn catalogo(repositorio: &RepositorioProductoSqlite) -> Vec<ProductoListado> {
    ListarProductos::nuevo(repositorio)
        .ejecutar(false)
        .expect("listar el catálogo")
}

/// El único producto del catálogo.
fn unico(repositorio: &RepositorioProductoSqlite) -> ProductoListado {
    let mut productos = catalogo(repositorio);
    assert_eq!(productos.len(), 1, "se esperaba un solo producto");
    productos.remove(0)
}

#[test]
fn deriva_el_sku_del_nombre_y_lo_guarda() {
    let repositorio = repositorio_en_memoria();

    RegistrarProducto::nuevo(&repositorio)
        .ejecutar(alta("Café molido 1 kg"))
        .expect("registrar el producto");

    assert_eq!(skus(&repositorio), ["CAFE-MOLIDO-1-KG"]);
}

#[test]
fn desempata_cuando_dos_productos_se_llaman_igual() {
    let repositorio = repositorio_en_memoria();

    // El mismo arroz de dos proveedores distintos. No es un error del
    // usuario: el catálogo tiene que admitirlo.
    for _ in 0..3 {
        RegistrarProducto::nuevo(&repositorio)
            .ejecutar(alta("Arroz"))
            .expect("registrar el producto");
    }

    assert_eq!(skus(&repositorio), ["ARROZ", "ARROZ-2", "ARROZ-3"]);
}

#[test]
fn respeta_el_sku_que_escribio_el_usuario() {
    let repositorio = repositorio_en_memoria();

    let mut comando = alta("Arroz blanco");
    comando.sku = "ARR-001".to_owned();

    RegistrarProducto::nuevo(&repositorio)
        .ejecutar(comando)
        .expect("registrar el producto");

    // Se guarda tal cual: no se normaliza ni se le añade sufijo.
    assert_eq!(skus(&repositorio), ["ARR-001"]);
}

#[test]
fn rechaza_el_sku_escrito_a_mano_que_ya_existe() {
    let repositorio = repositorio_en_memoria();

    let mut primero = alta("Arroz blanco");
    primero.sku = "ARR-001".to_owned();
    RegistrarProducto::nuevo(&repositorio)
        .ejecutar(primero)
        .expect("registrar el primero");

    let mut repetido = alta("Arroz integral");
    repetido.sku = "ARR-001".to_owned();
    let error = RegistrarProducto::nuevo(&repositorio)
        .ejecutar(repetido)
        .expect_err("el SKU repetido tiene que fallar");

    assert_eq!(error.codigo(), "SKU_DUPLICADO");
    assert!(error.es_del_usuario());
    assert_eq!(skus(&repositorio), ["ARR-001"]);
}

#[test]
fn lo_guardado_sobrevive_a_cerrar_la_aplicacion() {
    let ruta = ruta_temporal("persistencia");
    limpiar(&ruta);

    {
        let base = Arc::new(BaseDatos::abrir(&ruta).expect("abrir la base en disco"));
        let repositorio = RepositorioProductoSqlite::nuevo(base);

        RegistrarProducto::nuevo(&repositorio)
            .ejecutar(alta("Piña en almíbar"))
            .expect("registrar el producto");
    } // Aquí se cierra la conexión, como al cerrar la aplicación.

    let base = Arc::new(BaseDatos::abrir(&ruta).expect("reabrir la base en disco"));
    let repositorio = RepositorioProductoSqlite::nuevo(base);

    assert_eq!(skus(&repositorio), ["PINA-EN-ALMIBAR"]);

    limpiar(&ruta);
}

// =========================================================== apertura

#[test]
fn guarda_la_mercancia_con_la_que_nace_el_producto() {
    let repositorio = repositorio_en_memoria();

    let mut comando = alta("Arroz blanco");
    comando.unidad_base = "lb".to_owned();
    comando.precio_unitario = "180.00".to_owned();
    comando.costo_unitario = Some("120.00".to_owned());
    comando.cantidad_almacen = Some("50".to_owned());
    comando.cantidad_vitrina = Some("5.5".to_owned());

    RegistrarProducto::nuevo(&repositorio)
        .ejecutar(comando)
        .expect("registrar el producto");

    let producto = unico(&repositorio);
    assert_eq!(producto.en_almacen, "50.000");
    assert_eq!(producto.en_vitrina, "5.500");
    assert_eq!(producto.existencia_total, "55.500");
    // El costo no se almacena: se deriva del valor invertido entre la
    // existencia. 6 660 / 55,5 = 120 exactos.
    assert_eq!(producto.costo, "120.00");
    assert_eq!(producto.ganancia, "60.00");
    assert_eq!(producto.margen, "33.3 %");
    assert!(!producto.en_riesgo);
    assert!(!producto.agotado);
}

#[test]
fn avisa_del_producto_que_se_vende_perdiendo() {
    let repositorio = repositorio_en_memoria();

    let mut comando = alta("Refresco 500 ml");
    comando.precio_unitario = "145.00".to_owned();
    comando.costo_unitario = Some("150.00".to_owned());
    comando.cantidad_almacen = Some("12".to_owned());

    RegistrarProducto::nuevo(&repositorio)
        .ejecutar(comando)
        .expect("registrar el producto");

    let producto = unico(&repositorio);
    assert_eq!(producto.ganancia, "-5.00");
    assert!(producto.en_riesgo, "el costo se comió el precio");
}

#[test]
fn el_producto_sin_mercancia_no_tiene_costo_ni_margen() {
    let repositorio = repositorio_en_memoria();

    RegistrarProducto::nuevo(&repositorio)
        .ejecutar(alta("Arroz blanco"))
        .expect("registrar el producto");

    let producto = unico(&repositorio);
    assert_eq!(producto.costo, "—");
    assert_eq!(producto.margen, "—");
    assert_eq!(producto.existencia_total, "0");
    assert!(producto.agotado);
}

#[test]
fn exige_el_costo_cuando_entra_mercancia() {
    let repositorio = repositorio_en_memoria();

    let mut comando = alta("Arroz blanco");
    comando.cantidad_almacen = Some("50".to_owned());

    let error = RegistrarProducto::nuevo(&repositorio)
        .ejecutar(comando)
        .expect_err("entrar mercancía sin costo tiene que fallar");

    assert_eq!(error.codigo(), "COSTO_REQUERIDO");
    assert!(error.es_del_usuario());
    assert!(
        catalogo(&repositorio).is_empty(),
        "no debe quedar nada guardado"
    );
}

#[test]
fn exige_la_cantidad_cuando_se_indica_un_costo() {
    let repositorio = repositorio_en_memoria();

    let mut comando = alta("Arroz blanco");
    comando.costo_unitario = Some("120.00".to_owned());

    let error = RegistrarProducto::nuevo(&repositorio)
        .ejecutar(comando)
        .expect_err("un costo sin mercancía tiene que fallar");

    assert_eq!(error.codigo(), "CANTIDAD_INICIAL_REQUERIDA");
}

// ============================================================= granel

#[test]
fn lo_que_se_cuenta_por_unidades_no_admite_fracciones() {
    let repositorio = repositorio_en_memoria();

    let mut comando = alta("Refresco 500 ml");
    comando.costo_unitario = Some("150.00".to_owned());
    comando.cantidad_almacen = Some("12.5".to_owned());

    let error = RegistrarProducto::nuevo(&repositorio)
        .ejecutar(comando)
        .expect_err("media lata no existe");

    assert_eq!(error.codigo(), "CANTIDAD_FRACCIONARIA_NO_PERMITIDA");
}

#[test]
fn la_unidad_decide_si_se_vende_en_fracciones() {
    // Es la regla acordada: `unidad` no se fracciona; todo lo demás sí. No
    // hay casilla que marcar ni estado intermedio posible.
    for (unidad, admite_fracciones) in [
        ("unidad", false),
        ("lb", true),
        ("kg", true),
        ("g", true),
        ("L", true),
        ("ml", true),
    ] {
        let repositorio = repositorio_en_memoria();

        let mut comando = alta("Producto de prueba");
        comando.unidad_base = unidad.to_owned();

        RegistrarProducto::nuevo(&repositorio)
            .ejecutar(comando)
            .expect("registrar el producto");

        assert_eq!(
            unico(&repositorio).es_granel,
            admite_fracciones,
            "la unidad «{unidad}» decide mal si se fracciona"
        );
    }
}

fn ruta_temporal(etiqueta: &str) -> PathBuf {
    std::env::temp_dir().join(format!("kilo12-{etiqueta}-{}.db", std::process::id()))
}

/// Borra el archivo y los auxiliares que deja el modo WAL.
fn limpiar(ruta: &Path) {
    for sufijo in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{sufijo}", ruta.display()));
    }
}
