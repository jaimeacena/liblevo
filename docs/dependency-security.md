# Política de seguridad de dependencias

Política revisada: 22 de septiembre de 2026. Las ejecuciones de verificación se registran en el plan.

`pyproject.toml` declara las dependencias directas; `requirements.lock` fija el entorno de desarrollo
y CI. `requirements-windows-cpu.lock` es su variante de distribución: solo `torch` y `torchvision`
pueden diferir por el sufijo `+cpu`, conservando la versión base. No se mantiene un tercer lock de uv.
La instalación y regeneración se describen en [desarrollo](development.md).

Los dos lockfiles se generan con Python 3.12, fijan el grafo completo y contienen hashes. CI instala
con `--require-hashes`, ejecuta `pip check` y bloquea cualquier vulnerabilidad conocida mediante
`pip-audit` salvo las excepciones explícitas que siguen. La auditoría también falla si `pip-audit` omite
una dependencia inesperada, falta un paquete, aparece duplicado o su versión no coincide con el
lock. Un fallo informa solo paquete, versión e identificador de vulnerabilidad; no vuelca respuestas
del servidor. Las variantes Windows `torch` y `torchvision` con sufijo `+cpu` se
vinculan obligatoriamente con la misma versión canónica, auditada desde `requirements.lock`.

## Excepción temporal: CVE-2026-54499 en Stanza

Argos Translate 1.11.0 exige actualmente `stanza==1.10.1`. Stanza anterior a 1.12.2 tiene una
vulnerabilidad de deserialización insegura al cargar un modelo `.pt` malicioso. Liblevo no usa
esa ruta: antes de importar o preparar un traductor fuerza `ARGOS_CHUNK_TYPE=MINISBD` y también fija
el enum de Argos en memoria. De este modo la segmentación se realiza con MiniSBD y nunca se construye
ni carga un pipeline o modelo Stanza. Hay una prueba de regresión que impide retirar esta guarda por
accidente.

Por ello CI exceptúa `CVE-2026-54499` por esta ruta desactivada; el resto se audita según las
excepciones acotadas de esta política. La excepción debe eliminarse cuando Argos publique una versión compatible con Stanza 1.12.2 o
superior y esa versión supere las pruebas reales de traducción. No se instalará Stanza 1.12.2 a la
fuerza mientras contradiga el requisito exacto de Argos, porque produciría un entorno inconsistente.

Referencias primarias:

- [Aviso de seguridad de Stanza](https://github.com/stanfordnlp/stanza/security/advisories/GHSA-v5jw-96jm-7h2c)
- [Corrección publicada en Stanza 1.12.2](https://github.com/stanfordnlp/stanza/releases/tag/v1.12.2)
- [Argos Translate 1.11.0 en PyPI](https://pypi.org/project/argostranslate/)

## Mitigación temporal: CVE-2026-69112 en Accelerate

**IMPLEMENTADO en Liblevo; no corregido en la dependencia upstream.** Accelerate 1.14.0 llega por
Docling y sus modelos. `load_checkpoint_in_model` y `load_checkpoint_and_dispatch` aceptan rutas
no comprobadas desde índices fragmentados. La versión 1.15.0 consultada el 22 de septiembre conserva
esa construcción de rutas: actualizar solo para salir del rango publicado del aviso no es una
corrección demostrada.

Liblevo no utiliza estos cargadores. Antes de importar Docling, `ocr_dependency_guard.py` sustituye
ambas funciones y sus seis alias exportados por un rechazo explícito, antes de abrir cualquier
checkpoint. No cambia ni relaja el cargador safetensors que sí usa el OCR, ni autoriza modelos o
plugins arbitrarios. El smoke del paquete instala la misma protección. La mitigación vale para los
procesos de Liblevo, no para otros programas que utilicen ese entorno Python.

La opción `--allow-blocked-accelerate` de la auditoría solo permite `PYSEC-2026-3804` cuando el lock y
el entorno tienen exactamente Accelerate 1.14.0 y los seis alias rechazan una llamada de prueba con
la guarda de producción. Una versión distinta exige revalidación. Las pruebas verifican también
que el rechazo ocurre antes de abrir el índice. El registro de ejecución del OCR real y los gates
restantes se conservan en el plan; esta excepción no certifica por sí sola un instalador.

Retirar la mitigación y la excepción juntas cuando una versión compatible corrija las rutas y
supere las regresiones, el OCR y el paquete. No exceptuar otros identificadores por analogía.

Referencias primarias:

- [Aviso PYSEC-2026-3804](https://osv.dev/vulnerability/PYSEC-2026-3804)
- [Código de Accelerate 1.15.0](https://github.com/huggingface/accelerate/blob/v1.15.0/src/accelerate/utils/modeling.py)

## Componentes de IA local

Liblevo ya no descarga, actualiza ni ejecuta herramientas de recomendación de modelos. La interfaz
activa muestra únicamente las capacidades fijadas por el catálogo de Liblevo. Los trabajos nuevos
usan directamente los archivos GGUF del catálogo local mediante `llama-cpp-python`, sin Ollama ni
conexiones de red para procesar el documento. Antes de usarlos se verifican tamaño y SHA-256 del
archivo completo; la preparación solo acepta una capacidad fija y un modelo público verificado.

### Ruta heredada de Ollama

Los trabajos guardados con Ollama conservan su ruta local. Sus comprobaciones usan loopback y solo
leen `/api/version`, `/api/tags` y `/api/show`; no reciben documentos ni aceptan endpoints
configurables. Un componente heredado se considera preparado únicamente cuando el manifest local y
el digest anunciado por Ollama coinciden. Los estados `Descargable` e `Insuficiente` son decisiones
fail-closed y no autorizan descargar un tag arbitrario. `component_installer.py` recibe solo una
capacidad del catálogo y vuelve a comprobar `/api/tags` y `/api/show` antes de publicar el estado.

La descarga heredada usa una solicitud asíncrona cancelable dentro del trabajador local. La
cancelación se comprueba cada 100 ms incluso mientras Ollama no envía cabeceras o deja una línea de
progreso incompleta; se cierra y espera la solicitud antes de devolver el control. No hay un límite
total que corte descargas lentas. Cada línea NDJSON está limitada a 64 KiB antes de decodificarla. La
fase posterior de creación del alias conserva su espera máxima de 60 segundos y la verificación final.

## Revisión

No se actualiza todo automáticamente. Una corrección compatible se valida con lint, tipos, tests,
conversión real, traducción real y arranque del ejecutable. Las actualizaciones mayores requieren
una mejora concreta y medible. Tras cambiar dependencias se regeneran ambos lockfiles y los avisos de
terceros del paquete.
