import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';

import { ErrorDto, LineaVitrinaDto, MovimientoDto, VitrinaDto } from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';

/** Número decimal con hasta seis decimales. */
const DECIMAL = /^\d+(\.\d{1,6})?$/;

/**
 * Vitrina: lo que el cliente ve.
 *
 * La pantalla existe por una frase del documento que vale todo el módulo:
 * *existencia total alta con vitrina vacía es venta perdida*. Tener cuarenta
 * libras en la bodega no sirve de nada si quien entra no las ve.
 *
 * Por eso lo primero que se muestra no es lo que hay, sino **lo que falta
 * por bajar**.
 */
@Component({
  selector: 'app-vitrina',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [ReactiveFormsModule],
  templateUrl: './vitrina.html',
  styleUrl: './vitrina.css',
})
export class Vitrina {
  private readonly api = inject(Kilo12Api);
  private readonly fb = inject(FormBuilder);

  protected readonly vitrina = signal<VitrinaDto | null>(null);
  protected readonly error = signal<ErrorDto | null>(null);
  protected readonly cargando = signal(false);
  protected readonly ocupado = signal(false);

  /** Producto cuyo historial se está mirando. */
  protected readonly seleccionado = signal<LineaVitrinaDto | null>(null);
  protected readonly kardex = signal<MovimientoDto[]>([]);
  protected readonly cargandoKardex = signal(false);

  /**
   * Cuánto se va a bajar de cada producto.
   *
   * Arranca con la sugerencia, pero se puede ajustar: el sistema propone,
   * no manda (RF-VIT-05).
   */
  protected readonly aBajar = signal<Record<number, string>>({});

  /**
   * Cuánto se devuelve al almacén.
   *
   * Tiene su propia casilla y no reutiliza la de reposición: son dos
   * cantidades distintas que van en direcciones opuestas, y confundirlas
   * era exactamente el error que tenía esta pantalla.
   */
  protected readonly aDevolver = signal('');

  /**
   * Cuánto se trae del almacén a la vitrina.
   *
   * Tiene su propia casilla, como la de devolver: son dos cantidades que
   * van en direcciones opuestas y compartirlas era justo el error que esta
   * pantalla ya tuvo una vez.
   */
  protected readonly aTraer = signal('');

  protected readonly formularioMerma = this.fb.nonNullable.group({
    cantidad: ['', [Validators.required, Validators.pattern(DECIMAL)]],
    motivo: ['', Validators.required],
  });

  constructor() {
    void this.recargar();
  }

  protected async recargar(): Promise<void> {
    this.cargando.set(true);
    try {
      const vitrina = await this.api.consultarVitrina();
      this.vitrina.set(vitrina);
      this.error.set(null);

      // Las casillas de reposición se rellenan con lo que el dominio
      // sugiere, sin pisar lo que el usuario haya escrito a mano.
      const propuesto: Record<number, string> = { ...this.aBajar() };
      for (const producto of vitrina.productos) {
        if (producto.hayQueReponer && !propuesto[producto.id]) {
          propuesto[producto.id] = producto.sugerido;
        }
      }
      this.aBajar.set(propuesto);

      const actual = this.seleccionado();
      if (actual) {
        this.seleccionado.set(vitrina.productos.find((p) => p.id === actual.id) ?? null);
      }
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.cargando.set(false);
    }
  }

  protected get sugerencias(): readonly LineaVitrinaDto[] {
    return (this.vitrina()?.productos ?? []).filter((p) => p.hayQueReponer);
  }

  protected cantidadABajar(producto: LineaVitrinaDto): string {
    return this.aBajar()[producto.id] ?? producto.sugerido;
  }

  protected alCambiarCantidad(producto: LineaVitrinaDto, valor: string): void {
    this.aBajar.set({ ...this.aBajar(), [producto.id]: valor });
  }

