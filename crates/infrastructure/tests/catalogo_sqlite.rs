//! Recorrido completo del catálogo contra SQLite de verdad.
//!
//! Estas pruebas atraviesan caso de uso, dominio y base de datos. No usan
//! dobles: si el SKU derivado choca, el desempate se comprueba contra la
//! tabla real, que es donde el `UNIQUE` puede morder.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use application::casos::vender::CLAVE_TASA;
use application::casos::{
    AbrirCaja, AnularVenta, BorrarTodo, CerrarCaja, ComandoAbrirCaja, ComandoAnularVenta,
    ComandoCerrarCaja, ComandoMoverEfectivo, ComandoVender, ConsultarCaja, ConsultarInforme,
    ConsultarVenta, ConsultarVentas, HistorialCajas, LineaPedida, MoverEfectivo, PagoPedido,
    Vender, CLAVE_MANTENIMIENTO,
};
use application::casos::{
    AgregarPresentacion, CambiarPrecio, ComandoAgregarPresentacion, ComandoCambiarPrecio,
    ComandoEditarProducto, ComandoFijarObjetivo, ComandoPresentacion, ComandoRegistrarEntrada,
    ComandoRegistrarMerma, ComandoRegistrarProducto, ComandoSimular, ComandoTraspasar,
    ConsultarAlmacen, ConsultarHistorialPrecios, ConsultarKardex, ConsultarProducto,
    ConsultarVitrina, DesactivarPresentacion, EditarProducto, FijarObjetivoVitrina, LineaKardex,
    ListarProductos, MarcarPredeterminada, ProductoListado, RegistrarEntrada, RegistrarMerma,
    RegistrarProducto, ResumenVitrina, SimularMovimiento, Traspasar,
};
use application::puertos::RepositorioProducto;
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

// ==================================================== la venta

/// Un producto listo para vender: 20 latas en vitrina a 41,67 de costo.
/// Abre una caja para poder cobrar (RF-CAJ-06).
fn abrir_caja(repositorio: &RepositorioProductoSqlite) -> i64 {
    AbrirCaja::nuevo(repositorio)
        .ejecutar(ComandoAbrirCaja {
            operador: "Yaneisy".to_owned(),
            fondo_inicial: "500.00".to_owned(),
        })
        .expect("abrir la caja")
}

/// Producto listo para vender, **con la caja ya abierta**.
///
/// La caja va aquí y no en cada prueba porque sin ella no se puede cobrar,
/// y lo que estas pruebas miden es la venta, no el turno. El caso de cobrar
/// sin caja tiene su propia prueba.
fn producto_en_vitrina(repositorio: &RepositorioProductoSqlite) -> (i64, i64) {
    abrir_caja(repositorio);

    let mut comando = alta("Refresco 500 ml");
    comando.precio_unitario = "80.00".to_owned();
    comando.costo_unitario = Some("41.67".to_owned());
    comando.cantidad_vitrina = Some("20".to_owned());

    let producto = RegistrarProducto::nuevo(repositorio)
        .ejecutar(comando)
        .expect("registrar")
        .0;

    let ficha = ConsultarProducto::nuevo(repositorio)
        .ejecutar(producto)
        .expect("ficha");

    (producto, ficha.presentaciones[0].id)
}

fn efectivo(cantidad: &str) -> PagoPedido {
    PagoPedido {
        metodo: "EFECTIVO_CUP".to_owned(),
        entregado: cantidad.to_owned(),
    }
}

#[test]
fn una_venta_descuenta_de_la_vitrina_y_deja_su_asiento() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    let hecha = Vender::nuevo(&repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![LineaPedida {
                producto,
                presentacion,
                cantidad: "3".to_owned(),
            }],
            pagos: vec![efectivo("300.00")],
        })
        .expect("cobrar");

    assert_eq!(hecha.folio, 1);
    assert_eq!(hecha.total, "240.00");
    assert_eq!(hecha.vuelto, "60.00");

    let ficha = unico(&repositorio);
    assert_eq!(ficha.en_vitrina, "17");
    assert_eq!(ficha.en_almacen, "0");

    let historial = kardex(&repositorio, producto);
    assert_eq!(historial[0].tipo, "VENTA");
    assert_eq!(historial[0].origen.as_deref(), Some("Vitrina"));
    assert_eq!(historial[0].destino, None);
    assert_eq!(historial[0].cantidad, "3");
    // El asiento congela el costo del momento, no el precio.
    assert_eq!(historial[0].costo_unitario, "41.67");
}

#[test]
fn vender_un_paquete_descuenta_las_unidades_que_lleva_dentro() {
    let repositorio = repositorio_en_memoria();
    let (producto, _) = producto_en_vitrina(&repositorio);

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
        .expect("ficha");
    let six_pack = ficha.presentaciones[1].id;

    Vender::nuevo(&repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![LineaPedida {
                producto,
                presentacion: six_pack,
                cantidad: "2".to_owned(),
            }],
            pagos: vec![efectivo("600.00")],
        })
        .expect("cobrar");

    // Dos six-packs son doce refrescos: la existencia es una sola.
    assert_eq!(unico(&repositorio).en_vitrina, "8");
    assert_eq!(kardex(&repositorio, producto)[0].cantidad, "12");
}

