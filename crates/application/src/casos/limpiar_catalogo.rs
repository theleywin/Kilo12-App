//! Casos de uso: quitar del catálogo lo que ya no se vende (RF-CAT-13).
//!
//! Un mercadito pequeño acumula etiquetas. Se da de alta un producto porque
//! llegó una caja, se vende hasta la última unidad y no vuelve a llegar
//! nunca. La etiqueta se queda, y al año el catálogo tiene cuarenta nombres
//! de los que la mitad no existen. Eso no es un problema cosmético: la lista
//! desde la que se vende es la misma lista, y buscar entre ruido es más
//! lento y se equivoca más.
//!
//! ## Por qué hay dos destinos y no uno
//!
//! «Eliminar un producto» parece una sola operación y son dos, porque hay
//! dos clases de producto vacío:
//!
//! - **El que nunca se movió.** Una etiqueta creada por error, un producto
//!   que se dio de alta y jamás llegó a venderse. No hay ventas que lo
//!   apunten ni kárdex que lo explique. Se **borra** y no queda nada de él.
//! - **El que se vendió hasta agotarse.** Tiene renglones de venta con su
//!   nombre y su costo congelados, y un kárdex que cuenta cómo entró y cómo
//!   salió. Se **retira** de la venta: desaparece de Vender, de Vitrina, de
//!   Almacén y de los informes, y su historial se queda entero.
//!
//! No es una decisión de gusto. Es RF-CAT-06 y RF-CAT-07, y además es lo
//! único que el esquema permite: `venta_linea.producto_id`,
//! `movimiento.producto_id` y `venta_en_espera_linea.producto_id` son claves
//! foráneas al producto, y la base las aplica. Un producto que vendió **no
//! se puede** borrar sin llevarse por delante la contabilidad de esas
//! ventas, y la contabilidad no se toca.
//!
//! Lo que sí es decisión nuestra es **enseñar la diferencia**. El cartel de
//! confirmación lista los dos grupos por separado: lo que se va para
//! siempre y lo que solo se esconde. Un botón que dice «eliminar» y por
//! debajo hace dos cosas distintas tiene que decir cuál le toca a cada uno.
//!
//! ## La venta en espera frena las dos cosas (RF-CAT-14)
//!
//! Un producto puede estar en cero y, aun así, aparecer en una venta
//! apartada sin cobrar (RF-VTA-14): se apartó cuando había mercancía y se
//! agotó después. Esos no se tocan, ni para borrar ni para retirar. Una
//! venta a medias es una venta, y el catálogo no le quita las piezas por
//! debajo. Salen en el cartel como bloqueados, con el motivo, para que se
//! resuelva la espera primero.
//!
//! ## Por qué se vuelve a decidir al confirmar
//!
//! La vista previa dice lo que pasaría; al confirmar, **se recalcula todo
//! desde cero** y se actúa sobre lo que hay en ese momento, no sobre la
//! lista que viajó a la pantalla. Es el mismo motivo por el que la clave de
//! una revaluación se comprueba en Rust: lo que la interfaz tiene en la mano
//! es una foto, y entre la foto y el clic pudo entrar mercancía de un
//! producto que salía en la lista. Borrarlo entonces sería borrar algo que
//! ya no estaba vacío.

use domain::IdProducto;

use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{ProductoConInventario, RastroProducto, RepositorioProducto};

// ==================================================== lo que se devuelve

/// Qué se le puede hacer a un producto que se quedó sin mercancía.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destino {
    /// Nunca se movió: se borra y no queda nada de él.
    Borrar,
    /// Vendió o se movió: se retira de la venta y conserva su historial.
    Retirar,
    /// Está en una venta sin cobrar: no se toca hasta resolverla.
    Bloqueado,
}

impl Destino {
    /// Código estable para la interfaz, como los de los errores (DT-7).
    pub const fn como_texto(self) -> &'static str {
        match self {
            Self::Borrar => "BORRAR",
            Self::Retirar => "RETIRAR",
            Self::Bloqueado => "BLOQUEADO",
        }
    }
}

