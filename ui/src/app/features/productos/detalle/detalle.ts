import { ChangeDetectionStrategy, Component, DestroyRef, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { ActivatedRoute, RouterLink } from '@angular/router';

import {
  CambioPrecioDto,
  CostoListadoDto,
  DESTINO_PRODUCTO,
  ErrorDto,
  FichaProductoDto,
  PresentacionDto,
  ProductoVacioDto,
  RevaluacionDto,
} from '../../../core/api.types';
import { comoError, Kilo12Api } from '../../../core/kilo12-api';

/** Número decimal con hasta seis decimales. */
const DECIMAL = /^\d+(\.\d{1,6})?$/;

/**
 * La ficha de un producto: a cuánto se vende y cuánto deja.
 *
 * Esta pantalla **no crea productos ni mueve mercancía**. Los productos
 * nacen cuando llega su primera caja, en Entrada, y la existencia solo
 * cambia con un movimiento. Aquí se decide el lado comercial: los precios,
 * las formas de venderlo y si sigue estando a la venta.
 *
 * Un producto se vende de varias maneras a precios que **no son
 * proporcionales**: la lata suelta a 80 y el paquete de seis a 300. Cada
 * presentación trae su propia economía, y ver que el paquete deja menos
 * ganancia por refresco que la venta suelta es información de negocio, no
 * un detalle contable.
 */
@Component({
  selector: 'app-producto-detalle',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [ReactiveFormsModule, RouterLink],
  templateUrl: './detalle.html',
  styleUrl: './detalle.css',
})
export class ProductoDetalle {
  private readonly api = inject(Kilo12Api);
  private readonly fb = inject(FormBuilder);
  private readonly ruta = inject(ActivatedRoute);

  /** Producto que pide la dirección. */
  private readonly id = Number(this.ruta.snapshot.paramMap.get('id'));

  protected readonly ficha = signal<FichaProductoDto | null>(null);
  protected readonly historial = signal<readonly CambioPrecioDto[]>([]);
  protected readonly error = signal<ErrorDto | null>(null);
  protected readonly cargando = signal(false);
  protected readonly ocupado = signal(false);

  /** Precio que se está escribiendo para cada presentación. */
  protected readonly precios = signal<Record<number, string>>({});
  /** Margen objetivo por presentación, para calcular el precio al revés. */
  protected readonly margenes = signal<Record<number, string>>({});
  /** Precio que dejaría ese margen, calculado por Rust mientras se escribe. */
  protected readonly sugeridos = signal<Record<number, string>>({});

  /** Espera entre tecla y cálculo. */
  private temporizador?: ReturnType<typeof setTimeout>;

  protected readonly mostrarNuevaPresentacion = signal(false);
  protected readonly mostrarHistorial = signal(false);

  protected readonly formularioProducto = this.fb.nonNullable.group({
    nombre: ['', Validators.required],
    stockMinimo: ['', Validators.pattern(DECIMAL)],
    activo: [true],
  });

  protected readonly formularioPresentacion = this.fb.nonNullable.group({
    nombre: ['', Validators.required],
    factor: ['', [Validators.required, Validators.pattern(DECIMAL)]],
    precio: ['', [Validators.required, Validators.pattern(DECIMAL)]],
    codigoBarras: [''],
  });

  constructor() {
    void this.cargar();

    // Los dos temporizadores se cancelan al salir: uno pendiente dispara
    // su consulta sobre una pantalla que ya no está.
    inject(DestroyRef).onDestroy(() => {
      clearTimeout(this.temporizador);
      clearTimeout(this.temporizadorCosto);
    });
  }

  private async cargar(): Promise<void> {
    this.cargando.set(true);
    try {
      const ficha = await this.api.consultarProducto(this.id);
      this.ficha.set(ficha);
      this.error.set(null);

      this.formularioProducto.reset({
        nombre: ficha.nombre,
        stockMinimo: ficha.stockMinimo,
        activo: ficha.activo,
      });
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.cargando.set(false);
    }
  }

  private async refrescarFicha(): Promise<void> {
    this.ficha.set(await this.api.consultarProducto(this.id));
    this.precios.set({});
    this.margenes.set({});

    if (this.mostrarHistorial()) {
      this.historial.set(await this.api.consultarHistorialPrecios(this.id));
    }
  }

  // --------------------------------------------------- datos básicos

  protected async guardarProducto(): Promise<void> {
    if (this.formularioProducto.invalid) {
      this.formularioProducto.markAllAsTouched();
      return;
    }

    this.ocupado.set(true);
    const valores = this.formularioProducto.getRawValue();

    try {
      await this.api.editarProducto({
        producto: this.id,
        nombre: valores.nombre.trim(),
        stockMinimo: valores.stockMinimo.trim() || undefined,
        activo: valores.activo,
      });
      this.error.set(null);
      await this.refrescarFicha();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  // ----------------------------------------------------- los precios

  protected precioDe(presentacion: PresentacionDto): string {
    return this.precios()[presentacion.id] ?? presentacion.precio;
  }

  protected alEscribirPrecio(presentacion: PresentacionDto, valor: string): void {
    this.precios.set({ ...this.precios(), [presentacion.id]: valor });
  }

  protected margenDe(presentacion: PresentacionDto): string {
    return this.margenes()[presentacion.id] ?? '';
  }

  /**
   * Anota el margen y pide el precio que dejaría.
   *
   * Se calcula mientras se escribe, no al pulsar un botón: el número que
   * importa es el precio, y tenerlo delante mientras se tantea el margen
   * es toda la diferencia entre probar y adivinar.
   */
  protected alEscribirMargen(presentacion: PresentacionDto, valor: string): void {
    this.margenes.set({ ...this.margenes(), [presentacion.id]: valor });

    // Se espera un momento para no disparar una llamada por cada tecla.
    // Basta un temporizador: solo se edita una fila a la vez.
    clearTimeout(this.temporizador);
    this.temporizador = setTimeout(() => void this.calcularSugerido(presentacion), 250);
  }

  /** Precio que dejaría el margen escrito, o vacío si no se puede saber. */
  protected sugeridoDe(presentacion: PresentacionDto): string {
    return this.sugeridos()[presentacion.id] ?? '';
  }

  /** Lleva el precio sugerido a la casilla del precio. */
  protected usarSugerido(presentacion: PresentacionDto): void {
    const sugerido = this.sugeridoDe(presentacion);
    if (sugerido) {
      this.alEscribirPrecio(presentacion, sugerido);
    }
  }

  private async calcularSugerido(presentacion: PresentacionDto): Promise<void> {
    const ficha = this.ficha();
    const margen = this.margenDe(presentacion).trim();

    // Sin costo no hay margen que perseguir: el precio saldría de dividir
    // entre nada.
    if (!ficha || ficha.costo === '—' || !margen) {
      this.olvidarSugerido(presentacion);
      return;
    }

    try {
      const precio = await this.api.calcularPrecioParaMargen(presentacion.costo, margen);

      // Cancelar el temporizador no cancela la consulta que ya salió: si
      // mientras iba y venía se borró o se cambió el margen, esta respuesta
      // habla de un número que ya no está escrito y no se muestra.
      if (this.margenDe(presentacion).trim() !== margen) {
        return;
      }

      this.sugeridos.set({ ...this.sugeridos(), [presentacion.id]: precio });
      this.error.set(null);
    } catch {
      // Un margen imposible —del 100 % para arriba— no interrumpe a quien
      // está escribiendo: simplemente no hay precio que enseñar.
      if (this.margenDe(presentacion).trim() === margen) {
        this.olvidarSugerido(presentacion);
      }
    }
  }

  private olvidarSugerido(presentacion: PresentacionDto): void {
    const resto = { ...this.sugeridos() };
    delete resto[presentacion.id];
    this.sugeridos.set(resto);
  }

  protected async guardarPrecio(presentacion: PresentacionDto): Promise<void> {
    const precio = this.precioDe(presentacion).trim();
    if (!DECIMAL.test(precio) || precio === presentacion.precio) {
      return;
    }

    this.ocupado.set(true);
    try {
      await this.api.cambiarPrecio({
        producto: this.id,
        presentacion: presentacion.id,
        precio,
      });
      this.error.set(null);
      await this.refrescarFicha();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  // ----------------------------------------------- las presentaciones

  protected async agregarPresentacion(): Promise<void> {
    if (this.formularioPresentacion.invalid) {
      this.formularioPresentacion.markAllAsTouched();
      return;
    }

    this.ocupado.set(true);
    const valores = this.formularioPresentacion.getRawValue();

    try {
      await this.api.agregarPresentacion({
        producto: this.id,
        nombre: valores.nombre.trim(),
        factor: valores.factor.trim(),
        precio: valores.precio.trim(),
        codigoBarras: valores.codigoBarras.trim() || undefined,
      });
      this.error.set(null);
      this.formularioPresentacion.reset();
      this.mostrarNuevaPresentacion.set(false);
      await this.refrescarFicha();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  protected async retirar(presentacion: PresentacionDto): Promise<void> {
    this.ocupado.set(true);
    try {
      await this.api.desactivarPresentacion(this.id, presentacion.id);
      this.error.set(null);
      await this.refrescarFicha();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  protected async marcarPredeterminada(presentacion: PresentacionDto): Promise<void> {
    this.ocupado.set(true);
    try {
      await this.api.marcarPredeterminada(this.id, presentacion.id);
      this.error.set(null);
      await this.refrescarFicha();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  // ------------------------------------------- el costo (revaluación)

  /**
   * El cartel de cambiar el costo está abierto.
   *
   * No es un formulario más de la ficha: cambiar el costo mueve el valor
   * del inventario, así que va detrás de una confirmación con clave.
   */
  protected readonly cambiandoCosto = signal(false);
  protected readonly costoNuevo = signal('');
  protected readonly clave = signal('');

  /**
   * Lo que pasaría, calculado por Rust mientras se escribe.
   *
   * Aquí no se multiplica el costo por la existencia: el valor nuevo del
   * inventario es una cifra de dinero y sale del núcleo, como todas.
   */
  protected readonly revaluacion = signal<RevaluacionDto | null>(null);

  /** Espera entre tecla y vista previa del costo. */
  private temporizadorCosto?: ReturnType<typeof setTimeout>;

  protected readonly historialCostos = signal<readonly CostoListadoDto[]>([]);
  protected readonly mostrarHistorialCostos = signal(false);

  /**
   * Se puede confirmar.
   *
   * Hace falta una vista previa, un costo válido y una clave escrita. Y que
   * el costo **cambie**: igual que el precio, un no-cambio no se guarda. Se
   * comparan las dos cifras que formateó Rust y no el texto tecleado, para
   * que escribir «120.000» sobre un costo de 120.00 cuente como lo que es.
   */
  protected get puedeCambiarCosto(): boolean {
    const prevista = this.revaluacion();

    return (
      !this.ocupado() &&
      prevista !== null &&
      prevista.costoAnterior !== prevista.costoNuevo &&
      DECIMAL.test(this.costoNuevo().trim()) &&
      this.clave().trim().length > 0
    );
  }

  /** Abre el cartel con el costo de hoy ya escrito. */
  protected abrirCambioDeCosto(): void {
    const ficha = this.ficha();
    this.costoNuevo.set(ficha && ficha.costo !== '—' ? ficha.costo : '');
    this.clave.set('');
    this.revaluacion.set(null);
    this.cambiandoCosto.set(true);

    void this.previsualizarCosto();
    queueMicrotask(() => document.getElementById('costo-nuevo')?.focus());
  }

  /**
   * Cierra el cartel y olvida lo escrito.
   *
   * La clave sobre todo: no se queda en memoria esperando el próximo
   * cartel.
   */
  protected cerrarCambioDeCosto(): void {
    clearTimeout(this.temporizadorCosto);
    this.cambiandoCosto.set(false);
    this.costoNuevo.set('');
    this.clave.set('');
    this.revaluacion.set(null);
  }

  protected alEscribirCosto(valor: string): void {
    this.costoNuevo.set(valor);

    // Se espera un momento para no pedir una vista previa por cada tecla.
    clearTimeout(this.temporizadorCosto);
    this.temporizadorCosto = setTimeout(() => void this.previsualizarCosto(), 250);
  }

  private async previsualizarCosto(): Promise<void> {
    const costo = this.costoNuevo().trim();
    if (!DECIMAL.test(costo)) {
      this.revaluacion.set(null);
      return;
    }

    try {
      const prevista = await this.api.previsualizarCosto(this.id, costo);

      // Cancelar el temporizador no cancela la consulta que ya salió: si
      // mientras iba y venía se cambió lo escrito, esta respuesta habla de
      // un número que ya no está en la casilla.
      if (this.costoNuevo().trim() !== costo) {
        return;
      }

      this.revaluacion.set(prevista);
      this.error.set(null);
    } catch (fallo) {
      if (this.costoNuevo().trim() === costo) {
        this.revaluacion.set(null);
        this.error.set(comoError(fallo));
      }
    }
  }

  /**
   * Confirma el cambio de costo.
   *
   * La clave viaja al núcleo y se comprueba allí. Si está mal, el cartel
   * se queda abierto con lo escrito: lo único que se borra es la clave,
   * para que el siguiente intento empiece de cero.
   */
  protected async confirmarCambioDeCosto(): Promise<void> {
    if (!this.puedeCambiarCosto) {
      return;
    }

    this.ocupado.set(true);
    try {
      await this.api.cambiarCosto({
        producto: this.id,
        costo: this.costoNuevo().trim(),
        clave: this.clave().trim(),
      });

      this.error.set(null);
      this.cerrarCambioDeCosto();
      await this.refrescarFicha();

      if (this.mostrarHistorialCostos()) {
        this.historialCostos.set(await this.api.consultarHistorialCostos(this.id));
      }
    } catch (fallo) {
      this.error.set(comoError(fallo));
      this.clave.set('');
    } finally {
      this.ocupado.set(false);
    }
  }

  protected async alternarHistorialCostos(): Promise<void> {
    const abierto = !this.mostrarHistorialCostos();
    this.mostrarHistorialCostos.set(abierto);

    if (abierto) {
      try {
        this.historialCostos.set(await this.api.consultarHistorialCostos(this.id));
      } catch (fallo) {
        this.error.set(comoError(fallo));
      }
    }
  }

  // ------------------------------- quitar del catálogo (RF-CAT-06/07)

  /**
   * Lo que pasaría al quitar este producto. Con algo dentro, el cartel está
   * abierto.
   *
   * Lo calcula Rust porque depende de las ventas y del kárdex del producto,
   * que esta pantalla no conoce ni tiene por qué conocer.
   */
  protected readonly quitada = signal<ProductoVacioDto | null>(null);

  /** El producto se borró: ya no hay ficha que enseñar. */
  protected readonly seBorro = signal(false);

  /** Códigos de destino, para no escribir las cadenas en la plantilla. */
  protected readonly destinos = DESTINO_PRODUCTO;

  /**
   * Abre el cartel con lo que pasaría.
   *
   * Si el producto todavía tiene mercancía, el núcleo responde con un error
   * y el cartel no se abre: la pregunta no tiene sentido hasta que esté en
   * cero.
   */
  protected async abrirQuitar(): Promise<void> {
    this.ocupado.set(true);
    try {
      this.quitada.set(await this.api.previsualizarQuitarProducto(this.id));
      this.error.set(null);
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  protected cerrarQuitar(): void {
    this.quitada.set(null);
  }

  /**
   * Confirma.
   *
   * El destino lo vuelve a decidir Rust: lo que enseñó el cartel es una foto
   * y entre medias pudo entrar mercancía. Si borró, no se refresca la ficha
   * —no existe— y la pantalla ofrece volver al catálogo.
   */
  protected async confirmarQuitar(): Promise<void> {
    const prevista = this.quitada();
    if (!prevista || prevista.destino === DESTINO_PRODUCTO.BLOQUEADO) {
      return;
    }

    this.ocupado.set(true);
    try {
      const hecho = await this.api.quitarProducto(this.id);
      this.error.set(null);
      this.quitada.set(null);

      if (hecho.destino === DESTINO_PRODUCTO.BORRAR) {
        this.seBorro.set(true);
        this.ficha.set(null);
      } else {
        await this.refrescarFicha();
        this.formularioProducto.patchValue({ activo: false });
      }
    } catch (fallo) {
      this.error.set(comoError(fallo));
      this.quitada.set(null);
    } finally {
      this.ocupado.set(false);
    }
  }

  // -------------------------------------------------- el historial

  protected async alternarHistorial(): Promise<void> {
    const abierto = !this.mostrarHistorial();
    this.mostrarHistorial.set(abierto);

    if (abierto) {
      try {
        this.historial.set(await this.api.consultarHistorialPrecios(this.id));
      } catch (fallo) {
        this.error.set(comoError(fallo));
      }
    }
  }
}
