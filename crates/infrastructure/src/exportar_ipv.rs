//! Escribe el IPV en un `.xlsx` (RF-IPV-12).
//!
//! La hoja que las dueñas llevaban a mano era un Excel, y a Excel vuelve.
//! Una hoja por turno, con las mismas columnas que la pantalla y en el mismo
//! orden, para que la impresa y la exportada se puedan poner una al lado de
//! la otra sin traducir nada.
//!
//! Las cifras son de la vitrina, como en toda la hoja, y ENTRADA es lo que
//! entró al mostrador. El libro no lo explica: quien lo abre ya sabe qué
//! significa cada columna, y una nota al pie en una hoja de cálculo que se
//! va a filtrar y a sumar solo estorba.
//!
//! Las cifras se escriben como **números**, no como texto. Es la única
//! diferencia de fondo con la pantalla y la razón de que `FilaIpv` cargue
//! con `cifras`: una hoja de cálculo cuyas columnas son texto no se puede
//! sumar, y entonces no es una hoja de cálculo. La conversión de milésimas y
//! millonésimas a coma flotante se hace aquí y solo aquí: es el único sitio
//! donde hace falta, porque una celda de Excel es un `f64` y no hay más
//! remedio. Ningún cálculo del negocio pasa por ella (DT-1).
//!
//! No se escriben las columnas `Costo` ni `CASA gastos` del Excel original:
//! no se usan.

use std::path::Path;

use application::casos::{FilaIpv, FormaVendida, Ipv};
use domain::{Cantidad, Dinero};
use rust_xlsxwriter::{Color, Format, Workbook, Worksheet};

use crate::error::{ErrorInfra, ResultadoInfra};

/// Encabezados, en el orden de la hoja.
const ENCABEZADOS: [&str; 9] = [
    "MERCANCIA",
    "CANT. INICIAL",
    "ENTRADA",
    "MERMA",
    "CONSUMO",
    "SALIDA",
    "PRECIO",
    "IMPORTE",
    "CANT. FINAL",
];

/// Ancho de cada columna, en caracteres.
const ANCHOS: [f64; 9] = [30.0, 13.0, 10.0, 9.0, 10.0, 9.0, 11.0, 13.0, 12.0];

/// Escribe un libro con una hoja por turno.
///
/// Devuelve error si no le dan ningún turno: un libro sin hojas no lo abre
/// ni Excel, y es mejor decirlo que entregar un archivo roto.
pub fn escribir_ipv(hojas: &[Ipv], ruta: &Path) -> ResultadoInfra<()> {
    if hojas.is_empty() {
        return Err(ErrorInfra::DatoCorrupto(
            "no hay ningún turno que exportar".to_owned(),
        ));
    }

    let mut libro = Workbook::new();
    let estilos = Estilos::nuevos();

    for (orden, hoja) in hojas.iter().enumerate() {
        let pagina = libro.add_worksheet();
        escribir_hoja(pagina, hoja, orden, &estilos)?;
    }

    libro.save(ruta)?;
    Ok(())
}

/// Los formatos del libro, creados una vez y reusados.
struct Estilos {
    titulo: Format,
    encabezado: Format,
    mercancia: Format,
    forma: Format,
    cantidad: Format,
    cantidad_granel: Format,
    /// La CANT. FINAL de una fila que no se explica a sí misma.
    descuadre: Format,
    dinero: Format,
    total_texto: Format,
    total_cifra: Format,
}

impl Estilos {
    fn nuevos() -> Self {
        // Dos formatos de cantidad: lo que se cuenta no lleva decimales y lo
        // que se pesa lleva tres, igual que en la pantalla.
        let cantidad = Format::new().set_num_format("0");
        let cantidad_granel = Format::new().set_num_format("0.000");
        let dinero = Format::new().set_num_format("#,##0.00");

        Self {
            titulo: Format::new().set_bold(),
            encabezado: Format::new().set_bold(),
            mercancia: Format::new(),
            // La subfila va sangrada y en gris: pertenece a la de arriba.
            forma: Format::new().set_font_color(Color::RGB(0x60_60_60)),
            cantidad: cantidad.clone(),
            cantidad_granel,
            descuadre: cantidad
                .clone()
                .set_bold()
                .set_font_color(Color::RGB(0xC0_00_00)),
            dinero: dinero.clone(),
            total_texto: Format::new().set_bold(),
            total_cifra: dinero.set_bold(),
        }
    }
}

/// Cantidad en unidades base, lista para una celda.
///
/// Dividir aquí es inevitable: la celda es un `f64`. La cifra exacta ya se
/// decidió en Rust; esto solo la traslada.
fn como_numero(cantidad: Cantidad) -> f64 {
    cantidad.milesimas() as f64 / 1_000.0
}

