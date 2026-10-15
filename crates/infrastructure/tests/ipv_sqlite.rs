//! El IPV contra SQLite de verdad (RF-IPV-01).
//!
//! Lo que se mide aquí no se puede medir con dobles. El doble de
//! `ConsultaIpv` recibe los saldos dictados; el trabajo difícil es
//! justamente el que aquí se comprueba:
//!
//! * que la CANT. INICIAL sea la existencia **en vitrina** que había justo
//!   antes de abrir el turno, leída del último asiento anterior y no
//!   recalculada;
//! * que lo que baja del almacén cuente como ENTRADA y lo que sube reste de
//!   esa misma columna;
//! * que lo que solo pasa en el almacén no ensucie ninguna columna;
//! * que la ventana sea la del turno y no la del calendario, igual que en el
//!   arqueo;
//! * que lo que se movió en otro turno no se cuele en las columnas;
//! * y que la identidad de la hoja —final = inicial + entrada − merma −
//!   consumo − salida— se cumpla sobre asientos escritos por los casos de
//!   uso de verdad, no por una prueba que los imite.

use std::sync::Arc;

use application::casos::{
    AbrirCaja, AgregarPresentacion, AnularConsumo, AnularVenta, CerrarCaja, ComandoAbrirCaja,
    ComandoAgregarPresentacion, ComandoAnularConsumo, ComandoAnularVenta, ComandoCerrarCaja,
    ComandoRegistrarConsumo, ComandoRegistrarEntrada, ComandoRegistrarMerma,
    ComandoRegistrarProducto, ComandoTraspasar, ComandoVender, ConsultarIpv, ConsultarProducto,
    FilaIpv, Ipv, LineaConsumoPedida, LineaPedida, PagoPedido, PeticionIpv, RegistrarConsumo,
    RegistrarEntrada, RegistrarMerma, RegistrarProducto, Traspasar, Vender, CLAVE_MANTENIMIENTO,
};
use infrastructure::{BaseDatos, RepositorioConsumoSqlite, RepositorioProductoSqlite};

/// Los dos repositorios sobre la MISMA base, como en la aplicación.
struct Entorno {
    base: Arc<BaseDatos>,
    productos: RepositorioProductoSqlite,
    consumos: RepositorioConsumoSqlite,
}

impl Entorno {
    fn nuevo() -> Self {
        let base = Arc::new(BaseDatos::en_memoria().expect("abrir la base en memoria"));
        Self {
            productos: RepositorioProductoSqlite::nuevo(Arc::clone(&base)),
            consumos: RepositorioConsumoSqlite::nuevo(Arc::clone(&base)),
            base,
        }
    }

    fn ejecutar_sql(&self, sql: &str) {
        self.base
            .con(|conexion| Ok(conexion.execute_batch(sql)?))
            .expect("retocar la base");
    }

    /// Envejece una hora todo lo escrito hasta ahora.
    ///
    /// `datetime('now', 'localtime')` tiene resolución de un segundo y una
    /// prueba escribe decenas de filas dentro del mismo. La ventana del
    /// turno se apoya en ese sello —igual que la merma del arqueo—, así que
    /// sin separar los tramos la prueba mediría la resolución del reloj y no
    /// la consulta. Mover TODO hacia atrás conserva el orden relativo de lo
    /// ya escrito y garantiza que lo siguiente quede estrictamente después.
    fn envejecer(&self) {
        self.ejecutar_sql(
            "UPDATE movimiento SET ocurrido_en = datetime(ocurrido_en, '-1 hour');
             UPDATE venta      SET ocurrido_en = datetime(ocurrido_en, '-1 hour');
             UPDATE sesion_caja
                SET abierta_en = datetime(abierta_en, '-1 hour'),
                    cerrada_en = datetime(cerrada_en, '-1 hour');",
        );
    }

    fn abrir_caja(&self) -> i64 {
        AbrirCaja::nuevo(&self.productos)
            .ejecutar(ComandoAbrirCaja {
                operador: "Yaneisy".to_owned(),
                fondo_inicial: "500.00".to_owned(),
            })
            .expect("abrir la caja")
    }

    fn cerrar_caja(&self) {
        CerrarCaja::nuevo(&self.productos)
            .ejecutar(ComandoCerrarCaja {
                contado_cup: "0.00".to_owned(),
                contado_usd: "0.00".to_owned(),
                modo: String::new(),
            })
            .expect("cerrar la caja");
    }

