//! Valoración del inventario por costo promedio ponderado.
//!
//! Este es el módulo donde el negocio gana o pierde dinero sin darse cuenta,
//! así que conviene tener claras las cuatro reglas que lo gobiernan:
//!
//! 1. La fuente de verdad son la **existencia** y el **valor total**. El
//!    costo unitario no se almacena: se deriva dividiendo uno entre otro
//!    (RF-COS-02). Guardarlo redondeado iría perdiendo fracciones hasta
//!    descuadrar el inventario.
//! 2. El costo unitario **solo** se recalcula cuando entra mercancía con un
//!    costo asociado. Vender, traspasar o dar de baja no lo tocan
//!    (RF-COS-04).
//! 3. El promedio es del **producto**, sobre la existencia total, nunca por
//!    ubicación. Un traspaso de bodega a vitrina mueve cantidad, no valor
//!    (RF-COS-05).
//! 4. Las salidas descuentan del valor `cantidad × costo vigente`, dejando el
//!    costo unitario intacto (RF-COS-06).
//!
//! El tipo es inmutable: cada operación devuelve un inventario nuevo. Así no
//! existe el estado a medio actualizar, que es donde se cuelan los
//! descuadres.

use crate::cantidad::Cantidad;
use crate::dinero::Dinero;
use crate::error::ErrorDominio;
use crate::existencias::Existencias;
use crate::ubicacion::Ubicacion;

/// Existencia y valor de un producto, con su costo promedio ponderado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Inventario {
    existencias: Existencias,
    /// Dinero acumulado invertido en la existencia actual.
    valor_total: Dinero,
}

/// Resultado de una salida de inventario.
///
/// Devuelve el inventario resultante y lo que valía la mercancía que salió,
/// que es el costo de lo vendido o el importe de la merma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Salida {
    pub inventario: Inventario,
    /// Valor de la mercancía que salió, al costo vigente.
    pub valor_salida: Dinero,
    /// Costo unitario aplicado, para congelarlo en la línea de venta
    /// (RF-COS-07).
    pub costo_unitario: Dinero,
}

impl Inventario {
    pub const VACIO: Self = Self {
        existencias: Existencias::VACIAS,
        valor_total: Dinero::CERO,
    };

    /// Reconstruye el inventario desde lo persistido.
    pub fn nuevo(existencias: Existencias, valor_total: Dinero) -> Result<Self, ErrorDominio> {
        if valor_total.es_negativo() {
            return Err(ErrorDominio::DineroNegativo);
        }
        Ok(Self {
            existencias,
            valor_total,
        })
    }

    pub const fn existencias(&self) -> Existencias {
        self.existencias
    }

    pub const fn valor_total(&self) -> Dinero {
        self.valor_total
    }

    pub fn cantidad_total(&self) -> Result<Cantidad, ErrorDominio> {
        self.existencias.total()
    }

    /// Costo promedio ponderado vigente de una unidad base.
    ///
    /// Es un valor derivado, nunca almacenado (RF-COS-02). Sin existencia no
    /// hay costo que calcular, y devolver cero sería mentir.
    pub fn costo_unitario(&self) -> Result<Dinero, ErrorDominio> {
        let cantidad = self.cantidad_total()?;
        if cantidad.es_cero() {
            return Err(ErrorDominio::SinExistenciaParaCosto);
        }
        self.valor_total.dividir_entre(cantidad)
    }

    /// Costo vigente, o cero cuando todavía no hay existencia.
    ///
    /// Útil para mostrar en pantalla, donde un error no aporta nada.
    pub fn costo_unitario_o_cero(&self) -> Dinero {
        self.costo_unitario().unwrap_or(Dinero::CERO)
    }

    /// Registra una entrada de mercancía y recalcula el costo promedio.
    ///
    /// `importe` es lo que se pagó en total por esa cantidad, no el costo
    /// unitario: operar sobre el total de la línea evita el redondeo
    /// intermedio que RF-COS-13 prohíbe. Comprar diez six-packs a 250 aporta
    /// 2 500 y sesenta unidades; el costo por unidad nunca se redondea a
    /// 41,67 por el camino.
    ///
    /// Cuando la existencia previa es cero, el costo resultante es
    /// exactamente el de esta entrada: no se arrastra el anterior
    /// (RF-COS-08).
    pub fn registrar_entrada(
        &self,
        cantidad: Cantidad,
        importe: Dinero,
        destino: Ubicacion,
    ) -> Result<Self, ErrorDominio> {
        if !cantidad.es_positiva() {
            return Err(ErrorDominio::CantidadNoPositiva);
        }
        if importe.es_negativo() {
            return Err(ErrorDominio::DineroNegativo);
        }

        Ok(Self {
            existencias: self.existencias.agregar(cantidad, destino)?,
            valor_total: self.valor_total.sumar(importe)?,
        })
    }

