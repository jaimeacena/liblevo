# Entorno de desarrollo

Usar Python 3.12 en Windows y el lock canónico, también para pruebas locales. Desde el checkout:

```powershell
py -3.12 -m venv .venv
.venv/Scripts/python.exe -m pip install --require-hashes -r requirements.lock
.venv/Scripts/python.exe -m pip install -e . --no-deps --no-build-isolation
.venv/Scripts/python.exe -m pip check
.venv/Scripts/python.exe -m ruff check .
.venv/Scripts/python.exe -m ruff format --check .
.venv/Scripts/python.exe -m mypy src/liblevo
.venv/Scripts/python.exe -m pytest --cov=liblevo --cov-report=term-missing
```

Para distribución Windows usar un entorno separado con `requirements-windows-cpu.lock`, siguiendo
[la guía de publicación](releasing.md). No mezclar sus variantes de PyTorch con el entorno canónico.
No se requiere uv; instalar solo desde `pyproject.toml` no reproduce las versiones verificadas.

El motor directo de modelos para Windows 11 se añade después del lock CPU con
`requirements-windows-vulkan.lock`, que fija por hash `llama-cpp-python 0.3.35` Vulkan y `diskcache`.
La app instalada necesita ese motor incluido en el paquete: una instalación solo con el lock CPU
no ofrece la traducción directa. Para probarlo en un entorno nuevo, sin instalar nada global:

```powershell
.venv/Scripts/python.exe -m pip install --no-deps --require-hashes -r requirements-windows-vulkan.lock
.venv/Scripts/python.exe -m pip check
```

El empaquetado debe recoger las bibliotecas dinámicas de `llama_cpp`, regenerar los avisos de
terceros con el motor instalado y verificar que el ejecutable abre sin Ollama. La prueba sintética
directa y el informe local se guardan en `audits/2026-09-26-direct-runtime-01/`.

Para construir un **paquete local** desde un entorno preparado con ambos locks, indica su Python al
script. El primer comando solo comprueba Python 3.12, las versiones fijadas y el soporte Vulkan; el
segundo crea y prueba el ejecutable sin instalar la aplicación ni descargar un modelo:

```powershell
.\distribution\windows\build.ps1 -PreflightOnly -PythonPath .\.package-venv\Scripts\python.exe
.\distribution\windows\build.ps1 -PythonPath .\.package-venv\Scripts\python.exe
```

El script construye primero en una carpeta candidata de `outputs/package`. Solo después del smoke
mueve el candidato a `outputs/package/Liblevo`; si existía un paquete anterior, lo conserva como
`Liblevo.previous-<identificador>`. Un fallo previo deja el paquete anterior en su lugar y conserva
los archivos temporales para diagnóstico. Tras un resultado correcto, limpia solo las carpetas
temporales creadas por esa ejecución; la caché de PyInstaller se dirige a esas carpetas, no a la
caché compartida de Windows. La opción `-InstallInnoSetup` requiere que Inno Setup ya esté
instalado y no lo instala por su cuenta.

## Cambiar dependencias

Modificar las dependencias directas en `pyproject.toml`. Pillow pertenece a producto porque PDF y OCR
la importan directamente. Regenerar ambos locks con el `pip-tools` fijado, sin `--upgrade` salvo que
la actualización de versiones sea el propósito explícito del cambio:

```powershell
.venv/Scripts/python.exe -m piptools compile --allow-unsafe --extra=dev --extra=packaging --generate-hashes --strip-extras --output-file=requirements.lock pyproject.toml
.venv/Scripts/python.exe -m piptools compile --allow-unsafe --extra-index-url=https://download.pytorch.org/whl/cpu --extra=dev --extra=packaging --generate-hashes --strip-extras --output-file=requirements-windows-cpu.lock pyproject.toml
```

Revisar versiones, hashes y paridad entre locks antes de instalar; no aceptar cambios transitivos
ajenos al propósito. Ejecutar los controles anteriores y el auditor de dependencias descrito en
[seguridad de dependencias](dependency-security.md). Los cambios de motores requieren además las
comprobaciones reales de [aceptación](acceptance-checklist.md) y regenerar los avisos de terceros.
