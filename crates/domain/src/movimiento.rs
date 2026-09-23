//! El registro inmutable de todo cambio de existencia (RF-INV-03).
//!
//! La existencia actual es un saldo; el movimiento es el asiento que lo
//! explica. Sin los asientos, un día la bodega dice 47 libras, nadie sabe
//! por qué, y no hay forma de averiguarlo.
//!
//! Un movimiento **no se edita ni se borra jamás**. Un error se corrige con
//! otro movimiento que lo compense, igual que en contabilidad.
//!
//! El movimiento no guarda a qué producto pertenece: eso lo sabe quien lo
//! registra. Así puede construirse antes de que el producto tenga
//! identificador, que es justo lo que ocurre al dar de alta un producto con
//! su mercancía inicial.

use core::fmt;
use core::str::FromStr;

use crate::cantidad::Cantidad;
use crate::dinero::Dinero;
use crate::error::ErrorDominio;
use crate::ubicacion::Ubicacion;

/// Por qué cambió la existencia (RF-INV-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipoMovimiento {
    /// Compra de mercancía. Es el único que recalcula el costo (RF-COS-04).
    Entrada,
    Venta,
    /// Bodega ↔ vitrina. Mueve cantidad, nunca valor (RF-COS-05).
    Traspaso,
    /// Mercancía que se perdió: se echó a perder, se rompió, se derramó.
    Merma,
    /// Corrección tras un conteo físico.
    Ajuste,
    Devolucion,
}

impl TipoMovimiento {
    /// Texto con que se guarda en la base de datos.
    pub const fn como_texto(self) -> &'static str {
        match self {
            Self::Entrada => "ENTRADA",
            Self::Venta => "VENTA",
            Self::Traspaso => "TRASPASO",
            Self::Merma => "MERMA",
            Self::Ajuste => "AJUSTE",
            Self::Devolucion => "DEVOLUCION",
        }
    }

    /// Nombre para mostrar al usuario.
    pub const fn nombre(self) -> &'static str {
        match self {
            Self::Entrada => "Entrada",
            Self::Venta => "Venta",
            Self::Traspaso => "Traspaso",
            Self::Merma => "Merma",
            Self::Ajuste => "Ajuste",
            Self::Devolucion => "Devolución",
        }
    }

    /// Indica si el movimiento exige explicar por qué se hizo (RF-INV-08).
    ///
    /// Una merma o un ajuste sin motivo es mercancía que desaparece sin que
    /// nadie responda por ella.
    pub const fn exige_motivo(self) -> bool {
        matches!(self, Self::Merma | Self::Ajuste)
    }

    /// Indica si el movimiento suma existencia en lugar de restarla.
    pub const fn es_entrada(self) -> bool {
        matches!(self, Self::Entrada | Self::Devolucion)
    }
}

impl fmt::Display for TipoMovimiento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.nombre())
    }
}

impl FromStr for TipoMovimiento {
    type Err = ErrorDominio;

    fn from_str(texto: &str) -> Result<Self, Self::Err> {
        match texto {
            "ENTRADA" => Ok(Self::Entrada),
            "VENTA" => Ok(Self::Venta),
            "TRASPASO" => Ok(Self::Traspaso),
            "MERMA" => Ok(Self::Merma),
            "AJUSTE" => Ok(Self::Ajuste),
            "DEVOLUCION" => Ok(Self::Devolucion),
            _ => Err(ErrorDominio::TipoMovimientoDesconocido),
        }
    }
}

/// Un cambio de existencia que ya ocurrió.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Movimiento {
    tipo: TipoMovimiento,
    /// De dónde salió. Vacío en una entrada: la mercancía viene de fuera.
    origen: Option<Ubicacion>,
    /// A dónde fue. Vacío en una merma o una venta: se fue del negocio.
    destino: Option<Ubicacion>,
    cantidad: Cantidad,
    /// Costo unitario vigente en el momento, congelado aquí (RF-COS-07).
    ///
    /// Se guarda aunque sea derivable: dentro de un año el costo promedio
    /// será otro, y recalcular el pasado con el costo de hoy convertiría el
    /// historial en ficción.
    costo_unitario: Dinero,
    motivo: Option<String>,
}

