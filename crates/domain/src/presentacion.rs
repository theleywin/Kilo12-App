//! Formas comerciales en que se vende un producto.
//!
//! Un mismo refresco se vende suelto a 80 y en six-pack a 300. No son 480:
//! el precio de cada presentación es independiente y no se deriva del factor
//! (RF-PRS-04). Por eso la presentación guarda su propio precio.
//!
//! Lo que **no** guarda es existencia. La existencia es única por producto y
//! vive en su unidad base (RF-PRS-05); vender un six-pack descuenta seis
//! unidades. Modelar el paquete como un producto aparte produciría dos
//! existencias que se contradicen, y abrir un paquete obligaría a registrar
//! una conversión que nadie registra un martes con clientes esperando.

use crate::cantidad::Cantidad;
use crate::dinero::Dinero;
use crate::error::ErrorDominio;
use crate::porcentaje::Porcentaje;
use crate::unidad::UnidadBase;

/// Identificador de una presentación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdPresentacion(pub i64);

/// Forma comercial de venta de un producto, con su factor y su precio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presentacion {
    id: Option<IdPresentacion>,
    nombre: String,
    /// Unidades base que equivalen a una presentación. Un six-pack es 6.
    factor: Cantidad,
    precio: Dinero,
    es_predeterminada: bool,
    codigo_barras: Option<String>,
    activa: bool,
}

impl Presentacion {
    /// Crea una presentación validando sus invariantes.
    ///
    /// El factor debe ser mayor que cero, y si el producto se cuenta por
    /// unidades tiene que ser además un número entero: no existe el paquete
    /// de dos latas y media (RF-PRS-14).
    pub fn nueva(
        nombre: impl Into<String>,
        factor: Cantidad,
        precio: Dinero,
        unidad_base: UnidadBase,
    ) -> Result<Self, ErrorDominio> {
        let nombre = nombre.into();
        if nombre.trim().is_empty() {
            return Err(ErrorDominio::TextoObligatorio("nombre de la presentación"));
        }

        if !factor.es_positiva() {
            return Err(ErrorDominio::FactorInvalido);
        }

        if !unidad_base.admite_fracciones() && !factor.es_entera() {
            return Err(ErrorDominio::FactorInvalido);
        }

        if precio.es_negativo() {
            return Err(ErrorDominio::DineroNegativo);
        }

        Ok(Self {
            id: None,
            nombre,
            factor,
            precio,
            es_predeterminada: false,
            codigo_barras: None,
            activa: true,
        })
    }

    /// Presentación unitaria, con factor 1.
    ///
    /// Todo producto nace con una (RF-PRS-03).
    pub fn unitaria(precio: Dinero, unidad_base: UnidadBase) -> Result<Self, ErrorDominio> {
        let mut presentacion = Self::nueva(
            unidad_base.nombre_presentacion_unitaria(),
            Cantidad::desde_unidades(1)?,
            precio,
            unidad_base,
        )?;
        presentacion.es_predeterminada = true;
        Ok(presentacion)
    }

    pub fn con_id(mut self, id: IdPresentacion) -> Self {
        self.id = Some(id);
        self
    }

    pub fn con_codigo_barras(mut self, codigo: impl Into<String>) -> Self {
        let codigo = codigo.into();
        self.codigo_barras = if codigo.trim().is_empty() {
            None
        } else {
            Some(codigo)
        };
        self
    }

    pub fn marcar_predeterminada(mut self, valor: bool) -> Self {
        self.es_predeterminada = valor;
        self
    }

    pub const fn id(&self) -> Option<IdPresentacion> {
        self.id
    }

    pub fn nombre(&self) -> &str {
        &self.nombre
    }

    pub const fn factor(&self) -> Cantidad {
        self.factor
    }

    pub const fn precio(&self) -> Dinero {
        self.precio
    }

    pub const fn es_predeterminada(&self) -> bool {
        self.es_predeterminada
    }

    pub const fn esta_activa(&self) -> bool {
        self.activa
    }

    pub fn codigo_barras(&self) -> Option<&str> {
        self.codigo_barras.as_deref()
    }

    /// Desactiva la presentación conservando el historial que la usó
    /// (RF-PRS-15).
    pub fn desactivar(&mut self) {
        self.activa = false;
    }

    /// Cambia el precio de venta.
    pub fn cambiar_precio(&mut self, precio: Dinero) -> Result<(), ErrorDominio> {
        if precio.es_negativo() {
            return Err(ErrorDominio::DineroNegativo);
        }
        self.precio = precio;
        Ok(())
    }

    /// Convierte una cantidad de presentaciones a unidades base.
    ///
    /// Vender dos six-packs descuenta doce unidades (RF-PRS-06).
    pub fn a_unidades_base(&self, presentaciones: Cantidad) -> Result<Cantidad, ErrorDominio> {
        presentaciones.multiplicar_por_factor(self.factor)
    }

    /// Importe de vender `presentaciones` unidades de esta presentación.
    pub fn importe(&self, presentaciones: Cantidad) -> Result<Dinero, ErrorDominio> {
        self.precio.multiplicar_por(presentaciones)
    }

    /// Costo de una presentación, dado el costo de la unidad base.
    ///
    /// Es siempre un valor derivado, nunca se almacena (RF-COS-14).
    pub fn costo(&self, costo_unitario_base: Dinero) -> Result<Dinero, ErrorDominio> {
        costo_unitario_base.multiplicar_por(self.factor)
    }

    /// Ganancia bruta que deja esta presentación.
    pub fn ganancia(&self, costo_unitario_base: Dinero) -> Result<Dinero, ErrorDominio> {
        self.precio.restar(self.costo(costo_unitario_base)?)
    }

    /// Margen bruto de esta presentación, como porcentaje del precio
    /// (RF-PRS-11).
    pub fn margen(&self, costo_unitario_base: Dinero) -> Result<Porcentaje, ErrorDominio> {
        Porcentaje::de_razon(self.ganancia(costo_unitario_base)?, self.precio)
    }

    /// Precio equivalente por unidad base.
    ///
    /// Permite comparar presentaciones entre sí: el six-pack a 300 sale a 50
    /// por refresco frente a los 80 de la unidad suelta (RF-PRS-10).
    pub fn precio_por_unidad_base(&self) -> Result<Dinero, ErrorDominio> {
        self.precio.dividir_entre(self.factor)
    }
}
