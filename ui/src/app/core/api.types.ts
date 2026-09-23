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
