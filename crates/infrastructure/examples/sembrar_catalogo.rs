//! Siembra un catálogo largo para probar las pantallas con datos de verdad.
//!
//! **Es una herramienta de desarrollo, no parte de la aplicación.** Vive en
//! `examples/` a propósito: cargo no la compila dentro del binario que se
//! entrega, así que no hay forma de que un generador de datos falsos acabe
//! en la máquina de la tienda.
//!
//! Se diferencia de [`sembrar`](sembrar) en para qué sirve: aquel reparte
//! ventas en el pasado para que los informes tengan series que dibujar;
//! este llena el **catálogo** para que Vitrina y Almacén tengan una lista
//! larga de verdad. Una tabla de diez filas no prueba un buscador ni un
//! encabezado pegado: con diez filas no hay que desplazarse.
//!
//! Los productos se dan de alta **por el caso de uso de verdad**
//! (`RegistrarProducto`), no insertando filas a mano. Escribir SQL directo
//! sería más corto y produciría datos mentirosos: productos sin asiento de
//! apertura en el kárdex, con el valor del inventario descuadrado del costo,
//! y SKU repetidos que la aplicación jamás generaría.
//!
//! Los nombres no son decorativos. Llevan acentos (`Azúcar`, `Piña`,
//! `Limón`) para probar que el buscador los pliega, y comparten prefijos a
//! propósito —cuatro `Refresco de …`, tres `Jabón de …`— para que se note la
//! diferencia entre buscar por prefijo y buscar en cualquier posición.
//!
//! ## Cómo se usa
//!
//! ```sh
//! cargo run -p infrastructure --example sembrar_catalogo
//! cargo run -p infrastructure --example sembrar_catalogo -- --cuantos 40
//! ```
//!
//! | Opción | Qué hace | Por defecto |
//! |---|---|---|
//! | `--cuantos N` | Cuántos productos del catálogo sembrar | todos (80) |
//! | `--base RUTA` | Base de datos a sembrar | la de la aplicación |
//!
//! Es idempotente: un producto que ya existe por nombre se salta, así que
//! ejecutarlo dos veces no duplica el catálogo.

use std::path::PathBuf;
use std::sync::Arc;

use application::casos::{ComandoRegistrarProducto, RegistrarProducto};
use application::puertos::RepositorioProducto;
use infrastructure::{BaseDatos, RepositorioProductoSqlite};