#[test]
fn no_se_vende_lo_que_no_esta_en_la_vitrina() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    let error = Vender::nuevo(&repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![LineaPedida {
                producto,
                presentacion,
                cantidad: "25".to_owned(),
            }],
            pagos: vec![efectivo("2000.00")],
        })
        .expect_err("solo hay 20 en vitrina");

    assert_eq!(error.codigo(), "EXISTENCIA_INSUFICIENTE");
    // Nada se movió: la venta entera se deshizo.
    assert_eq!(unico(&repositorio).en_vitrina, "20");
    assert_eq!(kardex(&repositorio, producto).len(), 1);
}

#[test]
fn no_se_cobra_una_venta_con_lo_que_no_alcanza() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    let error = Vender::nuevo(&repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![LineaPedida {
                producto,
                presentacion,
                cantidad: "3".to_owned(),
            }],
            pagos: vec![efectivo("100.00")],
        })
        .expect_err("240 no se pagan con 100");

    assert_eq!(error.codigo(), "PAGO_INSUFICIENTE");
    assert_eq!(unico(&repositorio).en_vitrina, "20");
}

#[test]
fn un_cobro_mixto_se_suma_y_el_vuelto_sale_en_pesos() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    repositorio
        .guardar_configuracion(CLAVE_TASA, "420.00")
        .expect("fijar la tasa");

    // Venta de 800: paga 200 en efectivo, 100 por transferencia y 2 dólares.
    let hecha = Vender::nuevo(&repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![LineaPedida {
                producto,
                presentacion,
                cantidad: "10".to_owned(),
            }],
            pagos: vec![
                efectivo("200.00"),
                PagoPedido {
                    metodo: "TRANSFERENCIA".to_owned(),
                    entregado: "100.00".to_owned(),
                },
                PagoPedido {
                    metodo: "EFECTIVO_USD".to_owned(),
                    entregado: "2.00".to_owned(),
                },
            ],
        })
        .expect("cobrar");

    // 200 + 100 + (2 × 420) = 1 140 por una venta de 800.
    assert_eq!(hecha.total, "800.00");
    assert_eq!(hecha.entregado, "1140.00");
    assert_eq!(hecha.vuelto, "340.00");
}

#[test]
fn cobrar_en_dolares_exige_tener_la_tasa_puesta() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    let error = Vender::nuevo(&repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![LineaPedida {
                producto,
                presentacion,
                cantidad: "1".to_owned(),
            }],
            pagos: vec![PagoPedido {
                metodo: "EFECTIVO_USD".to_owned(),
                entregado: "5.00".to_owned(),
            }],
        })
        .expect_err("sin tasa no se puede convertir");

    assert_eq!(error.codigo(), "TASA_NO_CONFIGURADA");
}

#[test]
fn el_folio_es_consecutivo() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    for esperado in 1..=3 {
        let hecha = Vender::nuevo(&repositorio)
            .ejecutar(ComandoVender {
                lineas: vec![LineaPedida {
                    producto,
                    presentacion,
                    cantidad: "1".to_owned(),
                }],
                pagos: vec![efectivo("80.00")],
            })
            .expect("cobrar");

        assert_eq!(hecha.folio, esperado);
    }
}

#[test]
fn dos_lineas_del_mismo_producto_se_descuentan_las_dos() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    Vender::nuevo(&repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![
                LineaPedida {
                    producto,
                    presentacion,
                    cantidad: "3".to_owned(),
                },
                LineaPedida {
                    producto,
                    presentacion,
                    cantidad: "4".to_owned(),
                },
            ],
            pagos: vec![efectivo("560.00")],
        })
        .expect("cobrar");

    // 20 − 3 − 4 = 13, y un solo asiento por las siete que salieron.
    assert_eq!(unico(&repositorio).en_vitrina, "13");
    let historial = kardex(&repositorio, producto);
    assert_eq!(historial[0].tipo, "VENTA");
    assert_eq!(historial[0].cantidad, "7");
}

// ======================================================== historial de ventas

/// Cobra una venta de `cuantas` unidades pagando justo, y devuelve su id.
fn venta_de(
    repositorio: &RepositorioProductoSqlite,
    producto: i64,
    presentacion: i64,
    cuantas: &str,
    paga: &str,
) -> i64 {
    Vender::nuevo(repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![LineaPedida {
                producto,
                presentacion,
                cantidad: cuantas.to_owned(),
            }],
            pagos: vec![efectivo(paga)],
        })
        .expect("cobrar")
        .folio
}

#[test]
fn las_ventas_se_listan_de_la_mas_reciente_a_la_mas_vieja() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    venta_de(&repositorio, producto, presentacion, "1", "80.00");
    venta_de(&repositorio, producto, presentacion, "2", "160.00");
    venta_de(&repositorio, producto, presentacion, "3", "240.00");

    let historial = ConsultarVentas::nuevo(&repositorio)
        .ejecutar(100)
        .expect("listar las ventas");

    // La última cobrada encabeza la lista: es la que se va a consultar.
    let folios: Vec<i64> = historial.ventas.iter().map(|v| v.folio).collect();
    assert_eq!(folios, vec![3, 2, 1]);
    assert_eq!(historial.ventas[0].total, "240.00");
}

