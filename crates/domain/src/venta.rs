//! La venta y sus líneas.
//!
//! Una línea de venta **congela** todo lo que hacía falta para entenderla:
//! el nombre del producto, el de la presentación, su factor, su precio y el
//! costo vigente en ese momento (RF-VTA-13). No guarda referencias vivas a
//! nada de eso.
//!
//! El motivo está escrito en el documento: *si hoy vendo un producto con
//! costo 10 y mañana el costo sube a 14, el informe de ganancia de la venta
//! de hoy sigue calculándose con 10*. Una línea que apunte al producto y
//! pregunte su costo al leer el informe mentiría sobre el pasado.

use crate::cantidad::Cantidad;
use crate::dinero::Dinero;
use crate::error::ErrorDominio;
use crate::presentacion::IdPresentacion;
use crate::producto::IdProducto;

/// Identificador de una venta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdVenta(pub i64);

/// Un renglón de la venta, con todo lo suyo congelado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineaVenta {
    producto: IdProducto,
    presentacion: IdPresentacion,
    /// Copias del momento: si el producto se renombra, la venta vieja sigue
    /// diciendo lo que se vendió entonces.
    nombre_producto: String,
    nombre_presentacion: String,
    /// Cuántas presentaciones se llevaron: 2 six-packs son 2, no 12.
    cantidad: Cantidad,
    /// Unidades base que lleva cada presentación (RF-PRS-06).
    factor: Cantidad,
    /// Precio de la presentación en el momento de vender.
    precio: Dinero,
    /// Costo promedio ponderado de la unidad base en ese momento.
    costo_unitario_base: Dinero,
}

impl LineaVenta {
    #[allow(clippy::too_many_arguments)]
    pub fn nueva(
        producto: IdProducto,
        presentacion: IdPresentacion,
        nombre_producto: impl Into<String>,
        nombre_presentacion: impl Into<String>,
        cantidad: Cantidad,
        factor: Cantidad,
        precio: Dinero,
        costo_unitario_base: Dinero,
    ) -> Result<Self, ErrorDominio> {
        if !cantidad.es_positiva() {
            return Err(ErrorDominio::CantidadNoPositiva);
        }
        if !factor.es_positiva() {
            return Err(ErrorDominio::FactorInvalido);
        }
        if precio.es_negativo() || costo_unitario_base.es_negativo() {
            return Err(ErrorDominio::DineroNegativo);
        }

        Ok(Self {
            producto,
            presentacion,
            nombre_producto: nombre_producto.into(),
            nombre_presentacion: nombre_presentacion.into(),
            cantidad,
            factor,
            precio,
            costo_unitario_base,
        })
    }

    pub const fn producto(&self) -> IdProducto {
        self.producto
    }

    pub const fn presentacion(&self) -> IdPresentacion {
        self.presentacion
    }

    pub fn nombre_producto(&self) -> &str {
        &self.nombre_producto
    }

    pub fn nombre_presentacion(&self) -> &str {
        &self.nombre_presentacion
    }

    pub const fn cantidad(&self) -> Cantidad {
        self.cantidad
    }

    pub const fn factor(&self) -> Cantidad {
        self.factor
    }

    pub const fn precio(&self) -> Dinero {
        self.precio
    }

    pub const fn costo_unitario_base(&self) -> Dinero {
        self.costo_unitario_base
    }

    /// Unidades base que salen de la vitrina por esta línea (RF-VTA-11).
    ///
    /// Vender dos six-packs descuenta doce refrescos: la existencia es una
    /// sola y está en unidad base.
    pub fn unidades_base(&self) -> Result<Cantidad, ErrorDominio> {
        self.cantidad.multiplicar_por_factor(self.factor)
    }

    /// Lo que paga el cliente por esta línea.
    pub fn importe(&self) -> Result<Dinero, ErrorDominio> {
        self.precio.multiplicar_por(self.cantidad)
    }

    /// Lo que le costó al negocio la mercancía de esta línea.
    pub fn costo(&self) -> Result<Dinero, ErrorDominio> {
        self.costo_unitario_base
            .multiplicar_por(self.unidades_base()?)
    }

    /// Ganancia bruta de la línea. Puede ser negativa: eso es vender
    /// perdiendo, y el informe tiene que poder decirlo.
    pub fn ganancia(&self) -> Result<Dinero, ErrorDominio> {
        self.importe()?.restar(self.costo()?)
    }
}

/// Una venta con sus líneas.
///
/// Se construye mientras el cliente espera y se confirma entera o no se
/// confirma: media venta no existe.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Venta {
    lineas: Vec<LineaVenta>,
}

impl Venta {
    pub const fn nueva() -> Self {
        Self { lineas: Vec::new() }
    }