/// El catálogo ficticio: nombre, unidad base, costo y precio por unidad.
///
/// Ochenta productos de mercadito cubano. El orden es el de un inventario
/// real —granos, líquidos, conservas, viandas, aseo— y no alfabético, para
/// que ordenar la tabla se note.
const CATALOGO: &[(&str, &str, &str, &str)] = &[
    ("Arroz blanco", "lb", "120.00", "180.00"),
    ("Arroz integral", "lb", "150.00", "220.00"),
    ("Frijol negro", "lb", "210.00", "300.00"),
    ("Frijol colorado", "lb", "220.00", "310.00"),
    ("Frijol chícharo", "lb", "95.00", "150.00"),
    ("Aceite de girasol", "L", "480.00", "650.00"),
    ("Aceite de oliva", "L", "1200.00", "1650.00"),
    ("Aceite vegetal", "L", "420.00", "580.00"),
    ("Leche en polvo", "g", "2.80", "4.20"),
    ("Leche condensada", "unidad", "180.00", "260.00"),
    ("Leche evaporada", "unidad", "160.00", "235.00"),
    ("Café molido", "g", "5.50", "9.00"),
    ("Café en grano", "g", "6.20", "10.50"),
    ("Azúcar blanca", "lb", "85.00", "130.00"),
    ("Azúcar prieta", "lb", "75.00", "115.00"),
    ("Sal fina", "lb", "30.00", "55.00"),
    ("Refresco de cola", "unidad", "41.67", "80.00"),
    ("Refresco de naranja", "unidad", "41.67", "80.00"),
    ("Refresco de piña", "unidad", "41.67", "80.00"),
    ("Refresco de limón", "unidad", "41.67", "80.00"),
    ("Cerveza lata", "unidad", "150.00", "250.00"),
    ("Cerveza botella", "unidad", "165.00", "270.00"),
    ("Agua mineral", "L", "60.00", "110.00"),
    ("Jugo de mango", "L", "180.00", "270.00"),
    ("Jugo de guayaba", "L", "175.00", "265.00"),
    ("Galleta de sal", "unidad", "18.00", "35.00"),
    ("Galleta dulce", "unidad", "22.00", "42.00"),
    ("Pan suave", "unidad", "25.00", "50.00"),
    ("Pan de flauta", "unidad", "20.00", "40.00"),
    ("Harina de trigo", "lb", "70.00", "110.00"),
    ("Harina de maíz", "lb", "65.00", "100.00"),
    ("Espagueti", "lb", "95.00", "150.00"),
    ("Macarrones", "lb", "90.00", "145.00"),
    ("Puré de tomate", "unidad", "140.00", "210.00"),
    ("Atún en lata", "unidad", "290.00", "420.00"),
    ("Sardina en lata", "unidad", "180.00", "270.00"),
    ("Jamón pierna", "lb", "650.00", "890.00"),
    ("Jamón viking", "lb", "480.00", "680.00"),
    ("Salchicha", "lb", "320.00", "460.00"),
    ("Queso blanco", "lb", "520.00", "720.00"),
    ("Queso gouda", "lb", "780.00", "1050.00"),
    ("Mantequilla", "g", "3.20", "5.40"),
    ("Mayonesa", "ml", "0.85", "1.40"),
    ("Kétchup", "ml", "0.70", "1.20"),
    ("Mostaza", "ml", "0.65", "1.15"),
    ("Vinagre", "L", "85.00", "140.00"),
    ("Huevo", "unidad", "32.00", "55.00"),
    ("Pollo troceado", "lb", "380.00", "520.00"),
    ("Picadillo de res", "lb", "420.00", "580.00"),
    ("Pescado fileteado", "lb", "450.00", "620.00"),
    ("Papa", "lb", "45.00", "80.00"),
    ("Boniato", "lb", "35.00", "65.00"),
    ("Yuca", "lb", "40.00", "70.00"),
    ("Plátano burro", "unidad", "25.00", "45.00"),
    ("Plátano fruta", "unidad", "20.00", "38.00"),
    ("Malanga", "lb", "55.00", "95.00"),
    ("Calabaza", "lb", "30.00", "55.00"),
    ("Tomate", "lb", "60.00", "105.00"),
    ("Cebolla", "lb", "95.00", "150.00"),
    ("Ajo", "lb", "280.00", "390.00"),
    ("Pimiento", "lb", "110.00", "170.00"),
    ("Limón", "lb", "70.00", "120.00"),
    ("Naranja", "lb", "50.00", "90.00"),
    ("Piña", "unidad", "90.00", "150.00"),
    ("Melón", "unidad", "120.00", "190.00"),
    ("Mango", "lb", "65.00", "110.00"),
    ("Aguacate", "unidad", "85.00", "140.00"),
    ("Jabón de baño", "unidad", "95.00", "160.00"),
    ("Jabón de lavar", "unidad", "75.00", "125.00"),
    ("Jabón líquido", "ml", "0.40", "0.72"),
    ("Champú", "ml", "0.55", "0.95"),
    ("Pasta dental", "unidad", "180.00", "290.00"),
    ("Cepillo dental", "unidad", "110.00", "185.00"),
    ("Papel sanitario", "unidad", "65.00", "115.00"),
    ("Detergente en polvo", "g", "1.20", "2.10"),
    ("Detergente líquido", "ml", "0.95", "1.65"),
    ("Cloro", "L", "70.00", "120.00"),
    ("Fósforo", "unidad", "8.00", "18.00"),
    ("Vela", "unidad", "35.00", "62.00"),
    ("Pila AA", "unidad", "95.00", "155.00"),
];

/// En qué estado nace cada producto.
///
/// No sirve de nada sembrar ochenta productos normales: las pantallas
/// marcan seis situaciones distintas y hay que poder verlas todas sin
/// fabricarlas a mano una por una. Se reparten por posición, así que el
/// resultado es el mismo en cada ejecución.
#[derive(Debug, Clone, Copy)]
enum Estado {
    /// Vitrina por encima del objetivo y repuesto guardado.
    Normal,
    /// Vitrina por debajo del objetivo, con mercancía para bajar.
    HayQueReponer,
    /// Todo guardado y nada exhibido: el cliente no lo ve.
    SinExhibir,
    /// Por debajo del objetivo y sin repuesto: hay que comprar.
    FaltaComprar,
    /// Existencia por debajo del mínimo fijado.
    BajoMinimo,
    /// Sin nada, ni en almacén ni en vitrina.
    Agotado,
}

