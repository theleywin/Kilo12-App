/**
 * Cuánto hace que se apartó una venta, dicho como lo diría una persona.
 *
 * Las ventas en espera no caducan (RF-VTA-14), así que la lista enseña su
 * antigüedad: una de hace tres días salta a la vista y se decide qué hacer
 * con ella.
 *
 * Es una función pura a propósito: recibe el «ahora» en lugar de leer el
 * reloj, así se puede probar sin trucos y no hay dos maneras de contar el
 * tiempo en la misma pantalla.
 */

/** `YYYY-MM-DD HH:MM:SS`, como lo guarda la base, en hora local. */
const FECHA_BASE = /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2}):(\d{2})$/;

const MINUTO = 60_000;
const HORA = 60 * MINUTO;
const DIA = 24 * HORA;

/**
 * Lee una fecha de la base como hora local.
 *
 * No se usa `Date.parse`: con el espacio en medio cada motor web hace lo
 * que quiere, y WebKit la rechaza.
 */
export function leerFechaLocal(texto: string): Date | null {
  const partes = FECHA_BASE.exec(texto.trim());
  if (!partes) {
    return null;
  }

  const [, anio, mes, dia, hora, minuto, segundo] = partes.map(Number);
  return new Date(anio, mes - 1, dia, hora, minuto, segundo);
}

/** «hace un momento», «hace 20 min», «hace 3 h», «hace 3 días». */
export function antiguedad(creadaEn: string, ahora: Date): string {
  const creada = leerFechaLocal(creadaEn);
  if (!creada) {
    return creadaEn;
  }

  // Un reloj que se atrasó no debe producir «hace -2 min».
  const transcurrido = Math.max(0, ahora.getTime() - creada.getTime());

  if (transcurrido < MINUTO) {
    return 'hace un momento';
  }
  if (transcurrido < HORA) {
    return `hace ${Math.floor(transcurrido / MINUTO)} min`;
  }
  if (transcurrido < DIA) {
    return `hace ${Math.floor(transcurrido / HORA)} h`;
  }

  const dias = Math.floor(transcurrido / DIA);
  return dias === 1 ? 'hace 1 día' : `hace ${dias} días`;
}