/// Un producto sin mercancía, con lo que se haría con él y por qué.
///
/// Las cifras del rastro viajan a la pantalla a propósito: «se retira porque
/// tiene 142 ventas» explica la decisión, y «se retira» a secas obliga a
/// confiar en ella.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductoVacio {
    pub id: i64,
    pub sku: String,
    pub nombre: String,
    pub destino: Destino,
    /// Renglones de ventas cobradas que lo llevan.
    pub ventas: i64,
    /// Asientos del kárdex a su nombre.
    pub movimientos: i64,
    /// Renglones en ventas apartadas y sin cobrar.
    pub esperas: i64,
    /// Ya estaba retirado de la venta antes de todo esto.
    pub retirado: bool,
}

/// Lo que una limpieza del catálogo haría, o lo que acaba de hacer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Limpieza {
    /// Se van para siempre: nunca se movieron.
    pub se_borran: Vec<ProductoVacio>,
    /// Se esconden de la venta y conservan su historial.
    pub se_retiran: Vec<ProductoVacio>,
    /// No se tocan: están en una venta empezada y sin cobrar.
    pub bloqueados: Vec<ProductoVacio>,
}

impl Limpieza {
    /// No hay nada que hacer.
    pub fn no_hay_nada(&self) -> bool {
        self.se_borran.is_empty() && self.se_retiran.is_empty()
    }

    /// Identificadores de lo que se borra, para el puerto.
    fn ids_a_borrar(&self) -> Vec<IdProducto> {
        self.se_borran.iter().map(|f| IdProducto(f.id)).collect()
    }

    /// Identificadores de lo que se retira, para el puerto.
    fn ids_a_retirar(&self) -> Vec<IdProducto> {
        self.se_retiran.iter().map(|f| IdProducto(f.id)).collect()
    }
}

// ======================================================= la clasificación

/// Decide qué le toca a un producto vacío según lo que dejó escrito.
///
/// El orden de las preguntas es el orden en que mandan. Primero la venta en
/// espera: frena las dos cosas, y da igual lo demás. Después el rastro: sin
/// nada escrito se borra, con algo escrito se retira.
const fn destino_de(rastro: RastroProducto) -> Destino {
    if rastro.esperas > 0 {
        Destino::Bloqueado
    } else if rastro.ventas == 0 && rastro.movimientos == 0 {
        Destino::Borrar
    } else {
        Destino::Retirar
    }
}

/// Describe un producto vacío con su destino ya decidido.
fn describir(fila: &ProductoConInventario, rastro: RastroProducto) -> ProductoVacio {
    ProductoVacio {
        id: fila.producto.id().map_or(0, |id| id.0),
        sku: fila.producto.sku().to_owned(),
        nombre: fila.producto.nombre().to_owned(),
        destino: destino_de(rastro),
        ventas: rastro.ventas,
        movimientos: rastro.movimientos,
        esperas: rastro.esperas,
        retirado: !fila.producto.esta_activo(),
    }
}

/// Dice si al producto le queda mercancía.
///
/// Un total que no se puede sumar —existencias imposibles— se trata como
/// «tiene algo»: ante la duda, no se quita del catálogo.
fn tiene_mercancia(fila: &ProductoConInventario) -> bool {
    !fila
        .inventario
        .existencias()
        .total()
        .is_ok_and(|total| total.es_cero())
}

// ================================================ quitar uno, desde su ficha

