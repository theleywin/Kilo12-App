import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { Router, RouterLink } from '@angular/router';

import { ErrorDto, ProductoDto } from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';

/**
 * Quita acentos y mayúsculas para que buscar «azucar» encuentre «Azúcar
 * morena» (RF-CAT-09).
 */
function plegar(texto: string): string {
  return texto
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .trim();
}

/** Por qué columna se ordena. Los nombres los entiende Rust. */
type Orden = 'nombre' | 'costo' | 'margen' | 'precio' | 'existencia';

/**
 * Las columnas que se pueden pedir.
 *
 * El nombre no está: el catálogo ya llega alfabético y ordenarlo así no
 * responde ninguna pregunta del negocio. Las cuatro que quedan sí: qué me
 * cuesta más, qué me deja menos, qué vendo más caro y de qué tengo más.
 */
const ORDENES: ReadonlyArray<{ valor: Orden; nombre: string }> = [
  { valor: 'costo', nombre: 'Costo' },
  { valor: 'margen', nombre: 'Margen' },
  { valor: 'precio', nombre: 'Precio' },
  { valor: 'existencia', nombre: 'Existencia' },
];

/**
 * Catálogo: la lista desde la que se llega a cada producto.
 *
 * Es una tabla y no un mosaico de tarjetas a propósito. Una de las razones
 * de esta pantalla es cazar el producto que pierde dinero, y para eso hace
 * falta poder recorrer una columna de márgenes con la vista. Las tarjetas
 * se ven mejor y se comparan peor.
 *
 * El orden lo decide Rust: comparar márgenes es comparar dinero.
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
  private readonly router = inject(Router);

  protected readonly ordenes = ORDENES;
  protected readonly productos = signal<readonly ProductoDto[]>([]);
  protected readonly error = signal<ErrorDto | null>(null);
  protected readonly cargando = signal(false);

  protected readonly busqueda = signal('');
  protected readonly orden = signal<Orden>('nombre');
  protected readonly descendente = signal(false);
  /** Los retirados de la venta se esconden salvo que se pidan. */
  protected readonly incluirInactivos = signal(false);

  /** Lo que queda tras filtrar por lo escrito (RF-CAT-09). */
  protected readonly visibles = computed(() => {
    const aguja = plegar(this.busqueda());
    if (!aguja) {
      return this.productos();
    }
    // Empieza por, no contiene: escribir «ar» tiene que traer el arroz,
    // no todo lo que lleve esas dos letras en medio.
    return this.productos().filter(
      (producto) =>
        plegar(producto.nombre).startsWith(aguja) || plegar(producto.sku).startsWith(aguja),
    );
  });

  protected readonly enRiesgo = computed(
    () => this.productos().filter((producto) => producto.enRiesgo).length,
  );

  constructor() {
    void this.recargar();
  }

  protected async recargar(): Promise<void> {
    this.cargando.set(true);
    try {
      this.productos.set(
        await this.api.listarProductos(this.incluirInactivos(), this.orden(), this.descendente()),
      );
      this.error.set(null);
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.cargando.set(false);
    }
  }

  /** Hay un orden puesto por el usuario, distinto del natural. */
  protected readonly hayOrden = computed(() => this.orden() !== 'nombre');

  /**
   * Cambia el orden, en tres pasos sobre la misma columna.
   *
   * Primer clic ordena, el segundo lo invierte y el tercero lo quita. El
   * tercero es el que faltaba: un interruptor que se enciende y no se
   * apaga obliga a recargar la pantalla para volver atrás.
   */
  protected async ordenarPor(orden: Orden): Promise<void> {
    if (this.orden() !== orden) {
      this.orden.set(orden);
      this.descendente.set(false);
    } else if (!this.descendente()) {
      this.descendente.set(true);
    } else {
      await this.quitarOrden();
      return;
    }

    await this.recargar();
  }

  /** Vuelve al orden natural del catálogo, que es alfabético. */
  protected async quitarOrden(): Promise<void> {
    this.orden.set('nombre');
    this.descendente.set(false);
    await this.recargar();
  }

  protected limpiarBusqueda(): void {
    this.busqueda.set('');
  }

  protected async alternarInactivos(): Promise<void> {
    this.incluirInactivos.set(!this.incluirInactivos());
    await this.recargar();
  }

  protected abrir(producto: ProductoDto): void {
    void this.router.navigate(['/productos', producto.id]);
  }
}
