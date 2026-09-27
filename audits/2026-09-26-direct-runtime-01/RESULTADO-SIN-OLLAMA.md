# Parsezen sin dependencia de Ollama · revisión complementaria

Fecha: 26 de septiembre de 2026. Este documento complementa la [auditoría integral](../2026-09-26-integral-01/INFORME.md). Se trabajó en el checkout local existente y solo se usaron documentos sintéticos. El estado de uso cotidiano y la calidad de los libros de Jaime siguen sin validar.

## 1. Decisión comprensible

**Necesidad confirmada:** convertir un libro PDF largo, escrito en otro idioma, en un EPUB español cómodo para Kindle; importan la calidad de la traducción y el formato. Jaime quiere que los trabajos nuevos funcionen sin Ollama y que un modelo local mejor se pueda incorporar más adelante con cambios acotados.

**Resultado implementado:** la app nueva carga por sí misma un archivo GGUF verificado mediante un motor incluido en Parsezen. La traducción ya se ejecutó de PDF a EPUB con el acceso HTTP de la prueba bloqueado, sin usar Ollama. La pantalla de IA local mostró el traductor preparado y guardó su identidad directa en la configuración del trabajo. El modelo de revisión tiene la misma ruta opcional. Argos continúa disponible. Los trabajos antiguos que guardaron explícitamente una identidad de Ollama aún pueden necesitarlo; no se migraron ni se alteraron sus datos.

**Lo que esto no resuelve:** el mismo modelo produjo en una muestra pequeña «Llevaba cuadernos azules 12» para «She carried 12 blue notebooks». Esa frase ya aparecía en el resultado anterior con Ollama. La nueva infraestructura elimina la dependencia solicitada, pero **no demuestra una traducción suficientemente buena para leer un libro**. El EPUB sintético abrió en Calibre; no se probó un libro extenso ni Kindle.

**Dirección de producto:** conservar el procesamiento, la protección del contenido y el EPUB mientras se evalúa la experiencia real. Sigue siendo razonable replantear la entrada principal como «Crear libro en español», según la auditoría integral. Cambiar de motor no sustituye esa decisión de producto. La arquitectura ahora permite probar un GGUF futuro sin rehacer cola, EPUB ni guardas, pero cada modelo nuevo requiere verificación de licencia, memoria, compatibilidad, fidelidad lingüística y formato antes de activarlo.

## 2. Qué cambió y por qué

| Parte | Cambio | Consecuencia para Jaime |
|---|---|---|
| Modelo local | `direct_models.py` fija dos archivos, tamaños, SHA-256, contexto y adaptador; prepara la copia propia en la carpeta de modelos de Parsezen | Ollama se puede cerrar para trabajos nuevos. El traductor ocupa unos 4,6 GB adicionales; la revisión opcional, unos 2,2 GB si se prepara |
| Ejecución | `direct_ai_runtime.py` carga el GGUF con el motor Vulkan integrado, transmite la respuesta con límites y libera el modelo por fase | No necesita servidor; la cancelación se comprueba entre fragmentos. Una llamada nativa que tarde en devolver el primer fragmento puede demorarse en cancelar |
| Compatibilidad | `improvement.py`, `local_ai_adapters.py`, reglas de configuración y cola distinguen IDs directos de los de Ollama | Un checkpoint de un motor no se reutiliza como si perteneciera al otro; los trabajos antiguos se conservan |
| Interfaz | La ventana principal activa `LocalAIController(direct=True)`; los dos componentes se preparan por capacidad fija | No hay que elegir un modelo técnico ni instalar Ollama. Una descarga pública sí necesita red si no hay copia local |
| PDF difícil | La ruta directa omite el árbitro visual opcional que dependía de Ollama | Si un recorte sigue ambiguo, debe revisarse; no se ha demostrado equivalencia OCR en libros complejos |
| Paquete | El `.spec` incluye las bibliotecas GGUF/Vulkan y evita dos DLL ajenas que impedían arrancar Qt | Se obtuvo un ejecutable local sin instalarlo en el uso habitual; su apertura técnica no prueba un libro largo desde el paquete |