impl Movimiento {
    /// Entrada de mercancía comprada (RF-COM-03).
    pub fn entrada(
        cantidad: Cantidad,
        costo_unitario: Dinero,
        destino: Ubicacion,
    ) -> Result<Self, ErrorDominio> {
        Ok(Self {
            tipo: TipoMovimiento::Entrada,
            origen: None,
            destino: Some(destino),
            cantidad: cantidad_positiva(cantidad)?,
            costo_unitario: costo_no_negativo(costo_unitario)?,
            motivo: None,
        })
    }

    /// Baja de mercancía que se perdió (RF-MER).
    ///
    /// El motivo se exige en la firma, no se comprueba después: un tipo que
    /// no se puede construir mal es mejor que uno que hay que validar.
    pub fn merma(
        cantidad: Cantidad,
        costo_unitario: Dinero,
        origen: Ubicacion,
        motivo: impl Into<String>,
    ) -> Result<Self, ErrorDominio> {
        let motivo = motivo.into();
        if motivo.trim().is_empty() {
            return Err(ErrorDominio::MotivoObligatorio);
        }

        Ok(Self {
            tipo: TipoMovimiento::Merma,
            origen: Some(origen),
            destino: None,
            cantidad: cantidad_positiva(cantidad)?,
            costo_unitario: costo_no_negativo(costo_unitario)?,
            motivo: Some(motivo),
        })
    }

    /// Traslado entre bodega y vitrina (RF-VIT-01).
    ///
    /// No lleva costo porque no se le compró nada a nadie: la existencia
    /// total no cambia y el valor tampoco (RF-COS-05). El costo unitario se
    /// guarda solo como referencia de lo que valía la mercancía movida.
    pub fn traspaso(
        cantidad: Cantidad,
        costo_unitario: Dinero,
        origen: Ubicacion,
        destino: Ubicacion,
    ) -> Result<Self, ErrorDominio> {
        if origen == destino {
            return Err(ErrorDominio::UbicacionDesconocida);
        }

        Ok(Self {
            tipo: TipoMovimiento::Traspaso,
            origen: Some(origen),
            destino: Some(destino),
            cantidad: cantidad_positiva(cantidad)?,
            costo_unitario: costo_no_negativo(costo_unitario)?,
            motivo: None,
        })
    }

    /// Reconstruye un movimiento tal como está guardado.
    pub const fn reconstituir(
        tipo: TipoMovimiento,
        origen: Option<Ubicacion>,
        destino: Option<Ubicacion>,
        cantidad: Cantidad,
        costo_unitario: Dinero,
        motivo: Option<String>,
    ) -> Self {
        Self {
            tipo,
            origen,
            destino,
            cantidad,
            costo_unitario,
            motivo,
        }
    }

    pub const fn tipo(&self) -> TipoMovimiento {
        self.tipo
    }

    pub const fn origen(&self) -> Option<Ubicacion> {
        self.origen
    }

    pub const fn destino(&self) -> Option<Ubicacion> {
        self.destino
    }

    pub const fn cantidad(&self) -> Cantidad {
        self.cantidad
    }

    pub const fn costo_unitario(&self) -> Dinero {
        self.costo_unitario
    }

    pub fn motivo(&self) -> Option<&str> {
        self.motivo.as_deref()
    }

    /// Lo que valía la mercancía que se movió.
    pub fn importe(&self) -> Result<Dinero, ErrorDominio> {
        self.costo_unitario.multiplicar_por(self.cantidad)
    }
}

fn cantidad_positiva(cantidad: Cantidad) -> Result<Cantidad, ErrorDominio> {
    if cantidad.es_positiva() {
        Ok(cantidad)
    } else {
        Err(ErrorDominio::CantidadNoPositiva)
    }
}