    /// Da de alta un producto con `vitrina` unidades y devuelve su id y el
    /// de su presentación predeterminada.
    fn producto(&self, nombre: &str, precio: &str, costo: &str, vitrina: &str) -> (i64, i64) {
        let producto = RegistrarProducto::nuevo(&self.productos)
            .ejecutar(ComandoRegistrarProducto {
                sku: String::new(),
                nombre: nombre.to_owned(),
                unidad_base: "unidad".to_owned(),
                precio_unitario: precio.to_owned(),
                costo_unitario: Some(costo.to_owned()),
                cantidad_almacen: None,
                cantidad_vitrina: Some(vitrina.to_owned()),
                stock_minimo: None,
                objetivo_vitrina: None,
            })
            .expect("registrar")
            .0;

        let ficha = ConsultarProducto::nuevo(&self.productos)
            .ejecutar(producto)
            .expect("ficha");
        (producto, ficha.presentaciones[0].id)
    }

    /// Igual, con mercancía también en el almacén.
    ///
    /// Hace falta para todo lo que mide la frontera entre los dos estantes:
    /// sin nada guardado no se puede bajar nada al mostrador.
    fn producto_surtido(
        &self,
        nombre: &str,
        precio: &str,
        costo: &str,
        vitrina: &str,
        almacen: &str,
    ) -> (i64, i64) {
        let producto = RegistrarProducto::nuevo(&self.productos)
            .ejecutar(ComandoRegistrarProducto {
                sku: String::new(),
                nombre: nombre.to_owned(),
                unidad_base: "unidad".to_owned(),
                precio_unitario: precio.to_owned(),
                costo_unitario: Some(costo.to_owned()),
                cantidad_almacen: Some(almacen.to_owned()),
                cantidad_vitrina: Some(vitrina.to_owned()),
                stock_minimo: None,
                objetivo_vitrina: None,
            })
            .expect("registrar")
            .0;

        let ficha = ConsultarProducto::nuevo(&self.productos)
            .ejecutar(producto)
            .expect("ficha");
        (producto, ficha.presentaciones[0].id)
    }

    /// Mueve mercancía de un estante al otro. `origen` es de dónde sale.
    fn traspasar(&self, producto: i64, cantidad: &str, origen: &str) {
        Traspasar::nuevo(&self.productos)
            .ejecutar(ComandoTraspasar {
                producto,
                cantidad: cantidad.to_owned(),
                origen: origen.to_owned(),
            })
            .expect("traspasar");
    }

    fn presentacion(&self, producto: i64, nombre: &str, factor: &str, precio: &str) -> i64 {
        AgregarPresentacion::nuevo(&self.productos)
            .ejecutar(ComandoAgregarPresentacion {
                producto,
                nombre: nombre.to_owned(),
                factor: factor.to_owned(),
                precio: precio.to_owned(),
                codigo_barras: None,
            })
            .expect("agregar presentación");

        ConsultarProducto::nuevo(&self.productos)
            .ejecutar(producto)
            .expect("ficha")
            .presentaciones
            .iter()
            .find(|p| p.nombre == nombre)
            .expect("la presentación nueva")
            .id
    }

    /// Cobra en efectivo pagando de sobra y devuelve el id de la venta.
    fn vender(&self, lineas: &[(i64, i64, &str)]) -> i64 {
        let folio = Vender::nuevo(&self.productos)
            .ejecutar(ComandoVender {
                lineas: lineas
                    .iter()
                    .map(|(producto, presentacion, cantidad)| LineaPedida {
                        producto: *producto,
                        presentacion: *presentacion,
                        cantidad: (*cantidad).to_owned(),
                    })
                    .collect(),
                pagos: vec![PagoPedido {
                    metodo: "EFECTIVO_CUP".to_owned(),
                    entregado: "100000.00".to_owned(),
                }],
                espera_id: None,
            })
            .expect("cobrar")
            .folio;

        self.base
            .con(|conexion| {
                Ok(
                    conexion.query_row("SELECT id FROM venta WHERE folio = ?1", [folio], |f| {
                        f.get(0)
                    })?,
                )
            })
            .expect("id de la venta")
    }

    fn entrada(&self, producto: i64, cantidad: &str, costo: &str) {
        RegistrarEntrada::nuevo(&self.productos)
            .ejecutar(ComandoRegistrarEntrada {
                producto,
                cantidad: cantidad.to_owned(),
                costo_unitario: costo.to_owned(),
                destino: "VITRINA".to_owned(),
            })
            .expect("entrada");
    }

