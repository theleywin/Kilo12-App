import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { ActivatedRoute, RouterLink } from '@angular/router';

import {
  CuentaEnDeudaDto,
  DetalleCuentaDto,
  ErrorDto,
  LineaConsumoListadaDto,
} from '../../../core/api.types';
import { comoError, Kilo12Api } from '../../../core/kilo12-api';
import { Avatar } from '../../../shared/avatar/avatar';
import { ConsumoLiquidar } from '../../../shared/consumo-liquidar/consumo-liquidar';
import { ConsumoRegistrar } from '../../../shared/consumo-registrar/consumo-registrar';

/**
 * Una entrada del historial de una cuenta.
 *
 * Los consumos y las liquidaciones viven en la MISMA tabla porque son el
 * mismo eje: uno sube el saldo y el otro lo baja. Separarlos en dos listas
 * obligaba a ir y venir para entender un saldo que es la resta de las dos
 * cosas.
 */
interface Apunte {
  readonly id: number;
  readonly que: 'consumo' | 'pago';
  readonly cuando: string;
  /** «Yaneisy» en un consumo; la nota en un pago. */
  readonly quien: string;
  readonly importe: string;
  /** Un consumo suma deuda; una liquidación la resta. */
  readonly suma: boolean;
  readonly anulado: boolean;
  readonly motivoAnulacion: string | null;
}

/**
 * El historial de una cuenta de consumo (RF-CON-12).
 *
 * Tiene página propia y no una pestaña del panel: con tres libros de
 * apuntes a la vez el panel se volvía ilegible, y lo que se va a mirar allí
 * casi siempre es el saldo, no el detalle de un consumo de hace dos
 * semanas. Quien quiere el detalle viene aquí a propósito.
 *
 * El historial es **un solo libro cronológico**: lo que se llevó suma y lo
 * que pagó resta, como un estado de cuenta. Es el modelo mental de un saldo
 * corriente, y tenerlo en una tabla en vez de dos evita el viaje de ida y
 * vuelta para entender una resta.
 */
@Component({
  selector: 'app-consumo-cuenta',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [RouterLink, Avatar, ConsumoRegistrar, ConsumoLiquidar],
  templateUrl: './cuenta.html',
  styleUrl: './cuenta.css',
})
export class ConsumoCuenta {
  private readonly api = inject(Kilo12Api);
  private readonly ruta = inject(ActivatedRoute);

  /** Cuenta que pide la dirección. */
  private readonly id = Number(this.ruta.snapshot.paramMap.get('cuenta'));

  protected readonly detalle = signal<DetalleCuentaDto | null>(null);
  protected readonly error = signal<ErrorDto | null>(null);
  protected readonly cargando = signal(false);
  protected readonly ocupado = signal(false);

  /** Apunte desplegado, como `consumo-7`. */
  protected readonly abierto = signal<string | null>(null);
  protected readonly renglones = signal<readonly LineaConsumoListadaDto[]>([]);

  protected readonly registrando = signal<CuentaEnDeudaDto | null>(null);
  protected readonly liquidando = signal<CuentaEnDeudaDto | null>(null);

  /**
   * El libro, con los dos tipos de apunte mezclados y ordenados.
   *
   * El orden es por fecha descendente comparando los sellos como texto:
   * llegan en `YYYY-MM-DD hh:mm:ss`, así que el orden alfabético **es** el
   * cronológico. Comparar cadenas, no aritmética.
   */
  protected readonly apuntes = computed<readonly Apunte[]>(() => {
    const detalle = this.detalle();
    if (!detalle) {
      return [];
    }

    return [
      ...detalle.consumos.map((consumo) => ({
        id: consumo.id,
        que: 'consumo' as const,
        cuando: consumo.ocurridoEn,
        quien: consumo.tomadoPor,
        importe: consumo.total,
        suma: true,
        anulado: consumo.anulado,
        motivoAnulacion: consumo.motivoAnulacion,
      })),
      ...detalle.pagos.map((pago) => ({
        id: pago.id,
        que: 'pago' as const,
        cuando: pago.ocurridoEn,
        quien: pago.nota ?? 'liquidación',
        importe: pago.importe,
        suma: false,
        anulado: pago.anulado,
        motivoAnulacion: pago.motivoAnulacion,
      })),
    ].sort((a, b) => b.cuando.localeCompare(a.cuando));
  });

  constructor() {
    void this.recargar();
  }

  protected async recargar(): Promise<void> {
    this.cargando.set(true);
    try {
      this.detalle.set(await this.api.consultarCuentaConsumo(this.id));
      this.error.set(null);
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.cargando.set(false);
    }
  }

  protected clave(apunte: Apunte): string {
    return `${apunte.que}-${apunte.id}`;
  }

  /**
   * Despliega el detalle de un apunte, o lo cierra.
   *
   * Solo los consumos tienen renglones; una liquidación es un importe y una
   * nota, que ya están a la vista.
   */
  protected async alternar(apunte: Apunte): Promise<void> {
    const clave = this.clave(apunte);

    if (this.abierto() === clave) {
      this.abierto.set(null);
      return;
    }

    if (apunte.que === 'pago') {
      this.abierto.set(clave);
      this.renglones.set([]);
      return;
    }

    try {
      this.renglones.set(await this.api.consultarRenglonesConsumo(apunte.id));
      this.abierto.set(clave);
      this.error.set(null);
    } catch (fallo) {
      this.error.set(comoError(fallo));
    }
  }

  // -------------------------------------------------------- anular

  protected readonly anulando = signal<Apunte | null>(null);
  protected readonly motivo = signal('');

  protected abrirAnulacion(apunte: Apunte): void {
    this.anulando.set(apunte);
    this.motivo.set('');
    queueMicrotask(() => document.getElementById('consumo-motivo')?.focus());
  }

  protected cerrarAnulacion(): void {
    this.anulando.set(null);
    this.motivo.set('');
  }

  protected async confirmarAnulacion(): Promise<void> {
    const apunte = this.anulando();
    const motivo = this.motivo().trim();

    if (!apunte || !motivo) {
      return;
    }

    this.ocupado.set(true);
    try {
      const anulacion = { id: apunte.id, motivo };

      if (apunte.que === 'consumo') {
        await this.api.anularConsumo(anulacion);
      } else {
        await this.api.anularPagoConsumo(anulacion);
      }

      this.error.set(null);
      this.cerrarAnulacion();
      this.abierto.set(null);
      await this.recargar();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  // ------------------------------------------- los dos carteles

  protected async alRegistrar(): Promise<void> {
    this.registrando.set(null);
    await this.recargar();
  }

  protected async alLiquidar(): Promise<void> {
    this.liquidando.set(null);
    await this.recargar();
  }
}
