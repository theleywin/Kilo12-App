/**
 * Tipos que cruzan la frontera entre Rust y la interfaz.
 *
 * Los importes son `string`, no `number`, y no es un descuido: un número de
 * JavaScript es un flotante de doble precisión, de modo que convertir
 * «180.00» a número y de vuelta puede no devolver el mismo valor. Todo el
 * dominio en Rust se toma el trabajo de mantener la aritmética exacta; sería
 * absurdo perderla en el último metro.
 *
 * Regla que se aplica en toda la interfaz: **aquí nunca se hacen cuentas con
 * dinero**. Los importes se muestran tal como llegan, ya formateados, y si
 * hace falta una cuenta —el margen, por ejemplo— se le pide a Rust.
 */

/** Producto tal como lo devuelve el catálogo. */
export interface ProductoDto {
  readonly id: number;
  readonly sku: string;
  readonly nombre: string;
  /** Símbolo de la unidad: `u`, `lb`, `kg`… */
  readonly unidadBase: string;
  /** Nombre legible: «Libra», «Unidad»… */
  readonly unidadNombre: string;
  /** Se vende en fracciones. Lo decide la unidad, no es un dato aparte. */
  readonly esGranel: boolean;
  readonly activo: boolean;
  /** Ya formateados con dos decimales. Solo para mostrar. */
  readonly precio: string;
  readonly costo: string;
  readonly ganancia: string;
  readonly margen: string;
  /** El costo se comió el precio. */
  readonly enRiesgo: boolean;
  readonly enAlmacen: string;
  readonly enVitrina: string;
  readonly existenciaTotal: string;
  readonly bajoMinimo: boolean;
  readonly agotado: boolean;
  readonly presentacion: string;
  readonly totalPresentaciones: number;
}

/** Datos para dar de alta un producto. */
export interface NuevoProductoDto {
  /** Vacío significa «derívalo del nombre» (RF-CAT-02). */
  readonly sku: string;
  readonly nombre: string;
  /** `unidad` o una unidad de peso/volumen. Todo lo demás se fracciona. */
  readonly unidadBase: string;
  /** Precio de venta de una unidad base: «180.00». */
  readonly precioUnitario: string;
  /** Lo que te cuesta a ti. Va junto con la cantidad de apertura. */
  readonly costoUnitario?: string;
  readonly cantidadAlmacen?: string;
  readonly cantidadVitrina?: string;
  readonly stockMinimo?: string;
  readonly objetivoVitrina?: string;
}

/** Estado del almacén: qué hay y cuánto vale. */
export interface AlmacenDto {
  readonly productos: readonly ProductoDto[];
  /** Valor de todo el inventario, a costo promedio ponderado. */
  readonly valorTotal: string;
  readonly conExistencia: number;
  readonly bajoMinimo: number;
  readonly agotados: number;
}

/** Entrada de mercancía comprada. */
export interface NuevaEntradaDto {
  readonly producto: number;
  readonly cantidad: string;
  /** Lo que costó cada unidad en ESTA compra. */
  readonly costoUnitario: string;
  readonly destino: string;
}

/** Baja de mercancía perdida. El motivo es obligatorio. */
export interface NuevaMermaDto {
  readonly producto: number;
  readonly cantidad: string;
  readonly origen: string;
  readonly motivo: string;
}

/** Un producto visto desde la vitrina. */
export interface LineaVitrinaDto {
  readonly id: number;
  readonly sku: string;
  readonly nombre: string;
  readonly unidadBase: string;
  readonly unidadNombre: string;
  readonly enVitrina: string;
  /** Cuánto se quiere mantener exhibido. */
  readonly objetivo: string;
  readonly enAlmacen: string;
  /** Cuánto habría que bajar para alcanzar el objetivo. */
  readonly sugerido: string;
  readonly hayQueReponer: boolean;
  readonly estaExhibido: boolean;
  /** Falta para el objetivo y no hay repuesto guardado: hay que comprar. */
  readonly faltaComprar: boolean;
  /** Guardado en el almacén pero invisible para el cliente. */
  readonly disponibleSinExhibir: boolean;
  readonly agotado: boolean;
}

/** Estado de la vitrina. */
export interface VitrinaDto {
  readonly productos: readonly LineaVitrinaDto[];
  readonly exhibidos: number;
  readonly porReponer: number;
  readonly sinExhibir: number;
  readonly faltaComprar: number;
}

