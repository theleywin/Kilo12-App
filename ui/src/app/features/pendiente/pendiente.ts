import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { ActivatedRoute } from '@angular/router';

import { Seccion } from '../../layout/navegacion';

/**
 * Pantalla que todavía no existe.
 *
 * No dice «en construcción» y se queda tan ancha: dice exactamente qué va
 * a vivir aquí. Así el armazón se puede recorrer entero desde el primer
 * día y se ve si el reparto de secciones tiene sentido ANTES de escribir
 * las pantallas.
 */
@Component({
  selector: 'app-pendiente',
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './pendiente.html',
  styleUrl: './pendiente.css',
})
export class Pendiente {
  private readonly ruta = inject(ActivatedRoute);

  /**
   * La sección llega por la configuración de la ruta.
   *
   * Se lee una sola vez: cada sección es una ruta distinta, así que el
   * componente se vuelve a crear al cambiar de pantalla.
   */
  protected readonly seccion = this.ruta.snapshot.data['seccion'] as Seccion;
}