    fn merma(&self, producto: i64, cantidad: &str) {
        RegistrarMerma::nuevo(&self.productos)
            .ejecutar(ComandoRegistrarMerma {
                producto,
                cantidad: cantidad.to_owned(),
                origen: "VITRINA".to_owned(),
                motivo: "se echó a perder".to_owned(),
            })
            .expect("merma");
    }

    /// Carga un consumo a la primera cuenta y devuelve su id.
    fn consumo(&self, producto: i64, presentacion: i64, cantidad: &str) -> i64 {
        RegistrarConsumo::nuevo(&self.productos, &self.consumos)
            .ejecutar(ComandoRegistrarConsumo {
                cuenta: 1,
                tomado_por: "Ivett".to_owned(),
                lineas: vec![LineaConsumoPedida {
                    producto,
                    presentacion,
                    cantidad: cantidad.to_owned(),
                    origen: "VITRINA".to_owned(),
                }],
            })
            .expect("consumir")
            .id
    }

    /// La hoja del turno abierto, que se mira sin permiso.
    fn ipv(&self) -> Ipv {
        self.pedir(&PeticionIpv::del_turno_abierto())
            .expect("armar el IPV")
    }

    /// La hoja de un turno concreto, con la clave del sistema.
    fn ipv_de(&self, sesion: i64) -> Ipv {
        self.pedir(&PeticionIpv {
            sesion: Some(sesion),
            clave: Some(CLAVE_MANTENIMIENTO.to_owned()),
        })
        .expect("armar el IPV")
    }

    fn pedir(&self, peticion: &PeticionIpv) -> Result<Ipv, application::ErrorAplicacion> {
        ConsultarIpv::nuevo(&self.productos, &self.productos, &self.productos).ejecutar(peticion)
    }
}

fn fila(hoja: &Ipv, producto: i64) -> &FilaIpv {
    hoja.filas
        .iter()
        .find(|f| f.producto == producto)
        .expect("fila del producto")
}

/// Toda fila de toda hoja tiene que explicarse a sí misma.
fn todas_cuadran(hoja: &Ipv) -> bool {
    hoja.filas.iter().all(|f| f.cuadra)
}

#[test]
fn sin_ninguna_caja_la_hoja_esta_vacia_y_no_es_un_error() {
    let entorno = Entorno::nuevo();
    entorno.producto("Refresco", "80.00", "40.00", "20");

    let hoja = entorno.ipv();

    assert_eq!(hoja.sesion, None);
    assert!(hoja.filas.is_empty());
    assert_eq!(hoja.importe_total, "0.00");
}

#[test]
fn la_hoja_de_un_turno_recien_abierto_ensena_el_catalogo_en_reposo() {
    let entorno = Entorno::nuevo();
    entorno.producto("Refresco", "80.00", "40.00", "20");
    entorno.producto("Arroz", "180.00", "100.00", "50");
    entorno.envejecer();
    let sesion = entorno.abrir_caja();

    let hoja = entorno.ipv();

    let turno = hoja.sesion.as_ref().expect("hay turno");
    assert_eq!(turno.id, sesion);
    assert!(turno.abierta);

    // Los dos productos, con la existencia que dejó su asiento de apertura y
    // todas las columnas de movimiento en cero.
    assert_eq!(hoja.filas.len(), 2);
    let refresco = fila(&hoja, 1);
    assert_eq!(refresco.inicial, "20");
    assert_eq!(refresco.entrada, "0");
    assert_eq!(refresco.salida, "0");
    assert_eq!(refresco.cierre, "20");
    assert_eq!(refresco.precio, "80.00");
    assert_eq!(refresco.importe, "0.00");
    assert!(todas_cuadran(&hoja));
    assert_eq!(hoja.importe_total, "0.00");
}

#[test]
fn la_cantidad_inicial_es_la_de_antes_de_abrir_el_turno() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00", "20");

    // Primer turno: se venden 5 y queda en 15.
    entorno.envejecer();
    entorno.abrir_caja();
    entorno.vender(&[(refresco, lata, "5")]);
    entorno.cerrar_caja();

    // Segundo turno: se venden 3 más.
    entorno.envejecer();
    let segundo = entorno.abrir_caja();
    entorno.vender(&[(refresco, lata, "3")]);

    let hoja = entorno.ipv();

    assert_eq!(hoja.sesion.as_ref().expect("turno").id, segundo);
    let renglon = fila(&hoja, refresco);
    // 15, no 20: el turno arranca donde lo dejó el anterior.
    assert_eq!(renglon.inicial, "15");
    assert_eq!(renglon.salida, "3");
    assert_eq!(renglon.cierre, "12");
    assert_eq!(renglon.importe, "240.00");
    assert!(renglon.cuadra);

    // Y la hoja del primero sigue diciendo lo que dijo entonces.
    let primera = entorno.ipv_de(hoja.sesion.as_ref().expect("turno").id - 1);
    let renglon = fila(&primera, refresco);
    assert_eq!(renglon.inicial, "20");
    assert_eq!(renglon.salida, "5");
    assert_eq!(renglon.cierre, "15");
    assert_eq!(renglon.importe, "400.00");
    assert!(renglon.cuadra);
}

