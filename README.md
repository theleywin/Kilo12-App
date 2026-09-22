# Kilo12-App

Punto de venta y control de inventario para un mercadito, construido con **Rust + Tauri**.
Opera **100 % offline**: toda la información reside en el equipo del usuario, sin servidor ni nube.

## Documentación

| Documento | Descripción |
|---|---|
| [`docs/requerimientos-funcionales.md`](docs/requerimientos-funcionales.md) | Qué hace el sistema. Requerimientos funcionales. Fuente de verdad. |
| [`docs/diseno-tecnico.md`](docs/diseno-tecnico.md) | Cómo se construye. Decisiones técnicas, arquitectura y esquema de datos. |
| `docs/*.pdf` | Los mismos documentos en PDF. **Artefactos derivados: no se editan a mano.** |

## Plataformas

Windows 10 o superior (64 bits) es la plataforma principal; macOS es secundaria.
Los ejecutables se producen por integración continua, un runner por sistema operativo.

### Regenerar los PDF

Un PDF nunca se edita directamente. Se edita el `.md` y se regenera:

```bash
python3 scripts/build-pdf.py docs/requerimientos-funcionales.md
python3 scripts/build-pdf.py docs/diseno-tecnico.md
```

Requiere XeLaTeX (MacTeX o TeXLive). No necesita conexión a internet.

El script avisa si detecta glifos ausentes en la fuente o desbordes de margen; ambos
son errores silenciosos en LaTeX que, de otro modo, solo se descubren al leer el PDF.
