import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';

import { ErrorDto } from '../../core/api.types';
import { comoError, Kilo12Api } from '../../core/kilo12-api';

/**
 * Catálogo visual de Kilo12.
 *
 * Existe para que las piezas se puedan mirar juntas y decidir si el
 * conjunto se sostiene, en vez de descubrir a la quinta pantalla que hay
 * cuatro botones distintos. Si una pieza no está aquí, no existe.
 */
@Component({
  selector: 'app-guia',
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './guia.html',
  styleUrl: './guia.css',
})
export class Guia {
  private readonly api = inject(Kilo12Api);

  // ------------------------------------------- vaciar la aplicación

  /**
   * El borrado está detrás de dos puertas: abrir la zona y teclear la
   * clave. No es paranoia — es la única acción de toda la aplicación que
   * no se puede deshacer, y vive aquí, en la pantalla que menos se abre,
   * precisamente para que nadie tropiece con ella.
   */
  protected readonly zonaAbierta = signal(false);
  protected readonly clave = signal('');
  protected readonly borrando = signal(false);
  protected readonly borrado = signal(false);
  protected readonly errorBorrado = signal<ErrorDto | null>(null);

  protected abrirZona(): void {
    this.zonaAbierta.set(true);
    this.clave.set('');
    this.errorBorrado.set(null);
  }

  protected cerrarZona(): void {
    this.zonaAbierta.set(false);
    this.clave.set('');
    this.errorBorrado.set(null);
  }

  protected async borrarTodo(): Promise<void> {
    if (!this.clave().trim() || this.borrando()) {
      return;
    }

    this.borrando.set(true);
    try {
      await this.api.borrarTodosLosDatos(this.clave().trim());
      this.borrado.set(true);
      this.zonaAbierta.set(false);
      this.clave.set('');
      this.errorBorrado.set(null);
    } catch (fallo) {
      this.errorBorrado.set(comoError(fallo));
    } finally {
      this.borrando.set(false);
    }
  }

  protected readonly marca = [
    { nombre: 'Verde oliva', valor: '#687501', uso: 'Acción y navegación. La bolsa del logo.' },
    { nombre: 'Naranja', valor: '#EB8401', uso: 'Lo que exige atención. El círculo del 12.' },
  ];

  protected readonly superficies = [
    { ficha: '--fondo', uso: 'El papel: fondo de la pantalla y de las filas.' },
    { ficha: '--superficie', uso: 'Paneles y menú, un escalón por encima del fondo.' },
    { ficha: '--superficie-fuerte', uso: 'Cabeceras de tabla y elementos hundidos.' },
    { ficha: '--linea', uso: 'Bordes visibles: paneles, controles, separaciones.' },
    { ficha: '--tinta', uso: 'Texto principal.' },
    { ficha: '--apagado', uso: 'Texto secundario: etiquetas, pistas, unidades.' },
  ];

  protected readonly escala = [
    { ficha: '--texto-3xl', uso: 'El total a cobrar. Uno por pantalla, y a veces ninguno.' },
    { ficha: '--texto-2xl', uso: 'Título de la pantalla.' },
    { ficha: '--texto-xl', uso: 'Título de una sección dentro de la pantalla.' },
    { ficha: '--texto-l', uso: 'Texto destacado.' },
    { ficha: '--texto-m', uso: 'Texto de la interfaz: tablas, botones, campos.' },
    { ficha: '--texto-s', uso: 'Etiquetas de campo y textos de apoyo.' },
    { ficha: '--texto-xs', uso: 'Códigos, teclas, unidades.' },
  ];

  /** Estado de muestra para el selector de unidad. */
  protected readonly aGranel = signal(true);
  protected readonly unidad = signal('lb');

  protected readonly unidades = [
    { valor: 'lb', nombre: 'Libra' },
    { valor: 'kg', nombre: 'Kilogramo' },
    { valor: 'g', nombre: 'Gramo' },
    { valor: 'L', nombre: 'Litro' },
    { valor: 'ml', nombre: 'Mililitro' },
  ];

  protected elegirForma(aGranel: boolean): void {
    this.aGranel.set(aGranel);
  }

  protected elegirUnidad(valor: string): void {
    this.unidad.set(valor);
  }
}