fn costo_no_negativo(costo: Dinero) -> Result<Dinero, ErrorDominio> {
    if costo.es_negativo() {
        Err(ErrorDominio::DineroNegativo)
    } else {
        Ok(costo)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cantidad(unidades: i64) -> Cantidad {
        Cantidad::desde_unidades(unidades).expect("cantidad válida")
    }

    fn dinero(unidades: i64) -> Dinero {
        Dinero::desde_unidades(unidades).expect("importe válido")
    }

    #[test]
    fn una_entrada_viene_de_fuera_y_va_a_una_ubicacion() {
        let movimiento =
            Movimiento::entrada(cantidad(10), dinero(120), Ubicacion::Bodega).expect("entrada");

        assert_eq!(movimiento.tipo(), TipoMovimiento::Entrada);
        assert_eq!(movimiento.origen(), None);
        assert_eq!(movimiento.destino(), Some(Ubicacion::Bodega));
        assert_eq!(movimiento.importe().expect("importe"), dinero(1200));
    }

    #[test]
    fn una_merma_sale_de_una_ubicacion_y_no_va_a_ninguna() {
        let movimiento = Movimiento::merma(
            cantidad(3),
            dinero(680),
            Ubicacion::Vitrina,
            "Latas abolladas",
        )
        .expect("merma");

        assert_eq!(movimiento.tipo(), TipoMovimiento::Merma);
        assert_eq!(movimiento.origen(), Some(Ubicacion::Vitrina));
        assert_eq!(movimiento.destino(), None);
        assert_eq!(movimiento.motivo(), Some("Latas abolladas"));
        assert_eq!(movimiento.importe().expect("importe"), dinero(2040));
    }

    #[test]
    fn no_hay_merma_sin_motivo() {
        let error = Movimiento::merma(cantidad(3), dinero(680), Ubicacion::Vitrina, "   ")
            .expect_err("una merma sin motivo es mercancía que desaparece");

        assert_eq!(error, ErrorDominio::MotivoObligatorio);
    }

    #[test]
    fn un_traspaso_tiene_origen_y_destino_distintos() {
        let movimiento = Movimiento::traspaso(
            cantidad(5),
            dinero(120),
            Ubicacion::Bodega,
            Ubicacion::Vitrina,
        )
        .expect("traspaso");

        assert_eq!(movimiento.tipo(), TipoMovimiento::Traspaso);
        assert_eq!(movimiento.origen(), Some(Ubicacion::Bodega));
        assert_eq!(movimiento.destino(), Some(Ubicacion::Vitrina));
    }

    #[test]
    fn no_se_traspasa_algo_al_sitio_donde_ya_esta() {
        let error = Movimiento::traspaso(
            cantidad(5),
            dinero(120),
            Ubicacion::Bodega,
            Ubicacion::Bodega,
        )
        .expect_err("mover algo a donde ya está no es un movimiento");

        assert_eq!(error, ErrorDominio::UbicacionDesconocida);
    }

    #[test]
    fn no_se_registra_un_movimiento_de_cero() {
        assert_eq!(
            Movimiento::entrada(Cantidad::CERO, dinero(120), Ubicacion::Bodega)
                .expect_err("cero no es un movimiento"),
            ErrorDominio::CantidadNoPositiva
        );
    }

    #[test]
    fn la_merma_y_el_ajuste_son_los_que_exigen_motivo() {
        assert!(TipoMovimiento::Merma.exige_motivo());
        assert!(TipoMovimiento::Ajuste.exige_motivo());
        assert!(!TipoMovimiento::Entrada.exige_motivo());
        assert!(!TipoMovimiento::Venta.exige_motivo());
    }

    #[test]
    fn el_texto_guardado_y_el_tipo_son_la_misma_cosa_de_ida_y_vuelta() {
        for tipo in [
            TipoMovimiento::Entrada,
            TipoMovimiento::Venta,
            TipoMovimiento::Traspaso,
            TipoMovimiento::Merma,
            TipoMovimiento::Ajuste,
            TipoMovimiento::Devolucion,
        ] {
            assert_eq!(tipo.como_texto().parse::<TipoMovimiento>(), Ok(tipo));
        }
    }
}
