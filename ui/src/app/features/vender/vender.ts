import {
  ChangeDetectionStrategy,
  Component,
  computed,
  ElementRef,
  HostListener,
  inject,
  signal,
  viewChild,
} from '@angular/core';

import {
  CobroCalculadoDto,
  ErrorDto,
  METODOS_PAGO,
  PresentacionVendibleDto,
  ProductoVendibleDto,
  VentaHechaDto,
  VentaPrevistaDto,
} from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';

/** Número decimal con hasta seis decimales. */
const DECIMAL = /^\d+(\.\d{1,6})?$/;

/** Quita acentos y mayúsculas para buscar como se teclea con prisa. */
function plegar(texto: string): string {
  return texto
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .trim();
}

/** Un renglón de la venta en curso, tal como se ve en pantalla. */
interface Linea {
  readonly producto: ProductoVendibleDto;
  readonly presentacion: PresentacionVendibleDto;
  readonly cantidad: string;
}

/** Una parte del cobro que el usuario está componiendo. */
interface Parte {
  readonly metodo: string;
  readonly entregado: string;
}

/**
 * Vender: la pantalla donde se pasa el día.
 *
 * Se usa con gente esperando en el mostrador, así que manda una regla por
 * encima de todas: **el flujo completo tiene que poder hacerse sin tocar el
 * ratón** (RF-VTA-02). Escribir, elegir con las flechas, confirmar con
 * Enter, cobrar y volver a empezar.
 *
 * El total y el vuelto los calcula Rust. Aquí no se suma dinero.
 */
@Component({
  selector: 'app-vender',
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './vender.html',
  styleUrl: './vender.css',
})
export class Vender {
  private readonly api = inject(Kilo12Api);
  private readonly buscador = viewChild<ElementRef<HTMLInputElement>>('buscador');

  protected readonly metodos = METODOS_PAGO;
  protected readonly catalogo = signal<readonly ProductoVendibleDto[]>([]);
  protected readonly error = signal<ErrorDto | null>(null);
  protected readonly cobrando = signal(false);
  protected readonly tasa = signal<string | null>(null);

  /** Lo que el cliente se lleva. */
  protected readonly lineas = signal<readonly Linea[]>([]);
  /** Cómo paga. Empieza con una sola parte en efectivo. */
  protected readonly partes = signal<readonly Parte[]>([{ metodo: 'EFECTIVO_CUP', entregado: '' }]);

  protected readonly busqueda = signal('');
  /** Cuál de los resultados está resaltado por el teclado. */
  protected readonly resaltado = signal(0);
  /** Producto elegido, esperando cantidad y presentación. */
  protected readonly enCurso = signal<ProductoVendibleDto | null>(null);
  protected readonly presentacionElegida = signal<PresentacionVendibleDto | null>(null);
  protected readonly cantidad = signal('1');

  /** Lo cobrado, para enseñar el vuelto hasta que empiece la siguiente. */
  protected readonly ultima = signal<VentaHechaDto | null>(null);

  /**
   * La venta calculada por el núcleo: importes por línea y total.
   *
   * No se calcula aquí. Sumar precios en JavaScript arrastraría error, y
   * el total es justo la cifra que el cliente va a pagar.
   */
  protected readonly prevista = signal<VentaPrevistaDto | null>(null);

  /** Cuánto falta o cuánto se devuelve, según lo que se va tecleando. */
  protected readonly cobro = signal<CobroCalculadoDto | null>(null);

  /** Espera entre tecla y consulta, para no llamar por cada dígito. */
  private temporizador?: ReturnType<typeof setTimeout>;

  /** Resultados de la búsqueda, por prefijo de nombre o de código. */
  protected readonly resultados = computed(() => {
    const aguja = plegar(this.busqueda());
    if (!aguja) {
      return [];
    }
    return this.catalogo()
      .filter((p) => plegar(p.nombre).startsWith(aguja) || plegar(p.sku).startsWith(aguja))
      .slice(0, 6);
  });

  constructor() {
    void this.recargar();
  }

  protected async recargar(): Promise<void> {
    try {
      this.catalogo.set(await this.api.catalogoDeVenta());
      this.tasa.set(await this.api.consultarTasa());
      this.error.set(null);
    } catch (fallo) {
      this.error.set(comoError(fallo));
    }
  }

  // ------------------------------------------------------- el teclado