/** Traslado entre almacén y vitrina. No lleva costo. */
export interface NuevoTraspasoDto {
  readonly producto: number;
  readonly cantidad: string;
  /** De dónde sale. El destino es la otra ubicación. */
  readonly origen: string;
}

/** Cómo quedaría la existencia si el movimiento se hiciera. */
export interface SimulacionDto {
  readonly posible: boolean;
  /** Por qué no se puede, ya redactado. */
  readonly problema: string | null;
  /** Cuánto hay ahora en la ubicación implicada. */
  readonly disponible: string;
  readonly bodegaResultante: string;
  readonly vitrinaResultante: string;
}

/** Una línea del historial de un producto. */
export interface MovimientoDto {
  readonly id: number;
  /** Código estable: `ENTRADA`, `MERMA`, `VENTA`… */
  readonly tipo: string;
  readonly tipoNombre: string;
  readonly esEntrada: boolean;
  readonly cantidad: string;
  readonly costoUnitario: string;
  readonly importe: string;
  readonly origen: string | null;
  readonly destino: string | null;
  /** Existencia que quedó después de este movimiento. */
  readonly bodegaResultante: string;
  readonly vitrinaResultante: string;
  readonly motivo: string | null;
  readonly ocurridoEn: string;
}

/** Una forma de vender el producto, con su economía propia. */
export interface PresentacionDto {
  readonly id: number;
  readonly nombre: string;
  /** Cuántas unidades base se llevan al vender una. */
  readonly factor: string;
  readonly precio: string;
  /** `factor × costo unitario base`. */
  readonly costo: string;
  readonly ganancia: string;
  readonly margen: string;
  /** Para comparar presentaciones entre sí. */
  readonly precioPorUnidadBase: string;
  readonly enRiesgo: boolean;
  /** Sale más cara por unidad que una presentación menor. */
  readonly precioAnomalo: boolean;
  readonly esPredeterminada: boolean;
  readonly activa: boolean;
  readonly codigoBarras: string | null;
}

/** La ficha comercial completa de un producto. */
export interface FichaProductoDto {
  readonly id: number;
  readonly sku: string;
  readonly nombre: string;
  readonly unidadBase: string;
  readonly unidadNombre: string;
  readonly esGranel: boolean;
  readonly activo: boolean;
  readonly costo: string;
  readonly stockMinimo: string;
  readonly objetivoVitrina: string;
  readonly existenciaTotal: string;
  readonly presentaciones: readonly PresentacionDto[];
}

/** Un cambio de precio ya ocurrido. */
export interface CambioPrecioDto {
  readonly id: number;
  readonly presentacion: string;
  readonly anterior: string;
  readonly nuevo: string;
  readonly subio: boolean;
  readonly cambiadoEn: string;
}

/** Una forma de vender un producto, vista desde el mostrador. */
export interface PresentacionVendibleDto {
  readonly id: number;
  readonly nombre: string;
  readonly precio: string;
  readonly factor: string;
  readonly esPredeterminada: boolean;
}

/** Un producto disponible para vender. */
export interface ProductoVendibleDto {
  readonly id: number;
  readonly sku: string;
  readonly nombre: string;
  readonly unidadBase: string;
  /** Admite cantidades con decimales. */
  readonly esGranel: boolean;
  /** Lo que hay EN VITRINA: la bodega no está a la venta. */
  readonly enVitrina: string;
  readonly agotado: boolean;
  readonly presentaciones: readonly PresentacionVendibleDto[];
}

/** Un renglón de lo que el cliente se lleva. */
export interface LineaVentaDto {
  readonly producto: number;
  readonly presentacion: number;
  readonly cantidad: string;
}

/** Una parte del pago. El cobro puede ser mixto. */
export interface PagoDto {
  readonly metodo: string;
  readonly entregado: string;
}

/** Una línea de la venta en curso, ya calculada por el núcleo. */
export interface LineaPrevistaDto {
  readonly producto: number;
  readonly presentacion: number;
  readonly nombreProducto: string;
  readonly nombrePresentacion: string;
  readonly cantidad: string;
  readonly precio: string;
  readonly importe: string;
  readonly unidadesBase: string;
  /** La vitrina no da para esta línea. */
  readonly sinExistencia: boolean;
}

