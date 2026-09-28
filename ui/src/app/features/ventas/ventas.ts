import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { RouterLink } from '@angular/router';

import type {
  ErrorDto,
  HistorialVentasDto,
  SesionDeReferenciaDto,
  VentaDetalladaDto,
  VentaListadaDto,
  VentasPorProductoDto,
} from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';
import { etiquetaSesion } from './sesion';

/**
 * Ventas: qué se vendió y cuánto dejó.
 *
 * La pantalla contesta dos preguntas distintas, y por eso está partida en
 * dos: **cómo va la sesión de caja** arriba, de un vistazo, y **qué pasó en
 * esta venta** al lado, cuando hace falta mirar una en concreto.
 *
 * «La sesión» es la caja abierta o, si no hay ninguna, la última que se
 * cerró; no el día del calendario (RF-VTA-20). Las tarjetas salen de la misma suma con
 * la que se arquea la caja, así que las dos pantallas no pueden discrepar.
 *
 * Agrupar por productos cambia la tabla de abajo: en vez de venta por venta,
 * lo vendido en esa misma sesión, una fila por producto y presentación. Los
 * totales vienen hechos del núcleo; aquí no se suma nada.
 *
 * Ninguna cifra se calcula aquí. La ganancia sale del costo que tenía la
 * mercancía cuando se vendió, congelado en la línea: si se recalculara con
 * el costo de hoy, la ganancia de ayer cambiaría sola cada vez que llega
 * una remesa (RF-VTA-13).
 */
@Component({
  selector: 'app-ventas',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [RouterLink],
  templateUrl: './ventas.html',
  styleUrl: './ventas.css',
})
export class Ventas {
  private readonly api = inject(Kilo12Api);

  protected readonly historial = signal<HistorialVentasDto | null>(null);
  protected readonly error = signal<ErrorDto | null>(null);
  protected readonly cargando = signal(false);

  /** La venta que se está mirando por dentro. */
  protected readonly detalle = signal<VentaDetalladaDto | null>(null);
  protected readonly cargandoDetalle = signal(false);

  /** Anulación en curso: la venta que se está anulando y su motivo. */
  protected readonly anulando = signal(false);
  protected readonly motivo = signal('');
  protected readonly ocupado = signal(false);

  /**
   * La tabla enseña lo vendido por producto (RF-EST-03b) en vez de venta
   * por venta. Empieza sin agrupar y no se recuerda.
   */
  protected readonly agruparPorProductos = signal(false);

  /** Lo vendido en la sesión, por producto. `null` hasta la primera carga. */
  protected readonly agrupado = signal<VentasPorProductoDto | null>(null);
  protected readonly cargandoAgrupado = signal(false);

  /**
   * Número de la última consulta agrupada pedida.
   *
   * Marcar y desmarcar la casilla seguido lanza varias; una respuesta vieja que
   * llegue tarde no debe pisar a la nueva ni enseñar un error que ya no
   * viene a cuento.
   */
  private consultaAgrupado = 0;

  /** El «ahora» con el que se escribió la etiqueta: se renueva en cada carga. */
  private readonly ahora = signal(new Date());

  /** «Sesión abierta desde 08:00», «Sesión cerrada · 27/09 08:00–20:30». */
  protected readonly etiqueta = computed(() => {
    const sesion = this.sesion();
    return sesion ? etiquetaSesion(sesion, this.ahora()) : null;
  });

  /**
   * La sesión a la que se refieren las cifras.
   *
   * Agrupado por productos manda la de la tabla, que es la que se acaba de
   * pedir; en otro caso, la de las tarjetas. Se piden a la vez al pulsar
   * Actualizar, así que en la práctica son la misma.
   */
  private readonly sesion = computed<SesionDeReferenciaDto | null>(() => {
    const agrupado = this.agrupado();
    if (this.agruparPorProductos() && agrupado) {
      return agrupado.sesion;
    }
    return this.historial()?.sesion ?? null;
  });

  /** El rótulo de la tabla dice qué se está mirando. */
  protected readonly titulo = computed(() =>
    this.agruparPorProductos() ? 'Vendido en la sesión, por producto' : 'Últimas ventas',
  );