#[test]
fn lo_movido_en_otro_turno_no_se_cuela_en_las_columnas() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00", "20");

    entorno.envejecer();
    entorno.abrir_caja();
    entorno.entrada(refresco, "10", "40.00");
    entorno.merma(refresco, "2");
    entorno.vender(&[(refresco, lata, "1")]);
    entorno.cerrar_caja();

    entorno.envejecer();
    let segundo = entorno.abrir_caja();

    let hoja = entorno.ipv_de(segundo);

    let renglon = fila(&hoja, refresco);
    // 20 + 10 − 2 − 1 = 27, todo heredado y nada en las columnas.
    assert_eq!(renglon.inicial, "27");
    assert_eq!(renglon.entrada, "0");
    assert_eq!(renglon.merma, "0");
    assert_eq!(renglon.salida, "0");
    assert_eq!(renglon.cierre, "27");
    assert!(renglon.cuadra);
}

#[test]
fn cada_columna_recoge_su_tipo_de_asiento_y_la_fila_cuadra() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00", "20");

    entorno.envejecer();
    entorno.abrir_caja();
    entorno.entrada(refresco, "30", "45.00");
    entorno.merma(refresco, "4");
    entorno.consumo(refresco, lata, "2");
    entorno.vender(&[(refresco, lata, "6")]);

    let hoja = entorno.ipv();

    let renglon = fila(&hoja, refresco);
    assert_eq!(renglon.inicial, "20");
    assert_eq!(renglon.entrada, "30");
    assert_eq!(renglon.merma, "4");
    assert_eq!(renglon.consumo, "2");
    assert_eq!(renglon.salida, "6");
    // 20 + 30 − 4 − 2 − 6 = 38
    assert_eq!(renglon.cierre, "38");
    assert!(renglon.cuadra);
    assert_eq!(renglon.importe, "480.00");
    assert_eq!(hoja.importe_total, "480.00");
}

/// Lo que baja del almacén al mostrador es la ENTRADA de la hoja.
///
/// Es la vía habitual: se compra una vez, se guarda, y se va bajando al
/// estante. Para Kilo12 eso es un traspaso; para el IPV, una entrada.
#[test]
fn lo_que_baja_del_almacen_entra_a_la_hoja() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto_surtido("Refresco", "80.00", "40.00", "20", "50");

    entorno.envejecer();
    entorno.abrir_caja();
    entorno.traspasar(refresco, "12", "BODEGA");
    entorno.vender(&[(refresco, lata, "5")]);

    let hoja = entorno.ipv();
    let renglon = fila(&hoja, refresco);

    // 20 en vitrina + 12 bajadas − 5 vendidas = 27.
    assert_eq!(renglon.inicial, "20");
    assert_eq!(renglon.entrada, "12");
    assert_eq!(renglon.salida, "5");
    assert_eq!(renglon.cierre, "27");
    assert!(renglon.cuadra);
}

/// Devolver mercancía al almacén resta de la misma columna.
#[test]
fn lo_que_sube_al_almacen_resta_de_la_entrada() {
    let entorno = Entorno::nuevo();
    let (refresco, _) = entorno.producto_surtido("Refresco", "80.00", "40.00", "20", "50");

    entorno.envejecer();
    entorno.abrir_caja();
    entorno.traspasar(refresco, "10", "BODEGA");
    entorno.traspasar(refresco, "4", "VITRINA");

    let hoja = entorno.ipv();
    let renglon = fila(&hoja, refresco);

    assert_eq!(renglon.entrada, "6");
    assert_eq!(renglon.cierre, "26");
    assert!(renglon.cuadra);
}

