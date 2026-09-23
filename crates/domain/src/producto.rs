//! El producto y sus presentaciones.
//!
//! Un producto tiene una sola unidad base, en la que se cuentan su existencia
//! y su costo, y una o más presentaciones con las que se vende (R-9). La
//! unidad base no cambia una vez que hay movimientos: cambiarla convertiría
//! todo el historial en cifras sin sentido.

use crate::cantidad::Cantidad;
use crate::dinero::Dinero;
use crate::error::ErrorDominio;
use crate::presentacion::{IdPresentacion, Presentacion};
use crate::unidad::UnidadBase;

/// Identificador de un producto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdProducto(pub i64);

/// Artículo comercializable del catálogo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Producto {
    id: Option<IdProducto>,
    sku: String,
    nombre: String,
    unidad_base: UnidadBase,
    stock_minimo: Cantidad,
    /// Cantidad que se quiere mantener exhibida en vitrina (RF-VIT-03).
    objetivo_vitrina: Cantidad,
    presentaciones: Vec<Presentacion>,
    activo: bool,
}

impl Producto {
    /// Crea un producto con su presentación unitaria.
    ///
    /// Todo producto nace con al menos una presentación (RF-PRS-03); dejarlo
    /// sin ninguna lo haría invendible.
    pub fn nuevo(
        sku: impl Into<String>,
        nombre: impl Into<String>,
        unidad_base: UnidadBase,
        precio_unitario: Dinero,
    ) -> Result<Self, ErrorDominio> {
        let sku = sku.into();
        if sku.trim().is_empty() {
            return Err(ErrorDominio::TextoObligatorio("SKU"));
        }

        let nombre = nombre.into();
        if nombre.trim().is_empty() {
            return Err(ErrorDominio::TextoObligatorio("nombre del producto"));
        }

        let unitaria = Presentacion::unitaria(precio_unitario, unidad_base)?;

        Ok(Self {
            id: None,
            sku,
            nombre,
            unidad_base,
            stock_minimo: Cantidad::CERO,
            objetivo_vitrina: Cantidad::CERO,
            presentaciones: vec![unitaria],
            activo: true,
        })
    }

    /// Reconstruye un producto tal como está guardado.
    ///
    /// No valida ni crea presentación unitaria: recibe exactamente lo que hay
    /// en la base de datos. Aplicar aquí las reglas de creación impediría
    /// leer productos escritos por una versión anterior.
    #[allow(clippy::too_many_arguments)]
    pub fn reconstituir(
        id: IdProducto,
        sku: String,
        nombre: String,
        unidad_base: UnidadBase,
        stock_minimo: Cantidad,
        objetivo_vitrina: Cantidad,
        presentaciones: Vec<Presentacion>,
        activo: bool,
    ) -> Self {
        Self {
            id: Some(id),
            sku,
            nombre,
            unidad_base,
            stock_minimo,
            objetivo_vitrina,
            presentaciones,
            activo,
        }
    }

    pub fn con_id(mut self, id: IdProducto) -> Self {
        self.id = Some(id);
        self
    }

    pub fn con_stock_minimo(mut self, minimo: Cantidad) -> Result<Self, ErrorDominio> {
        if minimo.es_negativa() {
            return Err(ErrorDominio::CantidadNegativa);
        }
        self.stock_minimo = minimo;
        Ok(self)
    }

    pub fn con_objetivo_vitrina(mut self, objetivo: Cantidad) -> Result<Self, ErrorDominio> {
        if objetivo.es_negativa() {
            return Err(ErrorDominio::CantidadNegativa);
        }
        self.objetivo_vitrina = objetivo;
        Ok(self)
    }

    pub const fn id(&self) -> Option<IdProducto> {
        self.id
    }

    pub fn sku(&self) -> &str {
        &self.sku
    }

    pub fn nombre(&self) -> &str {
        &self.nombre
    }

    pub const fn unidad_base(&self) -> UnidadBase {
        self.unidad_base
    }

