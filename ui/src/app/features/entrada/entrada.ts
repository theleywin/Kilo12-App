import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { debounceTime } from 'rxjs';

import {
  ErrorDto,
  MargenDto,
  MovimientoDto,
  ProductoDto,
  UBICACIONES,
  UNIDAD_SUELTA,
  UNIDADES_GRANEL,
} from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';

/** Número decimal con hasta seis decimales. */
const DECIMAL = /^\d+(\.\d{1,6})?$/;

/**
 * Quita acentos y mayúsculas para que buscar «cafe» encuentre «Café».
 *
 * Solo sirve para BUSCAR en pantalla. La regla que genera el código del
 * producto vive en Rust y no se duplica aquí.
 */
function plegar(texto: string): string {
  return texto
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .trim();
}

/** Una cosa que acaba de entrar, para poder revisarla de un vistazo. */
interface Recibido {
  readonly nombre: string;
  readonly movimiento: MovimientoDto;
  readonly esNuevo: boolean;
}

/**
 * Entrada: llegó mercancía.
 *
 * Se registra una cosa detrás de otra, según van entrando por la puerta.
 * Todo está pensado para repetir rápido: el foco vuelve al buscador, la
 * ubicación se conserva y lo ya registrado se queda a la vista para poder
 * detectar un error sin irse a otra pantalla.
 *
 * Si el producto es la primera vez que llega, se crea aquí mismo: nadie
 * «da de alta un artículo», recibe cajas.
 */
@Component({
  selector: 'app-entrada',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [ReactiveFormsModule],
  templateUrl: './entrada.html',
  styleUrl: './entrada.css',
})
export class Entrada {
  private readonly api = inject(Kilo12Api);
  private readonly fb = inject(FormBuilder);

  protected readonly ubicaciones = UBICACIONES;
  protected readonly unidades = UNIDADES_GRANEL;

  protected readonly productos = signal<readonly ProductoDto[]>([]);
  protected readonly error = signal<ErrorDto | null>(null);
  protected readonly guardando = signal(false);
  protected readonly margen = signal<MargenDto | null>(null);

  /** Lo registrado en esta sesión, de lo más reciente a lo más antiguo. */
  protected readonly recibido = signal<readonly Recibido[]>([]);

  protected readonly busqueda = signal('');
  protected readonly elegido = signal<ProductoDto | null>(null);
  protected readonly creando = signal(false);

  protected readonly formulario = this.fb.nonNullable.group({
    cantidad: ['', [Validators.required, Validators.pattern(DECIMAL)]],
    costoUnitario: ['', [Validators.required, Validators.pattern(DECIMAL)]],
    ubicacion: ['BODEGA', Validators.required],
    // Solo cuando el producto es nuevo.
    porPeso: [false],
    unidadBase: ['lb'],
    precio: [''],
  });

  protected readonly coincidencias = computed(() => {
    const aguja = plegar(this.busqueda());
    if (!aguja) {
      return this.productos().slice(0, 8);
    }
    return this.productos()
      .filter((p) => plegar(p.nombre).includes(aguja) || plegar(p.sku).includes(aguja))
      .slice(0, 8);
  });

  protected readonly puedeCrear = computed(() => {
    const aguja = plegar(this.busqueda());
    return Boolean(aguja) && !this.productos().some((p) => plegar(p.nombre) === aguja);
  });

  constructor() {
    void this.recargar();

    this.formulario.valueChanges.pipe(debounceTime(200), takeUntilDestroyed()).subscribe(() => {
      void this.actualizarMargen();
    });
  }

  protected async recargar(): Promise<void> {
    try {
      this.productos.set(await this.api.listarProductos());
    } catch (fallo) {
      this.error.set(comoError(fallo));
    }
  }

  // ------------------------------------------------ elegir producto

  protected alEscribir(valor: string): void {
    this.busqueda.set(valor);
    this.elegido.set(null);
    this.creando.set(false);
  }

  protected elegirProducto(producto: ProductoDto): void {
    this.elegido.set(producto);
    this.creando.set(false);
    this.busqueda.set(producto.nombre);
  }

  protected crearProducto(): void {
    this.elegido.set(null);
    this.creando.set(true);
    this.formulario.controls.precio.setValidators([
      Validators.required,
      Validators.pattern(DECIMAL),
    ]);
    this.formulario.controls.precio.updateValueAndValidity();
  }

