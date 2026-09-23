import { ChangeDetectionStrategy, Component, HostListener, inject, signal } from '@angular/core';
import { NavigationEnd, Router, RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';
import { filter } from 'rxjs';

import { Icono } from './icono';
import { Densidad, GUIA, SECCIONES, seccionDe } from './navegacion';

/**
 * El armazón de la aplicación: menú a la izquierda, pantalla a la derecha.
 *
 * Se encarga de dos cosas que ninguna pantalla debería repetir: llevar la
 * navegación por teclado y decidir la densidad con la que se dibuja la
 * pantalla activa.
 */
@Component({
  selector: 'app-armazon',
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [RouterOutlet, RouterLink, RouterLinkActive, Icono],
  templateUrl: './armazon.html',
  styleUrl: './armazon.css',
})
export class Armazon {
  private readonly router = inject(Router);

  protected readonly secciones = SECCIONES;
  protected readonly guia = GUIA;
  protected readonly densidad = signal<Densidad>('amplia');

  /** Atajo → ruta, construido una sola vez desde la tabla de secciones. */
  private readonly atajos = new Map(
    [...SECCIONES, GUIA].map((seccion) => [seccion.tecla, seccion.ruta]),
  );

  constructor() {
    this.router.events
      .pipe(filter((evento): evento is NavigationEnd => evento instanceof NavigationEnd))
      .subscribe((evento) => {
        this.densidad.set(seccionDe(evento.urlAfterRedirects)?.densidad ?? 'amplia');
      });
  }

  /**
   * Las teclas de función abren cada sección.
   *
   * Kilo12 se opera con las manos en el teclado: llegar a Vender no puede
   * costar un viaje al ratón. Se escucha en la ventana porque el atajo
   * tiene que funcionar aunque el foco esté dentro de un formulario.
   */
  @HostListener('window:keydown', ['$event'])
  protected alPulsarTecla(evento: KeyboardEvent): void {
    // Con un modificador pulsado la tecla significa otra cosa.
    if (evento.ctrlKey || evento.metaKey || evento.altKey) {
      return;
    }

    const ruta = this.atajos.get(evento.key);
    if (!ruta) {
      return;
    }

    evento.preventDefault();
    void this.router.navigate([ruta]);
  }
}