#[test]
fn el_resumen_del_dia_suma_lo_cobrado_y_lo_ganado() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    // Precio 80, costo 41.67: cada unidad deja 38.33 de ganancia.
    venta_de(&repositorio, producto, presentacion, "1", "80.00");
    venta_de(&repositorio, producto, presentacion, "2", "160.00");

    let historial = ConsultarVentas::nuevo(&repositorio)
        .ejecutar(100)
        .expect("listar las ventas");

    assert_eq!(historial.hoy.cuantas, 2);
    assert_eq!(historial.hoy.total, "240.00");
    // 240 − (3 × 41.67) = 240 − 125.01
    assert_eq!(historial.hoy.ganancia, "114.99");
}

#[test]
fn el_detalle_trae_las_lineas_y_las_formas_de_pago() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    repositorio
        .guardar_configuracion(CLAVE_TASA, "420.00")
        .expect("fijar la tasa");

    Vender::nuevo(&repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![LineaPedida {
                producto,
                presentacion,
                cantidad: "10".to_owned(),
            }],
            pagos: vec![
                efectivo("200.00"),
                PagoPedido {
                    metodo: "EFECTIVO_USD".to_owned(),
                    entregado: "2.00".to_owned(),
                },
            ],
        })
        .expect("cobrar");

    let detalle = ConsultarVenta::nuevo(&repositorio)
        .ejecutar(1)
        .expect("el detalle de la venta");

    assert_eq!(detalle.folio, 1);
    assert_eq!(detalle.total, "800.00");
    // 200 + (2 × 420) = 1 040
    assert_eq!(detalle.entregado, "1040.00");
    assert_eq!(detalle.vuelto, "240.00");

    assert_eq!(detalle.lineas.len(), 1);
    assert_eq!(detalle.lineas[0].nombre_producto, "Refresco 500 ml");
    assert_eq!(detalle.lineas[0].cantidad, "10");
    assert_eq!(detalle.lineas[0].importe, "800.00");
    // 10 × 41.67 = 416.70
    assert_eq!(detalle.lineas[0].costo, "416.70");
    assert_eq!(detalle.lineas[0].ganancia, "383.30");

    assert_eq!(detalle.pagos.len(), 2);
    assert_eq!(detalle.pagos[0].metodo, "EFECTIVO_CUP");
    assert_eq!(detalle.pagos[0].tasa, None);
    // La tasa viaja congelada con el pago en dólares (RF-VTA-10b).
    assert_eq!(detalle.pagos[1].metodo, "EFECTIVO_USD");
    assert_eq!(detalle.pagos[1].entregado, "2.00");
    assert_eq!(detalle.pagos[1].tasa.as_deref(), Some("420.00"));
    assert_eq!(detalle.pagos[1].equivalente, "840.00");
}

#[test]
fn la_ganancia_de_una_venta_no_cambia_cuando_sube_el_costo() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    venta_de(&repositorio, producto, presentacion, "10", "800.00");

    // Llega mercancía mucho más cara: el costo promedio del producto sube.
    RegistrarEntrada::nuevo(&repositorio)
        .ejecutar(ComandoRegistrarEntrada {
            producto,
            cantidad: "100".to_owned(),
            costo_unitario: "70.00".to_owned(),
            destino: "BODEGA".to_owned(),
        })
        .expect("entrada cara");

    let detalle = ConsultarVenta::nuevo(&repositorio)
        .ejecutar(1)
        .expect("el detalle de la venta");

    // La venta de ayer sigue diciendo lo que dejó ayer. Si esto cambiara,
    // la ganancia del negocio se reescribiría sola cada vez que llega una
    // remesa (RF-VTA-13).
    assert_eq!(detalle.lineas[0].costo, "416.70");
    assert_eq!(detalle.ganancia, "383.30");
}

#[test]
fn pedir_una_venta_que_no_existe_da_un_error_claro() {
    let repositorio = repositorio_en_memoria();

    let error = ConsultarVenta::nuevo(&repositorio)
        .ejecutar(404)
        .expect_err("esa venta no existe");

    assert_eq!(error.codigo(), "NO_ENCONTRADO");
}

// ============================================================ caja (RF-CAJ)

fn cerrar(
    repositorio: &RepositorioProductoSqlite,
    contado_cup: &str,
    contado_usd: &str,
) -> application::casos::CierreCalculado {
    CerrarCaja::nuevo(repositorio)
        .ejecutar(ComandoCerrarCaja {
            contado_cup: contado_cup.to_owned(),
            contado_usd: contado_usd.to_owned(),
            modo: "SEPARADO".to_owned(),
        })
        .expect("cerrar la caja")
}