/** La venta en curso. */
export interface VentaPrevistaDto {
  readonly lineas: readonly LineaPrevistaDto[];
  readonly total: string;
  readonly hayFaltantes: boolean;
}

/** Lo que el cliente pone frente a lo que debe. */
export interface CobroCalculadoDto {
  readonly entregado: string;
  readonly falta: string;
  readonly vuelto: string;
  readonly alcanza: boolean;
}

/** El resultado de cobrar. */
export interface VentaHechaDto {
  readonly folio: number;
  readonly total: string;
  readonly entregado: string;
  /** Siempre en pesos, aunque se haya pagado en dólares. */
  readonly vuelto: string;
}

/** Una venta en la lista del historial. */
export interface VentaListadaDto {
  readonly id: number;
  readonly folio: number;
  /** Se anuló: sigue en el historial pero no cuenta para nada. */
  readonly anulada: boolean;
  readonly total: string;
  /** Total menos el costo congelado al cobrar. */
  readonly ganancia: string;
  readonly ocurridoEn: string;
  readonly fecha: string;
  readonly hora: string;
}

/** Lo vendido hoy. */
export interface ResumenDelDiaDto {
  readonly cuantas: number;
  readonly total: string;
  readonly ganancia: string;
}

/** El historial de ventas con el corte del día. */
export interface HistorialVentasDto {
  readonly hoy: ResumenDelDiaDto;
  readonly ventas: readonly VentaListadaDto[];
}

/** Una línea de una venta ya cobrada. */
export interface LineaVendidaDto {
  readonly producto: number;
  readonly nombreProducto: string;
  readonly nombrePresentacion: string;
  readonly cantidad: string;
  readonly precio: string;
  readonly importe: string;
  readonly costo: string;
  readonly ganancia: string;
}

/** Una de las formas en que se pagó una venta. */
export interface PagoHechoDto {
  readonly metodo: string;
  readonly metodoNombre: string;
  /** Lo que entregó el cliente, en su moneda. */
  readonly entregado: string;
  readonly moneda: string;
  /** Tasa congelada. Solo en los pagos en dólares. */
  readonly tasa: string | null;
  /** Cuánto valió en pesos. */
  readonly equivalente: string;
}

/** Una venta con todo su detalle. */
export interface VentaDetalladaDto {
  readonly id: number;
  readonly folio: number;
  readonly anulada: boolean;
  readonly motivoAnulacion: string | null;
  readonly total: string;
  readonly costoTotal: string;
  readonly ganancia: string;
  readonly vuelto: string;
  readonly entregado: string;
  readonly ocurridoEn: string;
  readonly fecha: string;
  readonly hora: string;
  readonly lineas: readonly LineaVendidaDto[];
  readonly pagos: readonly PagoHechoDto[];
}

/** Desglose de la venta de una sesión por forma de pago. */
export interface DesgloseVentaDto {
  readonly efectivoCup: string;
  readonly transferencia: string;
  /** Dólares recibidos, en dólares. */
  readonly efectivoUsd: string;
  readonly efectivoUsdEnCup: string;
  /** Efectivo más transferencia. */
  readonly totalEnPesos: string;
  /** Con los dólares convertidos y sumados. */
  readonly totalConsolidado: string;
}

/** Un movimiento de efectivo ajeno a la venta. */
export interface MovimientoEfectivoListadoDto {
  readonly id: number;
  /** `ENTRADA` o `SALIDA`. */
  readonly tipo: string;
  readonly tipoNombre: string;
  readonly suma: boolean;
  readonly importe: string;
  readonly motivo: string;
  readonly ocurridoEn: string;
  readonly hora: string;
}

/** La caja tal como está ahora mismo. */
export interface EstadoCajaDto {
  readonly id: number;
  readonly operador: string;
  readonly abiertaEn: string;
  readonly fondoInicial: string;
  readonly cuantasVentas: number;
  readonly desglose: DesgloseVentaDto;
  readonly entradas: string;
  readonly salidas: string;
  /** Billetes de peso que debería haber ahora (RF-CAJ-04). */
  readonly efectivoEsperado: string;
  /** Dólares que debería haber, en dólares. */
  readonly dolaresEsperados: string;
  readonly movimientos: readonly MovimientoEfectivoListadoDto[];
}

