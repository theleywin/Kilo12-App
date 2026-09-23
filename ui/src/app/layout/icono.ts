import { ChangeDetectionStrategy, Component, input } from '@angular/core';

import { Icono as NombreIcono } from './navegacion';

/**
 * Los iconos del menú, dibujados a mano en SVG.
 *
 * No se usa ninguna biblioteca de iconos: la aplicación funciona sin
 * internet y la política de seguridad de Tauri bloquea cualquier descarga
 * externa. Ocho trazos propios pesan menos que una fuente de iconos con
 * mil símbolos que nunca se van a usar.
 *
 * Heredan el color del texto (`currentColor`), así que el menú los pinta
 * sin que el icono sepa nada de la paleta.
 */
@Component({
  selector: 'app-icono',
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <svg
      width="20"
      height="20"
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      stroke-width="1.6"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      @switch (nombre()) {
        @case ('vender') {
          <!-- La bolsa del logo. -->
          <path d="M4 7h12l-1.2 9.5H5.2L4 7z" />
          <path d="M7.6 7V5.6a2.4 2.4 0 0 1 4.8 0V7" />
        }
        @case ('vitrina') {
          <rect x="2.8" y="4" width="14.4" height="12.6" rx="1.4" />
          <path d="M2.8 8.4h14.4M2.8 12.6h14.4" />
        }
        @case ('almacen') {
          <path d="M3 6.6 10 3.4l7 3.2v7.2l-7 3.2-7-3.2V6.6z" />
          <path d="m3 6.6 7 3.2 7-3.2M10 9.8V17" />
        }
        @case ('productos') {
          <path d="M10.4 3H17v6.6l-7.7 7.7-6.6-6.6L10.4 3z" />
          <circle cx="13.6" cy="6.4" r="1.1" />
        }
        @case ('ventas') {
          <path d="M5 3.2h10v14.2l-2.5-1.5-2.5 1.5-2.5-1.5L5 17.4V3.2z" />
          <path d="M7.8 7h4.4M7.8 10.4h4.4" />
        }
        @case ('caja') {
          <rect x="2.6" y="5.6" width="14.8" height="8.8" rx="1.4" />
          <circle cx="10" cy="10" r="2.1" />
          <path d="M5.4 10h.01M14.6 10h.01" />
        }
        @case ('informes') {
          <path d="M3.4 16.6h13.2" />
          <path d="M6.2 16.6V11M10 16.6V5.4M13.8 16.6v-7.8" />
        }
        @case ('guia') {
          <circle cx="7.6" cy="7.6" r="3.6" />
          <circle cx="12.4" cy="12.4" r="3.6" />
        }
      }
    </svg>
  `,
  styles: `
    :host {
      display: inline-flex;
      flex: none;
    }
  `,
})
export class Icono {
  readonly nombre = input.required<NombreIcono>();
}
