import { filtrarProductos, plegar } from './buscar';

/**
 * El buscador de Vitrina y Almacén.
 *
 * Son funciones puras: no hace falta montar un componente ni un
 * `TestBed`. Esa es justamente la razón de que vivan fuera de las
 * pantallas.
 */
describe('plegar', () => {
  it('quita los acentos para que «azucar» encuentre «Azúcar»', () => {
    expect(plegar('Azúcar')).toBe('azucar');
  });

  it('baja a minúsculas y recorta los espacios de los extremos', () => {
    expect(plegar('  ARROZ Blanco  ')).toBe('arroz blanco');
  });

  it('deja intactos los espacios de dentro: son parte del nombre', () => {
    expect(plegar('Refresco de cola')).toBe('refresco de cola');
  });

  // `normalize('NFD')` descompone la ñ en n + tilde combinante, y la tilde
  // cae con el resto de los acentos. Queda documentado con una prueba
  // porque es la clase de detalle que alguien «arregla» sin saber que es
  // deliberado.
  it('pliega la ñ a n, para que «pina» encuentre «Piña»', () => {
    expect(plegar('Piña')).toBe('pina');
    expect(plegar('Piña')).toBe(plegar('Pina'));
  });
});

describe('filtrarProductos', () => {
  const productos = [
    { nombre: 'Arroz blanco', sku: 'ARR-001' },
    { nombre: 'Azúcar refino', sku: 'AZU-001' },
    { nombre: 'Refresco de cola', sku: 'REF-010' },
    { nombre: 'Aceite de girasol', sku: 'ACE-003' },
  ];

  it('sin búsqueda devuelve la lista entera, y la misma', () => {
    // Misma referencia: no copia lo que no hace falta copiar.
    expect(filtrarProductos(productos, '')).toBe(productos);
    expect(filtrarProductos(productos, '   ')).toBe(productos);
  });

  it('encuentra por el nombre', () => {
    expect(filtrarProductos(productos, 'arroz').map((p) => p.sku)).toEqual(['ARR-001']);
  });

  it('encuentra por el código', () => {
    expect(filtrarProductos(productos, 'ref-010').map((p) => p.nombre)).toEqual([
      'Refresco de cola',
    ]);
  });

  it('coincide en cualquier posición, no solo al principio', () => {
    // Es la diferencia a propósito con la pantalla de Vender: aquí uno se
    // acuerda de «cola», no de cómo empieza el nombre.
    expect(filtrarProductos(productos, 'cola').map((p) => p.sku)).toEqual(['REF-010']);
  });

  it('ignora los acentos en los dos lados de la comparación', () => {
    expect(filtrarProductos(productos, 'azucar').map((p) => p.sku)).toEqual(['AZU-001']);
    expect(filtrarProductos(productos, 'AZÚCAR').map((p) => p.sku)).toEqual(['AZU-001']);
  });

  it('devuelve varios cuando varios coinciden, en el orden original', () => {
    // «ace» está en el nombre de uno y en el código de otro: el filtro no
    // reordena nada, deja la lista como venía.
    expect(filtrarProductos(productos, 'ref').map((p) => p.sku)).toEqual(['AZU-001', 'REF-010']);
  });

  it('no inventa coincidencias: «a» está en los cuatro', () => {
    // Hasta en «Refresco de colA». Una letra sola no filtra nada, y está
    // bien que no lo haga: el filtro no adivina, busca.
    expect(filtrarProductos(productos, 'a').length).toBe(4);
  });

  it('devuelve la lista vacía cuando nada coincide', () => {
    expect(filtrarProductos(productos, 'bicicleta')).toEqual([]);
  });

  it('no revienta con la lista vacía', () => {
    expect(filtrarProductos([], 'arroz')).toEqual([]);
  });
});
