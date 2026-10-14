import {
  ChangeDetectionStrategy,
  Component,
  DestroyRef,
  computed,
  inject,
  input,
  OnInit,
  output,
  signal,
} from '@angular/core';

import {
  ConsumoPrevistoDto,
  CuentaEnDeudaDto,
  ErrorDto,
  LineaConsumoDto,
  ProductoDto,
} from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';
import { filtrarProductos } from '../buscar/buscar';

/** Número decimal con hasta seis decimales. */
const DECIMAL = /^\d+(\.\d{1,6})?$/;

/** Una forma de llevarse un producto, para el selector del carrito. */
interface FormaDeLlevarlo {
  readonly id: number;
  readonly nombre: string;
}

/** Un renglón del carrito, mientras se arma. */
interface EnElCarrito {
  readonly producto: number;
  readonly nombre: string;
  readonly formas: readonly FormaDeLlevarlo[];
  presentacion: number;
  cantidad: string;
  origen: string;
}

/**
 * El cartel de registrar un consumo: el carrito de los dueños.
 *
 * Vive fuera de `features/` porque lo usan DOS pantallas —el panel de
 * Consumo y el historial de una cuenta— y copiarlo sería garantizar que el
 * día que cambie una regla, una de las dos se quede vieja.
 *
 * Qué hace y qué no:
 *
 * - **Sabe** armar el consumo: busca productos, los añade uno detrás de
 *   otro, deja cambiar presentación, cantidad y de dónde sale, y le pide a
 *   Rust lo que sumaría **antes** de registrarlo.
 * - **No sabe** qué hay alrededor. No recarga paneles ni navega: avisa de
 *   que registró y quien lo use decide qué hacer con eso.
 *
 * Ninguna cifra se suma aquí. El costo es promedio ponderado y nadie lo
 * lleva en la cabeza: la suma y el saldo resultante los calcula el núcleo.
 */
@Component({
  selector: 'app-consumo-registrar',
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './consumo-registrar.html',
  styleUrl: './consumo-registrar.css',
})
export class ConsumoRegistrar implements OnInit {
  private readonly api = inject(Kilo12Api);

  /** A quién se le carga. */
  readonly cuenta = input.required<CuentaEnDeudaDto>();

  /** Se registró: lleva el identificador del consumo nuevo. */
  readonly registrado = output<number>();
  readonly cerrado = output<void>();
  readonly fallo = output<ErrorDto>();

  protected readonly tomadoPor = signal('');
  protected readonly busqueda = signal('');
  protected readonly origen = signal('VITRINA');
  protected readonly carrito = signal<readonly EnElCarrito[]>([]);
  /** Lo que sumaría, calculado por Rust mientras se arma. */
  protected readonly previsto = signal<ConsumoPrevistoDto | null>(null);
  protected readonly ocupado = signal(false);

  private catalogo: readonly ProductoDto[] = [];
  private temporizador?: ReturnType<typeof setTimeout>;

  /** Lo que se puede añadir, filtrado por lo escrito. */
  protected readonly candidatos = computed(() => {
    const aguja = this.busqueda().trim();
    if (!aguja) {
      return [];
    }
    // Solo lo que tiene existencia: no se puede llevar lo que no hay.
    return filtrarProductos(this.catalogo, aguja)
      .filter((producto) => !producto.agotado)
      .slice(0, 6);
  });

  protected get puedeRegistrar(): boolean {
    return (
      !this.ocupado() &&
      this.carrito().length > 0 &&
      this.tomadoPor().trim().length > 0 &&
      this.previsto() !== null
    );
  }

  constructor() {
    inject(DestroyRef).onDestroy(() => clearTimeout(this.temporizador));
  }

  /**
   * El arranque va aquí y no en el constructor.
   *
   * `cuenta` es un `input` **requerido**: en el constructor todavía no está
   * puesto, y Angular lo rechaza al compilar la plantilla en lugar de
   * dejarlo fallar en ejecución.
   */
  ngOnInit(): void {
    // El nombre de la dueña entra escrito: es quien se lo lleva la mayoría
    // de las veces, y corregirlo cuesta menos que teclearlo siempre.
    this.tomadoPor.set(this.cuenta().nombre);
    void this.cargarCatalogo();

    queueMicrotask(() => document.getElementById('consumo-buscar')?.focus());
  }

  private async cargarCatalogo(): Promise<void> {
    try {
      // Los retirados también: retirar algo del mostrador no significa que
      // en casa no se coma (RF-CON-07).
      this.catalogo = await this.api.listarProductos(true);
    } catch (error) {
      this.fallo.emit(comoError(error));
    }
  }

