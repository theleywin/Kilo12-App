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
    /// Mercancía que se llevan los dueños para su casa (RF-CON-01).
    ///
    /// No es una venta —no entra dinero— ni una merma —no se perdió nada—:
    /// genera una deuda al costo contra la cuenta de consumo que la cargó.
    Consumo,
    /// Reingreso por un consumo anulado (RF-CON-16).
    ConsumoAnulado,
}

impl TipoMovimiento {
    /// Todos los tipos que existen.
    ///
    /// No es una comodidad: es lo que permite que una prueba recorra el
    /// juego completo en vez de una lista escrita a mano que se queda corta
    /// en silencio. `todos_los_tipos_estan_en_la_lista` la custodia con un
    /// `match` exhaustivo, así que añadir una variante **rompe la
    /// compilación** hasta que entra aquí también.
    pub const TODOS: &'static [Self] = &[
        Self::Entrada,
        Self::Venta,
        Self::Traspaso,
        Self::Merma,
        Self::Ajuste,
        Self::Devolucion,
        Self::Consumo,
        Self::ConsumoAnulado,
    ];

    /// Texto con que se guarda en la base de datos.
    pub const fn como_texto(self) -> &'static str {
        match self {
            Self::Entrada => "ENTRADA",
            Self::Venta => "VENTA",
            Self::Traspaso => "TRASPASO",
            Self::Merma => "MERMA",
            Self::Ajuste => "AJUSTE",
            Self::Devolucion => "DEVOLUCION",
            Self::Consumo => "CONSUMO",
            Self::ConsumoAnulado => "CONSUMO_ANULADO",
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
            Self::Consumo => "Consumo",
            Self::ConsumoAnulado => "Consumo anulado",
        }
    }

    /// Indica si el movimiento exige explicar por qué se hizo (RF-INV-08).
    ///
    /// Una merma o un ajuste sin motivo es mercancía que desaparece sin que
    /// nadie responda por ella. El consumo entra en la misma regla por el
    /// mismo motivo: el kárdex se lee solo (RF-INV-05), y un asiento que
    /// dijera «Consumo» a secas obligaría a ir a buscar de quién era. El
    /// motivo lleva la cuenta y quién lo tomó, copiados igual que los
    /// nombres de una venta.
    pub const fn exige_motivo(self) -> bool {
        matches!(
            self,
            Self::Merma | Self::Ajuste | Self::Consumo | Self::ConsumoAnulado
        )
    }

    /// Indica si el movimiento suma existencia en lugar de restarla.
    pub const fn es_entrada(self) -> bool {
        matches!(
            self,
            Self::Entrada | Self::Devolucion | Self::ConsumoAnulado
        )
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
            "CONSUMO" => Ok(Self::Consumo),
            "CONSUMO_ANULADO" => Ok(Self::ConsumoAnulado),
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

    /// Salida por venta (RF-VTA-11).
    ///
    /// Sale de la vitrina, que es lo único disponible para venta inmediata,
    /// y no va a ninguna parte: se fue con el cliente.
    pub fn venta(
        cantidad: Cantidad,
        costo_unitario: Dinero,
        origen: Ubicacion,
    ) -> Result<Self, ErrorDominio> {
        Ok(Self {
            tipo: TipoMovimiento::Venta,
            origen: Some(origen),
            destino: None,
            cantidad: cantidad_positiva(cantidad)?,
            costo_unitario: costo_no_negativo(costo_unitario)?,
            motivo: None,
        })
    }

    /// Reingreso de mercancía por una venta anulada (RF-VTA-15).
    ///
    /// Es el espejo exacto de [`Self::venta`]: la mercancía vuelve a la
    /// vitrina de donde salió, y vuelve **al costo con que se vendió**, no
    /// al de hoy. Reingresarla a otro costo movería el valor del inventario
    /// por una operación que solo deshace, y el promedio ponderado dejaría
    /// de cuadrar con lo que realmente se compró.
    ///
    /// El motivo se exige en la firma, como en la merma: una anulación sin
    /// explicación es indistinguible de un descuadre.
    pub fn devolucion(
        cantidad: Cantidad,
        costo_unitario: Dinero,
        destino: Ubicacion,
        motivo: impl Into<String>,
    ) -> Result<Self, ErrorDominio> {
        let motivo = motivo.into();
        if motivo.trim().is_empty() {
            return Err(ErrorDominio::MotivoObligatorio);
        }

        Ok(Self {
            tipo: TipoMovimiento::Devolucion,
            origen: None,
            destino: Some(destino),
            cantidad: cantidad_positiva(cantidad)?,
            costo_unitario: costo_no_negativo(costo_unitario)?,
            motivo: Some(motivo),
        })
    }

    /// Salida por consumo de los dueños (RF-CON-01).
    ///
    /// Es casi una merma —sale mercancía y no entra dinero— con una
    /// diferencia que lo cambia todo: **no se perdió nada**. El valor sale
    /// del inventario y se convierte en una deuda al costo contra la cuenta
    /// que lo cargó, así que el negocio no es más pobre, tiene otra cosa.
    ///
    /// Sale al costo vigente y lo congela aquí, como cualquier otra salida
    /// (RF-COS-07): la deuda que se acuerda hoy no puede moverse porque
    /// mañana suba el arroz.
    ///
    /// El motivo se exige en la firma, igual que en la merma, y lleva la
    /// cuenta y la persona que se lo llevó. Así el kárdex de un producto se
    /// lee de arriba abajo sin tener que cruzarlo con nada.
    pub fn consumo(
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
            tipo: TipoMovimiento::Consumo,
            origen: Some(origen),
            destino: None,
            cantidad: cantidad_positiva(cantidad)?,
            costo_unitario: costo_no_negativo(costo_unitario)?,
            motivo: Some(motivo),
        })
    }

    /// Reingreso de mercancía por un consumo anulado (RF-CON-16).
    ///
    /// Espejo exacto de [`Self::consumo`], y por el mismo motivo que
    /// [`Self::devolucion`] lo es de la venta: la mercancía vuelve **al
    /// costo con que salió**, no al de hoy. Reingresarla a otro costo
    /// movería el valor del inventario por una operación que solo deshace.
    ///
    /// Vuelve a la ubicación de la que salió, que no es siempre la vitrina:
    /// un consumo puede haber sacado de la bodega.
    pub fn consumo_anulado(
        cantidad: Cantidad,
        costo_unitario: Dinero,
        destino: Ubicacion,
        motivo: impl Into<String>,
    ) -> Result<Self, ErrorDominio> {
        let motivo = motivo.into();
        if motivo.trim().is_empty() {
            return Err(ErrorDominio::MotivoObligatorio);
        }

        Ok(Self {
            tipo: TipoMovimiento::ConsumoAnulado,
            origen: None,
            destino: Some(destino),
            cantidad: cantidad_positiva(cantidad)?,
            costo_unitario: costo_no_negativo(costo_unitario)?,
            motivo: Some(motivo),
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
        for tipo in TipoMovimiento::TODOS {
            assert_eq!(tipo.como_texto().parse::<TipoMovimiento>(), Ok(*tipo));
        }
    }

    /// Custodia de `TODOS`, y es la razón de que `TODOS` exista.
    ///
    /// Antes esta comprobación recorría una lista escrita a mano en la
    /// propia prueba. Eso tiene un defecto que no se ve: al añadir una
    /// variante al enum, la prueba **seguía pasando en verde** sin tocar el
    /// tipo nuevo, y una prueba que pasa sin cubrir lo que debería es peor
    /// que no tenerla, porque da por cubierto lo que no lo está.
    ///
    /// El `match` de abajo es exhaustivo a propósito: una variante nueva
    /// rompe la compilación aquí, y entonces el `assert` de la cuenta
    /// obliga a meterla también en `TODOS`.
    #[test]
    fn todos_los_tipos_estan_en_la_lista() {
        let mut vistos = 0;

        for tipo in TipoMovimiento::TODOS {
            match tipo {
                TipoMovimiento::Entrada
                | TipoMovimiento::Venta
                | TipoMovimiento::Traspaso
                | TipoMovimiento::Merma
                | TipoMovimiento::Ajuste
                | TipoMovimiento::Devolucion
                | TipoMovimiento::Consumo
                | TipoMovimiento::ConsumoAnulado => vistos += 1,
            }
        }

        assert_eq!(vistos, 8, "falta algún tipo en TipoMovimiento::TODOS");
    }

    #[test]
    fn un_consumo_sale_de_una_ubicacion_y_dice_de_quien_era() {
        let movimiento = Movimiento::consumo(
            cantidad(3),
            dinero(680),
            Ubicacion::Vitrina,
            "Consumo de Ivett · tomó Yaneisy",
        )
        .expect("consumo");

        assert_eq!(movimiento.tipo(), TipoMovimiento::Consumo);
        assert_eq!(movimiento.origen(), Some(Ubicacion::Vitrina));
        assert_eq!(movimiento.destino(), None);
        assert_eq!(movimiento.motivo(), Some("Consumo de Ivett · tomó Yaneisy"));
        // La deuda que genera: tres latas a 680.
        assert_eq!(movimiento.importe().expect("importe"), dinero(2040));
    }

    #[test]
    fn no_hay_consumo_sin_decir_de_quien_es() {
        let error = Movimiento::consumo(cantidad(3), dinero(680), Ubicacion::Vitrina, "  ")
            .expect_err("un consumo sin cuenta es mercancía que desaparece");

        assert_eq!(error, ErrorDominio::MotivoObligatorio);
    }

    #[test]
    fn anular_un_consumo_devuelve_la_mercancia_de_donde_salio() {
        // Salió de la bodega, así que vuelve a la bodega: no siempre es la
        // vitrina.
        let movimiento = Movimiento::consumo_anulado(
            cantidad(3),
            dinero(680),
            Ubicacion::Bodega,
            "Anulado: se registró en la cuenta equivocada",
        )
        .expect("consumo anulado");

        assert_eq!(movimiento.tipo(), TipoMovimiento::ConsumoAnulado);
        assert_eq!(movimiento.origen(), None);
        assert_eq!(movimiento.destino(), Some(Ubicacion::Bodega));
        assert!(movimiento.tipo().es_entrada());
    }

    #[test]
    fn el_consumo_anulado_vuelve_al_costo_con_que_salio() {
        // El costo del asiento es el congelado, no el de hoy. Si volviera a
        // otro costo, deshacer un consumo movería el valor del inventario.
        let salida = Movimiento::consumo(
            cantidad(3),
            dinero(680),
            Ubicacion::Vitrina,
            "Consumo de Ivett",
        )
        .expect("consumo");

        let reversa = Movimiento::consumo_anulado(
            salida.cantidad(),
            salida.costo_unitario(),
            Ubicacion::Vitrina,
            "Anulado",
        )
        .expect("reversa");

        assert_eq!(reversa.costo_unitario(), salida.costo_unitario());
        assert_eq!(
            reversa.importe().expect("importe"),
            salida.importe().expect("importe")
        );
    }

    #[test]
    fn el_consumo_tambien_exige_motivo() {
        assert!(TipoMovimiento::Consumo.exige_motivo());
        assert!(TipoMovimiento::ConsumoAnulado.exige_motivo());
    }
}
