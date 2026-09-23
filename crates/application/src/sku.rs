//! Derivación del SKU a partir del nombre del producto (RF-CAT-02).
//!
//! El SKU se teclea durante la venta (RF-VTA-01), así que tiene que ser
//! corto, en mayúsculas y sin acentos: obligar a escribir «CAFÉ» con tilde
//! en una caja es una forma elegante de no encontrar nunca nada.
//!
//! Estas funciones son **puras**: dado un nombre devuelven siempre el mismo
//! código. Quien resuelve los choques contra el catálogo es el caso de uso,
//! porque eso ya exige mirar la base de datos entera.

/// Longitud máxima del SKU generado.
///
/// No es un límite del almacenamiento —la columna es `TEXT`— sino de lo que
/// una persona tolera leer en una tabla y teclear en una venta.
pub const LARGO_MAXIMO: usize = 24;

/// Base que se usa cuando el nombre no aporta ni una letra ni un dígito.
pub const BASE_POR_DEFECTO: &str = "PRODUCTO";

/// Separador entre palabras. También une el sufijo que desempata.
const SEPARADOR: char = '-';

/// En qué se convierte cada carácter del nombre.
enum Trazo {
    /// Un carácter válido para el código, ya normalizado.
    Letra(char),
    /// Un corte entre palabras: espacios, signos, puntuación.
    Separador,
    /// No aporta ni corta: tildes sueltas de la forma descompuesta.
    Ignorar,
}

/// Convierte el nombre de un producto en un código tecleable.
///
/// «Café molido 1 kg» se vuelve `CAFE-MOLIDO-1-KG`. Nunca devuelve una
/// cadena vacía: si el nombre no aporta nada utilizable cae en
/// [`BASE_POR_DEFECTO`].
pub fn derivar(nombre: &str) -> String {
    let mut sku = String::with_capacity(LARGO_MAXIMO);

    for caracter in nombre.chars() {
        match plegar(caracter) {
            Trazo::Letra(letra) => sku.push(letra),
            Trazo::Ignorar => {}
            // Un separador al principio, o pegado a otro, no aporta nada.
            Trazo::Separador if sku.is_empty() || sku.ends_with(SEPARADOR) => {}
            Trazo::Separador => sku.push(SEPARADOR),
        }
    }

    recortar(&mut sku, LARGO_MAXIMO);

    if sku.is_empty() {
        BASE_POR_DEFECTO.to_owned()
    } else {
        sku
    }
}

/// Construye el candidato número `numero` para desempatar una colisión.
///
/// El sufijo entra **dentro** del largo máximo: se recorta la raíz para
/// hacerle sitio, en lugar de dejar que el código crezca sin control.
pub fn con_sufijo(base: &str, numero: u32) -> String {
    let sufijo = format!("{SEPARADOR}{numero}");
    let mut raiz = base.to_owned();
    recortar(&mut raiz, LARGO_MAXIMO.saturating_sub(sufijo.len()));

    format!("{raiz}{sufijo}")
}

/// Corta a `largo` caracteres sin dejar un separador colgando al final.
///
/// El corte por bytes es seguro porque todo lo que se escribe en el código
/// es ASCII: un carácter, un byte.
fn recortar(sku: &mut String, largo: usize) {
    sku.truncate(largo);

    while sku.ends_with(SEPARADOR) {
        sku.pop();
    }
}

/// Decide en qué se convierte un carácter del nombre.
fn plegar(caracter: char) -> Trazo {
    let mayuscula = caracter.to_ascii_uppercase();
    if mayuscula.is_ascii_alphanumeric() {
        return Trazo::Letra(mayuscula);
    }

    // Tildes y diéresis sueltas: en macOS «café» puede llegar como `e` más
    // un acento combinante. Sin esto, el acento partiría la palabra en dos.
    if matches!(caracter, '\u{0300}'..='\u{036F}') {
        return Trazo::Ignorar;
    }

    match caracter {
        'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => Trazo::Letra('A'),
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => Trazo::Letra('E'),
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => Trazo::Letra('I'),
        'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => Trazo::Letra('O'),
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => Trazo::Letra('U'),
        'ñ' | 'Ñ' => Trazo::Letra('N'),
        'ç' | 'Ç' => Trazo::Letra('C'),
        _ => Trazo::Separador,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn convierte_el_nombre_en_codigo_tecleable() {
        assert_eq!(derivar("Arroz blanco"), "ARROZ-BLANCO");
    }

    #[test]
    fn quita_las_tildes_y_la_enie() {
        assert_eq!(derivar("Café con leche"), "CAFE-CON-LECHE");
        assert_eq!(derivar("Piña"), "PINA");
    }

    #[test]
    fn ignora_las_tildes_de_la_forma_descompuesta() {
        // «Café» escrito como `e` + acento combinante, tal como lo produce
        // el teclado de macOS.
        assert_eq!(derivar("Cafe\u{0301} molido"), "CAFE-MOLIDO");
    }

    #[test]
    fn conserva_los_digitos() {
        assert_eq!(derivar("Refresco 500 ml"), "REFRESCO-500-ML");
    }

    #[test]
    fn colapsa_los_signos_y_los_espacios_repetidos() {
        assert_eq!(derivar("  Aceite   //  1 L  "), "ACEITE-1-L");
    }

    #[test]
    fn no_deja_separadores_en_los_extremos() {
        let sku = derivar("¡¿Jabón?!");
        assert_eq!(sku, "JABON");
    }

    #[test]
    fn recorta_al_largo_maximo_sin_dejar_separador_colgando() {
        let sku = derivar("Detergente líquido concentrado para ropa blanca");
        assert_eq!(sku, "DETERGENTE-LIQUIDO-CONCE");
        assert_eq!(sku.len(), LARGO_MAXIMO);
    }

    #[test]
    fn cae_en_la_base_por_defecto_cuando_el_nombre_no_aporta_nada() {
        assert_eq!(derivar("«»—"), BASE_POR_DEFECTO);
        assert_eq!(derivar("   "), BASE_POR_DEFECTO);
    }

    #[test]
    fn el_sufijo_desempata_sin_pasarse_del_largo() {
        assert_eq!(con_sufijo("ARROZ-BLANCO", 2), "ARROZ-BLANCO-2");

        // La raíz cede espacio para que el sufijo entre en el largo máximo.
        let largo = con_sufijo("DETERGENTE-LIQUIDO-CONCE", 999);
        assert_eq!(largo, "DETERGENTE-LIQUIDO-C-999");
        assert_eq!(largo.len(), LARGO_MAXIMO);
    }

    #[test]
    fn el_sufijo_tampoco_deja_separador_colgando() {
        // El recorte de la raíz cae justo sobre un separador: no puede
        // quedar «ARROZ-BLANCO-DEL-VAL--10».
        assert_eq!(
            con_sufijo("ARROZ-BLANCO-DEL-VAL-X", 10),
            "ARROZ-BLANCO-DEL-VAL-10"
        );
    }
}
