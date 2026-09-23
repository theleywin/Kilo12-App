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
