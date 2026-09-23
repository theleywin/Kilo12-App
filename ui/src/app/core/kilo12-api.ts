import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';

import { ErrorDto, MargenDto, NuevoProductoDto, ProductoDto } from './api.types';

/**
 * Única puerta de entrada al núcleo en Rust.
 *
 * Ningún componente llama a `invoke` directamente: si mañana cambia el
 * mecanismo de comunicación, se cambia aquí y nada más.
 */
@Injectable({ providedIn: 'root' })
export class Kilo12Api {
  /** Lista el catálogo. */
  listarProductos(incluirInactivos = false): Promise<ProductoDto[]> {
    return invoke<ProductoDto[]>('listar_productos', { incluirInactivos });
  }

  /** Da de alta un producto y devuelve su identificador. */
  registrarProducto(producto: NuevoProductoDto): Promise<number> {
    return invoke<number>('registrar_producto', { producto });
  }

  /**
   * Calcula la ganancia y el margen de un precio frente a su costo.
   *
   * La cuenta la hace Rust a propósito: en la interfaz no se hace
   * aritmética con dinero.
   */
  calcularMargen(costo: string, precio: string): Promise<MargenDto> {
    return invoke<MargenDto>('calcular_margen', { costo, precio });
  }
}

/**
 * Interpreta lo que rechaza un comando.
 *
 * Tauri propaga el error serializado tal cual, pero un fallo de la propia
 * llamada llega como cualquier otra cosa. Esto garantiza que la interfaz
 * siempre tenga un código y un mensaje que mostrar.
 */
export function comoError(valor: unknown): ErrorDto {
  if (
    typeof valor === 'object' &&
    valor !== null &&
    'codigo' in valor &&
    'mensaje' in valor
  ) {
    return valor as ErrorDto;
  }

  return {
    codigo: 'DESCONOCIDO',
    mensaje: String(valor),
    delUsuario: false,
  };
}
