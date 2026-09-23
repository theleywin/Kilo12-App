//! Caso de uso: dar de alta un producto en el catálogo.

use domain::{
    Cantidad, Dinero, IdProducto, Inventario, Movimiento, Producto, Ubicacion, UnidadBase,
};

use crate::error::{ErrorAplicacion, Resultado};
use crate::puertos::{Asiento, RepositorioProducto};
use crate::sku;

/// Cuántos desempates se prueban antes de rendirse con un SKU derivado.
///
/// Llegar aquí significa mil productos cuyo nombre produce el mismo código:
/// no es un problema de generación, es un catálogo que hay que revisar.
const INTENTOS_SKU: u32 = 999;

/// Datos que llegan desde la interfaz para crear un producto.
///
/// Los importes y las cantidades vienen como **texto**, no como números: es
/// la frontera que impone DT-7, porque un número de JSON es `f64` y no
/// preserva los valores exactos. El caso de uso los interpreta y valida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComandoRegistrarProducto {
    /// Código del producto. **Vacío significa «derívalo del nombre»**
    /// (RF-CAT-02): el usuario no tiene por qué inventar códigos.
    pub sku: String,
    pub nombre: String,
    /// Unidad base: `unidad`, `kg`, `lb`, `g`, `L` o `ml`. Todo lo que no
    /// sea `unidad` se vende en fracciones (RF-CAT-03).
    pub unidad_base: String,
    /// Precio de venta de una unidad base.
    pub precio_unitario: String,
    /// Lo que cuesta una unidad base al dueño. Solo tiene sentido junto a
    /// una cantidad de apertura: el costo se deriva del valor invertido
    /// entre la existencia, así que sin mercancía no hay dónde guardarlo
    /// (RF-COS-02).
    pub costo_unitario: Option<String>,
    /// Cuánto entra al almacén al dar de alta el producto.
    pub cantidad_almacen: Option<String>,
    /// Cuánto se pone en la vitrina al dar de alta el producto.
    pub cantidad_vitrina: Option<String>,
    /// Existencia mínima antes de avisar. Vacío equivale a cero.
    pub stock_minimo: Option<String>,
    /// Cantidad que se desea mantener exhibida en vitrina.
    pub objetivo_vitrina: Option<String>,
}

/// Da de alta un producto.
#[derive(Debug)]
pub struct RegistrarProducto<'a, R: RepositorioProducto> {
    repositorio: &'a R,
}

impl<'a, R: RepositorioProducto> RegistrarProducto<'a, R> {
    pub const fn nuevo(repositorio: &'a R) -> Self {
        Self { repositorio }
    }

    pub fn ejecutar(&self, comando: ComandoRegistrarProducto) -> Resultado<IdProducto> {
        let nombre = comando.nombre.trim();
        let sku = match comando.sku.trim() {
            "" => self.derivar_sku(nombre)?,
            escrito => self.reservar_sku(escrito)?,
        };

        let unidad_base: UnidadBase = comando.unidad_base.parse()?;
        let precio: Dinero = comando.precio_unitario.parse()?;

        let mut producto = Producto::nuevo(&sku, nombre, unidad_base, precio)?;

        if let Some(minimo) = cantidad_opcional(comando.stock_minimo.as_deref())? {
            producto = producto.con_stock_minimo(minimo)?;
        }

        if let Some(objetivo) = cantidad_opcional(comando.objetivo_vitrina.as_deref())? {
            producto = producto.con_objetivo_vitrina(objetivo)?;
        }

        let (inventario, asientos) = apertura(&producto, &comando)?;

        self.repositorio.crear(&producto, &inventario, &asientos)
    }

    /// Acepta el SKU que escribió el usuario, si está libre.
    ///
    /// Aquí **no** se desempata: si alguien tecleó un código a propósito,
    /// cambiárselo por la espalda es peor que decirle que ya existe.
    fn reservar_sku(&self, escrito: &str) -> Resultado<String> {
        // La unicidad no la puede comprobar el dominio: exige mirar todo el
        // catálogo, no solo el producto que se está creando.
        if self.repositorio.existe_sku(escrito)? {
            return Err(ErrorAplicacion::SkuDuplicado(escrito.to_owned()));
        }

        Ok(escrito.to_owned())
    }

