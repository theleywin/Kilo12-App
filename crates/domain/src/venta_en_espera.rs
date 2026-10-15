//! La venta en espera (RF-VTA-14).
//!
//! Es una venta que se aparta a medio armar para atender a otro cliente y
//! se retoma después. Guarda **solo la intención**: qué producto, en qué
//! presentación y cuánto. No guarda precio, ni costo, ni tasa, ni pagos.
//!
//! No es un descuido: una venta en espera todavía no ocurrió. Congelar el
//! precio al apartarla sería cobrar mañana con la lista de ayer, y congelar
//! el costo mentiría sobre la ganancia (RF-VTA-13). Todo eso se decide al
//! cobrar, como en cualquier venta. Por lo mismo tampoco reserva
//! existencia: la mercancía sigue en la vitrina, disponible para quien
//! llegue primero con el dinero.

use crate::cantidad::Cantidad;
use crate::error::ErrorDominio;
use crate::presentacion::IdPresentacion;
use crate::producto::IdProducto;

/// Largo máximo de la nota, en caracteres.
///
/// La nota sirve para reconocer la venta en la lista —«la señora del
/// pan», «mesa 3»—, no para escribir una carta. Un límite corto mantiene
/// la lista legible.
pub const LARGO_MAXIMO_NOTA: usize = 120;

/// Identificador de una venta en espera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdVentaEnEspera(pub i64);

/// Un renglón apartado: qué se lleva y cuánto, sin precio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineaEnEspera {
    producto: IdProducto,
    presentacion: IdPresentacion,
    /// Cuántas presentaciones, no unidades base: 2 six-packs son 2.
    cantidad: Cantidad,
}

impl LineaEnEspera {
    pub fn nueva(
        producto: IdProducto,
        presentacion: IdPresentacion,
        cantidad: Cantidad,
    ) -> Result<Self, ErrorDominio> {
        if !cantidad.es_positiva() {
            return Err(ErrorDominio::CantidadNoPositiva);
        }

        Ok(Self {
            producto,
            presentacion,
            cantidad,
        })
    }

    pub const fn producto(&self) -> IdProducto {
        self.producto
    }

    pub const fn presentacion(&self) -> IdPresentacion {
        self.presentacion
    }

    pub const fn cantidad(&self) -> Cantidad {
        self.cantidad
    }
}

/// Una venta apartada, con su nota opcional y sus renglones en orden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VentaEnEspera {
    nota: Option<String>,
    lineas: Vec<LineaEnEspera>,
}

impl VentaEnEspera {
    /// Crea una venta en espera validando sus invariantes.
    ///
    /// La nota se recorta y, si queda en blanco, se descarta: una nota
    /// vacía no identifica nada y solo ensucia la lista. Sin ningún renglón
    /// no hay nada que apartar.
    pub fn nueva(nota: Option<&str>, lineas: Vec<LineaEnEspera>) -> Result<Self, ErrorDominio> {
        if lineas.is_empty() {
            return Err(ErrorDominio::EsperaVacia);
        }

        let nota = match nota.map(str::trim) {
            Some(texto) if !texto.is_empty() => {
                // Caracteres, no bytes: «Señora Ñico» no es más larga por
                // llevar eñes.
                if texto.chars().count() > LARGO_MAXIMO_NOTA {
                    return Err(ErrorDominio::NotaDemasiadoLarga {
                        maximo: LARGO_MAXIMO_NOTA,
                    });
                }
                Some(texto.to_owned())
            }
            _ => None,
        };

        Ok(Self { nota, lineas })
    }

    pub fn nota(&self) -> Option<&str> {
        self.nota.as_deref()
    }

    pub fn lineas(&self) -> &[LineaEnEspera] {
        &self.lineas
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cantidad(texto: &str) -> Cantidad {
        texto.parse().expect("cantidad válida")
    }

    fn linea(texto: &str) -> LineaEnEspera {
        LineaEnEspera::nueva(IdProducto(1), IdPresentacion(1), cantidad(texto))
            .expect("línea válida")
    }

    #[test]
    fn sin_renglones_no_hay_nada_que_apartar() {
        assert_eq!(
            VentaEnEspera::nueva(Some("mesa 3"), Vec::new()),
            Err(ErrorDominio::EsperaVacia)
        );
    }

    #[test]
    fn una_linea_sin_cantidad_no_se_aparta() {
        for milesimas in [0, -1000] {
            assert_eq!(
                LineaEnEspera::nueva(
                    IdProducto(1),
                    IdPresentacion(1),
                    Cantidad::desde_milesimas(milesimas)
                ),
                Err(ErrorDominio::CantidadNoPositiva),
                "{milesimas} milésimas no deberían pasar"
            );
        }
    }

    #[test]
    fn la_nota_se_recorta() {
        let espera = VentaEnEspera::nueva(Some("  la señora del pan  "), vec![linea("2")])
            .expect("espera válida");

        assert_eq!(espera.nota(), Some("la señora del pan"));
    }

    #[test]
    fn la_nota_en_blanco_es_ninguna_nota() {
        for nota in [None, Some(""), Some("   ")] {
            let espera = VentaEnEspera::nueva(nota, vec![linea("1")]).expect("espera válida");
            assert_eq!(espera.nota(), None);
        }
    }

    #[test]
    fn la_nota_tiene_un_largo_maximo_en_caracteres() {
        // Justo en el límite, y con eñes: cuentan como un carácter.
        let al_limite = "ñ".repeat(LARGO_MAXIMO_NOTA);
        assert!(VentaEnEspera::nueva(Some(&al_limite), vec![linea("1")]).is_ok());

        let pasada = "a".repeat(LARGO_MAXIMO_NOTA.saturating_add(1));
        assert_eq!(
            VentaEnEspera::nueva(Some(&pasada), vec![linea("1")]),
            Err(ErrorDominio::NotaDemasiadoLarga {
                maximo: LARGO_MAXIMO_NOTA
            })
        );
    }

    #[test]
    fn los_renglones_conservan_su_orden() {
        let primero = LineaEnEspera::nueva(IdProducto(7), IdPresentacion(70), cantidad("1"))
            .expect("línea válida");
        let segundo = LineaEnEspera::nueva(IdProducto(3), IdPresentacion(30), cantidad("2.5"))
            .expect("línea válida");

        let espera = VentaEnEspera::nueva(None, vec![primero, segundo]).expect("espera válida");

        assert_eq!(espera.lineas(), [primero, segundo]);
    }
}
