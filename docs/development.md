# Entorno de desarrollo

## Rust

Toolchain fijado en `Cargo.toml` (`rust-version = "1.97"`). Desde la raíz del repositorio:

```powershell
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
cargo build --release --locked --workspace --bins
```

Detalles del motor en [native/README](../native/README.md).

Si el repositorio está dentro de una carpeta sincronizada, Cargo puede compilar fuera de ella con
`--target-dir`, por ejemplo `$env:LOCALAPPDATA/Liblevo/cargo-target`.

## App Tauri

`app/` es el backend Tauri 2 y `ui/` la interfaz en TypeScript sin framework (Vite 7). Durante el
desarrollo los componentes se pueden indicar con variables de entorno:

- `LIBLEVO_PDFIUM`: ruta de `pdfium.dll` (por ejemplo, la de `.venv/Lib/site-packages/pypdfium2_raw/`).
- `LIBLEVO_XBERG`: carpeta con `xberg.exe` 1.3.10 y sus dos DLL (huellas fijadas en `xberg_reader.rs`).
- `LIBLEVO_XBERG_MODELS`: carpeta con los modelos de maquetación y OCR de Xberg.
- `LIBLEVO_OVIS` y `LIBLEVO_OVIS_MODEL`: programa de llama.cpp y modelo OvisOCR2 (relectura visual).
- `LIBLEVO_TRADUCCION`, `LIBLEVO_TRADUCCION_RAPIDA` y `LIBLEVO_TRADUCCION_CALIDAD`: programa de
  traducción de llama.cpp y carpetas con Hy-MT2 1,8B y 7B (huellas en `translate.rs`).

Sin esas variables, la app usa los componentes que descarga su catálogo. El instalador de Windows
se construye con `.venv/Scripts/python.exe scripts/build_installer.py` (comprueba la huella de
PDFium y deja el instalador en `target/release/bundle/nsis/`). La traducción se mide con
`scripts/eval_translation.py` y la herramienta `liblevo-traducir`.

```powershell
npm --prefix ui install
npm --prefix ui run check          # tipos TypeScript
npm --prefix ui run dev            # servidor local de la interfaz, solo en 127.0.0.1
cargo run -p liblevo-app           # en otra consola, con el servidor anterior abierto
```

La prueba de integración real de la app (`conversion::tests::real_conversion…`) se ejecuta solo si
están definidas esas variables y `LIBLEVO_TEST_PDF`.

Para medir la conversión de la app con el conjunto:

```powershell
cargo build --release -p liblevo-worker --bin liblevo-convert
.venv/Scripts/python.exe scripts/eval_cli_candidate.py --candidate liblevo --tools <herramientas> `
  --pdf-dir <pdf> --out <salida>/liblevo
```

La política de contenido de `app/tauri.conf.json` bloquea cualquier origen remoto; la ventana solo
tiene permiso para sus comandos y para los diálogos de abrir y guardar.

## Herramientas Python

Python 3.12 solo se usa para las herramientas de `scripts/` (medición con el conjunto e
instalador); no forma parte de la app:

```powershell
py -3.12 -m venv .venv
.venv/Scripts/python.exe -m pip install "psutil==7.2.2" "pypdfium2==5.12.1" "ruff==0.15.22"
.venv/Scripts/python.exe -m ruff check .
.venv/Scripts/python.exe -m ruff format --check .
```

La CI comprueba además que el EPUB de `eval/muestras/epub-muestra.md` pasa EPUBCheck 5.3.0.

## Datos locales

Guarda las salidas, auditorías y documentos privados fuera del repositorio. No los subas a git.