/// Lo que pasa en el almacén no sale en esta hoja.
///
/// La CANT. FINAL tiene que poder comprobarse contra el estante. Si la
/// mercancía guardada entrara en la cuenta, la hoja dejaría de servir para
/// eso, que es lo único para lo que sirve.
#[test]
fn lo_que_solo_toca_el_almacen_no_sale_en_la_hoja() {
    let entorno = Entorno::nuevo();
    let (refresco, _) = entorno.producto_surtido("Refresco", "80.00", "40.00", "20", "50");

    entorno.envejecer();
    entorno.abrir_caja();
    // Llegan 100 del proveedor y se guardan sin colocar.
    RegistrarEntrada::nuevo(&entorno.productos)
        .ejecutar(ComandoRegistrarEntrada {
            producto: refresco,
            cantidad: "100".to_owned(),
            costo_unitario: "45.00".to_owned(),
            destino: "BODEGA".to_owned(),
        })
        .expect("entrada al almacén");
    // Y se echan a perder 7 de lo guardado.
    RegistrarMerma::nuevo(&entorno.productos)
        .ejecutar(ComandoRegistrarMerma {
            producto: refresco,
            cantidad: "7".to_owned(),
            origen: "BODEGA".to_owned(),
            motivo: "se mojó la caja".to_owned(),
        })
        .expect("merma del almacén");

    let hoja = entorno.ipv();
    let renglon = fila(&hoja, refresco);

    assert_eq!(renglon.inicial, "20");
    assert_eq!(renglon.entrada, "0");
    assert_eq!(renglon.merma, "0");
    assert_eq!(renglon.cierre, "20");
    // Y cuadra: la vitrina, en efecto, no se movió.
    assert!(renglon.cuadra);
}

/// Un producto que solo vive en el almacén sale con todo en cero.
#[test]
fn lo_que_nunca_se_ha_puesto_en_la_vitrina_sale_vacio() {
    let entorno = Entorno::nuevo();
    let (guardado, _) = entorno.producto_surtido("Arroz de reserva", "180.00", "100.00", "0", "80");

    entorno.envejecer();
    entorno.abrir_caja();

    let hoja = entorno.ipv();
    let renglon = fila(&hoja, guardado);

    // La fila está —hay que poder ver que no se ha sacado nada— y en cero.
    assert_eq!(renglon.inicial, "0");
    assert_eq!(renglon.cierre, "0");
    assert_eq!(renglon.precio, "180.00");
    assert!(renglon.cuadra);
}

#[test]
fn anular_una_venta_del_turno_la_quita_de_la_salida() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00", "20");

    entorno.envejecer();
    entorno.abrir_caja();
    entorno.vender(&[(refresco, lata, "5")]);
    let anulada = entorno.vender(&[(refresco, lata, "3")]);
    AnularVenta::nuevo(&entorno.productos)
        .ejecutar(ComandoAnularVenta {
            venta: anulada,
            motivo: "error de cobro".to_owned(),
        })
        .expect("anular");

    let hoja = entorno.ipv();

    let renglon = fila(&hoja, refresco);
    // La venta y su devolución se cancelan: salieron 5.
    assert_eq!(renglon.salida, "5");
    assert_eq!(renglon.cierre, "15");
    assert_eq!(renglon.importe, "400.00");
    assert!(renglon.cuadra);
    // Y la hoja cuadra con lo que la caja dice que cobró.
    assert_eq!(hoja.vendido_real, "400.00");
    assert!(hoja.cuadra);
}

#[test]
fn anular_un_consumo_del_turno_lo_quita_de_su_columna() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00", "20");

    entorno.envejecer();
    entorno.abrir_caja();
    entorno.consumo(refresco, lata, "4");
    let anulado = entorno.consumo(refresco, lata, "3");
    AnularConsumo::nuevo(&entorno.productos, &entorno.consumos)
        .ejecutar(ComandoAnularConsumo {
            id: anulado,
            motivo: "se devolvió".to_owned(),
        })
        .expect("anular el consumo");

    let hoja = entorno.ipv();

    let renglon = fila(&hoja, refresco);
    assert_eq!(renglon.consumo, "4");
    assert_eq!(renglon.cierre, "16");
    assert!(renglon.cuadra);
}

