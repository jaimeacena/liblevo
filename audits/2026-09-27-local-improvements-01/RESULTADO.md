# Parsezen · mejoras locales y pruebas del 27 de septiembre de 2026

## Resumen

El objetivo confirmado sigue siendo convertir un PDF largo de otro idioma en un EPUB español cómodo
para Kindle, sin depender de Ollama. Esta intervención reduce un riesgo concreto de mantenimiento:
**el paquete de Windows ya no borra el anterior antes de construir y probar el nuevo**. También
comprobé con un PDF sintético que una cancelación durante la traducción puede reanudar fragmentos ya
guardados, y expliqué en la guía qué archivos terminados conviene respaldar.

**Estado para probar un libro personal largo: PREPARACIÓN NO DETERMINADA.** El paquete local pasa su
arranque de prueba y la recuperación cooperativa funcionó en el caso sintético. La calidad de un
capítulo real, una interrupción brusca, el uso interactivo del paquete, el instalador y la lectura en
Kindle siguen pendientes. No se usaron libros ni datos habituales de Jaime.

## 1. Paquete local con IA integrada

**Problema demostrado antes del cambio.** [`build.ps1`](../../distribution/windows/build.ps1)
eliminaba `outputs/package/Parsezen` antes de generar avisos, construir o ejecutar el smoke. A la vez,
el paquete exige `llama_cpp`, pero el guion no comprobaba el runtime de Windows. Un fallo podía dejar
sin el paquete anterior y dar un mensaje tardío. La opción de instalador podía intentar instalar
Inno Setup globalmente.

**Cambio IMPLEMENTADO.** [`check_runtime.py`](../../distribution/windows/check_runtime.py) exige
Python 3.12, `llama-cpp-python` 0.3.35, `diskcache` 5.6.3, PyInstaller y una compilación GGUF con
soporte Vulkan antes de crear salidas. El guion construye en una carpeta candidata única, genera allí
los avisos, arranca el ejecutable candidato con `--package-smoke` y solo después lo coloca en la ruta
habitual. Si había un paquete, lo renombra como `Parsezen.previous-<identificador>`; no lo borra. El
guion acepta el Python de un entorno preparado mediante `-PythonPath`. Inno Setup debe estar instalado
de antemano; el guion no hace instalaciones globales. Tras un resultado correcto limpia únicamente
su caché y carpeta candidata, tras verificar sus rutas. Si falla antes, conserva esas carpetas para
diagnóstico y deja el paquete previo en su lugar. Después de la construcción de este candidato se
detectó que `--clean` había usado la caché estándar de PyInstaller en Windows; el guion se ajustó para
dirigir **futuras** construcciones a su propia carpeta temporal y restaurar la variable de entorno.

**Verificación.** La comprobación previa pasó en este Windows 11. Dos nuevas pruebas comprueban el
rechazo de una versión GGUF equivocada y de un motor sin Vulkan. El nuevo paquete PyInstaller se
construyó y superó `--package-smoke`, que abre una ventana Qt aislada e importa los componentes de
OCR, Argos, Docling y `llama_cpp`. Tiene **7.535 archivos y 1.152.963.941 bytes**; el anterior,
conservado, tiene **7.520 archivos y 1.090.638.951 bytes**. Las DLL `llama.dll` y
`ggml-vulkan.dll` están presentes. [Hashes y datos del paquete](package-evidence.json).

**Límite.** Ollama seguía encendido durante el smoke: este modo comprueba arranque e imports, no una
traducción desde el ejecutable ni funcionamiento del instalador sin Ollama. La ruta directa desde
Python ya se había ensayado con HTTP bloqueado en la revisión anterior. El workflow remoto de
paquetes aún instala solo el lock CPU y no se modificó ni ejecutó por quedar fuera del trabajo local.
El aislamiento de caché añadido al final se verificó en el código local de PyInstaller y con análisis
sintáctico; no se reconstruyó otro paquete solo para medir ese cambio. La caché compartida que
PyInstaller limpió en la primera construcción no se manipuló después.

## 2. Recuperación de una traducción interrumpida

**Hipótesis.** Una cancelación durante PDF→EPUB con el modelo integrado debe dejar progreso
reutilizable, sin publicar un EPUB incompleto ni alterar el PDF.

**Prueba VERIFICADA EN EJECUCIÓN, sintética.** El [guion del ensayo](exercise_resume.py) usó el PDF
sintético de 20 páginas de la auditoría anterior, rutas aisladas y HTTP bloqueado. Canceló en el
avance **5 de 40** fragmentos. Había **24 archivos intermedios** y ningún EPUB publicado. Al volver
a procesar con la misma configuración, **4 fragmentos** se reutilizaron; el EPUB final abrió como
ZIP válido, tenía navegación y conservó las **18 cifras de control**. El SHA-256 del PDF original no
cambió. [Resultados numéricos](resume-trial-2.json).

