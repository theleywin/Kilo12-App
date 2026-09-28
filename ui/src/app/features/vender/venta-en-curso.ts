import {
  LineaPrevistaDto,
  LineaRetomadaDto,
  LineaVentaDto,
  PresentacionVendibleDto,
  PROBLEMA_LINEA,
  ProblemaLineaDto,
  ProductoVendibleDto,
} from '../../core/api.types';

/**
 * El modelo de la venta en curso: los renglones que hay en pantalla.
 *
 * Casi todos salen del catálogo y se pueden cobrar. Pero una venta
 * retomada puede traer renglones que ya no se venden —el producto se
 * desactivó, la presentación desapareció— y esos también tienen que
 * verse, marcados, para que el usuario los quite (RF-VTA-14, decisión 6).
 * Esconderlos sería peor: el cliente pidió cuatro cosas y en pantalla hay
 * tres, sin explicación.
 *
 * Todo aquí es puro: sin señales, sin llamadas al núcleo.
 */

export const ESTADO_LINEA = {
  /** Sale del catálogo de venta: se calcula y se cobra. */
  VENDIBLE: 'VENDIBLE',
  /** Vino de una venta retomada y hoy no se puede vender. */
  MARCADA: 'MARCADA',
} as const;

export type EstadoLinea = (typeof ESTADO_LINEA)[keyof typeof ESTADO_LINEA];

/** Un renglón que se puede cobrar. */
export interface LineaVendible {
  readonly estado: typeof ESTADO_LINEA.VENDIBLE;
  readonly producto: ProductoVendibleDto;
  readonly presentacion: PresentacionVendibleDto;
  readonly cantidad: string;
}

/** Un renglón retomado que hay que quitar (o arreglar) antes de cobrar. */
export interface LineaMarcada {
  readonly estado: typeof ESTADO_LINEA.MARCADA;
  readonly retomada: LineaRetomadaDto;
  readonly problema: ProblemaLineaDto;
}

export type Linea = LineaVendible | LineaMarcada;

/** Un renglón de la tabla, ya con lo que se enseña. */
export interface FilaVenta {
  readonly nombreProducto: string;
  readonly nombrePresentacion: string;
  readonly cantidad: string;
  /** `null` mientras el núcleo no lo calcula, o si no se puede vender. */
  readonly precio: string | null;
  readonly importe: string | null;
  readonly sinExistencia: boolean;
  /** Por qué no se puede cobrar, si es una línea marcada. */
  readonly problema: string | null;
}

export function esVendible(linea: Linea): linea is LineaVendible {
  return linea.estado === ESTADO_LINEA.VENDIBLE;
}

export function esMarcada(linea: Linea): linea is LineaMarcada {
  return linea.estado === ESTADO_LINEA.MARCADA;
}

/** Lo que se le pide al núcleo: solo lo que se puede vender. */
export function pedidoDe(lineas: readonly Linea[]): LineaVentaDto[] {
  return lineas.filter(esVendible).map((linea) => ({
    producto: linea.producto.id,
    presentacion: linea.presentacion.id,
    cantidad: linea.cantidad,
  }));
}

/**
 * Lo que se aparta al pulsar «Pendiente»: la venta ENTERA, marcadas
 * incluidas.
 *
 * Un producto desactivado se puede volver a activar mañana; tirar su
 * renglón al apartar sería perder parte de lo que pidió el cliente. Si
 * el núcleo no lo admite (el producto ya no existe), lo dice y el usuario
 * lo quita.
 */
export function apartadoDe(lineas: readonly Linea[]): LineaVentaDto[] {
  return lineas.map((linea) =>
    esVendible(linea)
      ? {
          producto: linea.producto.id,
          presentacion: linea.presentacion.id,
          cantidad: linea.cantidad,
        }
      : {
          producto: linea.retomada.producto,
          presentacion: linea.retomada.presentacion,
          cantidad: linea.retomada.cantidad,
        },
  );
}

/** El aviso cuando el catálogo de la pantalla no tiene lo que el núcleo sí. */
const FUERA_DEL_CATALOGO: ProblemaLineaDto = {
  codigo: PROBLEMA_LINEA.PRODUCTO_NO_ENCONTRADO,
  mensaje: 'Ya no está en el catálogo de venta',
};

/**
 * Convierte los renglones de una venta retomada en renglones de la
 * pantalla.
 *
 * Cada renglón sano se busca en el catálogo de venta por producto y
 * presentación, igual que cuando se elige a mano: así la venta retomada y
 * la tecleada son la misma cosa y se cobran por el mismo camino.
 *
 * «No hay suficiente en vitrina» NO marca el renglón: el producto se sigue
 * vendiendo, y la vista previa ya lo señala como a cualquier otro que no
 * alcanza. Basta con bajar mercancía de la vitrina para cobrarlo.
 */
export function lineasDeRetomada(
  retomadas: readonly LineaRetomadaDto[],
  catalogo: readonly ProductoVendibleDto[],
): Linea[] {
  return retomadas.map((retomada): Linea => {
    const problema = retomada.problema;
    if (problema && problema.codigo !== PROBLEMA_LINEA.SIN_EXISTENCIA) {
      return { estado: ESTADO_LINEA.MARCADA, retomada, problema };
    }

    const producto = catalogo.find((p) => p.id === retomada.producto);
    const presentacion = producto?.presentaciones.find((p) => p.id === retomada.presentacion);
    if (!producto || !presentacion) {
      return { estado: ESTADO_LINEA.MARCADA, retomada, problema: FUERA_DEL_CATALOGO };
    }

    return {
      estado: ESTADO_LINEA.VENDIBLE,
      producto,
      presentacion,
      cantidad: retomada.cantidad,
    };
  });
}

/**
 * Junta los renglones con lo que calculó el núcleo.
 *
 * La vista previa solo recibe los vendibles, en el mismo orden: el
 * vendible número N se empareja con su línea prevista número N. Las
 * marcadas se enseñan con lo que se sabe de ellas, sin precio.
 *
 * Entre que cambia la venta y llega la cuenta nueva, la vista previa es la
 * de antes. Si ya no corresponde (otro número de renglones, otro producto
 * en esa posición) no se usa: mejor un «—» un instante que el precio de
 * otro renglón.
 */
export function filasDe(
  lineas: readonly Linea[],
  previstas: readonly LineaPrevistaDto[] | null,
): FilaVenta[] {
  const vendibles = lineas.filter(esVendible);
  const vigente =
    previstas !== null &&
    previstas.length === vendibles.length &&
    vendibles.every(
      (linea, i) =>
        previstas[i].producto === linea.producto.id &&
        previstas[i].presentacion === linea.presentacion.id,
    );
  let vendible = 0;

  return lineas.map((linea): FilaVenta => {
    if (esMarcada(linea)) {
      return {
        nombreProducto: linea.retomada.nombreProducto || 'Producto eliminado',
        nombrePresentacion: linea.retomada.nombrePresentacion || '—',
        cantidad: linea.retomada.cantidad,
        precio: null,
        importe: null,
        sinExistencia: false,
        problema: linea.problema.mensaje,
      };
    }

    const prevista = vigente ? previstas[vendible] : undefined;
    vendible += 1;

    return {
      nombreProducto: prevista?.nombreProducto ?? linea.producto.nombre,
      nombrePresentacion: prevista?.nombrePresentacion ?? linea.presentacion.nombre,
      cantidad: prevista?.cantidad ?? linea.cantidad,
      precio: prevista?.precio ?? null,
      importe: prevista?.importe ?? null,
      sinExistencia: prevista?.sinExistencia ?? false,
      problema: null,
    };
  });
}
