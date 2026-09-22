# Kilo12-App

Punto de venta y control de inventario para un mercadito, construido con **Rust + Tauri**.
Opera **100 % offline**: toda la información reside en el equipo del usuario, sin servidor ni nube.

## Documentación

| Documento | Descripción |
|---|---|
| [`docs/requerimientos-funcionales.md`](docs/requerimientos-funcionales.md) | Requerimientos funcionales del proyecto. Fuente de verdad. |
| `docs/requerimientos-funcionales.pdf` | Misma documentación en PDF. **Artefacto derivado: no se edita a mano.** |

### Regenerar el PDF

El PDF nunca se edita directamente. Se edita el `.md` y se regenera:

```bash
python3 scripts/build-pdf.py docs/requerimientos-funcionales.md
```

Requiere XeLaTeX (MacTeX o TeXLive). No necesita conexión a internet.

El script avisa si detecta glifos ausentes en la fuente o desbordes de margen; ambos
son errores silenciosos en LaTeX que, de otro modo, solo se descubren al leer el PDF.
