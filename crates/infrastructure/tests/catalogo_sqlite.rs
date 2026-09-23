//! Recorrido completo del catálogo contra SQLite de verdad.
//!
//! Estas pruebas atraviesan caso de uso, dominio y base de datos. No usan
//! dobles: si el SKU derivado choca, el desempate se comprueba contra la
//! tabla real, que es donde el `UNIQUE` puede morder.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use application::casos::{
    AgregarPresentacion, CambiarPrecio, ComandoAgregarPresentacion, ComandoCambiarPrecio,
    ComandoEditarProducto, ComandoFijarObjetivo, ComandoPresentacion, ComandoRegistrarEntrada,
    ComandoRegistrarMerma, ComandoRegistrarProducto, ComandoSimular, ComandoTraspasar,
    ConsultarAlmacen, ConsultarHistorialPrecios, ConsultarKardex, ConsultarProducto,
    ConsultarVitrina, DesactivarPresentacion, EditarProducto, FijarObjetivoVitrina, LineaKardex,
    ListarProductos, MarcarPredeterminada, ProductoListado, RegistrarEntrada, RegistrarMerma,
    RegistrarProducto, ResumenVitrina, SimularMovimiento, Traspasar,
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

// ============================================== movimientos y kárdex

/// Alta con mercancía, que es el punto de partida de casi toda prueba de
/// almacén: 50 libras a 120 el costo.
fn alta_con_mercancia(repositorio: &RepositorioProductoSqlite) -> i64 {
    let mut comando = alta("Arroz blanco");
    comando.unidad_base = "lb".to_owned();
    comando.precio_unitario = "180.00".to_owned();
    comando.costo_unitario = Some("120.00".to_owned());
    comando.cantidad_almacen = Some("50".to_owned());

    RegistrarProducto::nuevo(repositorio)
        .ejecutar(comando)
        .expect("registrar el producto")
        .0
}

fn kardex(repositorio: &RepositorioProductoSqlite, producto: i64) -> Vec<LineaKardex> {
    ConsultarKardex::nuevo(repositorio)
        .ejecutar(producto, 50)
        .expect("consultar el kárdex")
}

#[test]
fn la_mercancia_de_apertura_deja_su_asiento() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    let historial = kardex(&repositorio, producto);
    assert_eq!(historial.len(), 1);
    assert_eq!(historial[0].tipo, "ENTRADA");
    assert_eq!(historial[0].cantidad, "50.000");
    assert_eq!(historial[0].costo_unitario, "120.00");
    assert_eq!(historial[0].importe, "6000.00");
    assert_eq!(historial[0].destino.as_deref(), Some("Bodega"));
    assert_eq!(historial[0].bodega_resultante, "50.000");
}

#[test]
fn una_entrada_recalcula_el_costo_promedio_ponderado() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    // Segunda compra más cara: 50 lb a 160.
    RegistrarEntrada::nuevo(&repositorio)
        .ejecutar(ComandoRegistrarEntrada {
            producto,
            cantidad: "50".to_owned(),
            costo_unitario: "160.00".to_owned(),
            destino: "BODEGA".to_owned(),
        })
        .expect("registrar la entrada");

    // (6 000 + 8 000) / 100 = 140 exactos.
    let ficha = unico(&repositorio);
    assert_eq!(ficha.costo, "140.00");
    assert_eq!(ficha.en_almacen, "100.000");

    // El asiento guarda el costo de ESA compra, no el promedio resultante.
    let historial = kardex(&repositorio, producto);
    assert_eq!(historial.len(), 2);
    assert_eq!(historial[0].costo_unitario, "160.00");
    assert_eq!(historial[0].bodega_resultante, "100.000");
}

#[test]
fn una_merma_descuenta_existencia_y_valor_sin_mover_el_costo() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    RegistrarMerma::nuevo(&repositorio)
        .ejecutar(ComandoRegistrarMerma {
            producto,
            cantidad: "10".to_owned(),
            origen: "BODEGA".to_owned(),
            motivo: "Se mojó con la lluvia".to_owned(),
        })
        .expect("registrar la merma");

    let ficha = unico(&repositorio);
    assert_eq!(ficha.en_almacen, "40.000");
    // El costo unitario no cambia al salir mercancía (RF-COS-06).
    assert_eq!(ficha.costo, "120.00");

    let historial = kardex(&repositorio, producto);
    assert_eq!(historial[0].tipo, "MERMA");
    assert_eq!(historial[0].origen.as_deref(), Some("Bodega"));
    assert_eq!(historial[0].destino, None);
    assert_eq!(
        historial[0].motivo.as_deref(),
        Some("Se mojó con la lluvia")
    );
    assert_eq!(historial[0].importe, "1200.00");
    assert_eq!(historial[0].bodega_resultante, "40.000");
}