Para incorporar en el futuro un GGUF mejor: registrar un **perfil nuevo** con identidad, archivo, hash y contrato; conservar los perfiles antiguos para trabajos pendientes; añadir un adaptador solo si cambia el formato de petición; comparar en muestras autorizadas con criterios de fidelidad y lectura; activar el perfil cuando pase esos controles. Esto cubre modelos GGUF compatibles con el motor integrado. No promete poder conectar cualquier proveedor ni garantizar mejor calidad por cambiar el archivo.

## 3. Pruebas y trazabilidad

| ID · necesidad → riesgo | Esperado previo | Observado · estado | Evidencia |
|---|---|---|---|
| DIRECT-03 · PDF→EPUB español sin Ollama → dependencia oculta | Terminar sin HTTP, original intacto, índice, ZIP íntegro y cifras/nombres preservados | **PASS · verificado en ejecución**, 16,645 s en una sola muestra sintética de 2 páginas; no extrapolable a libro largo | [`product-direct-v3-evidence.json`](product-direct-v3-evidence.json), EPUB en `product-direct-v3-output/` |
| UI-02 · preparación comprensible → configuración incorrecta | Dos capacidades fijas; traducción preparada; identidad directa en trabajo | **PASS · recorrido simulado del agente** con Qt aislado, 1100 × 720; no aceptación humana | [`direct-ui-v2-evidence.json`](direct-ui-v2-evidence.json), [`captura`](direct-ui-v2-components.png) |
| REVIEW-01 · revisión opcional → respuesta inválida | Respuesta JSON utilizable sin HTTP | **PASS · verificado en ejecución** para una frase sintética; calidad y cobertura de revisión no demostradas | [`product-direct-review-evidence.json`](product-direct-review-evidence.json) |
| READER-03 · EPUB legible en PC → paquete válido pero ilegible | Calibre abre y representa el EPUB de DIRECT-03 | **PASS · verificado en ejecución**, 1 página renderizada; inspección visual local sin recortes visibles. Kindle pendiente | [`calibre-render-v3-evidence.json`](calibre-render-v3-evidence.json), [`render`](calibre-render-direct-v3-page1.png) |
| HELPER-01 · comprobación sencilla sin Ollama → herramienta auxiliar obsoleta | La comprobación sin argumentos del archivo `.cmd` elige el traductor directo y procesa una muestra sintética | **PASS · verificado en ejecución** del mismo comando Python con perfil aislado e intervalo 1–2 de un PDF sintético de 20 páginas; el informe dijo `OK`, que no acredita naturalidad de la prosa. El archivo `.cmd` no se abrió interactivamente | [`helper-direct-run-summary.json`](helper-direct-run-summary.json), [`helper-direct-evidence.json`](helper-direct-evidence.json) |
| PACKAGE-02 · uso sin herramientas de desarrollo → motor ausente o Qt bloqueado | Ejecutable local importa interfaz, OCR y motor GGUF; no contiene las DLL ajenas | **PASS · verificado en ejecución**, salida 0 en 37,75 s, Qt offscreen y perfil aislado; no se hizo un libro largo con el paquete | [`package-final2-smoke-evidence.json`](package-final2-smoke-evidence.json), `package-final2/Parsezen/Parsezen.exe` |
| REG-02 · cambio seguro → regresión | Suite, Ruff, formato, mypy y dependencias pasan | **PASS · verificado en ejecución:** 2461 pruebas superadas, 3 omitidas en 149,28 s tras el cambio del comprobador; el resumen final de herramientas está en el registro adjunto | [`validation-after-helper.json`](validation-after-helper.json) |
| QUALITY-01 · español cómodo → éxito falso | Frases fieles, naturales y sin falsos positivos | **FAIL del criterio de lectura en esta muestra:** «cuadernos azules 12»; el control automático preservó `12`, pero no detectó el orden anómalo | PDF sintético y render READER-03; el mismo defecto está en el EPUB previo con Ollama |