/// Quita del catálogo un producto concreto (RF-CAT-06, RF-CAT-07).
#[derive(Debug)]
pub struct QuitarDelCatalogo<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> QuitarDelCatalogo<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    /// Dice qué pasaría con ese producto **sin tocar nada**.
    ///
    /// Es lo que alimenta el cartel de confirmación: hace falta saber si lo
    /// que viene es un borrado definitivo o un retiro reversible antes de
    /// decidir, no después.
    pub fn previsualizar(&self, id: i64) -> Resultado<ProductoVacio> {
        let (fila, rastro) = self.preparar(id)?;
        Ok(describir(&fila, rastro))
    }

    /// Lo quita, y devuelve lo que hizo.
    ///
    /// Vuelve a mirar la existencia y el rastro: decide sobre lo que hay
    /// ahora, no sobre lo que enseñó la vista previa.
    pub fn ejecutar(&self, id: i64) -> Resultado<ProductoVacio> {
        let (fila, rastro) = self.preparar(id)?;
        let hecho = describir(&fila, rastro);

        match hecho.destino {
            // Una venta sin cobrar manda sobre la limpieza del catálogo.
            Destino::Bloqueado => return Err(ErrorAplicacion::ProductoEnVentaEnEspera),
            Destino::Borrar => self.repositorio.limpiar_catalogo(&[IdProducto(id)], &[])?,
            Destino::Retirar => self.repositorio.limpiar_catalogo(&[], &[IdProducto(id)])?,
        }

        Ok(hecho)
    }

    /// Carga el producto y comprueba lo único que no se negocia: que esté
    /// vacío.
    fn preparar(&self, id: i64) -> Resultado<(ProductoConInventario, RastroProducto)> {
        let fila =
            self.repositorio
                .obtener(IdProducto(id))?
                .ok_or(ErrorAplicacion::NoEncontrado {
                    entidad: "producto",
                    id,
                })?;

        // La regla que pidió el dueño, y la única que no admite matices: un
        // producto con mercancía no sale del catálogo. Si saliera, esa
        // mercancía dejaría de contarse en el valor del inventario sin que
        // nadie la haya vendido ni mermado.
        if tiene_mercancia(&fila) {
            return Err(ErrorAplicacion::ProductoConExistencia);
        }

        let rastro = self.repositorio.rastro_de_producto(IdProducto(id))?;
        Ok((fila, rastro))
    }
}

// =========================================== quitar todos los vacíos de golpe

/// Limpia de una vez todas las etiquetas de productos sin mercancía
/// (RF-CAT-13).
#[derive(Debug)]
pub struct LimpiarCatalogo<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> LimpiarCatalogo<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    /// Reúne los productos vacíos y reparte cada uno en su grupo, **sin
    /// tocar nada**.
    ///
    /// Mira también los ya retirados: uno que se retiró y que nunca llegó a
    /// moverse sigue ocupando su SKU y sigue saliendo al marcar «ver los
    /// retirados», así que esta es la ocasión de borrarlo de verdad.
    pub fn previsualizar(&self) -> Resultado<Limpieza> {
        let mut limpieza = Limpieza::default();

        for fila in self.repositorio.listar(true)? {
            if tiene_mercancia(&fila) {
                continue;
            }

            let id = fila.producto.id().map_or(0, |id| id.0);
            let candidato = describir(&fila, self.repositorio.rastro_de_producto(IdProducto(id))?);

            match candidato.destino {
                Destino::Borrar => limpieza.se_borran.push(candidato),
                // Retirar lo que ya está retirado no es nada: no se enseña
                // como trabajo pendiente porque no lo es.
                Destino::Retirar if candidato.retirado => {}
                Destino::Retirar => limpieza.se_retiran.push(candidato),
                Destino::Bloqueado => limpieza.bloqueados.push(candidato),
            }
        }

        Ok(limpieza)
    }

    /// Limpia el catálogo y devuelve lo que hizo.
    ///
    /// La lista se calcula aquí otra vez: la pantalla no manda identificadores
    /// y por tanto no puede pedir el borrado de algo que entre tanto dejó de
    /// estar vacío.
    pub fn ejecutar(&self) -> Resultado<Limpieza> {
        let limpieza = self.previsualizar()?;

        if limpieza.no_hay_nada() {
            return Ok(limpieza);
        }

        // Todo en una sola operación: media limpieza aplicada deja el
        // catálogo donde nadie lo dejó (RNF-5).
        self.repositorio
            .limpiar_catalogo(&limpieza.ids_a_borrar(), &limpieza.ids_a_retirar())?;

        Ok(limpieza)
    }
}

#[cfg(test)]
mod pruebas {
    use domain::UnidadBase;

    use super::*;
    use crate::dobles::ProductosEnMemoria;