/// Importe en pesos, listo para una celda.
fn como_importe(importe: Dinero) -> f64 {
    importe.millonesimas() as f64 / 1_000_000.0
}

fn escribir_hoja(
    pagina: &mut Worksheet,
    hoja: &Ipv,
    orden: usize,
    estilos: &Estilos,
) -> ResultadoInfra<()> {
    pagina.set_name(nombre_de_hoja(hoja, orden))?;

    for (columna, ancho) in ANCHOS.iter().enumerate() {
        pagina.set_column_width(columna as u16, *ancho)?;
    }

    pagina.write_string_with_format(0, 0, titulo(hoja), &estilos.titulo)?;

    for (columna, encabezado) in ENCABEZADOS.iter().enumerate() {
        pagina.write_string_with_format(1, columna as u16, *encabezado, &estilos.encabezado)?;
    }
    // Con el encabezado fijo se puede bajar por 150 productos sin perder de
    // vista qué columna se está leyendo.
    pagina.set_freeze_panes(2, 0)?;

    let mut renglon = 2_u32;
    for fila in &hoja.filas {
        escribir_fila(pagina, fila, renglon, estilos)?;
        renglon += 1;

        for forma in &fila.formas {
            escribir_forma(pagina, forma, renglon, estilos)?;
            renglon += 1;
        }
    }

    escribir_cierre(pagina, hoja, renglon + 1, estilos)?;

    Ok(())
}

/// La fila de una mercancía, con todas sus columnas.
fn escribir_fila(
    pagina: &mut Worksheet,
    fila: &FilaIpv,
    renglon: u32,
    estilos: &Estilos,
) -> ResultadoInfra<()> {
    // Los decimales los dice el núcleo, que sabe si la mercancía se pesa o
    // se cuenta. Deducirlos mirando si el texto trae coma parecía más
    // simple y era falso: una libra de arroz vendida entera se escribe «3»,
    // y con el formato de enteros media libra se imprimiría como una.
    let unidades = if fila.cifras.decimales > 0 {
        &estilos.cantidad_granel
    } else {
        &estilos.cantidad
    };

    pagina.write_string_with_format(renglon, 0, &fila.mercancia, &estilos.mercancia)?;
    pagina.write_number_with_format(renglon, 1, como_numero(fila.cifras.inicial), unidades)?;
    pagina.write_number_with_format(renglon, 2, como_numero(fila.cifras.entrada), unidades)?;
    pagina.write_number_with_format(renglon, 3, como_numero(fila.cifras.merma), unidades)?;
    pagina.write_number_with_format(renglon, 4, como_numero(fila.cifras.consumo), unidades)?;
    pagina.write_number_with_format(renglon, 5, como_numero(fila.cifras.salida), unidades)?;
    pagina.write_number_with_format(
        renglon,
        6,
        como_importe(fila.cifras.precio),
        &estilos.dinero,
    )?;
    pagina.write_number_with_format(
        renglon,
        7,
        como_importe(fila.cifras.importe),
        &estilos.dinero,
    )?;

    // La CANT. FINAL de una fila que no cuadra sale en rojo. Es la única
    // señal de descuadre que lleva la hoja: añadir una columna que las
    // dueñas no pidieron para avisar de algo que casi nunca pasa estorbaría
    // más de lo que ayuda.
    let final_ = if fila.cuadra {
        unidades
    } else {
        &estilos.descuadre
    };
    pagina.write_number_with_format(renglon, 8, como_numero(fila.cifras.cierre), final_)?;

    Ok(())
}

/// Una subfila: la presentación, lo que salió de ella y a cuánto.
///
/// Las columnas de existencia se dejan en blanco a propósito. La vitrina
/// guarda unidades base, no paquetes y sueltos por separado, así que una
/// existencia por presentación no existe y escribir cero diría que se agotó.
fn escribir_forma(
    pagina: &mut Worksheet,
    forma: &FormaVendida,
    renglon: u32,
    estilos: &Estilos,
) -> ResultadoInfra<()> {
    pagina.write_string_with_format(
        renglon,
        0,
        format!("    · {}", forma.nombre),
        &estilos.forma,
    )?;
    pagina.write_number_with_format(
        renglon,
        5,
        como_numero(forma.salida_cruda),
        &estilos.cantidad,
    )?;
    pagina.write_number_with_format(
        renglon,
        6,
        como_importe(forma.precio_crudo),
        &estilos.dinero,
    )?;
    pagina.write_number_with_format(
        renglon,
        7,
        como_importe(forma.importe_crudo),
        &estilos.dinero,
    )?;

    Ok(())
}