    /// Indica si el producto se puede vender en fracciones.
    ///
    /// No es un dato aparte: se deduce de la unidad. Lo que se cuenta por
    /// unidades se vende entero y lo que se pesa o se mide se puede
    /// fraccionar (RF-CAT-03). Guardarlo como campo independiente permitía
    /// estados que no existen en el mostrador, como «se cuenta en libras
    /// pero no se puede vender media libra».
    pub const fn es_granel(&self) -> bool {
        self.unidad_base.admite_fracciones()
    }

    pub const fn stock_minimo(&self) -> Cantidad {
        self.stock_minimo
    }

    pub const fn objetivo_vitrina(&self) -> Cantidad {
        self.objetivo_vitrina
    }

    pub const fn esta_activo(&self) -> bool {
        self.activo
    }

    pub fn presentaciones(&self) -> &[Presentacion] {
        &self.presentaciones
    }

    /// Presentaciones disponibles para vender hoy.
    pub fn presentaciones_activas(&self) -> impl Iterator<Item = &Presentacion> {
        self.presentaciones.iter().filter(|p| p.esta_activa())
    }

    /// Presentación que la venta rápida usa sin pedir selección (RF-PRS-08).
    pub fn presentacion_predeterminada(&self) -> Option<&Presentacion> {
        self.presentaciones_activas()
            .find(|p| p.es_predeterminada())
            .or_else(|| self.presentaciones_activas().next())
    }

    pub fn presentacion(&self, id: IdPresentacion) -> Option<&Presentacion> {
        self.presentaciones.iter().find(|p| p.id() == Some(id))
    }

    /// Agrega una presentación validándola contra la unidad base.
    pub fn agregar_presentacion(
        &mut self,
        presentacion: Presentacion,
    ) -> Result<(), ErrorDominio> {
        if !self.unidad_base.admite_fracciones() && !presentacion.factor().es_entera() {
            return Err(ErrorDominio::FactorInvalido);
        }

        if self
            .presentaciones
            .iter()
            .any(|p| p.nombre() == presentacion.nombre())
        {
            return Err(ErrorDominio::PresentacionDuplicada);
        }

        self.presentaciones.push(presentacion);
        Ok(())
    }

    /// Desactiva el producto en lugar de borrarlo (RF-CAT-06).
    ///
    /// Su historial de movimientos y ventas debe seguir siendo consultable.
    pub fn desactivar(&mut self) {
        self.activo = false;
    }

    /// Valida que una cantidad sea admisible para este producto.
    ///
    /// Un producto que no es a granel no acepta fracciones: 1,5 latas no
    /// existe (RF-CAT-03).
    pub fn validar_cantidad(&self, cantidad: Cantidad) -> Result<(), ErrorDominio> {
        if !cantidad.es_positiva() {
            return Err(ErrorDominio::CantidadNoPositiva);
        }

        if !self.es_granel() && !cantidad.es_entera() {
            return Err(ErrorDominio::CantidadFraccionariaNoPermitida);
        }

        Ok(())
    }

    /// Indica si la existencia está por debajo del mínimo configurado
    /// (RF-EST-06).
    pub fn esta_bajo_minimo(&self, existencia_total: Cantidad) -> bool {
        self.stock_minimo.es_positiva() && existencia_total < self.stock_minimo
    }

    /// Detecta una presentación mayor que sale más cara por unidad que una
    /// menor.
    ///
    /// Es una anomalía comercial —el paquete debería convenir— y suele ser el
    /// síntoma visible de un factor mal capturado (RF-PRS-12, riesgo RI-2).
    pub fn presentaciones_con_precio_anomalo(
        &self,
    ) -> Result<Vec<&Presentacion>, ErrorDominio> {
        let mut ordenadas: Vec<&Presentacion> = self.presentaciones_activas().collect();
        ordenadas.sort_by_key(|p| p.factor());

        let mut anomalas = Vec::new();
        let mut menor_equivalente: Option<Dinero> = None;

        for presentacion in ordenadas {
            let equivalente = presentacion.precio_por_unidad_base()?;
            if let Some(referencia) = menor_equivalente {
                if equivalente > referencia {
                    anomalas.push(presentacion);
                }
            }
            menor_equivalente = Some(match menor_equivalente {
                Some(actual) if actual < equivalente => actual,
                _ => equivalente,
            });
        }

        Ok(anomalas)
    }
}