#[test]
fn la_salida_va_en_unidades_base_y_las_subfilas_por_presentacion() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00", "100");
    let caja = entorno.presentacion(refresco, "Six-pack", "6", "420.00");

    entorno.envejecer();
    entorno.abrir_caja();
    entorno.vender(&[(refresco, lata, "2"), (refresco, caja, "3")]);

    let hoja = entorno.ipv();

    let renglon = fila(&hoja, refresco);
    // 2 latas + 3 × 6 = 20 unidades base.
    assert_eq!(renglon.salida, "20");
    // 2 × 80 + 3 × 420 = 1 420.
    assert_eq!(renglon.importe, "1420.00");
    // 1 420 / 20 = 71: ni 80 ni 70, el precio medio al que salió.
    assert_eq!(renglon.precio, "71.00");
    assert_eq!(renglon.cierre, "80");
    assert!(renglon.cuadra);

    let formas = &renglon.formas;
    assert_eq!(formas.len(), 2);
    assert_eq!(formas[0].presentacion, caja);
    assert_eq!(formas[0].nombre, "Six-pack");
    assert_eq!(formas[0].salida, "3");
    assert_eq!(formas[0].precio, "420.00");
    assert_eq!(formas[0].importe, "1260.00");
    assert_eq!(formas[1].presentacion, lata);
    assert_eq!(formas[1].salida, "2");
    assert_eq!(formas[1].importe, "160.00");
}

#[test]
fn un_turno_que_cruza_la_medianoche_es_uno_solo() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00", "20");

    entorno.envejecer();
    entorno.abrir_caja();
    entorno.vender(&[(refresco, lata, "1")]);
    entorno.vender(&[(refresco, lata, "2")]);

    // La mercancía entró anteayer, la caja se abrió ayer a las 22:00 y la
    // primera venta fue a las 23:50. La segunda ya es de hoy.
    entorno.ejecutar_sql(
        "UPDATE movimiento
            SET ocurrido_en = datetime('now', 'localtime', 'start of day', '-2 days')
          WHERE tipo = 'ENTRADA';
         UPDATE sesion_caja
            SET abierta_en = datetime('now', 'localtime', 'start of day', '-2 hours');
         UPDATE movimiento
            SET ocurrido_en = datetime('now', 'localtime', 'start of day', '-10 minutes')
          WHERE tipo = 'VENTA' AND id = (SELECT MIN(id) FROM movimiento WHERE tipo = 'VENTA');",
    );

    let hoja = entorno.ipv();

    let renglon = fila(&hoja, refresco);
    // Con el corte por calendario saldría 2; con el turno son las tres.
    assert_eq!(renglon.salida, "3");
    assert_eq!(renglon.inicial, "20");
    assert_eq!(renglon.cierre, "17");
    assert!(renglon.cuadra);
}

#[test]
fn un_producto_dado_de_alta_a_media_sesion_arranca_en_cero() {
    let entorno = Entorno::nuevo();
    entorno.producto("Refresco", "80.00", "40.00", "20");

    entorno.envejecer();
    entorno.abrir_caja();
    // Llega mercancía nueva ya con el turno abierto.
    let (arroz, libra) = entorno.producto("Arroz", "180.00", "100.00", "40");
    entorno.vender(&[(arroz, libra, "5")]);

    let hoja = entorno.ipv();

    let renglon = fila(&hoja, arroz);
    // No había nada antes de abrir: las 40 son la ENTRADA del asiento de
    // apertura, no la CANT. INICIAL.
    assert_eq!(renglon.inicial, "0");
    assert_eq!(renglon.entrada, "40");
    assert_eq!(renglon.salida, "5");
    assert_eq!(renglon.cierre, "35");
    assert!(renglon.cuadra);
    assert!(todas_cuadran(&hoja));
}

#[test]
fn se_puede_pedir_un_turno_cerrado_y_la_hoja_no_cambia_despues() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00", "20");

    entorno.envejecer();
    let primero = entorno.abrir_caja();
    entorno.vender(&[(refresco, lata, "4")]);
    entorno.cerrar_caja();
    let congelada = entorno.ipv_de(primero);

    // Pasa otro turno entero con más movimiento del mismo producto.
    entorno.envejecer();
    entorno.abrir_caja();
    entorno.entrada(refresco, "50", "45.00");
    entorno.vender(&[(refresco, lata, "9")]);
    entorno.cerrar_caja();

    let releida = entorno.ipv_de(primero);

    // La hoja de un turno cerrado es historia: dice lo mismo siempre. Los
    // sellos no se comparan porque es la propia prueba la que los reescribe
    // para separar los turnos.
    assert_eq!(congelada.filas, releida.filas);
    assert_eq!(congelada.importe_total, releida.importe_total);
    assert_eq!(congelada.vendido_real, releida.vendido_real);
    let renglon = fila(&releida, refresco);
    assert_eq!(renglon.inicial, "20");
    assert_eq!(renglon.entrada, "0");
    assert_eq!(renglon.salida, "4");
    assert_eq!(renglon.cierre, "16");
    assert_eq!(renglon.importe, "320.00");
    assert!(!releida.sesion.as_ref().expect("turno").abierta);
}

