import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { RouterLink } from '@angular/router';

import { ErrorDto, ProductoDto } from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';

/**
 * El catálogo: qué vendes, a cuánto lo compras y a cuánto lo vendes.
 *
 * Esta pantalla **no crea productos**, y es deliberado. Un producto nace
 * cuando llega su mercancía, y eso ocurre en Almacén. Tener dos formularios
 * de alta en dos pantallas distintas es la forma más segura de que dentro
 * de tres meses no hagan lo mismo.
 *
 * Aquí se mira el negocio: qué deja ganancia, qué se vende perdiendo, qué
 * está por agotarse.
 */
@Component({
  selector: 'app-productos',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [RouterLink],
  templateUrl: './productos.html',
  styleUrl: './productos.css',
})
export class Productos {
  private readonly api = inject(Kilo12Api);

  protected readonly productos = signal<ProductoDto[]>([]);
  protected readonly error = signal<ErrorDto | null>(null);
  protected readonly cargando = signal(false);

  constructor() {
    void this.recargar();
  }

  protected async recargar(): Promise<void> {
    this.cargando.set(true);
    try {
      this.productos.set(await this.api.listarProductos());
      this.error.set(null);
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.cargando.set(false);
    }
  }

  /** Productos cuyo costo se comió el precio (RF-COM-05). */
  protected get enRiesgo(): number {
    return this.productos().filter((producto) => producto.enRiesgo).length;
  }
}
