import { Routes } from '@angular/router';

import { Productos } from './features/productos/productos';
import { Guia } from './features/guia/guia';
import { Pendiente } from './features/pendiente/pendiente';
import { GUIA, SECCIONES } from './layout/navegacion';

/**
 * Las rutas salen de la tabla de secciones, no de una lista paralela.
 *
 * Agregar una pantalla es agregar una fila en `navegacion.ts`: el menú,
 * el atajo de teclado y la ruta aparecen solos. Una lista duplicada es
 * una lista que tarde o temprano se desincroniza.
 */
export const routes: Routes = [
  { path: '', pathMatch: 'full', redirectTo: SECCIONES[0].ruta },

  // Productos ya tiene pantalla: es el recorrido que funciona de punta a
  // punta contra la base de datos.
  { path: 'productos', component: Productos, title: 'Productos · Kilo12' },

  { path: GUIA.ruta, component: Guia, title: `${GUIA.titulo} · Kilo12` },

  ...SECCIONES.filter((seccion) => seccion.ruta !== 'productos').map((seccion) => ({
    path: seccion.ruta,
    component: Pendiente,
    title: `${seccion.titulo} · Kilo12`,
    data: { seccion },
  })),

  { path: '**', redirectTo: SECCIONES[0].ruta },
];
