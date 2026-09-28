import {
  ChangeDetectionStrategy,
  Component,
  computed,
  DestroyRef,
  ElementRef,
  HostListener,
  inject,
  signal,
  viewChild,
} from '@angular/core';

import {
  CobroCalculadoDto,
  ERROR_VENTA_EN_ESPERA,
  ErrorDto,
  LARGO_MAXIMO_NOTA,
  METODOS_PAGO,
  PresentacionVendibleDto,
  ProductoVendibleDto,
  VentaEnEsperaDto,
  VentaHechaDto,
  VentaPrevistaDto,
} from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';
import { antiguedad } from './antiguedad';
import {
  apartadoDe,
  ESTADO_LINEA,
  esMarcada,
  filasDe,
  type Linea,
  lineasDeRetomada,
  pedidoDe,
} from './venta-en-curso';

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

/** Cada cuánto se refresca la antigüedad de las ventas en espera. */
const REFRESCO_ANTIGUEDAD = 30_000;

/** Por qué no se retoma una venta con otra a medias (decisión 3). */
const RETOMAR_BLOQUEADO = 'Deja la venta actual en espera o cóbrala antes de retomar otra.';

/** La venta en espera que está ahora en pantalla. */
interface EsperaCargada {
  readonly id: number;
  readonly nota: string | null;
}