#[test]
fn no_se_puede_mermar_mas_de_lo_que_hay() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    let error = RegistrarMerma::nuevo(&repositorio)
        .ejecutar(ComandoRegistrarMerma {
            producto,
            cantidad: "60".to_owned(),
            origen: "BODEGA".to_owned(),
            motivo: "Se mojó con la lluvia".to_owned(),
        })
        .expect_err("no hay 60 libras que perder");

    assert_eq!(error.codigo(), "EXISTENCIA_INSUFICIENTE");
    // Nada se movió: la operación entera se deshizo.
    assert_eq!(unico(&repositorio).en_almacen, "50.000");
    assert_eq!(kardex(&repositorio, producto).len(), 1);
}

#[test]
fn no_hay_merma_sin_motivo() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    let error = RegistrarMerma::nuevo(&repositorio)
        .ejecutar(ComandoRegistrarMerma {
            producto,
            cantidad: "1".to_owned(),
            origen: "BODEGA".to_owned(),
            motivo: "   ".to_owned(),
        })
        .expect_err("la mercancía no desaparece sin explicación");

    assert_eq!(error.codigo(), "MOTIVO_OBLIGATORIO");
}

#[test]
fn el_almacen_suma_el_valor_de_todo_el_inventario() {
    let repositorio = repositorio_en_memoria();
    alta_con_mercancia(&repositorio);

    let mut otro = alta("Refresco 500 ml");
    otro.precio_unitario = "145.00".to_owned();
    otro.costo_unitario = Some("100.00".to_owned());
    otro.cantidad_almacen = Some("12".to_owned());
    RegistrarProducto::nuevo(&repositorio)
        .ejecutar(otro)
        .expect("registrar el segundo producto");

    let resumen = ConsultarAlmacen::nuevo(&repositorio)
        .ejecutar()
        .expect("consultar el almacén");

    // 50 × 120 + 12 × 100 = 7 200.
    assert_eq!(resumen.valor_total, "7200.00");
    assert_eq!(resumen.con_existencia, 2);
    assert_eq!(resumen.agotados, 0);
}

#[test]
fn el_kardex_de_un_producto_que_no_existe_no_inventa_nada() {
    let repositorio = repositorio_en_memoria();

    let error = ConsultarKardex::nuevo(&repositorio)
        .ejecutar(9999, 50)
        .expect_err("ese producto no existe");

    assert_eq!(error.codigo(), "NO_ENCONTRADO");
}

// ============================================== traspasos y previsión

#[test]
fn un_traspaso_mueve_cantidad_sin_tocar_el_valor() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    Traspasar::nuevo(&repositorio)
        .ejecutar(ComandoTraspasar {
            producto,
            cantidad: "12".to_owned(),
            origen: "BODEGA".to_owned(),
        })
        .expect("traspasar a la vitrina");

    let ficha = unico(&repositorio);
    assert_eq!(ficha.en_almacen, "38.000");
    assert_eq!(ficha.en_vitrina, "12.000");
    // El total y el costo no se mueven: la mercancía solo cambió de sitio.
    assert_eq!(ficha.existencia_total, "50.000");
    assert_eq!(ficha.costo, "120.00");

    let historial = kardex(&repositorio, producto);
    assert_eq!(historial[0].tipo, "TRASPASO");
    assert_eq!(historial[0].origen.as_deref(), Some("Bodega"));
    assert_eq!(historial[0].destino.as_deref(), Some("Vitrina"));
    assert_eq!(historial[0].bodega_resultante, "38.000");
    assert_eq!(historial[0].vitrina_resultante, "12.000");
}

#[test]
fn el_valor_del_almacen_no_cambia_al_traspasar() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    let antes = ConsultarAlmacen::nuevo(&repositorio)
        .ejecutar()
        .expect("consultar antes");

    Traspasar::nuevo(&repositorio)
        .ejecutar(ComandoTraspasar {
            producto,
            cantidad: "20".to_owned(),
            origen: "BODEGA".to_owned(),
        })
        .expect("traspasar");

    let despues = ConsultarAlmacen::nuevo(&repositorio)
        .ejecutar()
        .expect("consultar después");

    assert_eq!(antes.valor_total, despues.valor_total);
}