#[test]
fn sin_caja_abierta_no_se_cobra() {
    let repositorio = repositorio_en_memoria();
    // Ojo: se registra el producto SIN abrir caja.
    let mut comando = alta("Refresco 500 ml");
    comando.precio_unitario = "80.00".to_owned();
    comando.costo_unitario = Some("41.67".to_owned());
    comando.cantidad_vitrina = Some("20".to_owned());
    let producto = RegistrarProducto::nuevo(&repositorio)
        .ejecutar(comando)
        .expect("registrar")
        .0;
    let ficha = ConsultarProducto::nuevo(&repositorio)
        .ejecutar(producto)
        .expect("ficha");

    let error = Vender::nuevo(&repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![LineaPedida {
                producto,
                presentacion: ficha.presentaciones[0].id,
                cantidad: "1".to_owned(),
            }],
            pagos: vec![efectivo("80.00")],
        })
        .expect_err("sin caja no se cobra");

    // RF-CAJ-06: una venta sin sesión no aparecería en ningún arqueo.
    assert_eq!(error.codigo(), "SIN_SESION_ABIERTA");
    // Y no dejó rastro: la vitrina sigue intacta.
    assert_eq!(unico(&repositorio).en_vitrina, "20");
}

#[test]
fn no_se_abren_dos_cajas_a_la_vez() {
    let repositorio = repositorio_en_memoria();
    abrir_caja(&repositorio);

    let error = AbrirCaja::nuevo(&repositorio)
        .ejecutar(ComandoAbrirCaja {
            operador: "Otro".to_owned(),
            fondo_inicial: "100.00".to_owned(),
        })
        .expect_err("ya hay una abierta");

    assert_eq!(error.codigo(), "SESION_YA_ABIERTA");
}

#[test]
fn el_efectivo_esperado_no_cuenta_la_transferencia() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    // Una venta de 800 en efectivo justo y otra de 800 por transferencia.
    venta_de(&repositorio, producto, presentacion, "10", "800.00");
    Vender::nuevo(&repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![LineaPedida {
                producto,
                presentacion,
                cantidad: "10".to_owned(),
            }],
            pagos: vec![PagoPedido {
                metodo: "TRANSFERENCIA".to_owned(),
                entregado: "800.00".to_owned(),
            }],
        })
        .expect("cobrar por transferencia");

    let caja = ConsultarCaja::nuevo(&repositorio)
        .ejecutar()
        .expect("consultar")
        .expect("hay caja abierta");

    // Se vendieron 1 600, pero en la gaveta solo hay el fondo más los 800
    // en billetes: la transferencia fue a una cuenta (RF-CAJ-04).
    assert_eq!(caja.desglose.total_en_pesos, "1600.00");
    assert_eq!(caja.desglose.transferencia, "800.00");
    assert_eq!(caja.efectivo_esperado, "1300.00");
}

#[test]
fn el_vuelto_de_un_pago_en_dolares_sale_de_la_gaveta_de_pesos() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);
    repositorio
        .guardar_configuracion(CLAVE_TASA, "420.00")
        .expect("fijar la tasa");

    // Venta de 800: paga 200 en efectivo y 2 USD (840). Vuelto: 240.
    Vender::nuevo(&repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![LineaPedida {
                producto,
                presentacion,
                cantidad: "10".to_owned(),
            }],
            pagos: vec![
                efectivo("200.00"),
                PagoPedido {
                    metodo: "EFECTIVO_USD".to_owned(),
                    entregado: "2.00".to_owned(),
                },
            ],
        })
        .expect("cobrar");

    let caja = ConsultarCaja::nuevo(&repositorio)
        .ejecutar()
        .expect("consultar")
        .expect("hay caja");

    // Entraron 200 pesos y salieron 240 de vuelto: la gaveta perdió 40.
    // Fondo 500 − 40 = 460. Si el arqueo usara la venta diría 700.
    assert_eq!(caja.efectivo_esperado, "460.00");
    assert_eq!(caja.dolares_esperados, "2.00");
    assert_eq!(caja.desglose.total_consolidado, "800.00");
}

#[test]
fn las_entradas_y_salidas_mueven_el_efectivo_esperado() {
    let repositorio = repositorio_en_memoria();
    abrir_caja(&repositorio);

    MoverEfectivo::nuevo(&repositorio)
        .ejecutar(ComandoMoverEfectivo {
            tipo: "SALIDA".to_owned(),
            importe: "200.00".to_owned(),
            motivo: "pago de la luz".to_owned(),
        })
        .expect("salida");

    MoverEfectivo::nuevo(&repositorio)
        .ejecutar(ComandoMoverEfectivo {
            tipo: "ENTRADA".to_owned(),
            importe: "50.00".to_owned(),
            motivo: "ingreso de cambio".to_owned(),
        })
        .expect("entrada");

    let caja = ConsultarCaja::nuevo(&repositorio)
        .ejecutar()
        .expect("consultar")
        .expect("hay caja");

    // 500 − 200 + 50 = 350
    assert_eq!(caja.efectivo_esperado, "350.00");
    assert_eq!(caja.movimientos.len(), 2);
    assert_eq!(caja.movimientos[0].motivo, "ingreso de cambio");
}