/// Con la caja cerrada no hay hoja, aunque haya turnos anteriores.
///
/// Es el comportamiento que pidió el local: el IPV se llena y se cuadra con
/// el turno vivo. La hoja de ayer es historia y se consulta desde Caja
/// (RF-IPV-14).
#[test]
fn con_la_caja_cerrada_no_hay_hoja_que_ver() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00", "20");

    entorno.envejecer();
    let turno = entorno.abrir_caja();
    entorno.vender(&[(refresco, lata, "4")]);
    // Con la caja abierta hay hoja.
    assert!(entorno.ipv().sesion.is_some());

    entorno.cerrar_caja();

    let hoja = entorno.ipv();
    assert_eq!(hoja.sesion, None);
    assert!(hoja.filas.is_empty());
    // Pero el turno sigue ahí para quien tenga la clave.
    assert_eq!(entorno.ipv_de(turno).sesion.expect("turno").id, turno);
}

/// El pasado no se lee sin la clave del sistema.
#[test]
fn sin_la_clave_un_turno_pasado_no_se_lee() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00", "20");

    entorno.envejecer();
    let primero = entorno.abrir_caja();
    entorno.vender(&[(refresco, lata, "4")]);
    entorno.cerrar_caja();
    entorno.envejecer();
    entorno.abrir_caja();

    let sin_clave = entorno.pedir(&PeticionIpv {
        sesion: Some(primero),
        clave: None,
    });
    let clave_mala = entorno.pedir(&PeticionIpv {
        sesion: Some(primero),
        clave: Some("000000".to_owned()),
    });

    assert!(matches!(
        sin_clave,
        Err(application::ErrorAplicacion::ClaveIncorrecta)
    ));
    assert!(matches!(
        clave_mala,
        Err(application::ErrorAplicacion::ClaveIncorrecta)
    ));
    // Con la buena, sí.
    assert_eq!(entorno.ipv_de(primero).filas.len(), 1);
}

#[test]
fn un_turno_que_no_existe_no_es_un_error() {
    let entorno = Entorno::nuevo();
    entorno.producto("Refresco", "80.00", "40.00", "20");
    entorno.envejecer();
    entorno.abrir_caja();

    let hoja = entorno.ipv_de(999);

    assert_eq!(hoja.sesion, None);
    assert!(hoja.filas.is_empty());
}

/// El total de la hoja y lo que la caja dice que cobró son la misma cifra.
///
/// Si alguna vez discrepan sin que haya un motivo, el dueño imprime una hoja
/// que no cuadra con el cierre y deja de fiarse de las dos.
#[test]
fn el_total_de_la_hoja_cuadra_con_lo_que_cobro_la_caja() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00", "100");
    let caja = entorno.presentacion(refresco, "Six-pack", "6", "420.00");
    let (arroz, libra) = entorno.producto("Arroz", "180.00", "100.00", "60");

    entorno.envejecer();
    entorno.abrir_caja();
    entorno.entrada(arroz, "20", "110.00");
    entorno.merma(refresco, "3");
    entorno.consumo(arroz, libra, "2");
    entorno.vender(&[(refresco, lata, "5"), (arroz, libra, "4")]);
    entorno.vender(&[(refresco, caja, "2")]);
    let anulada = entorno.vender(&[(arroz, libra, "7")]);
    AnularVenta::nuevo(&entorno.productos)
        .ejecutar(ComandoAnularVenta {
            venta: anulada,
            motivo: "error".to_owned(),
        })
        .expect("anular");

    let hoja = entorno.ipv();

    // Todas las filas se explican y el total coincide al céntimo.
    assert!(todas_cuadran(&hoja));
    // 5 × 80 + 2 × 420 + 4 × 180 = 1 960.
    assert_eq!(hoja.importe_total, "1960.00");
    assert_eq!(hoja.vendido_real, "1960.00");
    assert_eq!(hoja.diferencia, "0.00");
    assert!(hoja.cuadra);

    let refresco = fila(&hoja, refresco);
    // 100 − 3 de merma − 5 sueltas − 12 del six-pack = 80.
    assert_eq!(refresco.merma, "3");
    assert_eq!(refresco.salida, "17");
    assert_eq!(refresco.cierre, "80");

    let arroz = fila(&hoja, arroz);
    // 60 + 20 − 2 de consumo − 4 vendidas (las 7 anuladas volvieron) = 74.
    assert_eq!(arroz.entrada, "20");
    assert_eq!(arroz.consumo, "2");
    assert_eq!(arroz.salida, "4");
    assert_eq!(arroz.cierre, "74");
}