#[test]
fn no_se_traspasa_mas_de_lo_que_hay() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    let error = Traspasar::nuevo(&repositorio)
        .ejecutar(ComandoTraspasar {
            producto,
            cantidad: "80".to_owned(),
            origen: "BODEGA".to_owned(),
        })
        .expect_err("no hay 80 libras en bodega");

    assert_eq!(error.codigo(), "EXISTENCIA_INSUFICIENTE");
    assert_eq!(unico(&repositorio).en_almacen, "50.000");
}

#[test]
fn la_vista_previa_dice_como_quedaria() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    let simulacion = SimularMovimiento::nuevo(&repositorio)
        .ejecutar(ComandoSimular {
            producto,
            tipo: "TRASPASO".to_owned(),
            cantidad: "12".to_owned(),
            ubicacion: "BODEGA".to_owned(),
        })
        .expect("simular");

    assert!(simulacion.posible);
    assert_eq!(simulacion.disponible, "50.000");
    assert_eq!(simulacion.bodega_resultante, "38.000");
    assert_eq!(simulacion.vitrina_resultante, "12.000");

    // Y simular no cambia nada.
    assert_eq!(unico(&repositorio).en_almacen, "50.000");
}

#[test]
fn la_vista_previa_avisa_antes_de_intentarlo() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    let simulacion = SimularMovimiento::nuevo(&repositorio)
        .ejecutar(ComandoSimular {
            producto,
            tipo: "MERMA".to_owned(),
            cantidad: "80".to_owned(),
            ubicacion: "BODEGA".to_owned(),
        })
        .expect("simular");

    assert!(!simulacion.posible);
    assert_eq!(
        simulacion.problema.as_deref(),
        Some("No alcanza: en bodega solo hay 50.000 lb")
    );
    // Cuando no se puede, se enseña la existencia actual sin cambios.
    assert_eq!(simulacion.bodega_resultante, "50.000");
}

// ==================================================== vitrina

fn vitrina(repositorio: &RepositorioProductoSqlite) -> ResumenVitrina {
    ConsultarVitrina::nuevo(repositorio)
        .ejecutar()
        .expect("consultar la vitrina")
}

fn fijar_objetivo(repositorio: &RepositorioProductoSqlite, producto: i64, objetivo: &str) {
    FijarObjetivoVitrina::nuevo(repositorio)
        .ejecutar(ComandoFijarObjetivo {
            producto,
            objetivo: objetivo.to_owned(),
        })
        .expect("fijar el objetivo");
}

#[test]
fn sin_objetivo_la_vitrina_no_sugiere_nada() {
    let repositorio = repositorio_en_memoria();
    alta_con_mercancia(&repositorio);

    let estado = vitrina(&repositorio);
    assert_eq!(estado.por_reponer, 0);
    assert!(!estado.productos[0].hay_que_reponer);
    // Pero sí avisa de que hay mercancía guardada que nadie ve.
    assert_eq!(estado.sin_exhibir, 1);
    assert!(estado.productos[0].disponible_sin_exhibir);
}

#[test]
fn sugiere_bajar_lo_que_falta_para_alcanzar_el_objetivo() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    fijar_objetivo(&repositorio, producto, "8");

    let estado = vitrina(&repositorio);
    assert_eq!(estado.productos[0].objetivo, "8.000");
    assert_eq!(estado.productos[0].en_vitrina, "0.000");
    assert_eq!(estado.productos[0].sugerido, "8.000");
    assert_eq!(estado.por_reponer, 1);
}

#[test]
fn la_sugerencia_descuenta_lo_que_ya_esta_exhibido() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    fijar_objetivo(&repositorio, producto, "8");
    Traspasar::nuevo(&repositorio)
        .ejecutar(ComandoTraspasar {
            producto,
            cantidad: "3".to_owned(),
            origen: "BODEGA".to_owned(),
        })
        .expect("bajar 3 a la vitrina");

    let estado = vitrina(&repositorio);
    assert_eq!(estado.productos[0].en_vitrina, "3.000");
    // Faltan 5 para los 8 que se quieren exhibir.
    assert_eq!(estado.productos[0].sugerido, "5.000");
    assert!(estado.productos[0].esta_exhibido);
}

#[test]
fn nunca_sugiere_bajar_mas_de_lo_que_hay_guardado() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    // Se quieren 200 exhibidas pero en el almacén solo hay 50.
    fijar_objetivo(&repositorio, producto, "200");

    let estado = vitrina(&repositorio);
    assert_eq!(estado.productos[0].sugerido, "50.000");
}