  protected cerrar(): void {
    clearTimeout(this.temporizador);
    this.cerrado.emit();
  }

  /**
   * Añade un producto al carrito.
   *
   * La ficha se pide al añadir porque el catálogo no trae los
   * identificadores de las presentaciones, solo el nombre de la
   * predeterminada. Es una llamada por producto añadido, dentro de un
   * cartel: no es la ruta caliente de nadie.
   */
  protected async agregar(producto: ProductoDto): Promise<void> {
    const repetido = this.carrito().some(
      (linea) => linea.producto === producto.id && linea.origen === this.origen(),
    );

    // El foco vuelve al buscador: se añade una cosa detrás de otra, como
    // en Entrada.
    this.busqueda.set('');
    queueMicrotask(() => document.getElementById('consumo-buscar')?.focus());

    if (repetido) {
      return;
    }

    try {
      const ficha = await this.api.consultarProducto(producto.id);
      const formas = ficha.presentaciones.map((p) => ({ id: p.id, nombre: p.nombre }));
      const predeterminada =
        ficha.presentaciones.find((p) => p.esPredeterminada) ?? ficha.presentaciones[0];

      if (!predeterminada) {
        return;
      }

      this.carrito.set([
        ...this.carrito(),
        {
          producto: producto.id,
          nombre: producto.nombre,
          formas,
          presentacion: predeterminada.id,
          cantidad: '1',
          origen: this.origen(),
        },
      ]);

      this.pedirVistaPrevia();
    } catch (error) {
      this.fallo.emit(comoError(error));
    }
  }

  protected quitar(indice: number): void {
    this.carrito.set(this.carrito().filter((_, posicion) => posicion !== indice));
    this.pedirVistaPrevia();
  }

  protected cambiarCantidad(indice: number, cantidad: string): void {
    this.reemplazar(indice, (linea) => ({ ...linea, cantidad }));
  }

  protected cambiarPresentacion(indice: number, presentacion: number): void {
    this.reemplazar(indice, (linea) => ({ ...linea, presentacion }));
  }

  protected cambiarOrigen(indice: number, origen: string): void {
    this.reemplazar(indice, (linea) => ({ ...linea, origen }));
  }

  private reemplazar(indice: number, como: (linea: EnElCarrito) => EnElCarrito): void {
    this.carrito.set(
      this.carrito().map((linea, posicion) => (posicion === indice ? como(linea) : linea)),
    );
    this.pedirVistaPrevia();
  }

  /**
   * Pregunta a Rust lo que sumaría.
   *
   * Se espera un momento para no disparar una llamada por cada tecla, y la
   * respuesta que llega tarde se descarta: si mientras iba y venía se
   * cambió el carrito, esa cifra habla de algo que ya no está en pantalla.
   */
  private pedirVistaPrevia(): void {
    clearTimeout(this.temporizador);
    this.temporizador = setTimeout(() => void this.previsualizar(), 250);
  }

  private async previsualizar(): Promise<void> {
    const lineas = this.lineasValidas();

    if (lineas.length === 0) {
      this.previsto.set(null);
      return;
    }

    const foto = JSON.stringify(lineas);

    try {
      const prevista = await this.api.previsualizarConsumo({
        cuenta: this.cuenta().id,
        tomadoPor: this.tomadoPor().trim() || '—',
        lineas,
      });

      if (JSON.stringify(this.lineasValidas()) !== foto) {
        return;
      }

      this.previsto.set(prevista);
    } catch (error) {
      if (JSON.stringify(this.lineasValidas()) === foto) {
        this.previsto.set(null);
        this.fallo.emit(comoError(error));
      }
    }
  }

  /** Las líneas con una cantidad que se puede mandar. */
  private lineasValidas(): LineaConsumoDto[] {
    return this.carrito()
      .filter((linea) => DECIMAL.test(linea.cantidad.trim()))
      .map((linea) => ({
        producto: linea.producto,
        presentacion: linea.presentacion,
        cantidad: linea.cantidad.trim(),
        origen: linea.origen,
      }));
  }

  protected async confirmar(): Promise<void> {
    if (!this.puedeRegistrar) {
      return;
    }

    this.ocupado.set(true);
    try {
      const hecho = await this.api.registrarConsumo({
        cuenta: this.cuenta().id,
        tomadoPor: this.tomadoPor().trim(),
        lineas: this.lineasValidas(),
      });

      this.registrado.emit(hecho.id);
    } catch (error) {
      this.fallo.emit(comoError(error));
    } finally {
      this.ocupado.set(false);
    }
  }
}