  /** Baja mercancía del almacén a la vitrina (RF-VIT-01). */
  protected async reponer(producto: LineaVitrinaDto): Promise<void> {
    const cantidad = this.cantidadABajar(producto).trim();
    if (!DECIMAL.test(cantidad)) {
      return;
    }

    this.ocupado.set(true);
    try {
      await this.api.traspasar({ producto: producto.id, cantidad, origen: 'BODEGA' });
      this.error.set(null);

      // La casilla se vacía para que la próxima recarga vuelva a proponer
      // lo que corresponda.
      const restante = { ...this.aBajar() };
      delete restante[producto.id];
      this.aBajar.set(restante);

      await this.recargar();
      if (this.seleccionado()?.id === producto.id) {
        await this.verHistorial(producto);
      }
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  /** Devuelve mercancía de la vitrina al almacén (RF-VIT-02). */
  protected async devolver(producto: LineaVitrinaDto): Promise<void> {
    const cantidad = this.aDevolver().trim();
    if (!DECIMAL.test(cantidad)) {
      return;
    }

    this.ocupado.set(true);
    try {
      await this.api.traspasar({ producto: producto.id, cantidad, origen: 'VITRINA' });
      this.error.set(null);
      this.aDevolver.set('');
      await this.recargar();
      await this.verHistorial(producto);
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  /**
   * Trae mercancía del almacén a la vitrina (RF-VIT-01).
   *
   * Es el movimiento que hace falta a diario y el que faltaba aquí: la
   * lista de reposición solo propone cuando hay un objetivo fijado, así que
   * sin objetivo no había forma de sacar nada del almacén desde esta
   * pantalla. Ahora está en la ficha del producto, que es donde se busca.
   */
  protected async traer(producto: LineaVitrinaDto): Promise<void> {
    const cantidad = this.aTraer().trim();
    if (!DECIMAL.test(cantidad)) {
      return;
    }

    this.ocupado.set(true);
    try {
      await this.api.traspasar({ producto: producto.id, cantidad, origen: 'BODEGA' });
      this.error.set(null);
      this.aTraer.set('');
      await this.recargar();
      await this.verHistorial(producto);
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  /** Rellena la casilla con todo lo que hay guardado. */
  protected traerTodo(producto: LineaVitrinaDto): void {
    this.aTraer.set(producto.enAlmacen);
  }

  /** Fija cuánto se quiere tener exhibido (RF-VIT-03). */
  protected async fijarObjetivo(producto: LineaVitrinaDto, valor: string): Promise<void> {
    const objetivo = valor.trim();
    if (objetivo === producto.objetivo) {
      return;
    }
    if (objetivo && !DECIMAL.test(objetivo)) {
      return;
    }

    this.ocupado.set(true);
    try {
      await this.api.fijarObjetivoVitrina(producto.id, objetivo);
      this.error.set(null);
      await this.recargar();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  /** Da de baja mercancía perdida de la vitrina. */
  protected async mermar(): Promise<void> {
    const producto = this.seleccionado();
    if (!producto || this.formularioMerma.invalid) {
      this.formularioMerma.markAllAsTouched();
      return;
    }

    this.ocupado.set(true);
    const { cantidad, motivo } = this.formularioMerma.getRawValue();

    try {
      // La ubicación no se pregunta: estás mirando la vitrina.
      await this.api.registrarMerma({
        producto: producto.id,
        cantidad: cantidad.trim(),
        origen: 'VITRINA',
        motivo: motivo.trim(),
      });

      this.error.set(null);
      this.formularioMerma.reset();
      await this.recargar();
      await this.verHistorial(producto);
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  protected async verHistorial(producto: LineaVitrinaDto): Promise<void> {
    this.seleccionado.set(producto);
    this.cargandoKardex.set(true);
    try {
      this.kardex.set(await this.api.consultarKardex(producto.id));
    } catch (fallo) {
      this.error.set(comoError(fallo));
      this.kardex.set([]);
    } finally {
      this.cargandoKardex.set(false);
    }
  }

  protected cerrarHistorial(): void {
    this.seleccionado.set(null);
    this.kardex.set([]);
    this.aDevolver.set('');
    this.aTraer.set('');
    this.formularioMerma.reset();
  }
}