#[test]
fn alcanzado_el_objetivo_deja_de_sugerir() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    fijar_objetivo(&repositorio, producto, "5");
    Traspasar::nuevo(&repositorio)
        .ejecutar(ComandoTraspasar {
            producto,
            cantidad: "5".to_owned(),
            origen: "BODEGA".to_owned(),
        })
        .expect("bajar 5");

    let estado = vitrina(&repositorio);
    assert_eq!(estado.productos[0].sugerido, "0.000");
    assert!(!estado.productos[0].hay_que_reponer);
    assert_eq!(estado.por_reponer, 0);
    // Ya no está "guardado sin exhibir": hay algo en la vitrina.
    assert_eq!(estado.sin_exhibir, 0);
}

#[test]
fn el_objetivo_en_blanco_desactiva_la_sugerencia() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    fijar_objetivo(&repositorio, producto, "8");
    assert_eq!(vitrina(&repositorio).por_reponer, 1);

    fijar_objetivo(&repositorio, producto, "");
    assert_eq!(vitrina(&repositorio).por_reponer, 0);
}

#[test]
fn el_objetivo_no_altera_la_existencia() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    fijar_objetivo(&repositorio, producto, "8");

    // Cambiar una ficha no mueve mercancía ni deja asiento.
    let ficha = unico(&repositorio);
    assert_eq!(ficha.en_almacen, "50.000");
    assert_eq!(ficha.costo, "120.00");
    assert_eq!(kardex(&repositorio, producto).len(), 1);
}

#[test]
fn lo_que_esta_solo_en_vitrina_no_cuenta_como_por_reponer() {
    let repositorio = repositorio_en_memoria();

    // Mercancía que entró DIRECTA a la vitrina: el almacén queda vacío.
    let mut comando = alta("Refresco 500 ml");
    comando.costo_unitario = Some("100.00".to_owned());
    comando.cantidad_vitrina = Some("6".to_owned());
    let producto = RegistrarProducto::nuevo(&repositorio)
        .ejecutar(comando)
        .expect("registrar")
        .0;

    fijar_objetivo(&repositorio, producto, "20");

    let estado = vitrina(&repositorio);
    assert_eq!(estado.productos[0].en_vitrina, "6");
    assert_eq!(estado.productos[0].objetivo, "20");
    assert_eq!(estado.productos[0].en_almacen, "0");
    // Falta para el objetivo, pero NO hay nada que bajar: reponer sería
    // imposible. Lo que hace falta es comprar.
    assert_eq!(estado.productos[0].sugerido, "0");
    assert!(!estado.productos[0].hay_que_reponer);
    assert_eq!(estado.por_reponer, 0);
    // Pero sí se avisa de que hay que comprarlo: lo contrario sería decir
    // una verdad inútil y dejar al dueño creyendo que está todo bien.
    assert!(estado.productos[0].falta_comprar);
    assert_eq!(estado.falta_comprar, 1);
}

// ============================================ presentaciones y precios

/// El ejemplo de validación del documento (§6.2), tal cual.
///
/// Refresco 500 ml · unidad base `unidad` · costo promedio 41.67
///   Unidad suelta  factor 1  precio 80.00   → ganancia 38.33 · margen 47.9 %
///   Six-pack       factor 6  precio 300.00  → ganancia 50.00 · margen 16.7 %
#[test]
fn reproduce_el_ejemplo_de_presentaciones_del_documento() {
    let repositorio = repositorio_en_memoria();

    // 12 unidades por 500.04 dejan un costo de 41.67 exactos.
    let mut comando = alta("Refresco 500 ml");
    comando.precio_unitario = "80.00".to_owned();
    comando.costo_unitario = Some("41.67".to_owned());
    comando.cantidad_almacen = Some("12".to_owned());
    let producto = RegistrarProducto::nuevo(&repositorio)
        .ejecutar(comando)
        .expect("registrar")
        .0;

    AgregarPresentacion::nuevo(&repositorio)
        .ejecutar(ComandoAgregarPresentacion {
            producto,
            nombre: "Six-pack".to_owned(),
            factor: "6".to_owned(),
            precio: "300.00".to_owned(),
            codigo_barras: None,
        })
        .expect("agregar el six-pack");

    let ficha = ConsultarProducto::nuevo(&repositorio)
        .ejecutar(producto)
        .expect("consultar la ficha");

    assert_eq!(ficha.costo, "41.67");

    let suelta = &ficha.presentaciones[0];
    assert_eq!(suelta.precio, "80.00");
    assert_eq!(suelta.costo, "41.67");
    assert_eq!(suelta.ganancia, "38.33");
    assert_eq!(suelta.margen, "47.9 %");
    assert_eq!(suelta.precio_por_unidad_base, "80.00");

    let paquete = &ficha.presentaciones[1];
    assert_eq!(paquete.precio, "300.00");
    assert_eq!(paquete.costo, "250.02");
    assert_eq!(paquete.ganancia, "49.98");
    assert_eq!(paquete.margen, "16.7 %");
    // El six-pack sale a 50 por refresco frente a los 80 de la unidad
    // suelta: deja menos ganancia por refresco, y eso hay que verlo.
    assert_eq!(paquete.precio_por_unidad_base, "50.00");
}

