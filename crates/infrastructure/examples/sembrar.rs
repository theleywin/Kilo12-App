//! Siembra ventas ficticias para ver cómo se comportan los informes.
//!
//! **Es una herramienta de desarrollo, no parte de la aplicación.** Vive en
//! `examples/` a propósito: cargo no la compila dentro del binario que se
//! entrega, así que no hay forma de que un generador de datos falsos acabe
//! en la máquina de la tienda.
//!
//! Las ventas se hacen **por los casos de uso de verdad**, no insertando
//! filas a mano. Escribir SQL directo sería más corto y produciría datos
//! mentirosos: ventas que no descuentan la vitrina, sin asiento en el
//! kárdex y sin sesión de caja. Las gráficas dirían una cosa y el resto de
//! la aplicación otra.
//!
//! Lo único que se toca a mano es la **fecha**: se retrasa después de
//! cobrar, porque `Vender` sella con el reloj de la máquina y aquí hace
//! falta repartir las ventas en el pasado. Se hace con una conexión propia
//! para no tener que añadirle al repositorio un método que solo sirve para
//! esto.
//!
//! **Los asientos del kárdex se quedan con la fecha de hoy.** La tabla
//! `movimiento` no guarda a qué venta pertenece —no hace falta para nada
//! más—, así que no hay forma fiable de emparejarlos. Los informes leen la
//! fecha de la venta, así que salen bien; el kárdex de un producto dirá que
//! todo se vendió hoy.
//!
//! ## Cómo se usa
//!
//! ```sh
//! cargo run -p infrastructure --example sembrar -- --dias 20
//! cargo run -p infrastructure --example sembrar -- --dias 30 --semilla 7 --ventas 12
//! ```
//!
//! | Opción | Qué hace | Por defecto |
//! |---|---|---|
//! | `--dias N` | Cuántos días hacia atrás | 20 |
//! | `--ventas N` | Ventas por día, de media | 8 |
//! | `--semilla N` | Cambia el azar; misma semilla, mismos datos | 42 |
//! | `--tendencia N` | Crecimiento porcentual del último día frente al primero | 60 |
//! | `--base RUTA` | Base de datos a sembrar | la de la aplicación |

use std::path::PathBuf;
use std::sync::Arc;

use application::casos::{
    AbrirCaja, ComandoAbrirCaja, ComandoRegistrarEntrada, ComandoRegistrarProducto, ComandoVender,
    ConsultarProducto, LineaPedida, PagoPedido, RegistrarEntrada, RegistrarProducto, Vender,
};
use application::puertos::RepositorioProducto;
use infrastructure::{BaseDatos, RepositorioProductoSqlite};

/// Un producto del catálogo ficticio: nombre, unidad, costo y precio.
const CATALOGO: &[(&str, &str, &str, &str)] = &[
    ("Arroz blanco", "lb", "120.00", "180.00"),
    ("Frijol negro", "lb", "210.00", "300.00"),
    ("Aceite girasol", "L", "480.00", "650.00"),
    ("Leche en polvo", "g", "2.80", "4.20"),
    ("Refresco lata", "unidad", "41.67", "80.00"),
    ("Galleta de sal", "unidad", "18.00", "35.00"),
    ("Café molido", "g", "5.50", "9.00"),
    ("Jabón de baño", "unidad", "95.00", "160.00"),
    ("Pasta dental", "unidad", "180.00", "290.00"),
    ("Cerveza lata", "unidad", "150.00", "250.00"),
];