#[test]
fn un_movimiento_de_efectivo_sin_motivo_no_pasa() {
    let repositorio = repositorio_en_memoria();
    abrir_caja(&repositorio);

    let error = MoverEfectivo::nuevo(&repositorio)
        .ejecutar(ComandoMoverEfectivo {
            tipo: "SALIDA".to_owned(),
            importe: "200.00".to_owned(),
            motivo: "   ".to_owned(),
        })
        .expect_err("sin motivo");

    assert_eq!(error.codigo(), "MOTIVO_OBLIGATORIO");
}

#[test]
fn el_cierre_marca_el_faltante_y_el_sobrante() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);
    venta_de(&repositorio, producto, presentacion, "10", "800.00");

    // Esperado: 500 de fondo + 800 en billetes = 1 300. Se cuentan 1 250.
    let cierre = cerrar(&repositorio, "1250.00", "0.00");

    assert_eq!(cierre.arqueo_cup.esperado, "1300.00");
    assert_eq!(cierre.arqueo_cup.contado, "1250.00");
    assert_eq!(cierre.arqueo_cup.diferencia, "-50.00");
    assert!(!cierre.arqueo_cup.sobra);
    assert!(!cierre.cuadra);
}

#[test]
fn el_cierre_trae_el_resumen_economico() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);
    // Precio 80, costo 41.67: 10 unidades dejan 383.30 de ganancia.
    venta_de(&repositorio, producto, presentacion, "10", "800.00");

    repositorio
        .guardar_configuracion("comision_operador", "5")
        .expect("fijar la comisión");

    let cierre = cerrar(&repositorio, "1300.00", "0.00");

    assert_eq!(cierre.economico.venta_total, "800.00");
    assert_eq!(cierre.economico.costo_vendido, "416.70");
    assert_eq!(cierre.economico.ganancia_bruta, "383.30");
    // La comisión sale de la VENTA, no de la ganancia: 5 % de 800 = 40.
    assert_eq!(cierre.economico.comision, "40.00");
    assert_eq!(cierre.economico.ganancia_neta, "343.30");
}

#[test]
fn una_caja_cerrada_es_inmutable() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);
    venta_de(&repositorio, producto, presentacion, "10", "800.00");

    repositorio
        .guardar_configuracion("comision_operador", "5")
        .expect("comisión al 5 %");
    let cierre = cerrar(&repositorio, "1300.00", "0.00");
    let sesion = cierre.sesion;

    // El dueño sube la comisión al 20 % después de cerrar.
    repositorio
        .guardar_configuracion("comision_operador", "20")
        .expect("comisión al 20 %");

    let guardado = HistorialCajas::nuevo(&repositorio)
        .cierre(sesion)
        .expect("leer el cierre");

    // RF-CAJ-08 y D-6: lo liquidado es lo liquidado.
    assert_eq!(guardado.economico.comision, "40.00");
    assert_eq!(guardado.economico.venta_total, "800.00");
    assert_eq!(guardado.arqueo_cup.esperado, "1300.00");
}

#[test]
fn no_se_cierra_dos_veces_la_misma_caja() {
    let repositorio = repositorio_en_memoria();
    abrir_caja(&repositorio);
    cerrar(&repositorio, "500.00", "0.00");

    let error = CerrarCaja::nuevo(&repositorio)
        .ejecutar(ComandoCerrarCaja {
            contado_cup: "500.00".to_owned(),
            contado_usd: "0.00".to_owned(),
            modo: "SEPARADO".to_owned(),
        })
        .expect_err("ya está cerrada");

    assert_eq!(error.codigo(), "SIN_SESION_ABIERTA");
}

#[test]
fn anular_devuelve_la_mercancia_a_la_vitrina() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);
    venta_de(&repositorio, producto, presentacion, "10", "800.00");

    assert_eq!(unico(&repositorio).en_vitrina, "10");
    let valor_antes = unico(&repositorio).costo.clone();

    AnularVenta::nuevo(&repositorio)
        .ejecutar(ComandoAnularVenta {
            venta: 1,
            motivo: "el cliente se arrepintió".to_owned(),
        })
        .expect("anular");

    // Vuelven las 10 unidades y el costo promedio no se mueve: entraron al
    // mismo costo con que salieron.
    assert_eq!(unico(&repositorio).en_vitrina, "20");
    assert_eq!(unico(&repositorio).costo, valor_antes);

    // Y queda su asiento en el kárdex, con el motivo.
    let historial = kardex(&repositorio, producto);
    assert_eq!(historial[0].tipo, "DEVOLUCION");
    assert_eq!(
        historial[0].motivo.as_deref(),
        Some("el cliente se arrepintió")
    );
}

#[test]
fn una_venta_anulada_no_cuenta_para_el_arqueo() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);
    venta_de(&repositorio, producto, presentacion, "10", "800.00");

    AnularVenta::nuevo(&repositorio)
        .ejecutar(ComandoAnularVenta {
            venta: 1,
            motivo: "error de cobro".to_owned(),
        })
        .expect("anular");

    let caja = ConsultarCaja::nuevo(&repositorio)
        .ejecutar()
        .expect("consultar")
        .expect("hay caja");

    // La venta desaparece de las cuentas y el efectivo vuelve al fondo.
    assert_eq!(caja.cuantas_ventas, 0);
    assert_eq!(caja.desglose.total_en_pesos, "0.00");
    assert_eq!(caja.efectivo_esperado, "500.00");
}

