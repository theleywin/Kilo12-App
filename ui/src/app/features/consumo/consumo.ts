import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { Router } from '@angular/router';

import {
  CuentaEnDeudaDto,
  ErrorDto,
  PanelConsumoDto,
  ProductoConsumidoDto,
} from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';
import { Avatar } from '../../shared/avatar/avatar';
import { ConsumoLiquidar } from '../../shared/consumo-liquidar/consumo-liquidar';
import { ConsumoRegistrar } from '../../shared/consumo-registrar/consumo-registrar';

/**
 * Consumo: el panel de las tres cuentas de la casa (RF-CON).
 *
 * Esta pantalla contesta **una** pregunta —quién debe cuánto— y ofrece las
 * tres cosas que se hacen con esa respuesta: cargar un consumo, anotar un
 * pago y mirar el historial. Nada más.
 *
 * El historial vive en **otra página** (`/consumo/:cuenta`) a propósito.
 * Caber no es lo mismo que convenir: tres libros de apuntes en la misma
 * pantalla la vuelven ilegible, y lo que se viene a ver aquí casi siempre
 * es el saldo, no el detalle de un consumo de hace dos semanas.
 *
 * Lo que sí se queda es «qué se está yendo»: son cinco o seis filas, es la
 * única pregunta de este módulo que no contestan los saldos, y mira las
 * tres cuentas a la vez, así que no pertenece a la página de ninguna.
 */
@Component({
  selector: 'app-consumo',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [Avatar, ConsumoRegistrar, ConsumoLiquidar],
  templateUrl: './consumo.html',
  styleUrl: './consumo.css',
})
export class Consumo {
  private readonly api = inject(Kilo12Api);
  private readonly router = inject(Router);

  protected readonly panel = signal<PanelConsumoDto | null>(null);
  protected readonly porProducto = signal<readonly ProductoConsumidoDto[]>([]);
  protected readonly error = signal<ErrorDto | null>(null);
  protected readonly cargando = signal(false);

  /** Cuenta a la que se le está cargando algo, o a la que paga. */
  protected readonly registrando = signal<CuentaEnDeudaDto | null>(null);
  protected readonly liquidando = signal<CuentaEnDeudaDto | null>(null);

  constructor() {
    void this.recargar();
  }

  protected async recargar(): Promise<void> {
    this.cargando.set(true);
    try {
      const [panel, porProducto] = await Promise.all([
        this.api.consultarPanelConsumo(),
        this.api.consultarConsumoPorProducto(),
      ]);

      this.panel.set(panel);
      this.porProducto.set(porProducto);
      this.error.set(null);
    } catch (fallo) {
      this.error.set(comoError(fallo));
    } finally {
      this.cargando.set(false);
    }
  }

  protected verHistorial(cuenta: CuentaEnDeudaDto): void {
    void this.router.navigate(['/consumo', cuenta.id]);
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
