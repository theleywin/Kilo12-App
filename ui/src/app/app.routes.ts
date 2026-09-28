import { Routes } from '@angular/router';

import { Almacen } from './features/almacen/almacen';
import { Caja } from './features/caja/caja';
import { Contar } from './features/contar/contar';
import { Informes } from './features/informes/informes';
import { Vender } from './features/vender/vender';
import { Entrada } from './features/entrada/entrada';
import { Ventas } from './features/ventas/ventas';
import { Vitrina } from './features/vitrina/vitrina';
import { Guia } from './features/guia/guia';
import { ProductoDetalle } from './features/productos/detalle/detalle';
import { Productos } from './features/productos/productos';
import { GUIA, SECCIONES } from './layout/navegacion';

/**
 * Una ruta por pantalla.
 *
 * El menú y los atajos de teclado salen de la tabla de `navegacion.ts`;
 * aquí solo se dice qué componente dibuja cada sección. Todas las
 * secciones tienen ya su pantalla, así que el marcador de «pantalla que
 * todavía no existe» se retiró: una sección nueva lleva su fila allí y su
 * ruta aquí.
 */
export const routes: Routes = [
  { path: '', pathMatch: 'full', redirectTo: SECCIONES[0].ruta },

  { path: 'productos', component: Productos, title: 'Productos · Kilo12' },
  { path: 'productos/:id', component: ProductoDetalle, title: 'Producto · Kilo12' },
  { path: 'vender', component: Vender, title: 'Vender · Kilo12' },
  { path: 'entrada', component: Entrada, title: 'Entrada · Kilo12' },
  { path: 'almacen', component: Almacen, title: 'Almacén · Kilo12' },
  { path: 'vitrina', component: Vitrina, title: 'Vitrina · Kilo12' },
  { path: 'ventas', component: Ventas, title: 'Ventas · Kilo12' },
  { path: 'caja', component: Caja, title: 'Caja · Kilo12' },
  { path: 'contar', component: Contar, title: 'Contar dinero · Kilo12' },
  { path: 'informes', component: Informes, title: 'Informes · Kilo12' },

  { path: GUIA.ruta, component: Guia, title: `${GUIA.titulo} · Kilo12` },

  { path: '**', redirectTo: SECCIONES[0].ruta },
];