/// El `.xlsx` se escribe de verdad y con las cifras como números.
///
/// Se leen las celdas de la hoja, no solo el tamaño del archivo: lo que hace
/// útil la exportación es que las columnas se puedan sumar en Excel, y eso
/// depende de que las cifras vayan en `<v>` y no como cadena compartida.
#[test]
fn el_excel_se_escribe_y_las_celdas_son_numeros() {
    let entorno = Entorno::nuevo();
    let (refresco, lata) = entorno.producto("Refresco", "80.00", "40.00", "100");
    let caja = entorno.presentacion(refresco, "Six-pack", "6", "420.00");

    entorno.envejecer();
    let turno = entorno.abrir_caja();
    entorno.merma(refresco, "3");
    entorno.vender(&[(refresco, lata, "5"), (refresco, caja, "2")]);

    let hoja = entorno.ipv();
    let ruta = std::env::temp_dir().join(format!("kilo12-ipv-{turno}.xlsx"));
    infrastructure::escribir_ipv(&[hoja], &ruta).expect("escribir el Excel");

    let bytes = std::fs::read(&ruta).expect("leer el Excel");
    // Un `.xlsx` es un zip: si no empieza por «PK» no lo abre nada.
    assert_eq!(&bytes[..2], b"PK");

    // Los textos viven en la tabla de cadenas compartidas; las cifras, en
    // la hoja. Que estén en sitios distintos es justamente lo que se quiere
    // comprobar: una cifra en la tabla de cadenas sería texto.
    let cadenas = dentro_del_libro(&ruta, "xl/sharedStrings.xml");
    let hoja = dentro_del_libro(&ruta, "xl/worksheets/sheet1.xml");
    std::fs::remove_file(&ruta).expect("borrar el Excel");

    // El encabezado completo.
    for encabezado in [
        "MERCANCIA",
        "CANT. INICIAL",
        "ENTRADA",
        "MERMA",
        "CONSUMO",
        "SALIDA",
        "PRECIO",
        "IMPORTE",
        "CANT. FINAL",
    ] {
        assert!(cadenas.contains(encabezado), "falta «{encabezado}»");
    }
    // Y ni rastro de las dos columnas que las dueñas no usan.
    assert!(!cadenas.contains("Costo"));
    assert!(!cadenas.contains("CASA"));
    // El pie con las tres cifras del turno.
    assert!(cadenas.contains("IMPORTE TOTAL"));
    assert!(cadenas.contains("Total vendido real"));
    assert!(cadenas.contains("Diferencia"));
    // La subfila sangrada del six-pack.
    assert!(cadenas.contains("· Six-pack"), "falta la subfila");

    // 100 − 3 de merma − 5 sueltas − 12 del six-pack = 80. Las celdas de
    // cifras van como número: `<v>80</v>`, no como texto.
    for esperada in ["<v>100</v>", "<v>3</v>", "<v>17</v>", "<v>80</v>"] {
        assert!(hoja.contains(esperada), "falta la celda {esperada}");
    }
    // 5 × 80 + 2 × 420 = 1 240.
    assert!(hoja.contains("<v>1240</v>"), "falta el importe");
    // El precio medio es 1240 / 17 = 72.941176. En la celda va la cifra
    // ENTERA que calculó Rust, con sus seis decimales; que Excel la enseñe
    // como «72.94» es cosa del formato. Si se escribiera ya redondeada, una
    // columna sumada en Excel no cuadraría con el IMPORTE.
    assert!(hoja.contains("<v>72.941176</v>"), "falta el precio medio");
    // La subfila del six-pack con su propio precio.
    assert!(hoja.contains("<v>420</v>"), "falta el precio del six-pack");
}

/// Descomprime el `.xlsx` y devuelve uno de sus XML.
///
/// Se usa el `unzip` del sistema en vez de añadir una dependencia de
/// pruebas: aquí solo hace falta mirar dentro del archivo.
fn dentro_del_libro(ruta: &std::path::Path, parte: &str) -> String {
    let salida = std::process::Command::new("unzip")
        .arg("-p")
        .arg(ruta)
        .arg(parte)
        .output()
        .expect("descomprimir el Excel");

    assert!(salida.status.success(), "unzip falló con {parte}");
    String::from_utf8(salida.stdout).expect("XML válido")
}
