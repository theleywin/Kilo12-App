import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';

import {
  AlmacenDto,
  CambioPrecioDto,
  CierreCajaPedido,
  CierreCalculadoDto,
  CobroCalculadoDto,
  ConteoCalculadoDto,
  EstadoCajaDto,
  HistorialVentasDto,
  InformeDto,
  ErrorDto,
  FichaProductoDto,
  MargenDto,
  MovimientoDto,
  NuevaEntradaDto,
  NuevaMermaDto,
  NuevoProductoDto,
  LineaVentaDto,
  NuevoTraspasoDto,
  PagoDto,
  ProductoDto,
  ProductoVendibleDto,
  SimulacionDto,
  SesionListadaDto,
  VentaDetalladaDto,
  VentaHechaDto,
  VentaPrevistaDto,
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

  // -------------------------------------------------------- vender

  /** Devuelve lo que se puede vender ahora mismo, con su existencia. */
  catalogoDeVenta(): Promise<ProductoVendibleDto[]> {
    return invoke<ProductoVendibleDto[]>('catalogo_de_venta');
  }

  /**
   * Calcula la venta en curso sin tocar nada.
   *
   * La pantalla no suma dinero: pregunta. Y así se entera de si algo no
   * cabe en la vitrina antes de intentar cobrarlo.
   */
  previsualizarVenta(lineas: LineaVentaDto[]): Promise<VentaPrevistaDto> {
    return invoke<VentaPrevistaDto>('previsualizar_venta', { lineas });
  }

  /**
   * Dice si lo entregado cubre la venta y cuánto hay que devolver.
   *
   * Se pregunta mientras se teclea: el vuelto se cuenta con los billetes
   * en la mano, no después de confirmar.
   */
  calcularCobro(total: string, pagos: PagoDto[]): Promise<CobroCalculadoDto> {
    return invoke<CobroCalculadoDto>('calcular_cobro', { total, pagos });
  }

  /**
   * Cobra una venta.
   *
   * Va entera en una sola llamada: si algo falla no se guarda nada, ni
   * media venta ni medio descuento de inventario.
   */
  vender(venta: {
    lineas: LineaVentaDto[];
    pagos: PagoDto[];
  }): Promise<VentaHechaDto> {
    return invoke<VentaHechaDto>('vender', { venta });
  }

  /**
   * El historial de ventas con el corte del día (RF-VTA-16).
   *
   * Los totales y la ganancia vienen calculados: la ganancia sale del costo
   * que tenía la mercancía cuando se vendió, no del de hoy.
   */
  consultarVentas(limite?: number): Promise<HistorialVentasDto> {
    return invoke<HistorialVentasDto>('consultar_ventas', { limite });
  }

  /** Una venta concreta, con sus líneas y sus formas de pago. */
  consultarVenta(id: number): Promise<VentaDetalladaDto> {
    return invoke<VentaDetalladaDto>('consultar_venta', { id });
  }

  // ------------------------------------------------- caja (RF-CAJ)

  /**
   * La caja abierta, o `null` si no hay ninguna.
   *
   * Que no haya caja no es un error: es el estado normal antes de empezar
   * el día, y la pantalla tiene que poder dibujarlo.
   */
  consultarCaja(): Promise<EstadoCajaDto | null> {
    return invoke<EstadoCajaDto | null>('consultar_caja');
  }

  /** Abre la caja con su fondo inicial y devuelve su identificador. */
  abrirCaja(apertura: { operador: string; fondoInicial: string }): Promise<number> {
    return invoke<number>('abrir_caja', { apertura });
  }

  /** Registra una entrada o salida de efectivo, con motivo obligatorio. */
  moverEfectivo(movimiento: { tipo: string; importe: string; motivo: string }): Promise<void> {
    return invoke<void>('mover_efectivo', { movimiento });
  }

  /** Enseña cómo quedaría el cierre sin cerrar nada. */
  previsualizarCierre(cierre: CierreCajaPedido): Promise<CierreCalculadoDto> {
    return invoke<CierreCalculadoDto>('previsualizar_cierre', { cierre });
  }

  /** Cierra la caja y congela su arqueo. No tiene vuelta atrás. */
  cerrarCaja(cierre: CierreCajaPedido): Promise<CierreCalculadoDto> {
    return invoke<CierreCalculadoDto>('cerrar_caja', { cierre });
  }

  /** Historial de sesiones de caja. */
  listarCajas(limite?: number): Promise<SesionListadaDto[]> {
    return invoke<SesionListadaDto[]>('listar_cajas', { limite });
  }

  /** El cierre congelado de una sesión ya cerrada. */
  consultarCierre(id: number): Promise<CierreCalculadoDto> {
    return invoke<CierreCalculadoDto>('consultar_cierre', { id });
  }

  /**
   * Anula una venta de la sesión vigente (RF-VTA-15).
   *
   * Devuelve la mercancía a vitrina. Las ventas de cajas ya cerradas no son
   * anulables: su arqueo se hizo contra dinero físico.
   */
  anularVenta(anulacion: { venta: number; motivo: string }): Promise<void> {
    return invoke<void>('anular_venta', { anulacion });
  }

  /**
   * El informe de un periodo (RF-EST).
   *
   * Todo llega en una sola llamada: los informes se miran juntos y
   * partirlos en siete consultas solo conseguiría que la pantalla enseñara
   * un rato cifras que no se corresponden entre sí.
   */
  consultarInforme(desde: string, hasta: string, dias: number): Promise<InformeDto> {
    return invoke<InformeDto>('consultar_informe', { desde, hasta, dias });
  }

  /**
   * Borra todos los datos de la aplicación. **No tiene vuelta atrás.**
   *
   * La clave la comprueba Rust, no esta pantalla: validar solo aquí sería
   * un cerrojo que se abre con las herramientas de desarrollo.
   */
  borrarTodosLosDatos(clave: string): Promise<void> {
    return invoke<void>('borrar_todos_los_datos', { clave });
  }

  /** Denominaciones con que se cuenta la caja, de menor a mayor. */
  denominacionesEfectivo(): Promise<number[]> {
    return invoke<number[]>('denominaciones_efectivo');
  }

  /**
   * Suma un recuento de billetes.
   *
   * `cuantos` va en el mismo orden que `denominacionesEfectivo`. La suma la
   * hace Rust: es la cifra contra la que se arquea la caja y no se calcula
   * en la pantalla, como ninguna otra cifra de dinero.
   */
  contarEfectivo(cuantos: number[]): Promise<ConteoCalculadoDto> {
    return invoke<ConteoCalculadoDto>('contar_efectivo', { cuantos });
  }

  /** Porcentaje de comisión del operador, si está configurado. */
  consultarComision(): Promise<string | null> {
    return invoke<string | null>('consultar_comision');
  }

  /** Fija el porcentaje de comisión del operador. */
  fijarComision(porcentaje: string): Promise<void> {
    return invoke<void>('fijar_comision', { porcentaje });
  }

  /** Tasa de cambio vigente, o nada si todavía no se fijó. */
  consultarTasa(): Promise<string | null> {
    return invoke<string | null>('consultar_tasa');
  }

  /** Fija la tasa con la que se convierten los dólares. */
  fijarTasa(tasa: string): Promise<void> {
    return invoke<void>('fijar_tasa', { tasa });
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