#[test]
fn no_se_anula_dos_veces() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);
    venta_de(&repositorio, producto, presentacion, "1", "80.00");

    let anular = || {
        AnularVenta::nuevo(&repositorio).ejecutar(ComandoAnularVenta {
            venta: 1,
            motivo: "error".to_owned(),
        })
    };

    anular().expect("la primera sí");
    assert_eq!(
        anular().expect_err("la segunda no").codigo(),
        "VENTA_YA_ANULADA"
    );
}

#[test]
fn no_se_anula_una_venta_de_una_caja_ya_cerrada() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);
    venta_de(&repositorio, producto, presentacion, "1", "80.00");

    cerrar(&repositorio, "580.00", "0.00");
    abrir_caja(&repositorio);

    let error = AnularVenta::nuevo(&repositorio)
        .ejecutar(ComandoAnularVenta {
            venta: 1,
            motivo: "tarde".to_owned(),
        })
        .expect_err("la caja de esa venta ya se arqueó");

    // D-5: el arqueo de una sesión cerrada es intocable.
    assert_eq!(error.codigo(), "SESION_CERRADA");
    assert_eq!(unico(&repositorio).en_vitrina, "19");
}

#[test]
fn la_venta_total_del_cierre_suma_las_tres_formas_de_cobro() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);
    repositorio
        .guardar_configuracion(CLAVE_TASA, "420.00")
        .expect("fijar la tasa");

    // Venta de 800 pagada con 200 en efectivo, 100 por transferencia y
    // 2 USD (840). Entregó 1 140; el vuelto son 340.
    Vender::nuevo(&repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![LineaPedida {
                producto,
                presentacion,
                cantidad: "10".to_owned(),
            }],
            pagos: vec![
                efectivo("200.00"),
                PagoPedido {
                    metodo: "TRANSFERENCIA".to_owned(),
                    entregado: "100.00".to_owned(),
                },
                PagoPedido {
                    metodo: "EFECTIVO_USD".to_owned(),
                    entregado: "2.00".to_owned(),
                },
            ],
        })
        .expect("cobrar");

    // Sin indicar modo: el cierre va consolidado.
    let cierre = CerrarCaja::nuevo(&repositorio)
        .ejecutar(ComandoCerrarCaja {
            contado_cup: "0.00".to_owned(),
            contado_usd: "2.00".to_owned(),
            modo: String::new(),
        })
        .expect("cerrar");

    assert_eq!(cierre.modo, "CONSOLIDADO");

    // La venta total es la suma de las tres, con los dólares en pesos.
    let d = &cierre.desglose;
    assert_eq!(d.transferencia, "100.00");
    assert_eq!(d.efectivo_usd_en_cup, "840.00");
    // 800 − 100 − 840 = −140 imputados a efectivo: el cliente pagó de más
    // en divisa y se le devolvió en pesos.
    assert_eq!(d.efectivo_cup, "-140.00");
    assert_eq!(cierre.economico.venta_total, "800.00");

    // Y los dólares se siguen arqueando aparte, en su moneda.
    assert_eq!(cierre.arqueo_usd.esperado, "2.00");
    assert!(cierre.arqueo_usd.cuadra);
}

#[test]
fn el_cierre_explica_de_donde_sale_lo_esperado() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    // Fondo 500 (lo pone abrir_caja), una venta de 800 en efectivo justo,
    // una salida de 200 y una entrada de 50.
    venta_de(&repositorio, producto, presentacion, "10", "800.00");

    MoverEfectivo::nuevo(&repositorio)
        .ejecutar(ComandoMoverEfectivo {
            tipo: "SALIDA".to_owned(),
            importe: "200.00".to_owned(),
            motivo: "pago de la luz".to_owned(),
        })
        .expect("salida");
    MoverEfectivo::nuevo(&repositorio)
        .ejecutar(ComandoMoverEfectivo {
            tipo: "ENTRADA".to_owned(),
            importe: "50.00".to_owned(),
            motivo: "ingreso de cambio".to_owned(),
        })
        .expect("entrada");

    let cierre = CerrarCaja::nuevo(&repositorio)
        .previsualizar(&ComandoCerrarCaja {
            contado_cup: "0.00".to_owned(),
            contado_usd: "0.00".to_owned(),
            modo: String::new(),
        })
        .expect("previsualizar");

    // Las cuatro piezas tienen que sumar exactamente lo esperado, o el
    // desglose sería un adorno que no explica nada.
    assert_eq!(cierre.fondo_inicial, "500.00");
    assert_eq!(cierre.ventas_efectivo, "800.00");
    assert_eq!(cierre.entradas, "50.00");
    assert_eq!(cierre.salidas, "200.00");
    // 500 + 800 + 50 − 200 = 1 150
    assert_eq!(cierre.arqueo_cup.esperado, "1150.00");
}

// ========================================================= informes (RF-EST)