/// El pie: el total de la hoja, lo que cobró la caja y la diferencia.
///
/// En la hoja de papel «Total vendido real» se dejaba en blanco para
/// apuntarlo a mano. Kilo12 lo sabe, así que lo escribe, y escribe también
/// la diferencia: las dos cifras juntas son lo que de verdad se quería
/// saber al dejar esa casilla.
fn escribir_cierre(
    pagina: &mut Worksheet,
    hoja: &Ipv,
    renglon: u32,
    estilos: &Estilos,
) -> ResultadoInfra<()> {
    let pie = [
        ("IMPORTE TOTAL", hoja.importe_total_crudo),
        ("Total vendido real", hoja.vendido_real_crudo),
        ("Diferencia", hoja.diferencia_cruda),
    ];

    for (desplazamiento, (etiqueta, importe)) in pie.iter().enumerate() {
        let renglon = renglon + desplazamiento as u32;
        pagina.write_string_with_format(renglon, 0, *etiqueta, &estilos.total_texto)?;
        // Debajo de IMPORTE, que es la columna que suman: en la hoja de papel
        // el total iba en la segunda celda porque se escribía a mano.
        pagina.write_number_with_format(
            renglon,
            7,
            como_importe(*importe),
            &estilos.total_cifra,
        )?;
    }

    Ok(())
}

/// Título de la hoja: qué turno es y entre qué horas fue.
fn titulo(hoja: &Ipv) -> String {
    match &hoja.sesion {
        Some(sesion) => {
            let hasta = sesion
                .cerrada_en
                .as_deref()
                .unwrap_or("sigue abierto")
                .to_owned();
            format!(
                "IPV · turno {} · {} → {}",
                sesion.id, sesion.abierta_en, hasta
            )
        }
        None => "IPV · sin turno".to_owned(),
    }
}

/// Nombre de la pestaña.
///
/// Excel no admite más de 31 caracteres ni los signos `[ ] : * ? / \`, así
/// que se arma a mano en vez de reusar el título. `orden` desempata cuando
/// por lo que sea no hay turno al que referirse.
fn nombre_de_hoja(hoja: &Ipv, orden: usize) -> String {
    match &hoja.sesion {
        // De «2026-10-13 08:00:00» se toma «10-13»: mes y día bastan para
        // distinguir las pestañas de un libro de varios turnos.
        Some(sesion) => {
            let fecha = sesion.abierta_en.get(5..10).unwrap_or_default();
            format!("Turno {} {fecha}", sesion.id)
        }
        None => format!("Hoja {}", orden + 1),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_nombre_de_la_pestana_cabe_en_lo_que_excel_admite() {
        let hoja = Ipv {
            sesion: Some(application::casos::SesionDeReferencia {
                id: 1234,
                abierta: false,
                abierta_en: "2026-10-13 08:00:00".to_owned(),
                cerrada_en: Some("2026-10-13 20:00:00".to_owned()),
            }),
            filas: Vec::new(),
            importe_total: "0.00".to_owned(),
            vendido_real: "0.00".to_owned(),
            diferencia: "0.00".to_owned(),
            cuadra: true,
            importe_total_crudo: Dinero::CERO,
            vendido_real_crudo: Dinero::CERO,
            diferencia_cruda: Dinero::CERO,
        };

        let nombre = nombre_de_hoja(&hoja, 0);

        assert_eq!(nombre, "Turno 1234 10-13");
        assert!(nombre.len() <= 31);
        assert!(!nombre.contains([':', '/', '\\', '[', ']', '*', '?']));
    }

    #[test]
    fn sin_turno_la_pestana_se_numera() {
        let hoja = Ipv {
            sesion: None,
            filas: Vec::new(),
            importe_total: "0.00".to_owned(),
            vendido_real: "0.00".to_owned(),
            diferencia: "0.00".to_owned(),
            cuadra: true,
            importe_total_crudo: Dinero::CERO,
            vendido_real_crudo: Dinero::CERO,
            diferencia_cruda: Dinero::CERO,
        };

        assert_eq!(nombre_de_hoja(&hoja, 2), "Hoja 3");
    }

    #[test]
    fn un_libro_sin_turnos_se_rechaza_en_vez_de_escribirse_roto() {
        let error = escribir_ipv(&[], Path::new("no-se-escribe.xlsx"));

        assert!(error.is_err());
    }

    /// Las milésimas y las millonésimas llegan a la celda sin perderse.
    #[test]
    fn las_cifras_escaladas_llegan_enteras_a_la_celda() {
        assert_eq!(como_numero(Cantidad::desde_milesimas(2_500)), 2.5);
        assert_eq!(como_numero(Cantidad::desde_milesimas(-2_000)), -2.0);
        assert_eq!(como_importe(Dinero::desde_millonesimas(58_330_000)), 58.33);
        assert_eq!(
            como_importe(Dinero::desde_millonesimas(-33_360_000)),
            -33.36
        );
    }
}
