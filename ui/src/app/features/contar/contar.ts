import { ChangeDetectionStrategy, Component, computed, signal, viewChild } from '@angular/core';

import { ConteoCalculadoDto, ErrorDto, Moneda, MONEDAS } from '../../core/api.types';
import { ContadorEfectivo } from '../../shared/contador-efectivo/contador-efectivo';

/**
 * Contar dinero: la calculadora de billetes, suelta.
 *
 * La misma pieza que el arqueo de Caja, pero sin turno ni cierre detrás. Se
 * abre para contar lo que haya sobre la mesa —el fajo de la mañana, lo que
 * trae un proveedor, el bolsillo al final del día— y no escribe nada en
 * ninguna parte: no hay caja que cuadrar aquí, y por eso se puede usar con
 * la caja cerrada o sin haberla abierto nunca.
 *
 * Cuenta pesos o dólares, uno a la vez. **Nunca los suma**: son dos fajos
 * distintos y mezclarlos sería inventarse una tasa de cambio que esta
 * pantalla no tiene por qué conocer.
 */
@Component({
  selector: 'app-contar',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [ContadorEfectivo],
  templateUrl: './contar.html',
  styleUrl: './contar.css',
})
export class Contar {
  private readonly contador = viewChild.required(ContadorEfectivo);

  protected readonly monedas = MONEDAS;
  protected readonly moneda = signal<Moneda>('CUP');
  protected readonly conteo = signal<ConteoCalculadoDto | null>(null);
  protected readonly error = signal<ErrorDto | null>(null);

  /** Encabezado del total: dice en qué moneda está la cifra. */
  protected readonly titulo = computed(
    () => this.monedas.find((una) => una.valor === this.moneda())?.titulo ?? 'Total contado',
  );

  /** Con las casillas vacías no hay nada que limpiar. */
  protected readonly hayAlgoContado = computed(() => (this.conteo()?.cuantosBilletes ?? 0) > 0);

  protected elegirMoneda(moneda: Moneda): void {
    this.moneda.set(moneda);
    // El recuento de la otra moneda ya no vale: la calculadora empieza de
    // cero al cambiar de billetes, y el total que quedara en pantalla
    // habilitaría el botón de limpiar sin haber nada escrito.
    this.conteo.set(null);
    this.error.set(null);
  }

  protected limpiar(): void {
    this.contador().limpiar();
  }
}
