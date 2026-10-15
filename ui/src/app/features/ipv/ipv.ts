import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  OnInit,
  signal,
} from '@angular/core';
import { ActivatedRoute, Router } from '@angular/router';

import type { ErrorDto, IpvDto } from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';
import { Clave } from '../../shared/clave/clave';
import { etiquetaSesion } from '../ventas/sesion';

/**
 * IPV: la hoja del turno.
 *
 * Es la tabla que el puntico llevaba a mano al cerrar: una fila por
 * mercancía, y en cada fila lo que había, lo que entró, lo que se perdió,
 * lo que se llevaron las dueñas, lo que se vendió y lo que quedó. Se
 * imprime y se cuadra contra el estante, y de ahí sale su forma: una sola
 * tabla ancha, sin tarjetas por medio y sin nada que haya que desplegar.
 * Quien la usa va bajando por los nombres con la mercancía delante.
 *
 * Habla de la **vitrina**. «Entrada» aquí no es una compra al proveedor:
 * es lo que entró al mostrador, que casi siempre es lo que se bajó del
 * almacén. Esa distinción la resuelve Rust; la pantalla solo la explica en
 * el pie para que nadie lea la columna como otra cosa.
 *
 * ## Dos direcciones, y por qué
 *
 * `/ipv` enseña **el turno abierto y nada más**. Si la caja está cerrada no
 * hay hoja: el IPV se llena mientras el turno está vivo, y el de ayer ya es
 * historia. Por eso esta pantalla no tiene selector de turnos —lo tuvo, y
 * ofrecía el pasado donde no toca.
 *
 * `/ipv/:sesion` enseña la hoja de un turno concreto, y para llegar hace
 * falta la clave del sistema. El camino previsto es Caja → el turno → la
 * clave, y entonces se llega aquí con la clave en el estado de navegación.
 * Si se entra a la dirección directamente —recargando, por ejemplo—, esta
 * pantalla pide la clave ella misma: no da por hecho que alguien la haya
 * pedido antes. Quien la comprueba, en todo caso, es Rust (RF-IPV-14).
 *
 * Ninguna cifra se calcula aquí, incluida la comprobación de que cada fila
 * cuadra: la hace Rust y llega hecha en `cuadra`. Esta pantalla solo la
 * pinta en rojo (DT-7).
 *
 * Imprimir es `window.print()` a secas. Las reglas de `estilos/impresion.css`
 * apagan el menú y la barra de acciones, así que lo que sale por la
 * impresora es la hoja y nada más. No hay vista previa propia porque la del
 * navegador ya existe y es la que el usuario conoce.
 *
 * Exportar escribe el `.xlsx` en Rust y devuelve dónde quedó. La ruta se
 * enseña tal cual: en un equipo de escritorio, saber el nombre del archivo
 * y la carpeta es lo que permite encontrarlo.
 */
@Component({
  selector: 'app-ipv',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [Clave],
  templateUrl: './ipv.html',
  styleUrl: './ipv.css',
})
export class Ipv implements OnInit {
  private readonly api = inject(Kilo12Api);
  private readonly ruta = inject(ActivatedRoute);
  private readonly router = inject(Router);

  protected readonly hoja = signal<IpvDto | null>(null);
  protected readonly error = signal<ErrorDto | null>(null);
  protected readonly cargando = signal(false);

  /**
   * Turno pedido por la dirección. `null` es el que está abierto ahora.
   */
  protected readonly turno = signal<number | null>(null);

  /** La clave con la que se está mirando un turno pasado. */
  private clave: string | null = null;

  /** El cartel de la clave está abierto. */
  protected readonly pidiendoClave = signal(false);
  protected readonly errorClave = signal<string | null>(null);

  /** Dónde quedó el último Excel exportado. */
  protected readonly exportado = signal<string | null>(null);
  protected readonly exportando = signal(false);

  ngOnInit(): void {
    const pedido = this.ruta.snapshot.paramMap.get('sesion');
    this.turno.set(pedido ? Number(pedido) : null);

    // Caja ya pidió la clave y la pasa por el estado de navegación, que no
    // deja rastro en la dirección: una clave en la barra se queda en el
    // historial y se comparte sin querer al copiar el enlace.
    const traida = (history.state as { clave?: unknown } | null)?.clave;
    this.clave = typeof traida === 'string' ? traida : null;

    if (this.turno() !== null && !this.clave) {
      this.pidiendoClave.set(true);
      return;
    }

    void this.cargar();
  }

  /** A qué turno pertenece la hoja, dicho en una línea. */
  protected readonly etiqueta = computed(() => {
    const sesion = this.hoja()?.sesion;
    return sesion ? etiquetaSesion(sesion, new Date()) : '';
  });

  /** Se está mirando un turno del pasado y no el de ahora. */
  protected readonly esDelPasado = computed(() => this.turno() !== null);

  /** Cuántas filas no se explican por sus propias columnas. */
  protected readonly descuadradas = computed(
    () => this.hoja()?.filas.filter((fila) => !fila.cuadra).length ?? 0,
  );

  /** La diferencia es negativa cuando la caja cobró más que la hoja. */
  protected readonly diferenciaEnContra = computed(() =>
    (this.hoja()?.diferencia ?? '').startsWith('-'),
  );

  protected async cargar(): Promise<void> {
    this.cargando.set(true);
    this.error.set(null);
    try {
      this.hoja.set(
        await this.api.consultarIpv(this.turno() ?? undefined, this.clave ?? undefined),
      );
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.cargando.set(false);
    }
  }

  protected imprimir(): void {
    window.print();
  }

  /** Vuelve a la hoja del turno abierto. */
  protected async verElDeAhora(): Promise<void> {
    await this.router.navigate(['/ipv']);
  }

  protected async confirmarClave(clave: string): Promise<void> {
    this.clave = clave;
    this.errorClave.set(null);
    await this.cargar();

    const fallo = this.error();
    if (fallo?.codigo === 'CLAVE_INCORRECTA') {
      // El cartel sigue abierto con el aviso dentro: cerrarlo obligaría a
      // volver a pedirlo desde cero por teclear un dígito mal.
      this.clave = null;
      this.error.set(null);
      this.errorClave.set(fallo.mensaje);
      return;
    }

    this.pidiendoClave.set(false);
  }

  protected cancelarClave(): void {
    this.pidiendoClave.set(false);
    void this.router.navigate(['/caja']);
  }

  protected async exportar(): Promise<void> {
    const sesion = this.hoja()?.sesion;
    if (!sesion) {
      return;
    }

    this.exportando.set(true);
    this.error.set(null);
    try {
      // Se manda el identificador y no `undefined` aunque sea el turno
      // abierto: entre ver la hoja y pulsar el botón pudo cerrarse la caja,
      // y entonces «el abierto» ya no sería este. La clave viaja porque
      // exportar un turno pasado pide permiso igual que mirarlo.
      this.exportado.set(await this.api.exportarIpv([sesion.id], this.clave ?? undefined));
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.exportando.set(false);
    }
  }
}