El primer empaquetado aislado falló al importar QtGui. El análisis rastreó el problema a `icuuc.dll` e `icudt78.dll` de Poppler, recogidos por PyInstaller desde el entorno del proceso. Excluir únicamente esas dos copias ajenas permitió que el primer paquete pasara el smoke. El segundo paquete se generó después de corregir `repeat_penalty=1.1` a `1.0`: el valor anterior contradecía la decisión documentada de no penalizar repeticiones en Hy-MT2. DIRECT-03 repitió el recorrido con ese ajuste y conservó la integridad técnica; **no corrigió la frase anómala**. Se corrigió además `Validar con IA real.cmd`: sin argumentos comprueba la traducción directa sintética; la comparación con tags de Ollama queda como opción explícita.

El ejecutable de `package-final2/Parsezen/` necesita permanecer junto a su carpeta `_internal`; no es un archivo autónomo. La prueba de paquete solo verificó importaciones y arranque sin ventana visible, no instalación ni conversión desde ese ejecutable.

Las comprobaciones no usaron libros privados, no instalaron el paquete sobre el perfil habitual y no publicaron nada. El bloqueo HTTP en el proceso de prueba y la revisión estática de esta ruta no equivalen a una auditoría de red de todo el ejecutable. La copia local de un GGUF ya presente en Ollama se usó únicamente como fuente de los bytes públicos verificados; el daemon no fue necesario.

## 4. Problemas, prioridades y cobertura

| Hallazgo | Impacto y confianza | Acción y aceptación |
|---|---|---|
| **P1 de producto para decidir el uso habitual:** español antinatural en una frase simple; no es un defecto exclusivo del motor directo | Alto impacto para leer un libro; demostrado en esta muestra, frecuencia desconocida | Comparar pasajes y capítulos completos de un libro autorizado; Jaime debe poder leerlos con comodidad y cotejar sentido, nombres y cifras. Si falla, evaluar un GGUF alternativo o una revisión editorial acotada antes de uso cotidiano |
| **P1 de incertidumbre para piloto:** libro largo, recuperación a mitad de traducción y Kindle no probados | Pérdida de tiempo o resultado poco útil; riesgo plausible, sin fallo demostrado | Ensayo con copia de un capítulo representativo, después un tramo mayor, interrupción y reapertura, inspección en Calibre y por último Kindle |
| **P2 de compatibilidad:** trabajos antiguos de Ollama siguen requiriéndolo | La app nueva funciona sin Ollama, pero una cola anterior puede detenerse | Conservar esos trabajos; reconfigurar solo los que Jaime quiera procesar sin Ollama y comprobar que sus checkpoints se invalidan de modo seguro |
| **P2 de recuperación del componente:** un GGUF ya existente con digest erróneo se bloquea, pero la pantalla no ofrece todavía una reparación guiada | El bloqueo evita usar pesos corruptos; una persona no técnica necesitaría ayuda para apartar el archivo y prepararlo otra vez. Verificado estáticamente en `prepare_direct_model` y la tarjeta de estado | Añadir una acción explícita que conserve la copia fallida y publique un reemplazo solo tras verificarlo; probar interrupción y falta de espacio sin tocar otros archivos |
| **P2 de OCR:** el árbitro visual opcional no se usa en modo directo | Puede dejar más dudas en páginas complejas; efecto real no medido | Probar páginas escaneadas o ambiguas y valorar un modelo visual local solo si aporta una mejora demostrable |
| **P3 de lenguaje técnico:** parte de la documentación y las métricas conserva nombres heredados de Ollama | Puede confundir a quien mantenga el código; las pantallas principales y guías se actualizaron | Ajustar términos internos al revisar el sistema de métricas; evitar cambiar formatos persistidos en este incremento |

La cobertura detallada A–M y la comparación entre reparación, rediseño parcial y reconstrucción están en el informe integral. En este incremento: arquitectura, privacidad de la ruta directa, configuración, integridad sintética, pantalla local y paquete se revisaron **parcialmente con ejecución**; calidad de libro, accesibilidad manual, varios monitores, suspensión, restauración de trabajo largo y lector Kindle siguen **pendientes**. No hay P0 demostrado. Pasar pruebas automáticas no valida la adecuación del producto.

## 5. Estado de piloto y guía de prueba personal

**PREPARACIÓN NO DETERMINADA** para un primer libro largo. La fiabilidad técnica del flujo corto es prometedora; la adecuación al objetivo de lectura, la recuperación de trabajos largos y la experiencia en Kindle siguen sin evidencia. No se debe presentar como «lista para uso cotidiano».