    /// Tres productos vacíos, uno de cada clase, y uno con mercancía.
    ///
    /// - 1 «Mango»: etiqueta pura, nunca se movió.
    /// - 2 «Arroz blanco»: se vendió hasta agotarse.
    /// - 3 «Aceite»: vacío, pero apartado en una venta sin cobrar.
    /// - 4 «Café»: todavía tiene mercancía.
    fn catalogo() -> ProductosEnMemoria {
        ProductosEnMemoria::default()
            .con_producto(
                1,
                "Mango",
                UnidadBase::Libra,
                &[(10, "Libra", "1", "80")],
                "0",
            )
            .con_producto(
                2,
                "Arroz blanco",
                UnidadBase::Libra,
                &[(20, "Libra", "1", "180")],
                "0",
            )
            .con_rastro(2, 142, 7, 0)
            .con_producto(
                3,
                "Aceite",
                UnidadBase::Unidad,
                &[(30, "Unidad", "1", "900")],
                "0",
            )
            .con_rastro(3, 0, 2, 1)
            .con_producto(
                4,
                "Café",
                UnidadBase::Libra,
                &[(40, "Libra", "1", "600")],
                "5",
            )
            .con_rastro(4, 9, 3, 0)
    }

    // ------------------------------------------------- quitar uno a uno

    #[test]
    fn un_producto_que_nunca_se_movio_se_borra() {
        let productos = catalogo();

        let hecho = QuitarDelCatalogo::nuevo(&productos)
            .ejecutar(1)
            .expect("quitar el mango");

        assert_eq!(hecho.destino, Destino::Borrar);
        assert!(productos.obtener(IdProducto(1)).expect("buscar").is_none());
    }

    #[test]
    fn un_producto_que_vendio_se_retira_y_conserva_su_historial() {
        let productos = catalogo();

        let hecho = QuitarDelCatalogo::nuevo(&productos)
            .ejecutar(2)
            .expect("quitar el arroz");

        assert_eq!(hecho.destino, Destino::Retirar);
        assert_eq!(hecho.ventas, 142);

        // Sigue en la base, y ya no sale al vender.
        let guardado = productos
            .obtener(IdProducto(2))
            .expect("buscar")
            .expect("sigue ahí");
        assert!(!guardado.producto.esta_activo());
    }

    #[test]
    fn con_mercancia_encima_no_se_quita_nada() {
        let productos = catalogo();

        let error = QuitarDelCatalogo::nuevo(&productos)
            .ejecutar(4)
            .expect_err("todavía queda café");

        assert_eq!(error.codigo(), "PRODUCTO_CON_EXISTENCIA");
        // Ni borrado ni retirado.
        let guardado = productos
            .obtener(IdProducto(4))
            .expect("buscar")
            .expect("sigue ahí");
        assert!(guardado.producto.esta_activo());
    }

    #[test]
    fn un_producto_en_una_venta_en_espera_no_se_toca() {
        let productos = catalogo();

        let error = QuitarDelCatalogo::nuevo(&productos)
            .ejecutar(3)
            .expect_err("hay una venta sin cobrar");

        assert_eq!(error.codigo(), "PRODUCTO_EN_VENTA_EN_ESPERA");
        let guardado = productos
            .obtener(IdProducto(3))
            .expect("buscar")
            .expect("sigue ahí");
        assert!(guardado.producto.esta_activo());
    }

    #[test]
    fn un_producto_que_no_existe_se_dice_tal_cual() {
        let error = QuitarDelCatalogo::nuevo(&catalogo())
            .ejecutar(999)
            .expect_err("no existe");

        assert_eq!(error.codigo(), "NO_ENCONTRADO");
    }

    #[test]
    fn la_vista_previa_no_toca_nada() {
        let productos = catalogo();

        let prevista = QuitarDelCatalogo::nuevo(&productos)
            .previsualizar(1)
            .expect("previsualizar");

        assert_eq!(prevista.destino, Destino::Borrar);
        assert_eq!(prevista.nombre, "Mango");
        // Y el mango sigue donde estaba.
        assert!(productos.obtener(IdProducto(1)).expect("buscar").is_some());
    }

