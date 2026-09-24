<p align="center">
  <img src="docs/logotipo.svg" alt="Kilo12" width="420">
</p>

<p align="center">
  <strong>Punto de venta e inventario para un mercadito.</strong><br>
  Funciona sin internet. Los datos no salen del equipo.
</p>

---

## Para qué sirve

Un mercadito pequeño vive de márgenes estrechos y de decisiones que se toman de
memoria: *¿me queda arroz?*, *¿este producto deja algo o lo estoy regalando?*,
*¿cuadró la caja hoy?*. Cuando esas respuestas están en la cabeza de una persona
y en un cuaderno, se pierden.

Kilo12 las contesta con números exactos:

**Cobra rápido, sin soltar el teclado.** La pantalla de venta se usa con gente
esperando: se busca el producto escribiendo, se elige con las flechas y se cobra
con Enter. Admite **pagos mixtos** —algo en efectivo, algo por transferencia y
algo en dólares— y calcula el vuelto, que siempre se devuelve en pesos.

**Sabe lo que de verdad ganas.** Cada venta congela el costo que tenía la
mercancía en ese momento, así que la ganancia de ayer no cambia cuando sube el
precio de compra mañana. El sistema distingue siempre tres cosas que es fácil
confundir: lo que **vendiste**, lo que **ganaste** y lo que te **queda** después
de la comisión de quien atiende.

**Cuadra la caja.** Cada turno se abre con un fondo y se cierra contando el
dinero físico, billete a billete. El sistema dice cuánto debería haber —sin
contar las transferencias, que nunca pasaron por la gaveta— y señala el
sobrante o el faltante. Una caja cerrada no se puede modificar.

**Avisa de lo que no ves.** Qué producto se está acabando y en cuántos días,
qué mercancía lleva semanas parada con tu dinero dentro, y qué tienes guardado
en el almacén sin exhibir —que es venta perdida, porque el cliente no lo ve.

**Te dice cuándo y qué se vende.** Informes con la evolución del negocio día a
día, las horas de más trabajo, en qué se paga y qué productos sostienen el
mostrador. Lo más vendido casi nunca es lo más rentable, y verlos juntos cambia
las decisiones de compra.

## Por qué sin internet

Porque la tienda no puede parar cuando se cae la conexión. Kilo12 no necesita
servidor ni cuenta: se instala, se abre y funciona. Toda la información vive en
un archivo en el equipo y **nunca sale de ahí**.

## Cómo está hecho

| Capa | Herramienta | Por qué |
|---|---|---|
| Núcleo | **Rust** | Aritmética entera exacta. El dinero nunca se representa con decimales flotantes. |
| Interfaz | **Angular** | Pantallas rápidas y tipadas. |
| Escritorio | **Tauri** | Usa el motor web del sistema: el instalador pesa unos pocos megabytes. |
| Datos | **SQLite** | Un solo archivo, sin instalar nada aparte. |

La arquitectura es hexagonal: las reglas del negocio no saben que existe una
base de datos ni una pantalla. **Ninguna cifra de dinero se calcula en la
interfaz** — todos los totales, márgenes y vueltos los produce el núcleo y
llegan ya formateados.

## Instalación

Descarga el instalador de tu sistema desde las
[publicaciones del repositorio](../../releases):

- **Windows**: ejecuta el `.exe`. Trae el motor web dentro, no hace falta nada más.
- **macOS**: abre el `.dmg` y arrastra Kilo12 a Aplicaciones. La primera vez
  ábrelo con clic derecho → Abrir, porque la aplicación no está firmada.

## Documentación

| Documento | Descripción |
|---|---|
| [`docs/requerimientos-funcionales.md`](docs/requerimientos-funcionales.md) | Qué hace el sistema. Fuente de verdad. |
| [`docs/diseno-tecnico.md`](docs/diseno-tecnico.md) | Cómo se construye: decisiones, arquitectura y esquema de datos. |
| `docs/*.pdf` | Los mismos documentos en PDF. **Artefactos derivados: no se editan a mano.** |

## Desarrollo

```bash
cargo install tauri-cli --version "^2"   # una sola vez
cd src-tauri && cargo tauri dev
```

Angular recarga en caliente; Rust recompila al cambiar un crate.

```bash
cargo test --workspace                              # las pruebas
cargo clippy --workspace --all-targets -- -D warnings
```

### Datos de prueba

Para ver los informes con volumen, sin inventar nada a mano:

```bash
cargo run -p infrastructure --example sembrar -- --dias 20
```

Cobra ventas ficticias **por los casos de uso reales**, así que descuentan
existencia y dejan asiento igual que una venta de verdad.

### Regenerar los PDF

Un PDF nunca se edita directamente. Se edita el `.md` y se regenera:

```bash
python3 scripts/build-pdf.py docs/requerimientos-funcionales.md
python3 scripts/build-pdf.py docs/diseno-tecnico.md
```

Requiere XeLaTeX (MacTeX o TeXLive). No necesita conexión a internet.
