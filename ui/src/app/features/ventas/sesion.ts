/**
 * A qué sesión de caja se refieren las cifras de Ventas, dicho en una línea.
 *
 * Las tarjetas y la vista por producto ya no cuentan el día del calendario
 * sino la sesión de caja: la abierta o, si no hay ninguna, la última que se
 * cerró. Sin esta etiqueta, «Cobrado» a las 7 de la mañana enseñaría lo de
 * anoche sin avisar de que es lo de anoche.
 *
 * Es pura a propósito, como `antiguedad`: recibe el «ahora» en lugar de
 * leer el reloj, así se prueba sin trucos. Aquí solo se formatean fechas;
 * ninguna cifra de dinero pasa por este archivo.
 */

import type { SesionDeReferenciaDto } from '../../core/api.types';
import { leerFechaLocal } from '../vender/antiguedad';

function dosCifras(numero: number): string {
  return String(numero).padStart(2, '0');
}

/** «08:00». */
function horaCorta(fecha: Date): string {
  return `${dosCifras(fecha.getHours())}:${dosCifras(fecha.getMinutes())}`;
}

/** «27/09», o «27/09/2025» si no es del año en curso. */
function fechaCorta(fecha: Date, ahora: Date): string {
  const diaMes = `${dosCifras(fecha.getDate())}/${dosCifras(fecha.getMonth() + 1)}`;
  return fecha.getFullYear() === ahora.getFullYear() ? diaMes : `${diaMes}/${fecha.getFullYear()}`;
}

function mismoDia(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

/** El día de antes de `ahora`, por calendario y no restando 24 h (horario de verano). */
function esAyer(fecha: Date, ahora: Date): boolean {
  const ayer = new Date(ahora.getFullYear(), ahora.getMonth(), ahora.getDate() - 1);
  return mismoDia(fecha, ayer);
}

/** «08:00» si fue hoy, «ayer 22:00», o «27/09 22:00». */
function desdeCuando(fecha: Date, ahora: Date): string {
  if (mismoDia(fecha, ahora)) {
    return horaCorta(fecha);
  }
  if (esAyer(fecha, ahora)) {
    return `ayer ${horaCorta(fecha)}`;
  }
  return `${fechaCorta(fecha, ahora)} ${horaCorta(fecha)}`;
}

/** «27/09 08:00–20:30», o «27/09 22:00 – 28/09 06:00» si cruzó la medianoche. */
function tramo(abierta: Date, cerrada: Date | null, ahora: Date): string {
  const inicio = `${fechaCorta(abierta, ahora)} ${horaCorta(abierta)}`;
  if (!cerrada) {
    return inicio;
  }
  if (mismoDia(abierta, cerrada)) {
    return `${inicio}–${horaCorta(cerrada)}`;
  }
  return `${inicio} – ${fechaCorta(cerrada, ahora)} ${horaCorta(cerrada)}`;
}

/**
 * «Sesión abierta desde 08:00», «Sesión abierta desde ayer 22:00»,
 * «Sesión cerrada · 27/09 08:00–20:30».
 *
 * Si la base devolviera una fecha que no se entiende, se enseña tal cual:
 * mejor una fecha fea que una etiqueta inventada.
 */
export function etiquetaSesion(sesion: SesionDeReferenciaDto, ahora: Date): string {
  const abierta = leerFechaLocal(sesion.abiertaEn);

  if (sesion.abierta) {
    return `Sesión abierta desde ${abierta ? desdeCuando(abierta, ahora) : sesion.abiertaEn}`;
  }

  const cerrada = sesion.cerradaEn ? leerFechaLocal(sesion.cerradaEn) : null;
  if (!abierta || (sesion.cerradaEn && !cerrada)) {
    const hasta = sesion.cerradaEn ? ` – ${sesion.cerradaEn}` : '';
    return `Sesión cerrada · ${sesion.abiertaEn}${hasta}`;
  }
  return `Sesión cerrada · ${tramo(abierta, cerrada, ahora)}`;
}