/// Rango que abarca cualquier fecha: las pruebas no dependen del reloj.
const SIEMPRE: (&str, &str) = ("2000-01-01", "2999-12-31");

fn informe(repositorio: &RepositorioProductoSqlite, dias: i64) -> application::casos::Informe {
    ConsultarInforme::nuevo(repositorio)
        .ejecutar(SIEMPRE.0, SIEMPRE.1, dias)
        .expect("armar el informe")
}

#[test]
fn el_informe_separa_venta_ganancia_bruta_y_neta() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    repositorio
        .guardar_configuracion("comision_operador", "5")
        .expect("comisión al 5 %");

    // Precio 80, costo 41.67. Diez unidades: 800 de venta, 416.70 de costo.
    venta_de(&repositorio, producto, presentacion, "10", "800.00");

    let resumen = informe(&repositorio, 7).resumen;

    // RF-EST-13: los tres niveles no se confunden.
    assert_eq!(resumen.venta, "800.00");
    assert_eq!(resumen.costo, "416.70");
    assert_eq!(resumen.ganancia_bruta, "383.30");
    // 5 % de 800 = 40, y la neta descuenta eso de la bruta.
    assert_eq!(resumen.comision, "40.00");
    assert_eq!(resumen.ganancia_neta, "343.30");
    assert_eq!(resumen.ticket_promedio, "800.00");
    assert!(!resumen.en_perdida);
}

#[test]
fn el_ticket_promedio_reparte_entre_las_ventas() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    venta_de(&repositorio, producto, presentacion, "5", "400.00");
    venta_de(&repositorio, producto, presentacion, "5", "400.00");
    venta_de(&repositorio, producto, presentacion, "10", "800.00");

    let resumen = informe(&repositorio, 7).resumen;

    assert_eq!(resumen.cuantas_ventas, 3);
    assert_eq!(resumen.venta, "1600.00");
    // 1 600 ÷ 3
    assert_eq!(resumen.ticket_promedio, "533.33");
}

#[test]
fn lo_mas_vendido_se_cuenta_en_unidad_base() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    // Se añade un six-pack para vender el mismo producto de dos formas.
    AgregarPresentacion::nuevo(&repositorio)
        .ejecutar(ComandoAgregarPresentacion {
            producto,
            nombre: "Six-pack".to_owned(),
            factor: "6".to_owned(),
            precio: "450.00".to_owned(),
            codigo_barras: None,
        })
        .expect("agregar el six-pack");

    let ficha = ConsultarProducto::nuevo(&repositorio)
        .ejecutar(producto)
        .expect("ficha");
    let paquete = ficha
        .presentaciones
        .iter()
        .find(|p| p.nombre == "Six-pack")
        .expect("el six-pack")
        .id;

    // 2 sueltas + 1 six-pack = 8 unidades base, no 3 «cosas».
    Vender::nuevo(&repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![
                LineaPedida {
                    producto,
                    presentacion,
                    cantidad: "2".to_owned(),
                },
                LineaPedida {
                    producto,
                    presentacion: paquete,
                    cantidad: "1".to_owned(),
                },
            ],
            pagos: vec![efectivo("610.00")],
        })
        .expect("cobrar");

    let vendidos = informe(&repositorio, 7).mas_vendidos;

    // RF-EST-03: el suelto y el paquete tienen que ser comparables.
    assert_eq!(vendidos.len(), 1);
    assert_eq!(vendidos[0].cantidad, "8");
    // 2 × 80 + 1 × 450 = 610
    assert_eq!(vendidos[0].importe, "610.00");
    // La barra del primero siempre llena: es el techo de su serie.
    assert_eq!(vendidos[0].peso, 1000);
}

#[test]
fn lo_que_no_se_vende_aparece_con_su_capital_detenido() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    // Un segundo producto que nadie compra, con mercancía pagada dentro.
    let mut parado = alta("Vino de mesa");
    parado.precio_unitario = "900.00".to_owned();
    parado.costo_unitario = Some("500.00".to_owned());
    parado.cantidad_almacen = Some("4".to_owned());
    RegistrarProducto::nuevo(&repositorio)
        .ejecutar(parado)
        .expect("registrar");

    venta_de(&repositorio, producto, presentacion, "1", "80.00");

    let datos = informe(&repositorio, 7);

    // RF-EST-05: 4 × 500 son 2 000 pesos gastados que siguen en el estante.
    assert_eq!(datos.sin_movimiento.len(), 1);
    assert_eq!(datos.sin_movimiento[0].nombre, "Vino de mesa");
    assert_eq!(datos.sin_movimiento[0].capital, "2000.00");
    assert_eq!(datos.capital_parado, "2000.00");
}

#[test]
fn los_dias_de_cobertura_salen_del_ritmo_de_venta() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    // Arranca con 20 en vitrina. Vende 10 en un periodo de 5 días: el
    // ritmo es 2 al día y quedan 10, así que aguanta 5 días.
    venta_de(&repositorio, producto, presentacion, "10", "800.00");

    let agotarse = informe(&repositorio, 5).por_agotarse;

    assert_eq!(agotarse.len(), 1);
    assert_eq!(agotarse[0].existencia, "10");
    assert_eq!(agotarse[0].venta_diaria, "2");
    assert_eq!(agotarse[0].dias_cobertura, Some(5));
    // RF-EST-06: menos de una semana es para actuar hoy.
    assert!(agotarse[0].critico);
}

