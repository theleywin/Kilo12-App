import { Routes } from '@angular/router';

import { Almacen } from './features/almacen/almacen';
import { Entrada } from './features/entrada/entrada';
import { Vitrina } from './features/vitrina/vitrina';
import { Guia } from './features/guia/guia';
import { Pendiente } from './features/pendiente/pendiente';
import { Productos } from './features/productos/productos';
import { GUIA, SECCIONES } from './layout/navegacion';

/** Secciones que ya tienen pantalla propia. */
const CONSTRUIDAS = ['productos', 'almacen', 'vitrina', 'entrada'];

/**
 * Las rutas salen de la tabla de secciones, no de una lista paralela.
 *
 * Agregar una pantalla es agregar una fila en `navegacion.ts`: el menú,
 * el atajo de teclado y la ruta aparecen solos. Una lista duplicada es
 * una lista que tarde o temprano se desincroniza.
 */
export const routes: Routes = [
  { path: '', pathMatch: 'full', redirectTo: SECCIONES[0].ruta },

  { path: 'productos', component: Productos, title: 'Productos · Kilo12' },
  { path: 'entrada', component: Entrada, title: 'Entrada · Kilo12' },
  { path: 'almacen', component: Almacen, title: 'Almacén · Kilo12' },
  { path: 'vitrina', component: Vitrina, title: 'Vitrina · Kilo12' },

  { path: GUIA.ruta, component: Guia, title: `${GUIA.titulo} · Kilo12` },

  ...SECCIONES.filter((seccion) => !CONSTRUIDAS.includes(seccion.ruta)).map((seccion) => ({
    path: seccion.ruta,
    component: Pendiente,
    title: `${seccion.titulo} · Kilo12`,
    data: { seccion },
  })),

  { path: '**', redirectTo: SECCIONES[0].ruta },
];