| Dimensión | Juicio actual |
|---|---|
| Fiabilidad técnica | Flujo corto y paquete técnico comprobados; trabajo largo, cancelación durante carga nativa y uso del paquete completo pendientes |
| Adecuación del producto | La meta está confirmada; la calidad de lectura del modelo actual no supera siquiera toda la muestra breve |
| Experiencia personal | Pantalla simulada y render de Calibre vistos; Jaime todavía no ha hecho una tarea neutral sin ayuda |
| Datos y privacidad | Original sintético intacto, SHA del modelo y EPUB íntegro; recuperación larga, auditoría de red exhaustiva y copia/restauración personal pendientes |

Cuando quieras contrastarlo sin poner en riesgo tu única copia:

1. Conserva el PDF original y elige un capítulo que contenga prosa, un título, una cifra y alguna página difícil. Trabaja con una copia.
2. Abre Parsezen, comprueba que `Traducción IA` figura `Preparado`, elige EPUB y español y procesa ese tramo. Anota si entiendes los pasos sin ayuda.
3. Abre el EPUB en Calibre. Lee varias páginas seguidas y coteja el original: sentido, orden de las frases, nombres, números, índice, imágenes y cansancio al leer.
4. Cierra y vuelve a abrir Parsezen. Después ensaya una interrupción durante un tramo más largo con datos de prueba y comprueba que puedes continuar sin repetir ni perder contenido.
5. Envía una copia de prueba al Kindle y revisa principio, mitad, final, índice y páginas con imágenes. Si aparecen frases torpes frecuentes, contenido perdido o recuperación dudosa, detén el piloto y conserva PDF y EPUB previo.

El asistente no observó el uso de Jaime; estas son tareas neutrales pendientes, no validación humana ya realizada.

## 6. Recuperación y segunda revisión crítica

Antes de cada lote se guardaron copias con SHA-256 en [`prechange/`](prechange/); hay copia de la configuración del entorno en `prechange-venv-freeze.txt`. Se añadieron dos paquetes fijados por hash al `.venv` de este checkout, no al sistema; se copiaron 4,6 GB del modelo público verificado a la carpeta propia de Parsezen. Los originales, la cola personal y los modelos de Ollama no se borraron. El paquete se construyó en esta carpeta y no se instaló. Los cambios de código se pueden revertir archivo por archivo desde esas copias, previa revisión de modificaciones posteriores; para reproducir el entorno previo debe restaurarse el `.venv` según el freeze, sin tocar datos. Volver al código anterior no borra ni migra trabajos guardados. El nuevo modelo puede permanecer sin afectar a los datos.

Archivos nuevos principales: `src/parsezen/direct_models.py`, `src/parsezen/direct_ai_runtime.py`, `requirements-windows-vulkan.lock`, `tests/test_direct_models.py` y `tests/test_direct_ai_runtime.py`. Archivos de integración modificados: `improvement.py`, `local_ai_adapters.py`, `visual_ocr.py`, `application/configuration_rules.py`, `presentation/local_ai_controller.py`, `local_ai_workflow.py`, `component_setup.py`, `job_configuration_dialog.py`, `main_window.py`, `__main__.py`, el `.spec` de Windows, el comprobador sintético y su archivo `.cmd`. Se actualizaron `AGENTS.md`, arquitectura, política de modelos, aceptación, plan de trabajo, README y guías. Los archivos anteriores ya tenían cambios locales antes de esta tarea; las copias previas conservan exactamente ese estado, no un commit anterior.

Reconsideraría la recomendación de conservar Hy-MT2 si Jaime encuentra errores frecuentes de sentido o lectura en capítulos reales; reconsideraría el rediseño visible si completa repetidamente PDF→EPUB sin ayuda y con claridad. Reconsideraría el motor directo en este PC si una prueba larga muestra bloqueos, memoria excesiva, cancelación inaceptable o peor resultado que el mismo GGUF con Ollama bajo condiciones comparables. La muestra corta solo descarta una imposibilidad técnica; no aprueba el producto ni demuestra superioridad del motor.