/// Cuántas ventas caen en cada hora, de 0 a 23.
///
/// No es plano a propósito: un mercadito vende por la mañana temprano y a
/// la salida del trabajo, y queda muerto de madrugada. Una distribución
/// uniforme haría que la gráfica de horas no enseñara nada.
const PESO_HORARIO: [u32; 24] = [
    0, 0, 0, 0, 0, 1, 3, 7, 12, 14, 11, 9, 8, 7, 6, 7, 10, 14, 13, 9, 5, 2, 1, 0,
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opciones = Opciones::leer();
    println!(
        "Sembrando {} días en {}",
        opciones.dias,
        opciones.base.display()
    );

    let base = Arc::new(BaseDatos::abrir(&opciones.base)?);
    let repositorio = RepositorioProductoSqlite::nuevo(base.clone());
    let mut azar = Azar::nuevo(opciones.semilla);

    let productos = preparar_catalogo(&repositorio, &opciones)?;
    println!("  {} productos con mercancía", productos.len());

    // Una sesión abierta para poder cobrar. Las ventas se retrasan después,
    // así que la sesión es solo el recipiente que exige RF-CAJ-06.
    if repositorio.sesion_abierta()?.is_none() {
        AbrirCaja::nuevo(&repositorio).ejecutar(ComandoAbrirCaja {
            operador: "Datos de prueba".to_owned(),
            fondo_inicial: "1000.00".to_owned(),
        })?;
    }

    // Las fechas se aplican al final, todas de una vez: veinte días de
    // ventas son cientos de UPDATE y no merecen una transacción cada uno.
    let mut fechas: Vec<(i64, u32, u32, u32)> = Vec::new();

    for dia in (0..opciones.dias).rev() {
        // La tendencia hace que los días recientes vendan más que los
        // viejos: sin ella la gráfica es una meseta y no se ve nada.
        let avance = (opciones.dias - 1 - dia) as i64;
        let factor = 100 + (opciones.tendencia * avance) / (opciones.dias.max(2) - 1) as i64;

        let cuantas = (opciones.ventas as i64 * factor / 100).max(1) as u32;
        // Y encima algo de ruido, para que no salgan veinte barras que
        // crecen en línea recta: un negocio real tiene días flojos.
        let cuantas = azar.entre(cuantas.saturating_sub(2).max(1), cuantas + 3);

        for _ in 0..cuantas {
            let hora = azar.hora_ponderada(&PESO_HORARIO);
            let minuto = azar.entre(0, 59);

            if let Some(folio) = cobrar_una(&repositorio, &productos, &mut azar)? {
                fechas.push((folio, dia, hora, minuto));
            }
        }

        print!("  día -{dia:<3} ");
        println!("{}", "▇".repeat((cuantas as usize).min(40)));
    }

    let cobradas = fechas.len();
    retrasar(&opciones.base, &fechas)?;

    println!("\n{cobradas} ventas sembradas. Abre Informes (F8) y elige 30 días.");
    Ok(())
}

/// Cobra una venta y la retrasa al día y hora indicados.
fn cobrar_una(
    repositorio: &RepositorioProductoSqlite,
    productos: &[(i64, i64)],
    azar: &mut Azar,
) -> Result<Option<i64>, Box<dyn std::error::Error>> {
    let cuantas_lineas = azar.entre(1, 3);
    let mut lineas = Vec::new();

    for _ in 0..cuantas_lineas {
        let (producto, presentacion) =
            productos[azar.entre(0, productos.len() as u32 - 1) as usize];
        lineas.push(LineaPedida {
            producto,
            presentacion,
            cantidad: azar.entre(1, 4).to_string(),
        });
    }

    // Se paga de más y ya se encarga el núcleo del vuelto: calcular aquí el
    // importe exacto sería repetir la aritmética que justamente vive allí.
    let pagos = vec![PagoPedido {
        metodo: metodo_de(azar).to_owned(),
        entregado: "100000.00".to_owned(),
    }];

    let hecha = match Vender::nuevo(repositorio).ejecutar(ComandoVender { lineas, pagos }) {
        Ok(hecha) => hecha,
        // Sin existencia suficiente: se salta esta y sigue. Es esperable
        // cuando un producto se vacía antes de terminar el periodo.
        Err(_) => return Ok(None),
    };

    Ok(Some(hecha.folio))
}

/// Elige la forma de pago.
///
/// La mayoría en efectivo y una parte por transferencia, que es como se
/// comporta un mercadito de verdad. Sin dólares: exigirían tasa fijada y el
/// sembrador no debe depender de la configuración del usuario.
fn metodo_de(azar: &mut Azar) -> &'static str {
    match azar.entre(0, 9) {
        0..=6 => "EFECTIVO_CUP",
        _ => "TRANSFERENCIA",
    }
}

/// Reparte las ventas en el pasado.
///
/// Es lo único que se escribe a mano. `Vender` sella con el reloj de ahora,
/// y para dibujar una serie de veinte días hacen falta veinte fechas.
fn retrasar(
    ruta: &std::path::Path,
    fechas: &[(i64, u32, u32, u32)],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut conexion = rusqlite::Connection::open(ruta)?;
    let transaccion = conexion.transaction()?;

    for (folio, dia, hora, minuto) in fechas {
        transaccion.execute(
            "UPDATE venta
                SET ocurrido_en = date('now', 'localtime', ?2) || ' ' || ?3
              WHERE folio = ?1",
            rusqlite::params![
                folio,
                format!("-{dia} days"),
                format!("{hora:02}:{minuto:02}:00"),
            ],
        )?;
    }

    transaccion.commit()?;
    Ok(())
}

