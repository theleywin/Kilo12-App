//! Existencia de un producto repartida entre bodega y vitrina.
//!
//! La distinción es el corazón del inventario de Kilo12. Un sistema que
//! guarda «cuarenta unidades» y nada más le dice al dueño que tiene producto
//! mientras el cliente se va porque el estante está vacío.
//!
//! Las cantidades se expresan siempre en la unidad base del producto (R-9).

use crate::cantidad::Cantidad;
use crate::error::ErrorDominio;
use crate::ubicacion::Ubicacion;

/// Existencia de un producto por ubicación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Existencias {
    bodega: Cantidad,
    vitrina: Cantidad,
}

impl Existencias {
    pub const VACIAS: Self = Self {
        bodega: Cantidad::CERO,
        vitrina: Cantidad::CERO,
    };

    /// Construye las existencias a partir de sus dos ubicaciones.
    ///
    /// Rechaza cualquier cantidad negativa: ninguna ubicación puede quedar en
    /// rojo (RF-INV-06).
    pub fn nuevas(bodega: Cantidad, vitrina: Cantidad) -> Result<Self, ErrorDominio> {
        if bodega.es_negativa() || vitrina.es_negativa() {
            return Err(ErrorDominio::CantidadNegativa);
        }
        Ok(Self { bodega, vitrina })
    }

    pub const fn bodega(&self) -> Cantidad {
        self.bodega
    }

    pub const fn vitrina(&self) -> Cantidad {
        self.vitrina
    }

    /// Cantidad en una ubicación concreta.
    pub const fn en(&self, ubicacion: Ubicacion) -> Cantidad {
        match ubicacion {
            Ubicacion::Bodega => self.bodega,
            Ubicacion::Vitrina => self.vitrina,
        }
    }

    /// Existencia total: bodega más vitrina.
    ///
    /// No se almacena, se deriva. Duplicarla sería abrir la puerta a que las
    /// dos versiones dejen de coincidir.
    pub fn total(&self) -> Result<Cantidad, ErrorDominio> {
        self.bodega.sumar(self.vitrina)
    }

    pub fn esta_agotado(&self) -> Result<bool, ErrorDominio> {
        Ok(self.total()?.es_cero())
    }

    /// Indica si hay mercancía guardada pero nada expuesto al cliente.
    ///
    /// Es venta perdida, y el sistema debe hacerla visible (RF-VIT-06).
    pub const fn hay_en_bodega_sin_exhibir(&self) -> bool {
        self.vitrina.es_cero() && self.bodega.es_positiva()
    }

    /// Suma cantidad en una ubicación.
    pub fn agregar(
        &self,
        cantidad: Cantidad,
        ubicacion: Ubicacion,
    ) -> Result<Self, ErrorDominio> {
        if cantidad.es_negativa() {
            return Err(ErrorDominio::CantidadNegativa);
        }

        let mut resultado = *self;
        match ubicacion {
            Ubicacion::Bodega => resultado.bodega = self.bodega.sumar(cantidad)?,
            Ubicacion::Vitrina => resultado.vitrina = self.vitrina.sumar(cantidad)?,
        }
        Ok(resultado)
    }

    /// Descuenta cantidad de una ubicación.
    ///
    /// Falla con el detalle de cuánto hay y cuánto se pidió, para que la
    /// interfaz pueda ofrecer un traspaso desde bodega sin abandonar la venta
    /// (RF-VTA-12).
    pub fn descontar(
        &self,
        cantidad: Cantidad,
        ubicacion: Ubicacion,
    ) -> Result<Self, ErrorDominio> {
        if cantidad.es_negativa() {
            return Err(ErrorDominio::CantidadNegativa);
        }

        let disponible = self.en(ubicacion);
        if disponible < cantidad {
            return Err(ErrorDominio::ExistenciaInsuficiente {
                disponible: disponible.milesimas(),
                solicitado: cantidad.milesimas(),
            });
        }

        let mut resultado = *self;
        match ubicacion {
            Ubicacion::Bodega => resultado.bodega = self.bodega.restar(cantidad)?,
            Ubicacion::Vitrina => resultado.vitrina = self.vitrina.restar(cantidad)?,
        }
        Ok(resultado)
    }

    /// Mueve cantidad de una ubicación a la otra.
    ///
    /// La existencia total no cambia: el traspaso mueve kilos, no valor
    /// (RF-VIT-01, RF-COS-05).
    pub fn traspasar(
        &self,
        cantidad: Cantidad,
        origen: Ubicacion,
        destino: Ubicacion,
    ) -> Result<Self, ErrorDominio> {
        if origen == destino {
            return Ok(*self);
        }
        self.descontar(cantidad, origen)?.agregar(cantidad, destino)
    }

    /// Cuánto falta en vitrina para alcanzar la cantidad objetivo, limitado
    /// por lo que haya en bodega.
    ///
    /// Es la sugerencia de reabastecimiento (RF-VIT-04).
    pub fn faltante_para_exhibir(&self, objetivo: Cantidad) -> Cantidad {
        if self.vitrina >= objetivo {
            return Cantidad::CERO;
        }

        let faltante = objetivo
            .restar(self.vitrina)
            .unwrap_or(Cantidad::CERO);

        if faltante > self.bodega {
            self.bodega
        } else {
            faltante
        }
    }
}