/** El arqueo de una moneda. */
export interface ArqueoDto {
  readonly esperado: string;
  readonly contado: string;
  /** Positiva es sobrante; negativa, faltante. */
  readonly diferencia: string;
  readonly cuadra: boolean;
  readonly sobra: boolean;
}

/** El resumen económico de la sesión (RF-CAJ-10). */
export interface ResumenEconomicoDto {
  readonly ventaTotal: string;
  readonly costoVendido: string;
  readonly merma: string;
  readonly gananciaBruta: string;
  readonly comisionPorcentaje: string;
  readonly comision: string;
  readonly gananciaNeta: string;
}

/** El cierre de caja, calculado o ya guardado. */
export interface CierreCalculadoDto {
  readonly sesion: number;
  readonly operador: string;
  readonly abiertaEn: string;
  readonly cerradaEn: string | null;
  /** `SEPARADO` o `CONSOLIDADO`. */
  readonly modo: string;
  readonly fondoInicial: string;
  /** Billetes que dejaron las ventas: lo entregado menos el vuelto. */
  readonly ventasEfectivo: string;
  readonly desglose: DesgloseVentaDto;
  readonly entradas: string;
  readonly salidas: string;
  readonly arqueoCup: ArqueoDto;
  readonly arqueoUsd: ArqueoDto;
  readonly cuadra: boolean;
  readonly economico: ResumenEconomicoDto;
}

/** Una sesión en el historial. */
export interface SesionListadaDto {
  readonly id: number;
  readonly operador: string;
  readonly abiertaEn: string;
  readonly cerradaEn: string | null;
  readonly fecha: string;
  readonly abierta: boolean;
}

/** Las tres formas de pagar (RF-VTA-09). */
export const METODOS_PAGO: ReadonlyArray<{ valor: string; nombre: string; moneda: string }> = [
  { valor: 'EFECTIVO_CUP', nombre: 'Efectivo', moneda: '$' },
  { valor: 'TRANSFERENCIA', nombre: 'Transferencia', moneda: '$' },
  { valor: 'EFECTIVO_USD', nombre: 'Dólares', moneda: 'USD' },
];

/** Las dos ubicaciones, tal como las nombra el negocio. */
export const UBICACIONES: ReadonlyArray<{ valor: string; nombre: string }> = [
  { valor: 'BODEGA', nombre: 'Almacén' },
  { valor: 'VITRINA', nombre: 'Vitrina' },
];

/** Ganancia y margen de un precio frente a su costo. */
export interface MargenDto {
  readonly ganancia: string;
  readonly porcentaje: string;
  readonly enRiesgo: boolean;
}

/**
 * Error devuelto por un comando.
 *
 * El código es estable: permite distinguir un fallo que el usuario puede
 * corregir de uno técnico, en lugar de mostrar un texto y encogerse de
 * hombros.
 */
export interface ErrorDto {
  readonly codigo: string;
  readonly mensaje: string;
  readonly delUsuario: boolean;
}

/** Unidad en que se cuenta un producto que se vende por peso o volumen. */
export const UNIDADES_GRANEL: ReadonlyArray<{ valor: string; nombre: string }> = [
  { valor: 'lb', nombre: 'Libra' },
  { valor: 'kg', nombre: 'Kilogramo' },
  { valor: 'g', nombre: 'Gramo' },
  { valor: 'L', nombre: 'Litro' },
  { valor: 'ml', nombre: 'Mililitro' },
];

/** Lo que se cuenta de uno en uno. */
export const UNIDAD_SUELTA = 'unidad';

/** Lo que se manda al previsualizar o confirmar un cierre de caja. */
export interface CierreCajaPedido {
  /** Billetes de peso contados físicamente. */
  readonly contadoCup: string;
  /** Dólares contados, en dólares. */
  readonly contadoUsd: string;
  /** `SEPARADO` o `CONSOLIDADO`. */
  readonly modo: string;
}

/** Un renglón del recuento de billetes. */
export interface LineaConteoDto {
  /** Valor del billete, en pesos enteros. */
  readonly denominacion: number;
  readonly cuantos: number;
  /** Lo que suman esos billetes. */
  readonly importe: string;
}

/** El recuento de billetes, ya sumado por el núcleo. */
export interface ConteoCalculadoDto {
  readonly lineas: readonly LineaConteoDto[];
  readonly total: string;
  readonly cuantosBilletes: number;
}
