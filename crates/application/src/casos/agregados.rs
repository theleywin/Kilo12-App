//! Agregación de renglones vendidos, compartida por informes y ventas.
//!
//! Aquí es donde se multiplica precio por cantidad, y por eso esto no se
//! hace en SQL: las escalas las conocen `Dinero` y `Cantidad`, no SQLite.
//! Vive en un solo sitio para que el informe y la pantalla de ventas no
//! puedan llegar a cifras distintas sumando las mismas líneas.

use std::collections::BTreeMap;

use domain::{Cantidad, Dinero};

use crate::error::Resultado;
use crate::puertos::LineaDelPeriodo;

/// Lo que un grupo de renglones movió.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Movido {
    /// Presentaciones vendidas, tal como se cobraron. Solo tiene sentido
    /// cuando el grupo es de una sola presentación: sumar cajas y latas
    /// sueltas no da ninguna cifra que signifique algo.
    pub(crate) cantidad: Cantidad,
    /// Unidades base, para que suelto y paquete sean comparables.
    pub(crate) unidades: Cantidad,
    pub(crate) importe: Dinero,
    pub(crate) costo: Dinero,
}

impl Movido {
    /// Suma un renglón al grupo.
    fn sumar(&mut self, linea: &LineaDelPeriodo) -> Resultado<()> {
        let unidades = linea.cantidad.multiplicar_por_factor(linea.factor)?;
        let importe = linea.precio.multiplicar_por(linea.cantidad)?;
        // El costo se cuenta sobre unidades base: una caja de 24 se llevó
        // 24 veces el costo unitario, no una.
        let costo = linea.costo_unitario.multiplicar_por(unidades)?;

        self.cantidad = self.cantidad.sumar(linea.cantidad)?;
        self.unidades = self.unidades.sumar(unidades)?;
        self.importe = self.importe.sumar(importe)?;
        self.costo = self.costo.sumar(costo)?;
        Ok(())
    }

    /// Importe menos costo congelado.
    pub(crate) fn ganancia(&self) -> Resultado<Dinero> {
        Ok(self.importe.restar(self.costo)?)
    }
}

/// Junta los renglones según la clave que se le pida.
///
/// La clave la decide quien llama: el informe agrupa por producto y la
/// pantalla de ventas por producto y presentación. La aritmética es la
/// misma en los dos casos, y por eso está aquí una sola vez.
pub(crate) fn agregar_por<K: Ord>(
    lineas: &[LineaDelPeriodo],
    clave: impl Fn(&LineaDelPeriodo) -> K,
) -> Resultado<BTreeMap<K, Movido>> {
    let mut acumulado: BTreeMap<K, Movido> = BTreeMap::new();

    for linea in lineas {
        acumulado.entry(clave(linea)).or_default().sumar(linea)?;
    }

    Ok(acumulado)
}

#[cfg(test)]
mod pruebas {
    use domain::{IdPresentacion, IdProducto};

    use super::*;

    fn linea(producto: i64, cantidad: &str, factor: &str, precio: &str) -> LineaDelPeriodo {
        LineaDelPeriodo {
            venta: 1,
            producto: IdProducto(producto),
            presentacion: IdPresentacion(producto),
            nombre_producto: "Refresco".to_owned(),
            nombre_presentacion: "Unidad".to_owned(),
            cantidad: cantidad.parse().expect("cantidad"),
            factor: factor.parse().expect("factor"),
            precio: precio.parse().expect("precio"),
            costo_unitario: "10.00".parse().expect("costo"),
        }
    }

    #[test]
    fn suma_importe_costo_y_unidades_base_por_clave() {
        let lineas = [
            linea(1, "2", "1", "15.00"),
            linea(1, "1", "6", "80.00"),
            linea(2, "1", "1", "5.00"),
        ];

        let agregados = agregar_por(&lineas, |l| l.producto.0).expect("agregar");

        let uno = agregados[&1];
        assert_eq!(uno.unidades.formatear(0), "8");
        // 2 × 15 + 1 × 80
        assert_eq!(uno.importe.formatear(2), "110.00");
        // 8 unidades base × 10
        assert_eq!(uno.costo.formatear(2), "80.00");
        assert_eq!(uno.ganancia().expect("ganancia").formatear(2), "30.00");
        assert_eq!(agregados[&2].importe.formatear(2), "5.00");
    }
}
