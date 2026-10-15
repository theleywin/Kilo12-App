/**
 * Búsqueda de productos por nombre o por código.
 *
 * Vive aquí porque la usan Vitrina, Almacén y Vender. Copiarla en cada
 * pantalla sería garantizar que un día busquen de formas distintas: el
 * mismo texto encontraría el producto en una y no en la otra, y nadie
 * sabría cuál de las dos está mal.
 *
 * No hay nada de Angular en este archivo a propósito: son funciones puras
 * y se prueban sin montar un componente.
 */

/** Signos diacríticos combinantes, los que deja suelta `normalize('NFD')`. */
const DIACRITICOS = /[\u0300-\u036f]/g;

/**
 * Quita acentos y mayúsculas para buscar como se teclea con prisa.
 *
 * Quien busca «azucar» tiene que encontrar «Azúcar». Con un cliente
 * delante nadie va a poner la tilde.
 *
 * **La ñ también se pliega a n.** `normalize('NFD')` la descompone en `n`
 * más una tilde combinante, y la tilde cae con el resto de los acentos, así
 * que «pina» encuentra «Piña». No es un descuido: para buscar es lo que se
 * quiere, porque la ñ es justo la tecla que falta o que cuesta encontrar.
 * El precio es que «niño» y «nino» se buscan igual, y en una lista de
 * productos de mercadito eso no estorba a nadie.
 */
export function plegar(texto: string): string {
  return texto.normalize('NFD').replace(DIACRITICOS, '').toLowerCase().trim();
}

/** Lo mínimo que hace falta para reconocer un producto en una lista. */
export interface Identificable {
  readonly nombre: string;
  readonly sku: string;
}

/**
 * Filtra una lista de productos por nombre o por código.
 *
 * Coincide en **cualquier posición**, no solo al principio: en una lista
 * larga uno se acuerda de «cola» dentro de «Refresco de cola», no de cómo
 * empieza. La pantalla de Vender hace lo contrario a propósito —allí se
 * teclea con el cliente esperando y el prefijo llega antes al resultado—,
 * así que cada pantalla elige su criterio, pero las dos pliegan el texto
 * con la misma función.
 *
 * Con la búsqueda vacía devuelve la lista tal cual, sin copiarla.
 */
export function filtrarProductos<T extends Identificable>(
  productos: readonly T[],
  busqueda: string,
): readonly T[] {
  const aguja = plegar(busqueda);
  if (!aguja) {
    return productos;
  }

  return productos.filter(
    (producto) => plegar(producto.nombre).includes(aguja) || plegar(producto.sku).includes(aguja),
  );
}
