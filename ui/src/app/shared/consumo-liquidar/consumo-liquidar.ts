import {
  ChangeDetectionStrategy,
  Component,
  inject,
  input,
  OnInit,
  output,
  signal,
} from '@angular/core';

import { CuentaEnDeudaDto, ErrorDto } from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';

/** Número decimal con hasta seis decimales. */
const DECIMAL = /^\d+(\.\d{1,6})?$/;

/**
 * El cartel de liquidar una deuda de consumo.
 *
 * Vive fuera de `features/` porque lo usan el panel de Consumo y el
 * historial de una cuenta.
 *
 * No comprueba que el importe no pase del saldo: pagar de más deja la
 * cuenta **a favor**, que es un estado legítimo y el mismo al que se llega
 * anulando un consumo ya pagado (RF-CON-14).
 */
@Component({
  selector: 'app-consumo-liquidar',
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './consumo-liquidar.html',
  styleUrl: './consumo-liquidar.css',
})
export class ConsumoLiquidar implements OnInit {
  private readonly api = inject(Kilo12Api);

  readonly cuenta = input.required<CuentaEnDeudaDto>();

  /** Se anotó el pago: lleva el saldo que quedó, ya formateado. */
  readonly liquidado = output<string>();
  readonly cerrado = output<void>();
  readonly fallo = output<ErrorDto>();

  protected readonly importe = signal('');
  protected readonly nota = signal('');
  protected readonly ocupado = signal(false);

  protected get puedeLiquidar(): boolean {
    const escrito = this.importe().trim();
    return !this.ocupado() && DECIMAL.test(escrito) && escrito !== '0';
  }

  /**
   * El arranque va aquí y no en el constructor: `cuenta` es un `input`
   * **requerido** y en el constructor todavía no está puesto.
   */
  ngOnInit(): void {
    // Se propone el saldo entero, que es lo que se paga casi siempre. Si
    // está a favor no se propone nada: no hay nada que liquidar.
    const cuenta = this.cuenta();
    this.importe.set(cuenta.aFavor ? '' : cuenta.saldo);

    queueMicrotask(() => document.getElementById('consumo-importe')?.focus());
  }

  protected async confirmar(): Promise<void> {
    if (!this.puedeLiquidar) {
      return;
    }

    this.ocupado.set(true);
    try {
      const saldo = await this.api.liquidarDeuda({
        cuenta: this.cuenta().id,
        importe: this.importe().trim(),
        nota: this.nota().trim() || undefined,
      });

      this.liquidado.emit(saldo);
    } catch (error) {
      this.fallo.emit(comoError(error));
    } finally {
      this.ocupado.set(false);
    }
  }
}