    #[test]
    fn la_vista_previa_de_un_bloqueado_lo_dice_en_vez_de_fallar() {
        // Al abrir el cartel hay que poder explicar por qué no se puede;
        // fallar ahí dejaría a la pantalla sin nada que contar.
        let prevista = QuitarDelCatalogo::nuevo(&catalogo())
            .previsualizar(3)
            .expect("previsualizar");

        assert_eq!(prevista.destino, Destino::Bloqueado);
        assert_eq!(prevista.esperas, 1);
    }

    // ------------------------------------------------ limpieza completa

    #[test]
    fn la_limpieza_reparte_cada_vacio_en_su_grupo() {
        let prevista = LimpiarCatalogo::nuevo(&catalogo())
            .previsualizar()
            .expect("previsualizar");

        assert_eq!(nombres(&prevista.se_borran), ["Mango"]);
        assert_eq!(nombres(&prevista.se_retiran), ["Arroz blanco"]);
        assert_eq!(nombres(&prevista.bloqueados), ["Aceite"]);
    }

    #[test]
    fn lo_que_tiene_mercancia_no_entra_en_la_limpieza() {
        let prevista = LimpiarCatalogo::nuevo(&catalogo())
            .previsualizar()
            .expect("previsualizar");

        let todos: Vec<&str> = prevista
            .se_borran
            .iter()
            .chain(&prevista.se_retiran)
            .chain(&prevista.bloqueados)
            .map(|fila| fila.nombre.as_str())
            .collect();

        assert!(!todos.contains(&"Café"));
    }

    #[test]
    fn limpiar_borra_unos_y_retira_otros_de_una_vez() {
        let productos = catalogo();

        let hecha = LimpiarCatalogo::nuevo(&productos)
            .ejecutar()
            .expect("limpiar");

        assert_eq!(hecha.se_borran.len(), 1);
        assert_eq!(hecha.se_retiran.len(), 1);

        // El mango ya no está.
        assert!(productos.obtener(IdProducto(1)).expect("buscar").is_none());
        // El arroz sí, retirado.
        assert!(!productos
            .obtener(IdProducto(2))
            .expect("buscar")
            .expect("sigue ahí")
            .producto
            .esta_activo());
        // El aceite, intacto: tiene una venta sin cobrar.
        assert!(productos
            .obtener(IdProducto(3))
            .expect("buscar")
            .expect("sigue ahí")
            .producto
            .esta_activo());
    }

    #[test]
    fn un_retirado_que_nunca_se_movio_se_puede_borrar_de_verdad() {
        // Se retiró en su día, pero nunca vendió nada: la etiqueta sigue
        // ocupando su SKU y saliendo al marcar «ver los retirados».
        let productos = catalogo();
        productos.desactivar_producto(1);

        let prevista = LimpiarCatalogo::nuevo(&productos)
            .previsualizar()
            .expect("previsualizar");

        assert_eq!(nombres(&prevista.se_borran), ["Mango"]);
        assert!(prevista.se_borran[0].retirado);
    }

    #[test]
    fn un_retirado_con_historial_ya_no_es_trabajo_pendiente() {
        // Está donde tiene que estar: fuera de la venta y con su historial.
        // Volver a ofrecerlo cada vez sería pedir confirmación de algo que
        // ya se hizo.
        let productos = catalogo();
        productos.desactivar_producto(2);

        let prevista = LimpiarCatalogo::nuevo(&productos)
            .previsualizar()
            .expect("previsualizar");

        assert!(prevista.se_retiran.is_empty());
        assert_eq!(nombres(&prevista.se_borran), ["Mango"]);
    }

    #[test]
    fn un_catalogo_sin_vacios_no_propone_nada() {
        let productos = ProductosEnMemoria::default().con_producto(
            1,
            "Café",
            UnidadBase::Libra,
            &[(10, "Libra", "1", "600")],
            "5",
        );

        let hecha = LimpiarCatalogo::nuevo(&productos)
            .ejecutar()
            .expect("limpiar");

        assert!(hecha.no_hay_nada());
        assert!(productos.limpiezas.borrow().is_empty());
    }

    fn nombres(filas: &[ProductoVacio]) -> Vec<&str> {
        filas.iter().map(|fila| fila.nombre.as_str()).collect()
    }
}
