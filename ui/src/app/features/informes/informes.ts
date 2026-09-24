import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';

import { ErrorDto, InformeDto, PERIODOS, PorcionMetodoDto } from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';

/** Alto útil de las gráficas de barras, en unidades del `viewBox`. */
const ALTO = 100;

/** Una porción del anillo, ya resuelta a geometría. */
export interface ArcoMetodo {
  readonly porcion: PorcionMetodoDto;
  /** Longitud pintada del trazo, sobre una circunferencia de 100. */
  readonly trazo: number;
  /** Dónde empieza, girando desde arriba. */
  readonly desfase: number;
  readonly color: string;
}

/**
 * Informes: qué pasó y qué conviene hacer con eso.
 *
 * La pantalla contesta cuatro preguntas, en este orden:
 *
 * 1. **¿Cuánto entró y cuánto quedó?** Los tres niveles —venta, ganancia
 *    bruta y ganancia neta— separados, porque confundirlos es la forma más
 *    común de creerse rico.
 * 2. **¿Cuándo se vende?** Por día y por hora: uno dice si el negocio
 *    crece, el otro a qué hora conviene estar detrás del mostrador.
 * 3. **¿Qué sostiene el negocio?** Lo más vendido no siempre es lo más
 *    rentable, así que van los dos escalafones, uno al lado del otro.
 * 4. **¿Dónde está el problema?** Lo que no se mueve y lo que se acaba.
 *
 * **Ninguna cifra se calcula aquí.** Las gráficas se dibujan con pesos de 0
 * a 1000 que manda el núcleo; la pantalla pone la geometría y nunca divide
 * un importe.
 */
@Component({
  selector: 'app-informes',
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './informes.html',
  styleUrl: './informes.css',
})
export class Informes {
  private readonly api = inject(Kilo12Api);

  protected readonly periodos = PERIODOS;
  protected readonly alto = ALTO;

  protected readonly informe = signal<InformeDto | null>(null);
  protected readonly error = signal<ErrorDto | null>(null);
  protected readonly cargando = signal(false);
  protected readonly periodo = signal('semana');

  /** Qué escalafón se está mirando: lo más vendido o lo más rentable. */
  protected readonly escalafon = signal<'vendidos' | 'rentables'>('vendidos');

  constructor() {
    void this.recargar();
  }

  protected async elegirPeriodo(valor: string): Promise<void> {
    this.periodo.set(valor);
    await this.recargar();
  }

  protected async recargar(): Promise<void> {
    this.cargando.set(true);
    try {
      const dias = this.periodos.find((p) => p.valor === this.periodo())?.dias ?? 7;
      const hasta = new Date();
      const desde = new Date();
      desde.setDate(hasta.getDate() - (dias - 1));

      this.informe.set(await this.api.consultarInforme(fecha(desde), fecha(hasta), dias));
      this.error.set(null);
    } catch (fallo) {
      this.error.set(comoError(fallo));
      this.informe.set(null);
    } finally {
      this.cargando.set(false);
    }
  }

  // ------------------------------------------------- las gráficas

  /**
   * El anillo de formas de pago, resuelto a arcos.
   *
   * Se dibuja con un único círculo por porción y `stroke-dasharray`: es la
   * forma de hacer una rosquilla en SVG sin calcular ni un seno.
   */
  protected readonly anillo = computed<ArcoMetodo[]>(() => {
    const porciones = this.informe()?.porMetodo ?? [];
    const colores: Record<string, string> = {
      EFECTIVO_CUP: 'var(--acento)',
      TRANSFERENCIA: 'var(--apagado)',
      EFECTIVO_USD: 'var(--destaque)',
    };

    let recorrido = 0;
    return porciones.map((porcion) => {
      // El peso viene de 0 a 1000; la circunferencia se toma como 100.
      const trazo = porcion.peso / 10;
      const arco: ArcoMetodo = {
        porcion,
        trazo,
        // El desfase gira en sentido contrario, de ahí el signo.
        desfase: -recorrido,
        color: colores[porcion.metodo] ?? 'var(--linea)',
      };
      recorrido += trazo;
      return arco;
    });
  });

  /** Puntos de la línea de ganancia sobre la gráfica diaria. */
  protected readonly lineaGanancia = computed(() => {
    const dias = this.informe()?.porDia ?? [];
    if (dias.length < 2) {
      return '';
    }

    const paso = 100 / (dias.length - 1);
    return dias
      .map((dia, i) => `${(i * paso).toFixed(2)},${(ALTO - (dia.pesoGanancia * ALTO) / 1000).toFixed(2)}`)
      .join(' ');
  });

  /** El escalafón que se está mirando. */
  protected readonly ranking = computed(() =>
    this.escalafon() === 'vendidos'
      ? (this.informe()?.masVendidos ?? [])
      : (this.informe()?.masRentables ?? []),
  );

  /** Altura de una barra, en unidades del `viewBox`. */
  protected altura(peso: number): number {
    return (peso * ALTO) / 1000;
  }

  /** Las horas en que de verdad se vendió algo. */
  protected readonly horasActivas = computed(
    () => (this.informe()?.porHora ?? []).filter((h) => h.cuantas > 0).length,
  );

  /** La hora con más venta del periodo. */
  protected readonly horaPunta = computed(() => {
    const horas = this.informe()?.porHora ?? [];
    return horas.reduce<(typeof horas)[number] | null>(
      (mejor, hora) => (!mejor || hora.peso > mejor.peso ? hora : mejor),
      null,
    );
  });

  protected conPerdida(importe: string): boolean {
    return importe.trim().startsWith('-');
  }
}

/** `YYYY-MM-DD` en hora local, que es la que usa la base de datos. */
function fecha(dia: Date): string {
  const mes = `${dia.getMonth() + 1}`.padStart(2, '0');
  const numero = `${dia.getDate()}`.padStart(2, '0');
  return `${dia.getFullYear()}-${mes}-${numero}`;
}
