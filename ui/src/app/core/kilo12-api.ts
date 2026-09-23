import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';

import {
  AlmacenDto,
  CambioPrecioDto,
  ErrorDto,
  FichaProductoDto,
  MargenDto,
  MovimientoDto,
  NuevaEntradaDto,
  NuevaMermaDto,
  NuevoProductoDto,
  NuevoTraspasoDto,
  ProductoDto,
  SimulacionDto,
  VitrinaDto,
} from './api.types';

/**
 * Única puerta de entrada al núcleo en Rust.
 *
 * Ningún componente llama a `invoke` directamente: si mañana cambia el
 * mecanismo de comunicación, se cambia aquí y nada más.
 */
@Injectable({ providedIn: 'root' })
export class Kilo12Api {
  /**
   * Lista el catálogo, ordenado por la columna que se pida.
   *
   * El orden lo resuelve Rust: comparar márgenes es comparar dinero.
   */
  listarProductos(
    incluirInactivos = false,
    orden?: string,
    descendente = false,
  ): Promise<ProductoDto[]> {
    return invoke<ProductoDto[]>('listar_productos', {
      incluirInactivos,
      orden,
      descendente,
    });
  }

  /** Da de alta un producto y devuelve su identificador. */
  registrarProducto(producto: NuevoProductoDto): Promise<number> {
    return invoke<number>('registrar_producto', { producto });
  }

  // ------------------------------------------------ ficha comercial

  /** Devuelve la ficha de un producto con todas sus presentaciones. */
  consultarProducto(producto: number): Promise<FichaProductoDto> {
    return invoke<FichaProductoDto>('consultar_producto', { producto });
  }

  /** Cambia el nombre, el mínimo o el estado de un producto. */
  editarProducto(edicion: {
    producto: number;
    nombre: string;
    stockMinimo?: string;
    activo: boolean;
  }): Promise<void> {
    return invoke<void>('editar_producto', { edicion });
  }

  /** Agrega una forma de vender el producto. */
  agregarPresentacion(presentacion: {
    producto: number;
    nombre: string;
    factor: string;
    precio: string;
    codigoBarras?: string;
  }): Promise<void> {
    return invoke<void>('agregar_presentacion', { presentacion });
  }

  /** Cambia el precio de una presentación y lo anota en el historial. */
  cambiarPrecio(cambio: {
    producto: number;
    presentacion: number;
    precio: string;
  }): Promise<void> {
    return invoke<void>('cambiar_precio', { cambio });
  }

  /** Retira una presentación de la venta sin borrarla. */
  desactivarPresentacion(producto: number, presentacion: number): Promise<void> {
    return invoke<void>('desactivar_presentacion', { referencia: { producto, presentacion } });
  }

  /** Elige la presentación que usa la venta rápida. */
  marcarPredeterminada(producto: number, presentacion: number): Promise<void> {
    return invoke<void>('marcar_predeterminada', { referencia: { producto, presentacion } });
  }

  /** Devuelve cómo ha ido cambiando el precio de un producto. */
  consultarHistorialPrecios(producto: number): Promise<CambioPrecioDto[]> {
    return invoke<CambioPrecioDto[]>('consultar_historial_precios', { producto });
  }

  /**
   * Calcula el precio que hay que cobrar para dejar el margen pedido.
   *
   * Es el camino inverso del margen, y la cuenta la hace Rust: dividir
   * entre `1 − margen` en JavaScript arrastraría imprecisión.
   */
  calcularPrecioParaMargen(costo: string, margen: string): Promise<string> {
    return invoke<string>('calcular_precio_para_margen', { costo, margen });
  }

  /** Devuelve el estado del almacén: qué hay y cuánto vale. */
  consultarAlmacen(): Promise<AlmacenDto> {
    return invoke<AlmacenDto>('consultar_almacen');
  }

  /** Registra la entrada de mercancía de una compra. */
  registrarEntrada(entrada: NuevaEntradaDto): Promise<void> {
    return invoke<void>('registrar_entrada', { entrada });
  }

  /** Da de baja mercancía perdida. */
  registrarMerma(merma: NuevaMermaDto): Promise<void> {
    return invoke<void>('registrar_merma', { merma });
  }

  /** Devuelve el estado de la vitrina y qué hace falta reponer. */
  consultarVitrina(): Promise<VitrinaDto> {
    return invoke<VitrinaDto>('consultar_vitrina');
  }

  /** Fija cuánto se quiere mantener exhibido de un producto. */
  fijarObjetivoVitrina(producto: number, objetivo: string): Promise<void> {
    return invoke<void>('fijar_objetivo_vitrina', { objetivo: { producto, objetivo } });
  }

  /**
   * Mueve mercancía entre el almacén y la vitrina.
   *
   * No lleva costo: no se le compró nada a nadie, solo cambia de sitio.
   */
  traspasar(traspaso: NuevoTraspasoDto): Promise<void> {
    return invoke<void>('traspasar', { traspaso });
  }

  /**
   * Pregunta cómo quedaría la existencia si el movimiento se hiciera.
   *
   * La cuenta la hace Rust con las mismas reglas que ejecutarían el
   * movimiento de verdad: una vista previa que discrepe del resultado es
   * peor que no tenerla.
   */
  simularMovimiento(consulta: {
    producto: number;
    tipo: string;
    cantidad: string;
    ubicacion: string;
  }): Promise<SimulacionDto> {
    return invoke<SimulacionDto>('simular_movimiento', { consulta });
  }

  /** Devuelve el historial de movimientos de un producto. */
  consultarKardex(producto: number, limite?: number): Promise<MovimientoDto[]> {
    return invoke<MovimientoDto[]>('consultar_kardex', { producto, limite });
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
