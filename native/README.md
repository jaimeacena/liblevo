# Motor de Liblevo

Tres crates de Rust; el diseño está en la [arquitectura](../docs/architecture.md).

- `liblevo-core`: exportación Markdown → EPUB 3, edición de un EPUB ajeno en su sitio y fórmulas
  sencillas.
- `liblevo-worker`: lectura con Xberg y PDFium, pulido, escaneos y relectura visual con OvisOCR2,
  lista de revisión y traducción con Hy-MT2. Herramientas de desarrollo en `src/bin/`:
  - `liblevo-convert`: convierte un documento como la app (para medir con el conjunto);
  - `liblevo-posproceso`: aplica el pulido a la salida de un lector;
  - `liblevo-revisar`: calcula la lista de revisión de un resultado;
  - `liblevo-traducir`: traduce un resultado o un EPUB;
  - `liblevo-epub`: abre y guarda un EPUB ajeno para comprobar que se conserva.
- `liblevo-eval`: mide resultados con el conjunto público ([evaluación](../docs/evaluation.md)).

`third-party/` guarda la licencia de Xberg. Ninguna herramienta escribe contenido documental en
la salida estándar: solo recuentos.

## Comprobaciones

```powershell
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
cargo build --release --locked --workspace --bins
```
