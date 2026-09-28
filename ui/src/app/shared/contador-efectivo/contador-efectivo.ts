import {
  ChangeDetectionStrategy,
  Component,
  effect,
  inject,
  input,
  output,
  signal,
} from '@angular/core';

import { ConteoCalculadoDto, ErrorDto, Moneda } from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';

/**
 * La calculadora de billetes: una rejilla de denominaciones y su total.
 *
 * Vive fuera de `features/` porque la usan DOS pantallas —el arqueo de Caja
 * y la sección de Contar dinero— y duplicarla sería garantizar que el día
 * que cambie una denominación o el estilo de una casilla, una de las dos se
 * quede vieja.
 *
 * Qué hace y qué no:
 *
 * - **Sabe** contar: pide las denominaciones de la moneda, recoge cuántos
 *   billetes hay de cada una y le pide a Rust la suma.
 * - **No sabe** para qué se cuenta. No consulta la caja, no cierra turnos y
 *   no guarda nada. Lo que el consumidor quiera colgar del total —cotejarlo
 *   con lo esperado, llevárselo a un formulario, limpiarlo— lo proyecta
 *   dentro con `<ng-content>` y lo decide él.
 *
 * Ninguna suma se hace aquí. El total es la cifra contra la que se arquea
 * una caja, y sumar doce productos en JavaScript sería la única cifra de
 * dinero de la aplicación calculada fuera del núcleo.
 */
@Component({
  selector: 'app-contador-efectivo',
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './contador-efectivo.html',
  styleUrl: './contador-efectivo.css',
})
export class ContadorEfectivo {
  private readonly api = inject(Kilo12Api);

  /**
   * Moneda que se cuenta.
   *
   * Cambiarla empieza otro recuento desde cero: las denominaciones no son
   * las mismas y arrastrar lo tecleado daría el total de una moneda con los
   * billetes de la otra.
   */
  readonly moneda = input.required<Moneda>();

  /** Encabezado del total. Lo pone el consumidor, que sabe qué cuenta. */
  readonly titulo = input('Total contado');

  /** El recuento cada vez que cambia, para quien necesite la cifra. */
  readonly conteoCambia = output<ConteoCalculadoDto | null>();

  /**
   * Un fallo del núcleo.
   *
   * No se pinta aquí: cada pantalla ya tiene su sitio para los avisos y dos
   * avisos del mismo error en la misma vista es ruido.
   */
  readonly fallo = output<ErrorDto>();

  /** Denominaciones de la moneda activa, tal como las da el núcleo. */
  protected readonly denominaciones = signal<readonly number[]>([]);
  /** Cuántos billetes de cada valor, en el orden de las denominaciones. */
  protected readonly billetes = signal<readonly string[]>([]);
  /** El recuento sumado por Rust. */
  protected readonly conteo = signal<ConteoCalculadoDto | null>(null);

  /**
   * Número de la última consulta pedida.
   *
   * Las respuestas pueden llegar desordenadas si se teclea rápido, y una
   * vieja pisando a una nueva dejaría en pantalla un total que no
   * corresponde a lo escrito. Al cambiar de moneda importa todavía más: una
   * respuesta en pesos aterrizando sobre una rejilla de dólares. Solo se
   * acepta la última.
   */
  private peticion = 0;

  constructor() {
    effect(() => void this.cargar(this.moneda()));
  }

  /** Vacía las casillas sin cambiar de moneda. */
  limpiar(): void {
    this.billetes.set(this.denominaciones().map(() => ''));
    void this.sumar(this.moneda());
  }

  protected alContarBilletes(indice: number, valor: string): void {
    // Solo dígitos: no existe medio billete ni un número negativo de ellos.
    const limpio = valor.replace(/\D/g, '');
    this.billetes.set(this.billetes().map((actual, i) => (i === indice ? limpio : actual)));
    void this.sumar(this.moneda());
  }

  /** Trae las denominaciones de una moneda y arranca su recuento en cero. */
  private async cargar(moneda: Moneda): Promise<void> {
    const mio = ++this.peticion;

    try {
      const valores = await this.api.denominacionesEfectivo(moneda);
      // Llegó tarde: ya se está contando otra moneda.
      if (mio !== this.peticion) {
        return;
      }

      this.denominaciones.set(valores);
      this.billetes.set(valores.map(() => ''));
      await this.sumar(moneda);
    } catch (fallo) {
      this.fallo.emit(comoError(fallo));
    }
  }

  /** Le pide al núcleo la suma de lo que hay escrito. */
  private async sumar(moneda: Moneda): Promise<void> {
    const mio = ++this.peticion;
    const cuantos = this.billetes().map((texto) => Number.parseInt(texto, 10) || 0);

    try {
      const conteo = await this.api.contarEfectivo(moneda, cuantos);
      if (mio !== this.peticion) {
        return;
      }

      this.conteo.set(conteo);
      this.conteoCambia.emit(conteo);
    } catch (fallo) {
      this.fallo.emit(comoError(fallo));
    }
  }
}
