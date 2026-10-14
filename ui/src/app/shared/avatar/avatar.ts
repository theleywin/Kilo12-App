import { ChangeDetectionStrategy, Component, input } from '@angular/core';

/**
 * El avatar de una cuenta de consumo: la figura genérica de una persona.
 *
 * Es deliberadamente neutro. Se intentaron retratos dibujados —con pelo,
 * con rasgos— y a 46 píxeles, que es el tamaño real en la tarjeta, no se
 * leían: o quedaban manchas, o la silueta se parecía más a una capucha que
 * a una persona. La figura de siempre se reconoce al instante a cualquier
 * tamaño, y eso es todo lo que se le pide: decir «aquí hay alguien» para
 * que el ojo encuentre la tarjeta antes de leer el nombre.
 *
 * Va rellena y no a trazo por la misma razón: un contorno fino se deshace
 * en pequeño. Los iconos del menú sí van a trazo porque son dos o tres
 * líneas a 20 px.
 *
 * Hereda el color del texto (`currentColor`), así que quien lo use decide
 * la paleta y el avatar no sabe nada de ella.
 */
@Component({
  selector: 'app-avatar',
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <svg viewBox="0 0 40 40" fill="currentColor" role="img" [attr.aria-label]="nombre()">
      <circle cx="20" cy="15" r="6.5" />
      <path d="M7.5 34a12.5 12.5 0 0 1 25 0Z" />
    </svg>
  `,
  styles: `
    :host {
      display: grid;
      place-items: center;
      overflow: hidden;
    }

    svg {
      width: 100%;
      height: 100%;
    }
  `,
})
export class Avatar {
  /** De quién es, y lo que lee un lector de pantalla. */
  readonly nombre = input.required<string>();
}
