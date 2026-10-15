import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { Router } from '@angular/router';

import {
  CierreCalculadoDto,
  ConteoCalculadoDto,
  ErrorDto,
  EstadoCajaDto,
  SesionListadaDto,
} from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';
import { Clave } from '../../shared/clave/clave';
import { ContadorEfectivo } from '../../shared/contador-efectivo/contador-efectivo';

/** Número decimal con hasta seis decimales. */
const DECIMAL = /^\d+(\.\d{1,6})?$/;

/** Porcentaje con hasta dos decimales (RF-CMS-01). */
const PORCENTAJE = /^\d{1,3}(\.\d{1,2})?$/;

/**
 * Caja: el turno y su arqueo.
 *
 * La pantalla tiene tres vidas según el momento del día:
 *
 * 1. **Sin caja abierta.** Solo se puede hacer una cosa, y es abrirla. No se
 *    puede cobrar hasta entonces (RF-CAJ-06).
 * 2. **Caja abierta.** Se ve cuánto debería haber en la gaveta ahora mismo y
 *    se registran los movimientos de efectivo que no son ventas.
 * 3. **Cerrando.** Se cuenta el dinero físico y se compara con lo esperado.
 *
 * La cifra que manda es **el efectivo esperado**, y no es lo mismo que lo
 * vendido: las transferencias no pasan por la gaveta y el vuelto de un pago
 * en dólares sale de ella. Esas cuentas las hace Rust.
 */
@Component({
  selector: 'app-caja',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [Clave, ContadorEfectivo],
  templateUrl: './caja.html',
  styleUrl: './caja.css',
})
export class Caja {
  private readonly api = inject(Kilo12Api);
  private readonly router = inject(Router);

  protected readonly caja = signal<EstadoCajaDto | null>(null);
  protected readonly historial = signal<readonly SesionListadaDto[]>([]);
  protected readonly error = signal<ErrorDto | null>(null);
  protected readonly cargando = signal(false);
  protected readonly ocupado = signal(false);

  /** Apertura. */
  protected readonly operador = signal('');
  protected readonly fondoInicial = signal('');

  /** Movimiento de efectivo en curso. */
  protected readonly tipoMovimiento = signal('SALIDA');
  protected readonly importe = signal('');
  protected readonly motivo = signal('');

  /** Cierre. */
  protected readonly cerrando = signal(false);
  protected readonly contadoCup = signal('');
  protected readonly contadoUsd = signal('0');
  /**
   * El cierre va siempre consolidado.
   *
   * La venta total es la suma de las tres formas de cobro —efectivo,
   * transferencia y dólares— con la divisa ya convertida a pesos. Los
   * dólares se siguen **arqueando aparte**, que es otra cosa: una es la
   * cuenta del negocio y la otra es contar dos fajos distintos.
   */
  private readonly modo = 'CONSOLIDADO';
  protected readonly prevista = signal<CierreCalculadoDto | null>(null);
  /** El cierre ya confirmado, para enseñar el resultado. */
  protected readonly cerrada = signal<CierreCalculadoDto | null>(null);

  /**
   * Porcentaje que se lleva quien atiende la caja (RF-CMS-01).
   *
   * Se puede cambiar en cualquier momento, pero solo afecta a lo que se
   * cierre a partir de entonces: al cerrar se congela con la sesión, y lo
   * ya liquidado no se recalcula nunca (D-6).
   */
  protected readonly comision = signal<string | null>(null);
  protected readonly editandoComision = signal(false);
  protected readonly comisionNueva = signal('');

  // -------------------------------------------- contar los billetes

  /** El recuento está abierto. */
  protected readonly contando = signal(false);
  /**
   * Lo último que sumó la calculadora.
   *
   * Contar los billetes lo hace `app-contador-efectivo`; aquí solo se
   * guarda su resultado, que es lo que se lleva a la casilla del cierre.
   */
  protected readonly conteo = signal<ConteoCalculadoDto | null>(null);