    pub fn con_lineas(lineas: Vec<LineaVenta>) -> Result<Self, ErrorDominio> {
        if lineas.is_empty() {
            return Err(ErrorDominio::VentaVacia);
        }
        Ok(Self { lineas })
    }

    pub fn agregar(&mut self, linea: LineaVenta) {
        self.lineas.push(linea);
    }

    pub fn lineas(&self) -> &[LineaVenta] {
        &self.lineas
    }

    pub fn esta_vacia(&self) -> bool {
        self.lineas.is_empty()
    }

    /// Lo que hay que cobrar.
    pub fn total(&self) -> Result<Dinero, ErrorDominio> {
        self.lineas
            .iter()
            .try_fold(Dinero::CERO, |suma, linea| suma.sumar(linea.importe()?))
    }

    /// Lo que costó la mercancía vendida, al costo congelado de cada línea.
    pub fn costo_total(&self) -> Result<Dinero, ErrorDominio> {
        self.lineas
            .iter()
            .try_fold(Dinero::CERO, |suma, linea| suma.sumar(linea.costo()?))
    }

    /// Ganancia bruta de la venta completa.
    pub fn ganancia(&self) -> Result<Dinero, ErrorDominio> {
        self.total()?.restar(self.costo_total()?)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cantidad(texto: &str) -> Cantidad {
        texto.parse().expect("cantidad válida")
    }

    fn dinero(texto: &str) -> Dinero {
        texto.parse().expect("importe válido")
    }

    /// Dos six-packs a 300, con la lata a 41,67 de costo.
    fn linea_six_packs() -> LineaVenta {
        LineaVenta::nueva(
            IdProducto(1),
            IdPresentacion(2),
            "Refresco 500 ml",
            "Six-pack",
            cantidad("2"),
            cantidad("6"),
            dinero("300.00"),
            dinero("41.67"),
        )
        .expect("línea válida")
    }

    #[test]
    fn la_linea_descuenta_la_unidad_base_no_la_presentacion() {
        // Dos six-packs son doce refrescos saliendo de la vitrina.
        assert_eq!(linea_six_packs().unidades_base().unwrap(), cantidad("12"));
    }

    #[test]
    fn el_importe_va_por_presentacion_y_el_costo_por_unidad_base() {
        let linea = linea_six_packs();
        assert_eq!(linea.importe().unwrap().formatear(2), "600.00");
        // 12 × 41,67 = 500,04
        assert_eq!(linea.costo().unwrap().formatear(2), "500.04");
        assert_eq!(linea.ganancia().unwrap().formatear(2), "99.96");
    }

    #[test]
    fn una_venta_suma_sus_lineas() {
        let mut venta = Venta::nueva();
        venta.agregar(linea_six_packs());
        venta.agregar(
            LineaVenta::nueva(
                IdProducto(2),
                IdPresentacion(3),
                "Arroz blanco",
                "Libra",
                cantidad("2.5"),
                cantidad("1"),
                dinero("180.00"),
                dinero("120.00"),
            )
            .expect("línea válida"),
        );

        assert_eq!(venta.total().unwrap().formatear(2), "1050.00");
        assert_eq!(venta.costo_total().unwrap().formatear(2), "800.04");
        assert_eq!(venta.ganancia().unwrap().formatear(2), "249.96");
    }

    #[test]
    fn no_existe_la_venta_sin_lineas() {
        assert_eq!(
            Venta::con_lineas(Vec::new()).expect_err("no hay nada que cobrar"),
            ErrorDominio::VentaVacia
        );
    }

    #[test]
    fn no_se_vende_una_cantidad_de_cero() {
        let error = LineaVenta::nueva(
            IdProducto(1),
            IdPresentacion(1),
            "Arroz",
            "Libra",
            Cantidad::CERO,
            cantidad("1"),
            dinero("180.00"),
            dinero("120.00"),
        )
        .expect_err("cero no es una venta");

        assert_eq!(error, ErrorDominio::CantidadNoPositiva);
    }

    /// Lo que el documento pone como criterio de aceptación de RF-VTA-13.
    #[test]
    fn la_ganancia_no_cambia_si_despues_sube_el_costo() {
        let vendida = LineaVenta::nueva(
            IdProducto(1),
            IdPresentacion(1),
            "Arroz blanco",
            "Libra",
            cantidad("1"),
            cantidad("1"),
            dinero("180.00"),
            dinero("120.00"),
        )
        .expect("línea");

        // El costo del producto sube a 140 mañana; la línea de hoy no se
        // entera porque no le pregunta a nadie: lleva su copia.
        assert_eq!(vendida.costo().unwrap().formatear(2), "120.00");
        assert_eq!(vendida.ganancia().unwrap().formatear(2), "60.00");
    }
}
