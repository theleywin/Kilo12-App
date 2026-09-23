import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { debounceTime, startWith } from 'rxjs';

import {
  ErrorDto,
  MargenDto,
  ProductoDto,
  UNIDAD_SUELTA,
  UNIDADES_GRANEL,
} from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';

/** Número decimal con hasta seis decimales. Vacío se admite aparte. */
const DECIMAL = /^\d+(\.\d{1,6})?$/;

/**
 * Catálogo de productos: alta y listado.
 *
 * El formulario pregunta en el orden en que piensa quien vende: qué es,
 * cómo se vende, qué cuesta, a cuánto se vende y cuánto tienes. El SKU, la
 * unidad base y las fracciones son consecuencia de esas respuestas, no
 * preguntas propias.
 */
@Component({
  selector: 'app-productos',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [ReactiveFormsModule],
  templateUrl: './productos.html',
  styleUrl: './productos.css',
})
export class Productos {
  private readonly api = inject(Kilo12Api);
  private readonly fb = inject(FormBuilder);

  protected readonly unidades = UNIDADES_GRANEL;
  protected readonly productos = signal<ProductoDto[]>([]);
  protected readonly error = signal<ErrorDto | null>(null);
  protected readonly cargando = signal(false);
  protected readonly guardando = signal(false);
  /** Ganancia y margen del precio que se está escribiendo. */
  protected readonly margen = signal<MargenDto | null>(null);

  protected readonly formulario = this.fb.nonNullable.group({
    nombre: ['', Validators.required],
    // Vacío = se deriva del nombre (RF-CAT-02).
    sku: [''],
    // Primero la pregunta que entiende cualquiera; la unidad es el detalle.
    porPeso: [false],
    unidadBase: ['lb'],
    costo: ['', Validators.pattern(DECIMAL)],
    precio: ['', [Validators.required, Validators.pattern(DECIMAL)]],
    cantidadAlmacen: ['', Validators.pattern(DECIMAL)],
    cantidadVitrina: ['', Validators.pattern(DECIMAL)],
    stockMinimo: ['', Validators.pattern(DECIMAL)],
    objetivoVitrina: ['', Validators.pattern(DECIMAL)],
  });

  /** Símbolo de la unidad elegida, para acompañar a las cantidades. */
  protected readonly simbolo = signal('u');

  protected readonly hayMercancia = signal(false);

  constructor() {
    void this.recargar();

    // El margen lo calcula Rust mientras se escribe. Se espera un momento
    // para no disparar una llamada por cada tecla.
    const costo = this.formulario.controls.costo;
    const precio = this.formulario.controls.precio;

    this.formulario.valueChanges
      .pipe(startWith(null), debounceTime(200), takeUntilDestroyed())
      .subscribe(() => {
        this.sincronizarUnidad();
        this.actualizarMercancia();
        void this.actualizarMargen(costo.value.trim(), precio.value.trim());
      });
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

  protected async guardar(): Promise<void> {
    if (this.formulario.invalid) {
      this.formulario.markAllAsTouched();
      return;
    }

    this.guardando.set(true);
    const valores = this.formulario.getRawValue();

    try {
      await this.api.registrarProducto({
        sku: valores.sku.trim(),
        nombre: valores.nombre.trim(),
        unidadBase: valores.porPeso ? valores.unidadBase : UNIDAD_SUELTA,
        precioUnitario: valores.precio.trim(),
        costoUnitario: valores.costo.trim() || undefined,
        cantidadAlmacen: valores.cantidadAlmacen.trim() || undefined,
        cantidadVitrina: valores.cantidadVitrina.trim() || undefined,
        stockMinimo: valores.stockMinimo.trim() || undefined,
        objetivoVitrina: valores.objetivoVitrina.trim() || undefined,
      });

      this.error.set(null);
      this.formulario.reset({ porPeso: valores.porPeso, unidadBase: valores.unidadBase });
      await this.recargar();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.guardando.set(false);
    }
  }

  protected elegirForma(porPeso: boolean): void {
    this.formulario.controls.porPeso.setValue(porPeso);
  }

  protected elegirUnidad(valor: string): void {
    this.formulario.controls.unidadBase.setValue(valor);
  }

  protected get porPeso(): boolean {
    return this.formulario.controls.porPeso.value;
  }

  protected get unidadElegida(): string {
    return this.formulario.controls.unidadBase.value;
  }

  private sincronizarUnidad(): void {
    const porPeso = this.formulario.controls.porPeso.value;
    this.simbolo.set(porPeso ? this.formulario.controls.unidadBase.value : 'u');
  }

  private actualizarMercancia(): void {
    const { cantidadAlmacen, cantidadVitrina } = this.formulario.getRawValue();
    this.hayMercancia.set(Boolean(cantidadAlmacen.trim() || cantidadVitrina.trim()));
  }

  /**
   * Pide a Rust la ganancia del precio frente al costo.
   *
   * Si falta alguno de los dos, no hay margen que enseñar: es mejor no
   * mostrar nada que mostrar un número inventado.
   */
  private async actualizarMargen(costo: string, precio: string): Promise<void> {
    if (!DECIMAL.test(costo) || !DECIMAL.test(precio)) {
      this.margen.set(null);
      return;
    }

    try {
      this.margen.set(await this.api.calcularMargen(costo, precio));
    } catch {
      // Un margen que no se puede calcular simplemente no se muestra: no
      // es un error que merezca interrumpir a quien está escribiendo.
      this.margen.set(null);
    }
  }
}