#[test]
fn cambiar_el_precio_deja_rastro_en_el_historial() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);
    let ficha = ConsultarProducto::nuevo(&repositorio)
        .ejecutar(producto)
        .expect("ficha");
    let presentacion = ficha.presentaciones[0].id;

    CambiarPrecio::nuevo(&repositorio)
        .ejecutar(ComandoCambiarPrecio {
            producto,
            presentacion,
            precio: "210.00".to_owned(),
        })
        .expect("cambiar el precio");

    let ficha = ConsultarProducto::nuevo(&repositorio)
        .ejecutar(producto)
        .expect("ficha");
    assert_eq!(ficha.presentaciones[0].precio, "210.00");

    let historial = ConsultarHistorialPrecios::nuevo(&repositorio)
        .ejecutar(producto)
        .expect("historial");
    assert_eq!(historial.len(), 1);
    assert_eq!(historial[0].anterior, "180.00");
    assert_eq!(historial[0].nuevo, "210.00");
    assert!(historial[0].subio);
}

#[test]
fn no_se_puede_dejar_un_producto_sin_forma_de_venderse() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);
    let ficha = ConsultarProducto::nuevo(&repositorio)
        .ejecutar(producto)
        .expect("ficha");

    let error = DesactivarPresentacion::nuevo(&repositorio)
        .ejecutar(ComandoPresentacion {
            producto,
            presentacion: ficha.presentaciones[0].id,
        })
        .expect_err("es la única que queda");

    assert_eq!(error.codigo(), "ULTIMA_PRESENTACION");
}

#[test]
fn solo_una_presentacion_puede_ser_la_predeterminada() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    AgregarPresentacion::nuevo(&repositorio)
        .ejecutar(ComandoAgregarPresentacion {
            producto,
            nombre: "Saco de 25".to_owned(),
            factor: "25".to_owned(),
            precio: "4000.00".to_owned(),
            codigo_barras: None,
        })
        .expect("agregar el saco");

    let ficha = ConsultarProducto::nuevo(&repositorio)
        .ejecutar(producto)
        .expect("ficha");
    let saco = ficha.presentaciones[1].id;

    MarcarPredeterminada::nuevo(&repositorio)
        .ejecutar(ComandoPresentacion {
            producto,
            presentacion: saco,
        })
        .expect("marcar el saco");

    let ficha = ConsultarProducto::nuevo(&repositorio)
        .ejecutar(producto)
        .expect("ficha");
    assert!(!ficha.presentaciones[0].es_predeterminada);
    assert!(ficha.presentaciones[1].es_predeterminada);
}

#[test]
fn un_producto_por_unidades_no_admite_presentaciones_fraccionarias() {
    let repositorio = repositorio_en_memoria();
    let producto = RegistrarProducto::nuevo(&repositorio)
        .ejecutar(alta("Refresco 500 ml"))
        .expect("registrar")
        .0;

    let error = AgregarPresentacion::nuevo(&repositorio)
        .ejecutar(ComandoAgregarPresentacion {
            producto,
            nombre: "Media lata".to_owned(),
            factor: "0.5".to_owned(),
            precio: "50.00".to_owned(),
            codigo_barras: None,
        })
        .expect_err("no existe el paquete de media lata");

    assert_eq!(error.codigo(), "FACTOR_INVALIDO");
}

#[test]
fn desactivar_un_producto_lo_saca_del_catalogo_sin_borrarlo() {
    let repositorio = repositorio_en_memoria();
    let producto = alta_con_mercancia(&repositorio);

    EditarProducto::nuevo(&repositorio)
        .ejecutar(ComandoEditarProducto {
            producto,
            nombre: "Arroz blanco".to_owned(),
            stock_minimo: Some("10".to_owned()),
            activo: false,
        })
        .expect("desactivar");

    // Fuera de las listas de venta…
    assert!(catalogo(&repositorio).is_empty());
    // …pero su ficha y su historial siguen ahí.
    let ficha = ConsultarProducto::nuevo(&repositorio)
        .ejecutar(producto)
        .expect("la ficha sigue existiendo");
    assert!(!ficha.activo);
    assert_eq!(ficha.stock_minimo, "10.000");
    assert_eq!(kardex(&repositorio, producto).len(), 1);
}
