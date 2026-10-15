import { ChangeDetectionStrategy, Component, input, output, signal } from '@angular/core';

/**
 * El cartel que pide la clave del sistema.
 *
 * Vive fuera de `features/` porque lo usan Caja —para abrir el IPV de un
 * turno pasado— y el propio IPV, cuando se llega a la dirección de un turno
 * sin haber pasado por Caja.
 *
 * **No valida nada.** Recoge lo que se teclea y lo entrega; quien comprueba
 * la clave es Rust, porque el comando queda expuesto a la interfaz y una
 * puerta que solo viva en JavaScript se salta abriendo las herramientas de
 * desarrollo. Por eso el error llega de fuera, en `error`: lo pone quien
 * intentó la operación y recibió la negativa.
 */
@Component({
  selector: 'app-clave',
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './clave.html',
  styleUrl: './clave.css',
})
export class Clave {
  readonly titulo = input('Clave del sistema');
  /** Para qué se está pidiendo. Una frase. */
  readonly explicacion = input('');
  /** Texto del botón que confirma. */
  readonly accion = input('Continuar');
  readonly ocupado = input(false);
  /** Lo que falló al intentarlo. Se enseña dentro del cartel. */
  readonly error = input<string | null>(null);

  readonly confirmado = output<string>();
  readonly cerrado = output<void>();

  protected readonly clave = signal('');

  protected get puedeConfirmar(): boolean {
    return this.clave().trim().length > 0 && !this.ocupado();
  }

  protected confirmar(): void {
    if (this.puedeConfirmar) {
      this.confirmado.emit(this.clave().trim());
    }
  }

  protected cerrar(): void {
    this.cerrado.emit();
  }
}