/// Da de alta el catálogo ficticio y le mete mercancía de sobra.
///
/// Si el producto ya existe se reutiliza: sembrar dos veces no debe
/// duplicar el catálogo.
fn preparar_catalogo(
    repositorio: &RepositorioProductoSqlite,
    opciones: &Opciones,
) -> Result<Vec<(i64, i64)>, Box<dyn std::error::Error>> {
    // Existencia con holgura para aguantar todo el periodo sin agotarse.
    let existencia = (opciones.dias * opciones.ventas * 4).max(400).to_string();
    let existentes = repositorio.listar(false)?;
    let mut productos = Vec::new();

    for (nombre, unidad, costo, precio) in CATALOGO {
        let id = match existentes
            .iter()
            .find(|f| f.producto.nombre().eq_ignore_ascii_case(nombre))
        {
            Some(fila) => fila.producto.id().expect("producto guardado").0,
            None => {
                RegistrarProducto::nuevo(repositorio)
                    .ejecutar(ComandoRegistrarProducto {
                        sku: String::new(),
                        nombre: (*nombre).to_owned(),
                        unidad_base: (*unidad).to_owned(),
                        precio_unitario: (*precio).to_owned(),
                        costo_unitario: Some((*costo).to_owned()),
                        cantidad_almacen: None,
                        cantidad_vitrina: Some(existencia.clone()),
                        stock_minimo: None,
                        objetivo_vitrina: None,
                    })?
                    .0
            }
        };

        // Reponer siempre: puede ser un producto viejo con la vitrina seca.
        RegistrarEntrada::nuevo(repositorio).ejecutar(ComandoRegistrarEntrada {
            producto: id,
            cantidad: existencia.clone(),
            costo_unitario: (*costo).to_owned(),
            destino: "VITRINA".to_owned(),
        })?;

        let ficha = ConsultarProducto::nuevo(repositorio).ejecutar(id)?;
        productos.push((id, ficha.presentaciones[0].id));
    }

    Ok(productos)
}

// ============================================================== opciones

struct Opciones {
    dias: u32,
    ventas: u32,
    semilla: u64,
    /// Crecimiento porcentual del último día frente al primero.
    tendencia: i64,
    base: PathBuf,
}

impl Opciones {
    fn leer() -> Self {
        let argumentos: Vec<String> = std::env::args().collect();
        let valor = |nombre: &str| -> Option<String> {
            argumentos
                .iter()
                .position(|a| a == nombre)
                .and_then(|i| argumentos.get(i + 1))
                .cloned()
        };

        Self {
            dias: valor("--dias").and_then(|v| v.parse().ok()).unwrap_or(20),
            ventas: valor("--ventas").and_then(|v| v.parse().ok()).unwrap_or(8),
            semilla: valor("--semilla")
                .and_then(|v| v.parse().ok())
                .unwrap_or(42),
            tendencia: valor("--tendencia")
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            base: valor("--base").map_or_else(ruta_de_la_aplicacion, PathBuf::from),
        }
    }
}

/// Dónde guarda la aplicación su base en macOS y en Linux.
fn ruta_de_la_aplicacion() -> PathBuf {
    let casa = std::env::var("HOME").unwrap_or_default();
    let mac =
        PathBuf::from(&casa).join("Library/Application Support/com.kilo12.mercadito/kilo12.db");

    if mac.exists() {
        return mac;
    }

    PathBuf::from(casa).join(".local/share/com.kilo12.mercadito/kilo12.db")
}

// ================================================================= azar

/// Generador reproducible.
///
/// Es un congruencial lineal de manual: no sirve para criptografía y aquí
/// no hace falta que sirva. Lo que sí importa es que la misma semilla dé
/// siempre los mismos datos, para poder comparar dos ejecuciones.
struct Azar(u64);

impl Azar {
    const fn nuevo(semilla: u64) -> Self {
        Self(
            semilla
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1),
        )
    }

    fn siguiente(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 16
    }

    /// Un número entre `desde` y `hasta`, ambos incluidos.
    fn entre(&mut self, desde: u32, hasta: u32) -> u32 {
        if hasta <= desde {
            return desde;
        }
        desde + (self.siguiente() % u64::from(hasta - desde + 1)) as u32
    }

    /// Elige una hora respetando los pesos: las horas punta salen más.
    fn hora_ponderada(&mut self, pesos: &[u32; 24]) -> u32 {
        let total: u32 = pesos.iter().sum();
        let mut tirada = self.entre(0, total.saturating_sub(1));

        for (hora, peso) in pesos.iter().enumerate() {
            if tirada < *peso {
                return hora as u32;
            }
            tirada -= peso;
        }

        12
    }
}