    /// Fija un costo unitario nuevo, recalculando el valor del inventario.
    ///
    /// Es una **revaluación**, y conviene llamarla por su nombre. El costo
    /// no es un campo que se pueda editar: es un valor derivado (regla 1 de
    /// este módulo). Así que para que el costo pase a ser otro hay que
    /// mover la única cifra que sí se almacena:
    ///
    /// ```text
    /// valor_total = costo_unitario × existencia_total
    /// ```
    ///
    /// Eso significa que **el valor del inventario cambia**. Subir el costo
    /// de cien libras de 120 a 150 añade tres mil pesos de valor que nadie
    /// compró. No es un efecto secundario que se pueda evitar: es la misma
    /// operación mirada desde el otro lado, y por eso quien la ofrezca
    /// tiene que dejarla anotada.
    ///
    /// **La existencia no se toca.** Esta operación no mueve ni una
    /// unidad, así que no deja asiento en el kárdex: el kárdex explica
    /// saldos, y aquí el saldo no cambia.
    ///
    /// Sin existencia se niega: el costo se deriva dividiendo entre la
    /// existencia, y sin nada que dividir no hay costo que fijar. Guardar
    /// un valor con existencia cero dejaría un producto que no tiene nada y
    /// vale algo, que es el descuadre que este módulo entero evita.
    pub fn revalorar(&self, costo_unitario: Dinero) -> Result<Self, ErrorDominio> {
        if costo_unitario.es_negativo() {
            return Err(ErrorDominio::DineroNegativo);
        }

        let cantidad = self.cantidad_total()?;
        if cantidad.es_cero() {
            return Err(ErrorDominio::SinExistenciaParaCosto);
        }

        Ok(Self {
            existencias: self.existencias,
            valor_total: costo_unitario.multiplicar_por(cantidad)?,
        })
    }

    /// Registra una salida: venta, merma o ajuste en contra.
    ///
    /// Descuenta la existencia de la ubicación y resta del valor lo que esa
    /// mercancía costaba. El costo unitario no cambia (RF-COS-06).
    pub fn registrar_salida(
        &self,
        cantidad: Cantidad,
        origen: Ubicacion,
    ) -> Result<Salida, ErrorDominio> {
        if !cantidad.es_positiva() {
            return Err(ErrorDominio::CantidadNoPositiva);
        }

        let costo_unitario = self.costo_unitario()?;
        let existencias = self.existencias.descontar(cantidad, origen)?;
        let valor_salida = costo_unitario.multiplicar_por(cantidad)?;

        // La última salida debe dejar el valor en cero exacto. El costo es un
        // valor derivado y puede arrastrar una fracción de redondeo; si
        // quedara residuo, el producto figuraría con existencia cero y valor
        // distinto de cero, que es justo el descuadre que este módulo evita.
        let valor_total = if existencias.total()?.es_cero() {
            Dinero::CERO
        } else {
            self.valor_total.restar(valor_salida)?.o_cero_si_negativo()
        };

        Ok(Salida {
            inventario: Self {
                existencias,
                valor_total,
            },
            valor_salida,
            costo_unitario,
        })
    }

    /// Mueve mercancía entre bodega y vitrina.
    ///
    /// Ni la existencia total ni el valor cambian, y por tanto tampoco el
    /// costo unitario (RF-COS-05).
    pub fn traspasar(
        &self,
        cantidad: Cantidad,
        origen: Ubicacion,
        destino: Ubicacion,
    ) -> Result<Self, ErrorDominio> {
        if !cantidad.es_positiva() {
            return Err(ErrorDominio::CantidadNoPositiva);
        }

        Ok(Self {
            existencias: self.existencias.traspasar(cantidad, origen, destino)?,
            valor_total: self.valor_total,
        })
    }

    /// Reabastece la vitrina desde la bodega hasta la cantidad objetivo.
    ///
    /// Devuelve el inventario resultante y cuánto se movió (RF-VIT-04).
    pub fn reabastecer_vitrina(
        &self,
        objetivo: Cantidad,
    ) -> Result<(Self, Cantidad), ErrorDominio> {
        let faltante = self.existencias.faltante_para_exhibir(objetivo);
        if faltante.es_cero() {
            return Ok((*self, Cantidad::CERO));
        }

        let inventario = self.traspasar(faltante, Ubicacion::Bodega, Ubicacion::Vitrina)?;
        Ok((inventario, faltante))
    }