impl Estado {
    const TODOS: [Self; 6] = [
        Self::Normal,
        Self::HayQueReponer,
        Self::SinExhibir,
        Self::FaltaComprar,
        Self::BajoMinimo,
        Self::Agotado,
    ];

    const fn nombre(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::HayQueReponer => "falta reponer",
            Self::SinExhibir => "guardado sin exhibir",
            Self::FaltaComprar => "falta comprar",
            Self::BajoMinimo => "por agotarse",
            Self::Agotado => "agotado",
        }
    }

    /// Cantidades de apertura: `(almacén, vitrina, mínimo, objetivo)`.
    ///
    /// Van como texto porque es lo que recibe el caso de uso: un número de
    /// JSON es `f64` y no preserva los valores exactos (DT-7).
    const fn cantidades(
        self,
    ) -> (
        Option<&'static str>,
        Option<&'static str>,
        Option<&'static str>,
        Option<&'static str>,
    ) {
        match self {
            Self::Normal => (Some("80"), Some("40"), Some("10"), Some("20")),
            Self::HayQueReponer => (Some("60"), Some("5"), Some("10"), Some("20")),
            Self::SinExhibir => (Some("40"), None, Some("10"), Some("12")),
            Self::FaltaComprar => (None, Some("2"), Some("10"), Some("20")),
            Self::BajoMinimo => (Some("2"), Some("3"), Some("50"), Some("10")),
            // Sin cantidad no hay dónde guardar el costo: el costo se
            // deriva del valor invertido entre la existencia (RF-COS-02).
            Self::Agotado => (None, None, Some("10"), Some("15")),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opciones = Opciones::leer();
    let catalogo = &CATALOGO[..opciones.cuantos.min(CATALOGO.len())];

    println!(
        "Sembrando {} productos en {}",
        catalogo.len(),
        opciones.base.display()
    );

    let base = Arc::new(BaseDatos::abrir(&opciones.base)?);
    let repositorio = RepositorioProductoSqlite::nuevo(base);

    // Se listan los inactivos también: un producto desactivado sigue
    // ocupando su nombre y su SKU.
    let existentes = repositorio.listar(true)?;
    let mut creados = 0_u32;
    let mut saltados = 0_u32;
    let mut por_estado = [0_u32; 6];

    for (indice, (nombre, unidad, costo, precio)) in catalogo.iter().enumerate() {
        if existentes
            .iter()
            .any(|fila| fila.producto.nombre().eq_ignore_ascii_case(nombre))
        {
            saltados += 1;
            continue;
        }

        let estado = Estado::TODOS[indice % Estado::TODOS.len()];
        let (almacen, vitrina, minimo, objetivo) = estado.cantidades();
        let hay_mercancia = almacen.is_some() || vitrina.is_some();

        RegistrarProducto::nuevo(&repositorio).ejecutar(ComandoRegistrarProducto {
            // Vacío significa «derívalo del nombre» (RF-CAT-02). Las
            // colisiones las desempata el caso de uso, no este archivo.
            sku: String::new(),
            nombre: (*nombre).to_owned(),
            unidad_base: (*unidad).to_owned(),
            precio_unitario: (*precio).to_owned(),
            costo_unitario: hay_mercancia.then(|| (*costo).to_owned()),
            cantidad_almacen: almacen.map(str::to_owned),
            cantidad_vitrina: vitrina.map(str::to_owned),
            stock_minimo: minimo.map(str::to_owned),
            objetivo_vitrina: objetivo.map(str::to_owned),
        })?;

        creados += 1;
        por_estado[indice % Estado::TODOS.len()] += 1;
    }

    println!("\n  {creados} creados, {saltados} ya existían\n");
    for (estado, cuantos) in Estado::TODOS.iter().zip(por_estado) {
        if cuantos > 0 {
            println!("  {cuantos:>3}  {}", estado.nombre());
        }
    }

    println!("\nAbre Vitrina (F3) y Almacén (F4): la lista ya da para desplazarse.");
    Ok(())
}

// ============================================================== opciones

struct Opciones {
    cuantos: usize,
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
            cuantos: valor("--cuantos")
                .and_then(|v| v.parse().ok())
                .unwrap_or(CATALOGO.len()),
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