El [primer intento](resume-attempts.json) falló antes del procesamiento porque el guion diagnóstico
no había creado la carpeta de salida exigida por Parsezen. Se corrigió el guion y se usó una carpeta
nueva, sin ocultar el fallo. La reanudación coincidió con una suspensión prolongada de Windows: el
tiempo de pared registrado **no sirve para comparar rendimiento**. Esta prueba cubre cancelación
cooperativa, no matar el proceso o cortar la alimentación. Tampoco mide calidad lingüística.

## 3. Copias comprensibles para una persona

La [guía de uso](../../docs/user-guide.md) ahora explica cómo localizar y copiar el EPUB terminado y
el PDF original, y pide abrir la copia del EPUB en Calibre. Aclara que los artefactos cifrados de un
trabajo pendiente dependen de la cuenta de Windows y **no constituyen un respaldo portable**. Es una
corrección documental; no se añadió un botón ni se probó restaurar trabajo pendiente en otro PC.

## 4. Validación y cambios exactos

- Pruebas focales del paquete: **11 PASS**; preflight real y análisis sintáctico de PowerShell:
  **PASS**. La función exacta que limpia temporales se ejecutó sobre carpetas sintéticas: retiró solo
  el objetivo elegido y dejó intacta una carpeta hermana. Los temporales de esta construcción se
  retiraron tras verificar que estaban dentro del checkout; los dos paquetes permanecen.
- Suite completa con cobertura: **2.463 PASS, 3 SKIP, 263,37 s; cobertura 89,03 %**, superior al
  mínimo configurado del 88 %. Las omisiones son dos casos de EPUBCheck externo y uno de Ollama real
  optativo. Esta pasada fue anterior al ajuste final que aisló la caché de PyInstaller; tras ese
  ajuste se repitieron las 11 pruebas focales y el análisis sintáctico del guion, con resultado
  correcto. [`coverage.json`](coverage.json).
- `ruff check .`, `ruff format --check .` (282 archivos), `mypy src/parsezen` (139 módulos),
  `pip check`, sincronización de versión y `git diff --check`: **PASS**. Los avisos de Git sobre
  finales de línea no fueron errores.

Cambios de producto local: [`distribution/windows/build.ps1`](../../distribution/windows/build.ps1)
y el nuevo [`check_runtime.py`](../../distribution/windows/check_runtime.py). Se añadieron
[`tests/test_package_runtime.py`](../../tests/test_package_runtime.py) y el guion sintético de
reanudación. Cambiaron también las fuentes de uso y verificación:
[`docs/development.md`](../../docs/development.md),
[`docs/acceptance-checklist.md`](../../docs/acceptance-checklist.md),
[`docs/work-plan.md`](../../docs/work-plan.md) y [`docs/user-guide.md`](../../docs/user-guide.md).
No se cambiaron el procesador de PDF, el traductor, los formatos de estado ni los libros personales.

Antes de editar se guardaron copias byte por byte y SHA-256 en
[`prechange/`](prechange/) con [manifiesto](prechange-manifest.json). El
[registro de cambios](change-manifest.json) distingue esas ediciones de las muchas modificaciones
locales previas. Para revertir el código o las guías, comparar primero el hash actual con el registro
y restaurar solo los archivos de este lote; si hubo ediciones posteriores, revertir únicamente sus
fragmentos. Para recuperar el paquete antiguo, verificar sus hashes en `package-evidence.json` y
moverlo de vuelta solo después de apartar el candidato actual. No se hizo commit, push, PR,
instalación global ni operación remota.

## 5. Segunda revisión y siguiente decisión

El smoke del paquete prueba que sus imports y su ventana básica funcionan, pero no que produzca un
libro traducido desde el ejecutable. La cancelación sintética muestra que se reutilizan fragmentos,
pero no representa un apagón ni demuestra restauración portable. La guía reduce la posibilidad de
perder un EPUB terminado, aunque no automatiza copias. Estos límites mantienen vigente la
recomendación de **reparar y simplificar la base actual** antes de reconstruirla.

La evidencia que más podría cambiar la prioridad es un capítulo representativo autorizado: comparar
PDF y EPUB en español, leer varias páginas seguidas en Calibre y después en Kindle, y comprobar
omisiones, sentido, imágenes e índice. Si aparece un problema sistemático de fidelidad, esa causa
debe atenderse antes de optimizar velocidad o rediseñar pantallas. También queda pendiente probar
una interrupción brusca y el paquete instalado en un perfil limpio. El primer recorrido de PDF sigue
proponiendo Markdown sin traducción; cambiarlo sin observar tu uso podría introducir una elección
automática equivocada, así que no se alteró en este lote.