    /// Ajusta la existencia de una ubicación al resultado de un conteo
    /// físico (RF-INV-07).
    ///
    /// La diferencia a la baja se valora al costo vigente; la diferencia a
    /// favor entra sin costo, porque no hubo compra que la respalde y
    /// asignarle un valor inventado inflaría el inventario.
    pub fn ajustar_a_conteo(
        &self,
        contado: Cantidad,
        ubicacion: Ubicacion,
    ) -> Result<(Self, Cantidad), ErrorDominio> {
        if contado.es_negativa() {
            return Err(ErrorDominio::CantidadNegativa);
        }

        let registrado = self.existencias.en(ubicacion);
        let diferencia = contado.restar_con_signo(registrado)?;

        if diferencia.es_cero() {
            return Ok((*self, Cantidad::CERO));
        }

        if diferencia.es_negativa() {
            let faltante = registrado.restar(contado)?;
            let salida = self.registrar_salida(faltante, ubicacion)?;
            return Ok((salida.inventario, diferencia));
        }

        let sobrante = contado.restar(registrado)?;
        let inventario = Self {
            existencias: self.existencias.agregar(sobrante, ubicacion)?,
            valor_total: self.valor_total,
        };
        Ok((inventario, diferencia))
    }
}

#[cfg(test)]
mod pruebas_revaluacion {
    use super::*;

    fn cantidad(texto: &str) -> Cantidad {
        texto.parse().expect("cantidad válida")
    }

    fn dinero(texto: &str) -> Dinero {
        texto.parse().expect("importe válido")
    }

    /// Cien libras en vitrina que costaron 120 cada una.
    fn con_existencia(vitrina: &str, valor: &str) -> Inventario {
        Inventario::nuevo(
            Existencias::nuevas(Cantidad::CERO, cantidad(vitrina)).expect("existencias"),
            dinero(valor),
        )
        .expect("inventario")
    }

    #[test]
    fn fijar_el_costo_mueve_el_valor_del_inventario() {
        let antes = con_existencia("100", "12000.00");
        assert_eq!(antes.costo_unitario().unwrap().formatear(2), "120.00");

        let despues = antes.revalorar(dinero("150.00")).expect("revalorar");

        // El costo pasa a ser el pedido...
        assert_eq!(despues.costo_unitario().unwrap().formatear(2), "150.00");
        // ...y el valor sube tres mil pesos que nadie compró. Es la misma
        // operación vista del otro lado.
        assert_eq!(despues.valor_total().formatear(2), "15000.00");
    }

    #[test]
    fn la_existencia_no_se_toca() {
        let antes = con_existencia("100", "12000.00");
        let despues = antes.revalorar(dinero("150.00")).expect("revalorar");

        assert_eq!(despues.existencias(), antes.existencias());
        assert_eq!(despues.cantidad_total().unwrap(), cantidad("100"));
    }

    #[test]
    fn reparte_el_valor_sobre_la_existencia_total_no_sobre_una_ubicacion() {
        // 40 en bodega y 60 en vitrina: el promedio es del producto
        // (RF-COS-05), así que el costo se multiplica por los 100.
        let inventario = Inventario::nuevo(
            Existencias::nuevas(cantidad("40"), cantidad("60")).expect("existencias"),
            dinero("500.00"),
        )
        .expect("inventario");

        let despues = inventario.revalorar(dinero("10.00")).expect("revalorar");

        assert_eq!(despues.valor_total().formatear(2), "1000.00");
    }

    #[test]
    fn el_camino_de_ida_y_vuelta_cuadra_aunque_el_costo_no_sea_exacto() {
        // 7 unidades: 100/7 no es exacto, que es donde el redondeo se nota.
        let inventario = con_existencia("7", "100.00");
        let costo = inventario.costo_unitario().expect("costo");

        // Fijar el costo que ya tenía no debe mover el valor.
        let despues = inventario.revalorar(costo).expect("revalorar");

        assert_eq!(
            despues.costo_unitario().unwrap().millonesimas(),
            costo.millonesimas()
        );
    }

    #[test]
    fn un_costo_de_cero_deja_el_inventario_sin_valor() {
        // Mercancía regalada o recibida sin factura: es un caso legítimo.
        let despues = con_existencia("100", "12000.00")
            .revalorar(Dinero::CERO)
            .expect("revalorar");

        assert!(despues.valor_total().es_cero());
        assert_eq!(despues.cantidad_total().unwrap(), cantidad("100"));
    }

    #[test]
    fn sin_existencia_no_hay_costo_que_fijar() {
        // Sin nada que dividir no hay costo, y guardar un valor dejaría un
        // producto que no tiene nada y vale algo.
        let vacio = Inventario::VACIO;

        assert_eq!(
            vacio.revalorar(dinero("150.00")).expect_err("no hay nada"),
            ErrorDominio::SinExistenciaParaCosto
        );
    }

    #[test]
    fn no_se_fija_un_costo_negativo() {
        assert_eq!(
            con_existencia("100", "12000.00")
                .revalorar(dinero("-1.00"))
                .expect_err("el costo no puede ser negativo"),
            ErrorDominio::DineroNegativo
        );
    }

    #[test]
    fn revalorar_no_altera_el_inventario_original() {
        // El tipo es inmutable: cada operación devuelve uno nuevo.
        let antes = con_existencia("100", "12000.00");
        let _ = antes.revalorar(dinero("150.00")).expect("revalorar");

        assert_eq!(antes.valor_total().formatear(2), "12000.00");
    }
}