  /** Hay alguna carga en marcha para lo que se está viendo. */
  protected readonly ocupadoRecargando = computed(
    () => this.cargando() || (this.agruparPorProductos() && this.cargandoAgrupado()),
  );

  constructor() {
    void this.recargar();
  }

  /** Actualizar: las tarjetas siempre, y la tabla del modo que esté puesto. */
  protected async recargar(): Promise<void> {
    if (this.agruparPorProductos()) {
      await Promise.all([this.recargarHistorial(), this.cargarAgrupado()]);
      return;
    }
    await this.recargarHistorial();
  }

  /** La casilla «Agrupar por productos» cambió. */
  protected cambiarAgrupacion(agrupar: boolean): void {
    if (agrupar === this.agruparPorProductos()) {
      return;
    }
    this.agruparPorProductos.set(agrupar);

    if (agrupar) {
      void this.cargarAgrupado();
      return;
    }

    // De vuelta a la lista: lo que quede pendiente de la vista agrupada ya
    // no interesa.
    this.consultaAgrupado++;
    this.cargandoAgrupado.set(false);
  }

  /**
   * Pide lo vendido por producto.
   *
   * No vacía la tabla mientras carga: lo anterior se queda a la vista hasta
   * que llegue lo nuevo, y así cambiar de modo no parpadea.
   */
  private async cargarAgrupado(): Promise<void> {
    const numero = ++this.consultaAgrupado;
    this.cargandoAgrupado.set(true);
    try {
      const agrupado = await this.api.consultarVentasPorProducto();
      if (numero !== this.consultaAgrupado) {
        return;
      }
      this.agrupado.set(agrupado);
      this.ahora.set(new Date());
      this.error.set(null);
    } catch (fallo) {
      if (numero !== this.consultaAgrupado) {
        return;
      }
      this.error.set(comoError(fallo));
    } finally {
      if (numero === this.consultaAgrupado) {
        this.cargandoAgrupado.set(false);
      }
    }
  }

  private async recargarHistorial(): Promise<void> {
    this.cargando.set(true);
    try {
      const historial = await this.api.consultarVentas();
      this.historial.set(historial);
      this.ahora.set(new Date());
      this.error.set(null);

      // Abre la última venta sola: entrar aquí y no ver nada obliga a un
      // clic que siempre es el mismo.
      const primera = historial.ventas[0];
      if (primera && !this.detalle()) {
        await this.abrir(primera);
      }
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.cargando.set(false);
    }
  }

  protected async abrir(venta: VentaListadaDto): Promise<void> {
    this.cargandoDetalle.set(true);
    try {
      this.detalle.set(await this.api.consultarVenta(venta.id));
      this.error.set(null);
    } catch (fallo) {
      this.error.set(comoError(fallo));
      this.detalle.set(null);
    } finally {
      this.cargandoDetalle.set(false);
    }
  }

  protected empezarAnulacion(): void {
    this.anulando.set(true);
    this.motivo.set('');
  }

  protected cancelarAnulacion(): void {
    this.anulando.set(false);
  }

  /**
   * Anula la venta abierta y devuelve la mercancía a vitrina.
   *
   * Solo funciona con ventas de la caja vigente: si la sesión ya se cerró,
   * el núcleo lo rechaza y la pantalla enseña por qué. No es un capricho —
   * ese arqueo se hizo contra dinero físico.
   */
  protected async anular(): Promise<void> {
    const venta = this.detalle();
    if (!venta || !this.motivo().trim() || this.ocupado()) {
      return;
    }

    this.ocupado.set(true);
    try {
      await this.api.anularVenta({ venta: venta.id, motivo: this.motivo().trim() });
      this.anulando.set(false);
      this.detalle.set(null);
      this.error.set(null);
      await this.recargar();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  /** Si la ganancia de una venta salió en negativo, se vendió con pérdida. */
  protected conPerdida(ganancia: string): boolean {
    return ganancia.trim().startsWith('-');
  }
}