  /** El cartel de la clave para abrir el IPV de un turno pasado. */
  protected readonly pidiendoIpv = signal(false);
  protected readonly comprobandoClave = signal(false);
  protected readonly errorClave = signal<string | null>(null);

  /** Sesión del historial que se está mirando. */
  protected readonly detalle = signal<CierreCalculadoDto | null>(null);

  protected readonly puedeAbrir = computed(
    () => this.operador().trim().length > 0 && DECIMAL.test(this.fondoInicial().trim()),
  );

  protected readonly puedeMover = computed(
    () => DECIMAL.test(this.importe().trim()) && this.motivo().trim().length > 0,
  );

  protected readonly puedeCerrar = computed(
    () => DECIMAL.test(this.contadoCup().trim()) && DECIMAL.test(this.contadoUsd().trim()),
  );

  constructor() {
    void this.recargar();
  }

  protected async recargar(): Promise<void> {
    this.cargando.set(true);
    try {
      this.caja.set(await this.api.consultarCaja());
      this.historial.set(await this.api.listarCajas());
      this.comision.set(await this.api.consultarComision());
      this.error.set(null);
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.cargando.set(false);
    }
  }

  // ------------------------------------------------------ abrir

  protected async abrir(): Promise<void> {
    if (!this.puedeAbrir() || this.ocupado()) {
      return;
    }

    this.ocupado.set(true);
    try {
      await this.api.abrirCaja({
        operador: this.operador().trim(),
        fondoInicial: this.fondoInicial().trim(),
      });
      this.operador.set('');
      this.fondoInicial.set('');
      this.cerrada.set(null);
      await this.recargar();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  // --------------------------------------------- mover efectivo

  protected async mover(): Promise<void> {
    if (!this.puedeMover() || this.ocupado()) {
      return;
    }

    this.ocupado.set(true);
    try {
      await this.api.moverEfectivo({
        tipo: this.tipoMovimiento(),
        importe: this.importe().trim(),
        motivo: this.motivo().trim(),
      });
      this.importe.set('');
      this.motivo.set('');
      await this.recargar();
      // Lo que se acaba de mover cambia el esperado, así que la vista
      // previa del cierre que hubiera en pantalla ya no vale.
      await this.recalcularCierre();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  // ------------------------------------------------------ cerrar

  protected empezarCierre(): void {
    this.cerrando.set(true);
    // Arranca con lo esperado ya escrito: en la mayoría de los cierres la
    // caja cuadra, y obligar a teclear la misma cifra invita a equivocarse.
    this.contadoCup.set(this.caja()?.efectivoEsperado ?? '');
    this.contadoUsd.set(this.caja()?.dolaresEsperados ?? '0');
    void this.recalcularCierre();
  }

  protected cancelarCierre(): void {
    this.cerrando.set(false);
    this.prevista.set(null);
  }

  protected async alCambiarConteo(): Promise<void> {
    await this.recalcularCierre();
  }

  /** Pide el cierre calculado sin confirmarlo. */
  protected async recalcularCierre(): Promise<void> {
    if (!this.cerrando() || !this.puedeCerrar()) {
      this.prevista.set(null);
      return;
    }

    try {
      this.prevista.set(
        await this.api.previsualizarCierre({
          contadoCup: this.contadoCup().trim(),
          contadoUsd: this.contadoUsd().trim(),
          modo: this.modo,
        }),
      );
      this.error.set(null);
    } catch (fallo) {
      this.prevista.set(null);
      this.error.set(comoError(fallo));
    }
  }

  protected async confirmarCierre(): Promise<void> {
    if (!this.puedeCerrar() || this.ocupado()) {
      return;
    }

    this.ocupado.set(true);
    try {
      const cerrada = await this.api.cerrarCaja({
        contadoCup: this.contadoCup().trim(),
        contadoUsd: this.contadoUsd().trim(),
        modo: this.modo,
      });

      this.cerrada.set(cerrada);
      this.cerrando.set(false);
      this.prevista.set(null);
      await this.recargar();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  // ---------------------------------------- contar los billetes

  protected abrirConteo(): void {
    this.conteo.set(null);
    this.contando.set(true);
  }

  protected cerrarConteo(): void {
    this.contando.set(false);
  }

  /** Lleva el total contado a la casilla del cierre y cierra el modal. */
  protected async usarConteo(): Promise<void> {
    const total = this.conteo()?.total;
    if (!total) {
      return;
    }

    this.contadoCup.set(total);
    this.contando.set(false);
    await this.recalcularCierre();
  }

  // --------------------------------------------- la comisión

  protected editarComision(): void {
    this.comisionNueva.set(this.comision() ?? '');
    this.editandoComision.set(true);
    queueMicrotask(() => document.getElementById('comision')?.focus());
  }

  protected cancelarComision(): void {
    this.editandoComision.set(false);
  }

  /**
   * Guarda el porcentaje y rehace el cierre en pantalla.
   *
   * Cambiarlo no toca ninguna caja ya cerrada: aquellas se liquidaron con
   * el porcentaje que había entonces y así se quedan.
   */
  protected async guardarComision(): Promise<void> {
    const valor = this.comisionNueva().trim();
    if (!PORCENTAJE.test(valor)) {
      return;
    }

    this.ocupado.set(true);
    try {
      await this.api.fijarComision(valor);
      this.comision.set(await this.api.consultarComision());
      this.editandoComision.set(false);
      this.error.set(null);
      await this.recalcularCierre();
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.ocupado.set(false);
    }
  }

  // --------------------------------------------------- historial

  protected async abrirSesion(sesion: SesionListadaDto): Promise<void> {
    if (sesion.abierta) {
      return;
    }

    try {
      this.detalle.set(await this.api.consultarCierre(sesion.id));
      this.error.set(null);
    } catch (fallo) {
      this.error.set(comoError(fallo));
    }
  }

  protected cerrarDetalle(): void {
    this.detalle.set(null);
  }

  // --------------------------------------------------- el IPV del turno

  /**
   * Pide la clave para ver el IPV de un turno que ya cerró.
   *
   * La hoja del turno en curso se mira sin permiso en su propia pantalla;
   * la de un turno pasado es historia y se abre a conciencia (RF-IPV-14).
   */
  protected pedirClaveParaIpv(): void {
    this.errorClave.set(null);
    this.pidiendoIpv.set(true);
  }

  /**
   * Comprueba la clave y, si sirve, lleva a la hoja de ese turno.
   *
   * Se comprueba **consultando**, no comparando aquí: la clave la valida
   * Rust y esta pantalla solo mira si la respuesta fue una negativa. Si
   * todo va bien, la clave viaja en el estado de navegación —no en la
   * dirección— para que no quede escrita en el historial del navegador.
   */
  protected async abrirIpv(clave: string): Promise<void> {
    const sesion = this.detalle()?.sesion;
    if (sesion === undefined) {
      return;
    }

    this.comprobandoClave.set(true);
    try {
      await this.api.consultarIpv(sesion, clave);
      this.pidiendoIpv.set(false);
      await this.router.navigate(['/ipv', sesion], { state: { clave } });
    } catch (fallo) {
      const error = comoError(fallo);
      if (error.codigo === 'CLAVE_INCORRECTA') {
        this.errorClave.set(error.mensaje);
      } else {
        this.pidiendoIpv.set(false);
        this.error.set(error);
      }
    } finally {
      this.comprobandoClave.set(false);
    }
  }

  protected cancelarIpv(): void {
    this.pidiendoIpv.set(false);
  }

  /** Una diferencia que empieza por «-» es un faltante. */
  protected falta(diferencia: string): boolean {
    return diferencia.trim().startsWith('-');
  }
}