  protected empezarDeNuevo(): void {
    this.busqueda.set('');
    this.elegido.set(null);
    this.creando.set(false);
    this.margen.set(null);
    this.formulario.controls.precio.clearValidators();
    this.formulario.controls.precio.updateValueAndValidity();
    this.enfocarBuscador();
  }

  protected get hayProducto(): boolean {
    return this.elegido() !== null || this.creando();
  }

  protected get simbolo(): string {
    const existente = this.elegido();
    if (existente) {
      return existente.unidadBase;
    }
    return this.formulario.controls.porPeso.value
      ? this.formulario.controls.unidadBase.value
      : 'u';
  }

  protected elegirForma(porPeso: boolean): void {
    this.formulario.controls.porPeso.setValue(porPeso);
  }

  protected elegirUnidad(valor: string): void {
    this.formulario.controls.unidadBase.setValue(valor);
  }

  protected elegirUbicacion(valor: string): void {
    this.formulario.controls.ubicacion.setValue(valor);
  }

  // ------------------------------------------------------ registrar

  protected async registrar(): Promise<void> {
    if (this.formulario.invalid || !this.hayProducto) {
      this.formulario.markAllAsTouched();
      return;
    }

    this.guardando.set(true);
    const valores = this.formulario.getRawValue();
    const enVitrina = valores.ubicacion === 'VITRINA';
    const esNuevo = this.creando();
    const nombre = esNuevo ? this.busqueda().trim() : this.elegido()!.nombre;

    try {
      if (esNuevo) {
        await this.api.registrarProducto({
          sku: '',
          nombre,
          unidadBase: valores.porPeso ? valores.unidadBase : UNIDAD_SUELTA,
          precioUnitario: valores.precio.trim(),
          costoUnitario: valores.costoUnitario.trim(),
          cantidadAlmacen: enVitrina ? undefined : valores.cantidad.trim(),
          cantidadVitrina: enVitrina ? valores.cantidad.trim() : undefined,
        });
      } else {
        await this.api.registrarEntrada({
          producto: this.elegido()!.id,
          cantidad: valores.cantidad.trim(),
          costoUnitario: valores.costoUnitario.trim(),
          destino: valores.ubicacion,
        });
      }

      this.error.set(null);
      await this.recargar();
      await this.anotarRecibido(nombre, esNuevo);

      // Listo para lo siguiente: se conserva la ubicación porque una
      // remesa suele ir entera al mismo sitio.
      this.formulario.patchValue({ cantidad: '', costoUnitario: '', precio: '' });
      this.formulario.markAsUntouched();
      this.margen.set(null);
      this.busqueda.set('');
      this.elegido.set(null);
      this.creando.set(false);
      this.formulario.controls.precio.clearValidators();
      this.formulario.controls.precio.updateValueAndValidity();
      this.enfocarBuscador();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.guardando.set(false);
    }
  }

  /**
   * Anota en la lista de la sesión lo que se acaba de registrar.
   *
   * Se lee del historial del producto en vez de recomponerlo aquí: así las
   * cifras son exactamente las que quedaron guardadas, no una copia
   * calculada por otro camino.
   */
  private async anotarRecibido(nombre: string, esNuevo: boolean): Promise<void> {
    const producto = this.productos().find((p) => plegar(p.nombre) === plegar(nombre));
    if (!producto) {
      return;
    }

    try {
      const [ultimo] = await this.api.consultarKardex(producto.id, 1);
      if (ultimo) {
        this.recibido.set([{ nombre: producto.nombre, movimiento: ultimo, esNuevo }, ...this.recibido()]);
      }
    } catch {
      // Que no se pueda pintar el resumen no invalida la entrada, que ya
      // quedó registrada.
    }
  }

  private enfocarBuscador(): void {
    // El foco vuelve al principio para poder encadenar entradas sin ratón.
    queueMicrotask(() => document.getElementById('buscador')?.focus());
  }

  private async actualizarMargen(): Promise<void> {
    if (!this.creando()) {
      this.margen.set(null);
      return;
    }

    const { costoUnitario, precio } = this.formulario.getRawValue();
    const costo = costoUnitario.trim();
    const venta = precio.trim();

    if (!DECIMAL.test(costo) || !DECIMAL.test(venta)) {
      this.margen.set(null);
      return;
    }

    try {
      this.margen.set(await this.api.calcularMargen(costo, venta));
    } catch {
      this.margen.set(null);
    }
  }
}
