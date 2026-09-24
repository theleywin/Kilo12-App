/**
 * Las secciones de Kilo12, en un solo sitio.
 *
 * De esta tabla salen las rutas, el menú lateral, los atajos de teclado y
 * el texto de las pantallas que todavía no existen. Tener una sola fuente
 * evita el clásico: agregar una pantalla y olvidarse del menú.
 *
 * El orden es el de uso real, no el del organigrama: arriba lo que se abre
 * cien veces al día, abajo lo que se abre una vez por semana.
 */

/** Cuánto aire tiene la pantalla. */
export type Densidad = 'densa' | 'amplia';

export interface Seccion {
  /** Fragmento de ruta, sin barra. */
  readonly ruta: string;
  readonly titulo: string;
  /** Una frase que explique la pantalla a quien nunca la ha visto. */
  readonly descripcion: string;
  /** Atajo de teclado que la abre. */
  readonly tecla: string;
  readonly icono: Icono;
  readonly densidad: Densidad;
  /** Qué va a vivir aquí. Se muestra mientras la pantalla está pendiente. */
  readonly incluye: readonly string[];
}

export type Icono =
  | 'vender'
  | 'entrada'
  | 'vitrina'
  | 'almacen'
  | 'productos'
  | 'ventas'
  | 'caja'
  | 'informes'
  | 'guia';

export const SECCIONES: readonly Seccion[] = [
  {
    ruta: 'vender',
    titulo: 'Vender',
    descripcion: 'Cobrar. La pantalla donde pasas el día.',
    tecla: 'F1',
    icono: 'vender',
    // La única densa: se usa con gente esperando en el mostrador.
    densidad: 'densa',
    incluye: [
      'Buscar productos por nombre o código desde una sola caja',
      'Cantidades fraccionarias para lo que se vende a granel',
      'Tres formas de pago y cambio de divisa',
      'Cobrar y devolver el vuelto sin soltar el teclado',
    ],
  },
  {
    ruta: 'entrada',
    titulo: 'Entrada',
    descripcion: 'Llegó mercancía. Se registra aquí, una cosa detrás de otra.',
    tecla: 'F2',
    icono: 'entrada',
    densidad: 'amplia',
    incluye: [],
  },
  {
    ruta: 'vitrina',
    titulo: 'Vitrina',
    descripcion: 'Qué hay exhibido y qué falta reponer desde el almacén.',
    tecla: 'F3',
    icono: 'vitrina',
    densidad: 'amplia',
    incluye: [
      'Existencia exhibida de cada producto',
      'Lista de reabastecimiento: qué bajar del almacén y cuánto',
      'Aviso de producto disponible en almacén pero no exhibido',
      'Reposición masiva, ajustable producto por producto',
    ],
  },
  {
    ruta: 'almacen',
    titulo: 'Almacén',
    descripcion: 'Qué hay guardado, cuánto vale y qué se perdió.',
    tecla: 'F4',
    icono: 'almacen',
    densidad: 'amplia',
    incluye: [
      'Existencia y valor de todo lo guardado',
      'Bajas por merma, siempre con su motivo',
      'Historial completo de cada producto',
      'Conteo físico y ajustes',
    ],
  },
  {
    ruta: 'productos',
    titulo: 'Productos',
    descripcion: 'El catálogo: qué vendes, a cuánto lo compras y a cuánto lo vendes.',
    tecla: 'F5',
    icono: 'productos',
    densidad: 'amplia',
    incluye: [
      'Costo, precio, ganancia y margen de cada producto',
      'Aviso cuando el costo se come el precio',
      'Cambiar precios sin tocar el inventario',
      'Presentaciones: paquete, caja, media libra',
    ],
  },
  {
    ruta: 'ventas',
    titulo: 'Ventas',
    descripcion: 'Todo lo que se vendió, con su detalle.',
    tecla: 'F6',
    icono: 'ventas',
    densidad: 'amplia',
    incluye: [
      'Historial de ventas completadas, filtrable por fecha',
      'Detalle de cada venta con sus líneas y su forma de pago',
      'Anulación de una venta del turno vigente',
      'Ganancia real de cada venta',
    ],
  },
  {
    ruta: 'caja',
    titulo: 'Caja',
    descripcion: 'Abrir el turno, cerrarlo y cuadrar el efectivo.',
    tecla: 'F7',
    icono: 'caja',
    densidad: 'amplia',
    incluye: [
      'Apertura de turno con el fondo inicial',
      'Arqueo: lo que dice el sistema contra lo que hay en la gaveta',
      'Comisión del operador sobre la venta del turno',
      'Cierre con su resumen',
    ],
  },
  {
    ruta: 'informes',
    titulo: 'Informes',
    descripcion: 'Qué se vende, qué deja ganancia y qué está por agotarse.',
    tecla: 'F8',
    icono: 'informes',
    densidad: 'amplia',
    incluye: [
      'Ventas por día, semana y mes',
      'Productos más vendidos y más rentables',
      'Productos por agotarse según el mínimo y el ritmo de venta',
      'Ganancia del período, descontada la comisión',
    ],
  },
];

/** Sección de uso interno: el catálogo visual de la aplicación. */
export const GUIA: Seccion = {
  ruta: 'guia',
  titulo: 'Guía de estilo',
  descripcion: 'El lenguaje visual de Kilo12: colores, textos y piezas.',
  tecla: 'F9',
  icono: 'guia',
  densidad: 'amplia',
  incluye: [],
};

export const TODAS: readonly Seccion[] = [...SECCIONES, GUIA];

/** Encuentra la sección a la que pertenece una dirección. */
export function seccionDe(url: string): Seccion | undefined {
  const raiz = url.split('?')[0].split('/').filter(Boolean)[0];
  return TODAS.find((seccion) => seccion.ruta === raiz);
}