#[test]
fn el_reparto_por_metodo_suma_el_cien_por_ciento() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    // Mitad en efectivo, mitad por transferencia.
    venta_de(&repositorio, producto, presentacion, "5", "400.00");
    Vender::nuevo(&repositorio)
        .ejecutar(ComandoVender {
            lineas: vec![LineaPedida {
                producto,
                presentacion,
                cantidad: "5".to_owned(),
            }],
            pagos: vec![PagoPedido {
                metodo: "TRANSFERENCIA".to_owned(),
                entregado: "400.00".to_owned(),
            }],
        })
        .expect("cobrar por transferencia");

    let metodos = informe(&repositorio, 7).por_metodo;

    assert_eq!(metodos.len(), 2);
    for metodo in &metodos {
        assert_eq!(metodo.porcentaje, "50.0");
        assert_eq!(metodo.peso, 500);
    }
}

#[test]
fn un_periodo_sin_ventas_lo_dice_en_vez_de_enseñar_ceros() {
    let repositorio = repositorio_en_memoria();
    producto_en_vitrina(&repositorio);

    let datos = informe(&repositorio, 7);

    assert!(datos.sin_datos);
    assert_eq!(datos.resumen.venta, "0.00");
    // Sin ventas no se divide por cero en ningún sitio.
    assert_eq!(datos.resumen.ticket_promedio, "0.00");
    assert_eq!(datos.resumen.margen, "0.00");
    assert!(datos.por_dia.is_empty());
}

#[test]
fn el_informe_compara_el_ultimo_dia_con_el_anterior() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);

    // Dos ventas en el mismo día: no hay con qué comparar todavía.
    venta_de(&repositorio, producto, presentacion, "5", "400.00");
    venta_de(&repositorio, producto, presentacion, "5", "400.00");

    let datos = informe(&repositorio, 7);

    // Un solo día con ventas: la comparativa no se inventa nada.
    assert!(datos.comparativa.is_none());
    assert_eq!(datos.por_dia.len(), 1);
}

#[test]
fn sin_dia_anterior_no_hay_porcentaje_que_calcular() {
    // La variación se calcula sobre la venta del día anterior; si aquella
    // fue cero, dividir daría una cifra inventada.
    let repositorio = repositorio_en_memoria();
    producto_en_vitrina(&repositorio);

    assert!(informe(&repositorio, 7).comparativa.is_none());
}

// ==================================================== mantenimiento

#[test]
fn la_clave_equivocada_no_borra_nada() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);
    venta_de(&repositorio, producto, presentacion, "1", "80.00");

    let error = BorrarTodo::nuevo(&repositorio)
        .ejecutar("000000")
        .expect_err("la clave no es esa");

    assert_eq!(error.codigo(), "CLAVE_INCORRECTA");
    // Y todo sigue en su sitio: la comprobación va ANTES de tocar nada.
    assert_eq!(catalogo(&repositorio).len(), 1);
    assert_eq!(
        ConsultarVentas::nuevo(&repositorio)
            .ejecutar(10)
            .expect("listar")
            .ventas
            .len(),
        1
    );
}

#[test]
fn con_la_clave_correcta_la_aplicacion_queda_vacia() {
    let repositorio = repositorio_en_memoria();
    let (producto, presentacion) = producto_en_vitrina(&repositorio);
    venta_de(&repositorio, producto, presentacion, "1", "80.00");
    repositorio
        .guardar_configuracion(CLAVE_TASA, "420.00")
        .expect("fijar la tasa");

    BorrarTodo::nuevo(&repositorio)
        .ejecutar(CLAVE_MANTENIMIENTO)
        .expect("borrar");

    assert!(catalogo(&repositorio).is_empty());
    assert!(repositorio.sesion_abierta().expect("consultar").is_none());
    assert!(repositorio
        .configuracion(CLAVE_TASA)
        .expect("consultar")
        .is_none());
    assert_eq!(
        ConsultarVentas::nuevo(&repositorio)
            .ejecutar(10)
            .expect("listar")
            .ventas
            .len(),
        0
    );
}

#[test]
fn despues_de_vaciar_la_aplicacion_se_puede_volver_a_usar() {
    let repositorio = repositorio_en_memoria();
    producto_en_vitrina(&repositorio);

    BorrarTodo::nuevo(&repositorio)
        .ejecutar(CLAVE_MANTENIMIENTO)
        .expect("borrar");

    // El esquema sigue en pie: se vaciaron las filas, no las tablas.
    let (producto, presentacion) = producto_en_vitrina(&repositorio);
    venta_de(&repositorio, producto, presentacion, "1", "80.00");

    // Y el folio arranca de nuevo en 1, porque no quedó ninguno.
    let historial = ConsultarVentas::nuevo(&repositorio)
        .ejecutar(10)
        .expect("listar");
    assert_eq!(historial.ventas[0].folio, 1);
}