  /**
   * La venta entera se maneja desde aquí.
   *
   * Las flechas recorren los resultados, Enter elige y confirma, Escape
   * deshace el paso actual. Sin esto la pantalla sería inservible con un
   * cliente delante.
   */
  @HostListener('window:keydown', ['$event'])
  protected alPulsar(evento: KeyboardEvent): void {
    // Las teclas de función son del menú, no de la venta.
    if (evento.key.startsWith('F') && evento.key.length > 1) {
      return;
    }

    if (evento.key === 'Escape') {
      evento.preventDefault();
      this.retroceder();
      return;
    }

    // Con un producto en curso, las flechas y Enter son suyos.
    if (this.enCurso()) {
      return;
    }

    const resultados = this.resultados();
    if (!resultados.length) {
      return;
    }

    if (evento.key === 'ArrowDown') {
      evento.preventDefault();
      this.resaltado.set(Math.min(this.resaltado() + 1, resultados.length - 1));
    } else if (evento.key === 'ArrowUp') {
      evento.preventDefault();
      this.resaltado.set(Math.max(this.resaltado() - 1, 0));
    } else if (evento.key === 'Enter') {
      evento.preventDefault();
      this.elegir(resultados[this.resaltado()]);
    }
  }

  /** Escape deshace un paso, no la venta entera. */
  private retroceder(): void {
    if (this.enCurso()) {
      this.cancelarLinea();
    } else if (this.busqueda()) {
      this.busqueda.set('');
    }
  }

  protected alEscribir(valor: string): void {
    this.busqueda.set(valor);
    this.resaltado.set(0);
    this.ultima.set(null);
  }

  // ------------------------------------------------- armar las líneas

  protected elegir(producto: ProductoVendibleDto): void {
    if (producto.agotado) {
      return;
    }

    this.enCurso.set(producto);
    // Con una sola forma de venderlo no hay nada que preguntar (RF-VTA-03).
    const predeterminada =
      producto.presentaciones.find((p) => p.esPredeterminada) ?? producto.presentaciones[0];
    this.presentacionElegida.set(predeterminada ?? null);
    this.cantidad.set('1');

    queueMicrotask(() => document.getElementById('cantidad')?.focus());
  }

  protected elegirPresentacion(presentacion: PresentacionVendibleDto): void {
    this.presentacionElegida.set(presentacion);
  }

  protected cancelarLinea(): void {
    this.enCurso.set(null);
    this.presentacionElegida.set(null);
    this.busqueda.set('');
    this.enfocarBuscador();
  }

  protected agregar(): void {
    const producto = this.enCurso();
    const presentacion = this.presentacionElegida();
    const cantidad = this.cantidad().trim();

    if (!producto || !presentacion || !DECIMAL.test(cantidad)) {
      return;
    }

    this.lineas.set([...this.lineas(), { producto, presentacion, cantidad }]);
    this.cancelarLinea();
    void this.recalcular();
  }

  protected quitar(indice: number): void {
    this.lineas.set(this.lineas().filter((_, i) => i !== indice));
    void this.recalcular();
  }

  /** Le pide al núcleo los importes y el total de lo que hay puesto. */
  private async recalcular(): Promise<void> {
    if (!this.lineas().length) {
      this.prevista.set(null);
      return;
    }

    try {
      this.prevista.set(
        await this.api.previsualizarVenta(
          this.lineas().map((linea) => ({
            producto: linea.producto.id,
            presentacion: linea.presentacion.id,
            cantidad: linea.cantidad,
          })),
        ),
      );
      this.error.set(null);
      this.recalcularCobro();
    } catch (fallo) {
      this.error.set(comoError(fallo));
      this.prevista.set(null);
    }
  }

  /** Vacía la venta sin dejar rastro en el inventario (RF-VTA-07). */
  protected cancelarVenta(): void {
    this.lineas.set([]);
    this.prevista.set(null);
    this.cobro.set(null);
    this.partes.set([{ metodo: 'EFECTIVO_CUP', entregado: '' }]);
    this.enCurso.set(null);
    this.busqueda.set('');
    this.error.set(null);
    this.enfocarBuscador();
  }

  // ---------------------------------------------------------- el pago

  protected alCambiarParte(indice: number, campo: 'metodo' | 'entregado', valor: string): void {
    this.partes.set(
      this.partes().map((parte, i) => (i === indice ? { ...parte, [campo]: valor } : parte)),
    );
    this.recalcularCobro();
  }

  protected agregarParte(): void {
    this.partes.set([...this.partes(), { metodo: 'EFECTIVO_CUP', entregado: '' }]);
    this.recalcularCobro();
  }

