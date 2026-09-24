import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';

import { ErrorDto, HistorialVentasDto, VentaDetalladaDto, VentaListadaDto } from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';

/**
 * Ventas: qué se vendió y cuánto dejó.
 *
 * La pantalla contesta dos preguntas distintas, y por eso está partida en
 * dos: **cómo va el día** arriba, de un vistazo, y **qué pasó en esta
 * venta** al lado, cuando hace falta mirar una en concreto.
 *
 * Ninguna cifra se calcula aquí. La ganancia sale del costo que tenía la
 * mercancía cuando se vendió, congelado en la línea: si se recalculara con
 * el costo de hoy, la ganancia de ayer cambiaría sola cada vez que llega
 * una remesa (RF-VTA-13).
 */
@Component({
  selector: 'app-ventas',
  changeDetection: ChangeDetectionStrategy.OnPush,
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

  constructor() {
    void this.recargar();
  }

  protected async recargar(): Promise<void> {
    this.cargando.set(true);
    try {
      const historial = await this.api.consultarVentas();
      this.historial.set(historial);
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