/** Caracteres, no unidades UTF-16: el núcleo cuenta igual. */
function largoDe(texto: string): number {
  return [...texto.trim()].length;
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
  /**
   * Hay caja abierta.
   *
   * Se comprueba al entrar y no solo al cobrar: enterarse de que no se
   * puede vender cuando ya tienes la venta montada y el cliente delante es
   * la peor forma de enterarse (RF-CAJ-06).
   */
  protected readonly hayCaja = signal(true);

  /** Lo que el cliente se lleva. */
  protected readonly lineas = signal<readonly Linea[]>([]);

  /** Renglones retomados que hoy no se pueden vender (decisión 6). */
  protected readonly marcadas = computed(() => this.lineas().filter(esMarcada).length);

  /** Lo que enseña la tabla: cada renglón con lo que calculó el núcleo. */
  protected readonly filas = computed(() =>
    filasDe(this.lineas(), this.prevista()?.lineas ?? null),
  );
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

  /**
   * Número de la última vista previa pedida.
   *
   * La tabla empareja cada renglón con su línea prevista por posición: una
   * respuesta vieja que llegue tarde emparejaría renglones que ya no son.
   */
  private consulta = 0;

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

    // La antigüedad de las ventas en espera avanza sola mientras la
    // pantalla está abierta, que es casi todo el día.
    const reloj = setInterval(() => this.ahora.set(new Date()), REFRESCO_ANTIGUEDAD);
    inject(DestroyRef).onDestroy(() => {
      clearInterval(reloj);
      clearTimeout(this.temporizador);
    });
  }

  protected async recargar(): Promise<void> {
    try {
      this.catalogo.set(await this.api.catalogoDeVenta());
      this.tasa.set(await this.api.consultarTasa());
      this.hayCaja.set((await this.api.consultarCaja()) !== null);
      this.esperas.set(await this.api.listarVentasEnEspera());
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

    // Con la nota de la espera abierta, Enter y Escape son de la nota: el
    // campo los atiende él mismo, y aquí no deben elegir un resultado ni
    // borrar la búsqueda de paso.
    if (this.apartando()) {
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

    this.lineas.set([
      ...this.lineas(),
      { estado: ESTADO_LINEA.VENDIBLE, producto, presentacion, cantidad },
    ]);
    this.cancelarLinea();
    void this.recalcular();
  }

  protected quitar(indice: number): void {
    this.lineas.set(this.lineas().filter((_, i) => i !== indice));
    if (!this.lineas().length) {
      this.avisoEspera.set(null);
      this.apartando.set(false);
    }
    void this.recalcular();
  }

  /**
   * Le pide al núcleo los importes y el total de lo que hay puesto.
   *
   * Solo van los renglones vendibles: los marcados harían fallar la
   * cuenta entera (el producto no existe o ya no se vende), y lo que el
   * usuario necesita es ver el total de lo que sí puede cobrar.
   */
  private async recalcular(): Promise<void> {
    const numero = ++this.consulta;
    const pedido = pedidoDe(this.lineas());

    if (!pedido.length) {
      this.prevista.set(null);
      this.cobro.set(null);
      return;
    }

    try {
      const prevista = await this.api.previsualizarVenta(pedido);
      if (numero !== this.consulta) {
        return;
      }
      this.prevista.set(prevista);
      this.error.set(null);
      this.recalcularCobro();
    } catch (fallo) {
      if (numero !== this.consulta) {
        return;
      }
      this.error.set(comoError(fallo));
      this.prevista.set(null);
    }
  }

  /**
   * Deja la pantalla lista para la siguiente venta.
   *
   * Lo comparten cobrar, cancelar y apartar: las tres terminan la venta en
   * curso, y olvidarse de un paso en una de ellas (el pago a medias, la
   * espera cargada) es el tipo de error que no se ve hasta que cobra mal.
   */
  private vaciarVenta(): void {
    this.consulta++;
    this.lineas.set([]);
    this.prevista.set(null);
    this.cobro.set(null);
    this.partes.set([{ metodo: 'EFECTIVO_CUP', entregado: '' }]);
    this.cargada.set(null);
    this.avisoEspera.set(null);
  }

  /** Vacía la venta sin dejar rastro en el inventario (RF-VTA-07). */
  protected cancelarVenta(): void {
    this.vaciarVenta();
    this.apartando.set(false);
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

  /**
   * Se puede cobrar.
   *
   * Ni con renglones marcados ni con algo que no alcanza en vitrina: el
   * núcleo lo rechazaría igual, y es mejor que el botón lo diga antes de
   * que el cliente saque la cartera (decisión 6).
   */
  protected get puedeCobrar(): boolean {
    return (
      this.hayCaja() &&
      this.lineas().length > 0 &&
      this.marcadas() === 0 &&
      !(this.prevista()?.hayFaltantes ?? false) &&
      (this.cobro()?.alcanza ?? false)
    );
  }

  /**
   * Cobra la venta.
   *
   * Se manda entera de una vez: las líneas, los pagos y nada más. Rust
   * valida la existencia, congela los costos, descuenta de la vitrina y
   * devuelve el vuelto ya calculado.
   *
   * Si la venta salió de una espera, va su identificador: el núcleo la
   * borra en la MISMA operación que registra la venta. Así no puede
   * quedar cobrada y además en la lista, lista para cobrarse dos veces.
   */
  protected async cobrar(): Promise<void> {
    if (!this.puedeCobrar) {
      return;
    }

    const espera = this.cargada();
    this.cobrando.set(true);
    try {
      const hecha = await this.api.vender({
        lineas: pedidoDe(this.lineas()),
        pagos: this.partes()
          .filter((parte) => DECIMAL.test(parte.entregado.trim()))
          .map((parte) => ({ metodo: parte.metodo, entregado: parte.entregado.trim() })),
        ...(espera ? { esperaId: espera.id } : {}),
      });

      this.error.set(null);
      this.ultima.set(hecha);
      this.vaciarVenta();
      await this.recargar();
      this.enfocarBuscador();
    } catch (fallo) {
      const error = comoError(fallo);
      this.error.set(error);
      // La espera ya no existe (se cobró o se eliminó por otro camino). La
      // venta sigue en pantalla: cobrarla otra vez es una venta normal.
      if (error.codigo === ERROR_VENTA_EN_ESPERA.VENTA_EN_ESPERA_NO_ENCONTRADA) {
        this.cargada.set(null);
        await this.cargarEsperas();
      }
    } finally {
      this.cobrando.set(false);
    }
  }

  // ------------------------------------ la venta en espera (RF-VTA-14)

  /** Las ventas apartadas, de la más antigua a la más reciente. */
  protected readonly esperas = signal<readonly VentaEnEsperaDto[]>([]);

  /** La espera que está en pantalla, si la venta salió de una. */
  protected readonly cargada = signal<EsperaCargada | null>(null);

  /** La casilla de la nota para apartar está abierta. */
  protected readonly apartando = signal(false);
  protected readonly nota = signal('');
  protected readonly largoMaximoNota = LARGO_MAXIMO_NOTA;
  protected readonly largoNota = computed(() => largoDe(this.nota()));
  protected readonly notaValida = computed(() => this.largoNota() <= LARGO_MAXIMO_NOTA);

  /** Una operación sobre las esperas en marcha: evita el doble clic. */
  protected readonly ocupadoEspera = signal(false);

  /** La espera cuya eliminación se está confirmando. */
  protected readonly porEliminar = signal<number | null>(null);

  /** Lo que hay que decirle al usuario en el panel de las esperas. */
  protected readonly avisoEspera = signal<string | null>(null);

  /** El reloj de la antigüedad. */
  private readonly ahora = signal(new Date());

  protected antiguedadDe(espera: VentaEnEsperaDto): string {
    return antiguedad(espera.creadaEn, this.ahora());
  }

  private async cargarEsperas(): Promise<void> {
    try {
      this.esperas.set(await this.api.listarVentasEnEspera());
    } catch (fallo) {
      this.error.set(comoError(fallo));
    }
  }

  /**
   * Abre la casilla de la nota.
   *
   * Si la venta ya salía de una espera, trae su nota: es el mismo cliente
   * que se vuelve a apartar, no uno nuevo.
   */
  protected empezarEspera(): void {
    if (!this.lineas().length) {
      return;
    }
    this.nota.set(this.cargada()?.nota ?? '');
    this.apartando.set(true);
    queueMicrotask(() => document.getElementById('nota-espera')?.focus());
  }

  protected cancelarEspera(): void {
    this.apartando.set(false);
    this.enfocarBuscador();
  }

  /**
   * Aparta la venta en curso y deja la pantalla libre.
   *
   * No hace falta caja abierta: apartar no mueve dinero ni mercancía
   * (decisión 4).
   *
   * Si la venta salía de otra espera, esa se borra DESPUÉS de guardar la
   * nueva: lo apartado ahora ya la contiene. En ese orden, un fallo a medio
   * camino deja una espera repetida, que se ve y se elimina; en el orden
   * contrario dejaría la venta perdida.
   */
  protected async dejarEnEspera(): Promise<void> {
    if (!this.lineas().length || !this.notaValida() || this.ocupadoEspera()) {
      return;
    }

    const anterior = this.cargada();
    const nota = this.nota().trim();
    this.ocupadoEspera.set(true);
    try {
      await this.api.dejarVentaEnEspera({
        ...(nota ? { nota } : {}),
        lineas: apartadoDe(this.lineas()),
      });

      if (anterior) {
        await this.eliminarSinAvisar(anterior.id);
      }

      this.error.set(null);
      this.ultima.set(null);
      this.vaciarVenta();
      this.apartando.set(false);
      this.nota.set('');
      await this.cargarEsperas();
      this.enfocarBuscador();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupadoEspera.set(false);
    }
  }

  /** Borra una espera que ya no hace falta; si ya no estaba, da igual. */
  private async eliminarSinAvisar(id: number): Promise<void> {
    try {
      await this.api.eliminarVentaEnEspera(id);
    } catch (fallo) {
      if (comoError(fallo).codigo !== ERROR_VENTA_EN_ESPERA.VENTA_EN_ESPERA_NO_ENCONTRADA) {
        throw fallo;
      }
    }
  }

  /**
   * Trae una venta en espera a la pantalla, con el precio de hoy.
   *
   * Con otra venta a medias, NO se retoma (decisión 3): mezclarlas o
   * tirar una sin preguntar son las dos formas de perder trabajo. El
   * usuario decide qué hacer con la que tiene delante.
   *
   * La espera no se borra: sigue en la lista hasta que se cobra o se
   * elimina.
   */
  protected async retomar(espera: VentaEnEsperaDto): Promise<void> {
    if (this.lineas().length) {
      this.avisoEspera.set(RETOMAR_BLOQUEADO);
      return;
    }
    if (this.ocupadoEspera()) {
      return;
    }

    this.ocupadoEspera.set(true);
    try {
      // El catálogo se pide de nuevo: los renglones se emparejan contra él,
      // y desde que se abrió la pantalla pudo cambiar.
      const [retomada, catalogo] = await Promise.all([
        this.api.retomarVentaEnEspera(espera.id),
        this.api.catalogoDeVenta(),
      ]);

      this.catalogo.set(catalogo);
      this.vaciarVenta();
      this.enCurso.set(null);
      this.presentacionElegida.set(null);
      this.busqueda.set('');
      this.ultima.set(null);
      this.porEliminar.set(null);
      this.lineas.set(lineasDeRetomada(retomada.lineas, catalogo));
      this.cargada.set({ id: retomada.id, nota: retomada.nota });
      this.error.set(null);
      await this.recalcular();
      this.enfocarBuscador();
    } catch (fallo) {
      const error = comoError(fallo);
      this.error.set(error);
      if (error.codigo === ERROR_VENTA_EN_ESPERA.VENTA_EN_ESPERA_NO_ENCONTRADA) {
        await this.cargarEsperas();
      }
    } finally {
      this.ocupadoEspera.set(false);
    }
  }

  protected pedirEliminar(id: number): void {
    this.avisoEspera.set(null);
    this.porEliminar.set(id);
  }

  protected cancelarEliminar(): void {
    this.porEliminar.set(null);
  }

  /**
   * Descarta una venta en espera sin cobrarla.
   *
   * Si es la que está en pantalla, la venta se queda: solo deja de estar
   * ligada a una espera, y cobrarla será una venta normal.
   */
  protected async eliminar(id: number): Promise<void> {
    if (this.ocupadoEspera()) {
      return;
    }

    this.ocupadoEspera.set(true);
    try {
      await this.eliminarSinAvisar(id);
      if (this.cargada()?.id === id) {
        this.cargada.set(null);
      }
      this.error.set(null);
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.porEliminar.set(null);
      this.ocupadoEspera.set(false);
      await this.cargarEsperas();
    }
  }

  private enfocarBuscador(): void {
    queueMicrotask(() => this.buscador()?.nativeElement.focus());
  }
}