  protected quitarParte(indice: number): void {
    const restantes = this.partes().filter((_, i) => i !== indice);
    this.partes.set(restantes.length ? restantes : [{ metodo: 'EFECTIVO_CUP', entregado: '' }]);
    this.recalcularCobro();
  }

  /**
   * Pregunta al núcleo si lo entregado cubre la venta.
   *
   * Se espera un momento para no disparar una consulta por cada dígito, y
   * se descartan las partes a medio escribir: mientras alguien teclea «1»
   * de camino a «100» no hay nada útil que enseñar.
   */
  protected recalcularCobro(): void {
    clearTimeout(this.temporizador);
    this.temporizador = setTimeout(() => void this.consultarCobro(), 200);
  }

  private async consultarCobro(): Promise<void> {
    const total = this.prevista()?.total;
    if (!total) {
      this.cobro.set(null);
      return;
    }

    const pagos = this.partes()
      .filter((parte) => DECIMAL.test(parte.entregado.trim()))
      .map((parte) => ({ metodo: parte.metodo, entregado: parte.entregado.trim() }));

    try {
      this.cobro.set(await this.api.calcularCobro(total, pagos));
      this.error.set(null);
    } catch (fallo) {
      // Sin tasa puesta no se puede convertir: se avisa, pero no se
      // interrumpe a quien está tecleando.
      this.cobro.set(null);
      this.error.set(comoError(fallo));
    }
  }

  // ------------------------------------------------ la tasa del dólar

  /** La casilla para cambiar la tasa está abierta. */
  protected readonly editandoTasa = signal(false);
  protected readonly tasaNueva = signal('');

  /**
   * Abre la casilla de la tasa con el valor vigente ya escrito.
   *
   * Casi siempre se cambia de 420 a 425, no se escribe desde cero.
   */
  protected editarTasa(): void {
    this.tasaNueva.set(this.tasa() ?? '');
    this.editandoTasa.set(true);
    queueMicrotask(() => document.getElementById('tasa')?.focus());
  }

  protected cancelarTasa(): void {
    this.editandoTasa.set(false);
  }

  /**
   * Guarda la tasa y rehace la cuenta.
   *
   * Cambiarla no toca ninguna venta anterior: cada una se quedó con la
   * suya congelada (RF-VTA-10b). Solo afecta a lo que se cobre a partir
   * de ahora, incluida la venta que está en pantalla.
   */
  protected async guardarTasa(): Promise<void> {
    const valor = this.tasaNueva().trim();
    if (!DECIMAL.test(valor)) {
      return;
    }

    try {
      await this.api.fijarTasa(valor);
      this.tasa.set(await this.api.consultarTasa());
      this.editandoTasa.set(false);
      this.error.set(null);
      this.recalcularCobro();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    }
  }

  protected simboloDe(metodo: string): string {
    return this.metodos.find((m) => m.valor === metodo)?.moneda ?? '$';
  }

  protected get hayDolares(): boolean {
    return this.partes().some((parte) => parte.metodo === 'EFECTIVO_USD');
  }

  protected get puedeCobrar(): boolean {
    return this.lineas().length > 0 && (this.cobro()?.alcanza ?? false);
  }

  /**
   * Cobra la venta.
   *
   * Se manda entera de una vez: las líneas, los pagos y nada más. Rust
   * valida la existencia, congela los costos, descuenta de la vitrina y
   * devuelve el vuelto ya calculado.
   */
  protected async cobrar(): Promise<void> {
    if (!this.puedeCobrar) {
      return;
    }

    this.cobrando.set(true);
    try {
      const hecha = await this.api.vender({
        lineas: this.lineas().map((linea) => ({
          producto: linea.producto.id,
          presentacion: linea.presentacion.id,
          cantidad: linea.cantidad,
        })),
        pagos: this.partes()
          .filter((parte) => DECIMAL.test(parte.entregado.trim()))
          .map((parte) => ({ metodo: parte.metodo, entregado: parte.entregado.trim() })),
      });

      this.error.set(null);
      this.ultima.set(hecha);
      this.lineas.set([]);
      this.prevista.set(null);
      this.cobro.set(null);
      this.partes.set([{ metodo: 'EFECTIVO_CUP', entregado: '' }]);
      await this.recargar();
      this.enfocarBuscador();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.cobrando.set(false);
    }
  }

  private enfocarBuscador(): void {
    queueMicrotask(() => this.buscador()?.nativeElement.focus());
  }
}
