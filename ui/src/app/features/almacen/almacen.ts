import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';

import { AlmacenDto, ErrorDto, MovimientoDto, ProductoDto } from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';

/** Número decimal con hasta seis decimales. */
const DECIMAL = /^\d+(\.\d{1,6})?$/;

/**
 * Almacén: qué hay guardado, cuánto vale y por qué.
 *
 * Es una pantalla de consulta, no de captura. La mercancía entra en
 * Entrada y se exhibe desde Vitrina; aquí se mira lo guardado, se da de
 * baja lo que se perdió y se revisa el historial de cada producto.
 *
 * La merma no pregunta de dónde sale: estás mirando el almacén.
 */
@Component({
  selector: 'app-almacen',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [ReactiveFormsModule],
  templateUrl: './almacen.html',
  styleUrl: './almacen.css',
})
export class Almacen {
  private readonly api = inject(Kilo12Api);
  private readonly fb = inject(FormBuilder);

  protected readonly almacen = signal<AlmacenDto | null>(null);
  protected readonly error = signal<ErrorDto | null>(null);
  protected readonly cargando = signal(false);
  protected readonly ocupado = signal(false);

  protected readonly seleccionado = signal<ProductoDto | null>(null);
  protected readonly kardex = signal<MovimientoDto[]>([]);
  protected readonly cargandoKardex = signal(false);

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
      const almacen = await this.api.consultarAlmacen();
      this.almacen.set(almacen);
      this.error.set(null);

      const actual = this.seleccionado();
      if (actual) {
        this.seleccionado.set(almacen.productos.find((p) => p.id === actual.id) ?? null);
      }
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.cargando.set(false);
    }
  }

  protected async verHistorial(producto: ProductoDto): Promise<void> {
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
    this.formularioMerma.reset();
  }

  /** Da de baja mercancía perdida del almacén. */
  protected async mermar(): Promise<void> {
    const producto = this.seleccionado();
    if (!producto || this.formularioMerma.invalid) {
      this.formularioMerma.markAllAsTouched();
      return;
    }

    this.ocupado.set(true);
    const { cantidad, motivo } = this.formularioMerma.getRawValue();

    try {
      // La ubicación no se pregunta: estás mirando el almacén.
      await this.api.registrarMerma({
        producto: producto.id,
        cantidad: cantidad.trim(),
        origen: 'BODEGA',
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
}