    /// Construye un SKU a partir del nombre y lo desempata si hace falta
    /// (RF-CAT-02).
    ///
    /// Dos productos pueden llamarse igual —«Arroz» del proveedor A y del B—
    /// y eso no es un error del usuario: el código lleva un sufijo y sigue
    /// adelante.
    fn derivar_sku(&self, nombre: &str) -> Resultado<String> {
        let base = sku::derivar(nombre);

        if !self.repositorio.existe_sku(&base)? {
            return Ok(base);
        }

        for numero in 2..=INTENTOS_SKU {
            let candidato = sku::con_sufijo(&base, numero);
            if !self.repositorio.existe_sku(&candidato)? {
                return Ok(candidato);
            }
        }

        Err(ErrorAplicacion::SkuAgotado(base))
    }
}

/// Construye la existencia con la que nace el producto.
///
/// Dar de alta un producto y registrar la mercancía que ya tienes son, en
/// el modelo, dos cosas distintas: la segunda es una ENTRADA con su costo,
/// que es lo que fija el costo promedio ponderado (RF-COM-03, RF-COS-04).
/// En la pantalla son un solo formulario, y así debe ser.
fn apertura(
    producto: &Producto,
    comando: &ComandoRegistrarProducto,
) -> Resultado<(Inventario, Vec<Asiento>)> {
    let almacen = cantidad_opcional(comando.cantidad_almacen.as_deref())?.unwrap_or(Cantidad::CERO);
    let vitrina = cantidad_opcional(comando.cantidad_vitrina.as_deref())?.unwrap_or(Cantidad::CERO);
    let costo = importe_opcional(comando.costo_unitario.as_deref())?;
    let hay_mercancia = almacen.es_positiva() || vitrina.es_positiva();

    let costo = match (hay_mercancia, costo) {
        // Producto de catálogo, todavía sin comprar. Es válido: la
        // mercancía entrará después desde Almacén.
        (false, None) => return Ok((Inventario::VACIO, Vec::new())),
        // El costo no se puede guardar solo: se deriva del valor invertido
        // entre la existencia. Sin existencia, no hay dónde ponerlo.
        (false, Some(_)) => return Err(ErrorAplicacion::CantidadInicialRequerida),
        // Existencia sin costo dejaría el inventario valorado en cero, y
        // toda venta parecería ganancia pura.
        (true, None) => return Err(ErrorAplicacion::CostoRequerido),
        (true, Some(costo)) => costo,
    };

    let mut inventario = Inventario::VACIO;
    let mut asientos = Vec::new();

    for (cantidad, ubicacion) in [(almacen, Ubicacion::Bodega), (vitrina, Ubicacion::Vitrina)] {
        if !cantidad.es_positiva() {
            continue;
        }

        // Un producto que se cuenta por unidades no admite media unidad,
        // ni siquiera al darlo de alta.
        producto.validar_cantidad(cantidad)?;

        // Se pasa el importe total de la línea, no el costo unitario: es lo
        // que evita el redondeo intermedio que RF-COS-13 prohíbe.
        let importe = costo.multiplicar_por(cantidad)?;
        inventario = inventario.registrar_entrada(cantidad, importe, ubicacion)?;

        // El saldo se toma DESPUÉS de aplicar la entrada: el kárdex tiene
        // que poder leerse como una sucesión de fotos, no como un total.
        asientos.push(Asiento {
            movimiento: Movimiento::entrada(cantidad, costo, ubicacion)?,
            resultante: inventario.existencias(),
        });
    }

    Ok((inventario, asientos))
}

/// Interpreta una cantidad opcional, tratando el texto vacío como ausencia.
fn cantidad_opcional(texto: Option<&str>) -> Resultado<Option<Cantidad>> {
    match texto.map(str::trim) {
        None | Some("") => Ok(None),
        Some(valor) => Ok(Some(valor.parse::<Cantidad>()?)),
    }
}

/// Interpreta un importe opcional, tratando el texto vacío como ausencia.
fn importe_opcional(texto: Option<&str>) -> Resultado<Option<Dinero>> {
    match texto.map(str::trim) {
        None | Some("") => Ok(None),
        Some(valor) => Ok(Some(valor.parse::<Dinero>()?)),
    }
}
