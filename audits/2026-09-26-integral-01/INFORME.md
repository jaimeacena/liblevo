# Auditoría integral de Parsezen · 26 de septiembre de 2026

## 1. Resumen para decidir

**Necesidad confirmada por Jaime:** convertir un libro extenso en PDF, originalmente en otro idioma, en un EPUB traducido al español, con traducción y formato suficientemente buenos para leerlo cómodamente en Kindle. El destino final es Kindle; para examinar el EPUB en el PC acepta Calibre u otro lector. El sistema objetivo confirmado es Windows 11. No se ha observado todavía su uso cotidiano ni se ha probado uno de sus libros en esta auditoría.

**Qué hace Parsezen hoy:** es un conversor local de varios formatos con cola de trabajos, extracción de PDF y OCR, traducción mediante Argos u Ollama, revisión, editor y publicación EPUB. Es una implementación bastante protegida por pruebas automáticas. En ejecuciones aisladas produjo EPUB desde un PDF sintético de 20 páginas, y EPUB españoles desde muestras sintéticas de 2 y 20 páginas usando Ollama. Calibre abrió los tres. **Esto acredita rutas técnicas acotadas, no la calidad de un libro largo.**

**Problema principal:** el trabajo que Jaime quiere empieza con un PDF y acaba en un libro español cómodo, pero la configuración inicial de un PDF propone Markdown y ninguna traducción. La ruta esencial existe, aunque queda escondida entre decisiones de conversión más generales. Además, la traducción sintética terminó con **cero avisos automáticos** y aun así contiene frases españolas poco naturales. Un resultado «sin incidencias» no demuestra que apetezca leerlo ni que sea fiel en un libro extenso.

**Dirección recomendada:** mantener provisionalmente el motor actual y sus protecciones de datos; **rediseñar parcialmente el recorrido visible en torno a «Crear libro en español»**, sin reconstruir el procesamiento ni eliminar capacidades secundarias ahora. Primero validar un tramo completo con un PDF representativo autorizado: entrada, traducción, revisión en Calibre, cierre, reapertura y EPUB final. Si esa validación muestra fallos fundamentales de fidelidad o recuperación, reconsiderar el alcance. Cambiar Ollama por un motor directo es una hipótesis separada, no una solución demostrada a los problemas de traducción o formato.

**Estado para el primer uso real:** **PREPARACIÓN NO DETERMINADA.** La ruta sintética y la suite funcionan, pero faltan una prueba representativa de libro largo, revisión lingüística y visual, recuperación de un trabajo interrumpido en traducción o revisión, y comprobación en Kindle. No recomiendo empezar con el único ejemplar de un libro sin copia externa del PDF y del resultado.

## 2. Alcance, fuentes y capacidad real de comprobación

- Checkout existente: `[ruta local omitida]`; rama `main`. Antes de esta auditoría había **107 archivos rastreados modificados o eliminados**, además de archivos nuevos. Se conservaron todos. No se usó Git remoto, se hizo commit ni se modificó código, pruebas o documentos existentes.
- Entorno observado: Windows 11 Home, compilación 26200, x64; Ryzen 9 8945HX, 32 GB de RAM, RTX 5060 Laptop GPU con 8.151 MiB de VRAM; pantalla 2560 × 1600. Esto describe el PC inspeccionado; solo Windows 11 está confirmado como objetivo habitual.
- Herramientas disponibles y utilizadas: lectura y edición local, terminal, Python 3.12.10, PySide6 6.11.1, pytest 9.1.1, Ruff 0.15.22, mypy 1.20.2, acceso limitado a la GUI, capturas de **salida sintética** convertida con Calibre, inspección de ZIP/EPUB, contraste de tokens. Calibre está instalado. Se consultaron páginas públicas oficiales sin adjuntar archivos del proyecto.
- Límites: la automatización visual de la ventana nativa dejó de asociar con fiabilidad las capturas a Parsezen cuando había otra aplicación en primer plano. Se detuvo sin realizar más acciones. No hubo lector de pantalla, prueba en Kindle, varios monitores, escalado de Windows, suspensión/reanudación, fallo de disco, instalación del paquete actual ni uso observado de Jaime. La representación de Calibre a PDF **no equivale** a Kindle ni a todos los lectores EPUB.
- Datos: solo PDF sintéticos creados en esta carpeta. Las rutas de estado, ajustes y resultados de la prueba de GUI se redirigieron a perfiles de esta carpeta. No se leyó ni envió un libro personal. Ollama se consultó en `127.0.0.1`; la traducción sintética se procesó allí. El proveedor del asistente puede procesar esta conversación; trabajar con archivos locales no demuestra procesamiento local del modelo de asistencia.

Fuentes de estado revisadas: [`agent-operating-model.md`](../../docs/agent-operating-model.md), bloque «Ahora» de [`work-plan.md`](../../docs/work-plan.md), [`architecture.md`](../../docs/architecture.md), [`acceptance-checklist.md`](../../docs/acceptance-checklist.md), [`local-ai-model-policy.md`](../../docs/local-ai-model-policy.md), [`SECURITY.md`](../../SECURITY.md), código y pruebas. La documentación y el corpus anterior se trataron como descripción y antecedentes; sus veredictos históricos **requieren revalidación humana**.

## 3. Ficha provisional del producto y supuestos críticos

| Aspecto | Estado y fuente |
|---|---|
| Resultado | **Confirmado por Jaime:** EPUB español legible y bien presentado para un libro PDF extenso destinado a Kindle. |
| Entradas | **Confirmado:** PDF de libros en otros idiomas. **Desconocido:** proporción de PDF escaneados, tablas, ilustraciones, notas, columnas, idiomas y protección del PDF. |
| Tareas prioritarias | **Confirmado:** PDF→EPUB, buena traducción y formato decente. |
| Frecuencia | **Desconocida:** no se ha indicado cuántos libros ni ritmo de uso. |
| Privacidad | **Restricción confirmada en esta tarea:** las pruebas no deben enviar documentos ni usar datos personales. El proyecto afirma una política local; no se ha demostrado exhaustivamente ausencia de toda conectividad. |
| Modo de envío al Kindle | **No es una preferencia:** Jaime acepta comprobar primero con lector EPUB de PC. La lectura final en Kindle sí debe validarse. |
| Error de mayor impacto | **Inferido del objetivo:** perder o cambiar significado, cifras, orden de lectura, capítulos o progreso, o afirmar éxito con un EPUB que no se puede leer bien. |
| Funciones imprescindibles | **Propuesta pendiente de validar:** importar PDF, detectar calidad de origen, traducir al español, preservar contenido, revisar puntos dudosos, abrir/guardar EPUB y recuperar trabajo. |

### Tareas representativas y éxito observable

| ID | Tarea neutral para Jaime | Éxito observable | Situación actual |
|---|---|---|---|
| T1 | Dar un PDF con texto seleccionable y obtener un EPUB español | Capítulos navegables, pasajes fieles, cifras correctas, lectura fluida en Calibre y después en Kindle | Ruta técnica probada solo con 2 páginas sintéticas; calidad no validada |
| T2 | Dar un PDF escaneado o de maquetación difícil | Orden de lectura comprensible; imágenes/tablas y dudas conservadas o señaladas | Contratos y pruebas de código; ejecución actual representativa pendiente |
| T3 | Interrumpir un libro largo y continuar al día siguiente | Recupera trabajo sin perder ni duplicar decisiones, y final reconocible | Cola pendiente recuperada aisladamente; fase media real pendiente |
| T4 | Revisar y corregir una traducción o capítulo dudoso | Jaime identifica problema, lo corrige y ve la corrección en EPUB reabierto | Controles implementados y tests; recorrido humano no observado |

### Registro de supuestos que pueden cambiar la decisión

| Supuesto | Evidencia actual | Si es falso… | Prueba mínima |
|---|---|---|---|
| El PDF de interés permite extraer texto en orden útil | Un PDF limpio sintético pasó; antecedentes de corpus pendientes de revalidación | La traducción parte de texto desordenado, aunque el EPUB sea válido | Inspección de 5–10 páginas representativas con autorización y comparación lado a lado |
| Hy-MT2 local produce español que Jaime leería durante horas | Ejemplo sintético breve mostró rarezas y cero avisos | El producto central no cumple su fin | Dos pasajes cortos y uno largo del mismo género: lectura ciega de Jaime frente al original y alternativa permitida |
| Un EPUB legible en Calibre se lee bien en Kindle | Calibre abrió las muestras; Kindle no probado | Paginación, índice o imágenes pueden fallar al final | Enviar **una copia de prueba** a Kindle por el método que Jaime prefiera y leer varias secciones |
| La interfaz generalista no impide completar la tarea | La ruta existe, pero PDF nace como Markdown sin traducir | Jaime elige mal o debe aprender opciones técnicas | Observar a Jaime iniciar T1 sin indicarle clics; registrar ayuda y errores |
| Ollama aporta más simplicidad/fiabilidad que incrustar un motor | Runtime local actual preparado; no hay comparación directa | El mantenimiento de Ollama puede ser una carga innecesaria | Prototipo aislado con mismo modelo y parámetros, sin tocar Parsezen |

## 4. Referencia inicial y pruebas ejecutadas

**Entorno y comandos.** Desde la raíz del checkout se ejecutaron `.venv\Scripts\python.exe -m ruff check . --no-cache`, `ruff format --check . --no-cache`, `mypy src/parsezen --no-incremental`, `pip check`, una selección de pruebas de recorridos/regresiones y toda la suite con `pytest -q -p no:cacheprovider`. No se instalaron paquetes. Los diagnósticos añadidos a esta carpeta se formatearon y verificaron después con Ruff.

| Caso | Precondición y datos | Esperado definido antes | Observado y estado | Evidencia / límite |
|---|---|---|---|---|
| BASE-01 | Checkout actual, `.venv` | Código y entorno verificables | **PASS:** Ruff check, Ruff format (276 archivos), mypy (137 módulos), `pip check`; `git diff --check` sin errores de espacios | Terminal; no prueba adecuación del producto |
| BASE-02 | Suite existente | Regresión automática sin fallos | **PASS:** 2.449 pruebas, 3 omitidas, 242,29 s. Selección focal: 67 PASS en 17,24 s | Dos omisiones de EPUBCheck externo y una de Ollama real optativo; no se equiparan a ejecución humana |
| SYN-PDF-20 | PDF creado aquí, 20 páginas de texto, sin traducción | Original intacto; EPUB ZIP sano, índice, títulos y 18 cifras centinela | **PASS:** original sin cambios, 8 XHTML, 7 capítulos informados, cifras/títulos presentes, 8.775 bytes, 0,94 s en **una** ejecución | [`run_synthetic_path.py`](run_synthetic_path.py), [`EPUB`](synthetic-output/synthetic-20-pages.epub); no OCR ni maquetación compleja |
| SYN-TR-02 | PDF inglés sintético de 2 páginas; Ollama local Hy-MT2 | EPUB español válido, cifras y nombres presentes, original intacto | **PASS técnico:** 8,659 s en **una** ejecución, 2.739 bytes, ZIP íntegro, 12/7:30/48/84/Ana/Bruno presentes, 0 incidencias informadas. **Calidad lingüística: FAIL en muestra**, por dos construcciones poco naturales observadas | [`run_synthetic_translation.py`](run_synthetic_translation.py), [`EPUB`](translated-output/synthetic-translation-2-pages.es.epub), [render de Calibre](calibre-render-translation-page1.png); no evaluación humana de fidelidad |
| SYN-TR-20 | Mismo PDF limpio de 20 páginas; Ollama local Hy-MT2 | Original intacto; 18 cifras presentes, EPUB íntegro con índice | **PASS técnico:** 76,238 s en **una** ejecución, 11.756 bytes, ZIP íntegro, 18 cifras presentes, índice, 10 capítulos informados, 0 incidencias automáticas. La traducción no tiene revisión humana | [`run_synthetic_20_translation.py`](run_synthetic_20_translation.py), [`evidencia JSON`](translated-20-evidence.json), [`EPUB`](translated-20-output/synthetic-20-pages.es.epub). El mismo origen sin traducir informó 7 capítulos; la diferencia requiere revisión editorial, no se clasifica aún como error |
| CAL-01 | Tres EPUB sintéticos, configuración temporal de Calibre | Importación y representación sin error | **PASS:** Calibre 9.11.0 convirtió EPUB→PDF A5 sin errores. Salidas de 1 página (traducción breve), 28 (sin traducción) y 30 (traducción de 20 páginas). No se vieron recortes en las páginas inspeccionadas | [traducción breve](calibre-render-translation-page1.png), [20 páginas sin traducir, media](calibre-render-20-pages-page14.png), [20 páginas traducidas, primera](calibre-render-translated-20-page1.png), [media](calibre-render-translated-20-page15.png), [última](calibre-render-translated-20-page30.png). La paginación cambia al convertir; no prueba Kindle |
| UI-01 | Aplicación real con perfil aislado y PDF sintético pendiente | Abrir, encontrar configuración y reabrir cola | **PARCIAL:** se vio la ventana vacía, cola con 1 PDF, `Configurar`, valores iniciales Markdown y «No traducir», aviso «Cambios guardados». Tras detener solo el proceso de prueba, SQLite `integrity_check=ok`; reapertura aislada recuperó 1 trabajo `queued` | [`run_isolated_ui.py`](run_isolated_ui.py); sin capturas fiables de ventana, sin iniciar trabajo desde GUI ni medir tiempos |
| A11Y-01 | Tokens de colores actuales | Lectura contrastada de combinaciones relevantes | **PASS limitado:** pares claros 16,57/7,61/4,80/5,39/5,29; oscuros 14,80/9,56/6,53/8,04/9,41 (texto principal, secundario, tenue, texto sobre acción, borde de foco) | [`check_contrast.py`](check_contrast.py); faltan todos los estados y lector de pantalla |
| OLL-01 | Ollama encendido por Jaime; consultas locales sin documentos | Instancia y modelos del catálogo presentes | **PASS:** Ollama 0.32.5 en `127.0.0.1:11434`; `/api/version`, `/api/tags` y preparación del catálogo local; Hy-MT2 y LFM marcados `prepared` | Estado del equipo en esta sesión; no garantiza calidad, exclusividad de red ni que otras máquinas coincidan |

**Incidencias de la propia prueba:** el primer intento de `SYN-PDF-20` falló porque el guion de auditoría no había creado su carpeta de salida; se corrigió **solo ese guion** y después pasó. La primera repetición de `UI-RECOVER-01` falló por invocar el perfil vacío sin `--synthetic`; la invocación correcta recuperó un trabajo. Ninguno de estos fallos se atribuye a Parsezen. La inspección de GUI se detuvo cuando la herramienta de capturas confundió ventanas; su proceso aislado se interrumpió voluntariamente. No hubo fallo de arranque de la app demostrado. Las cifras de tiempos no son benchmarks: una ejecución, entradas muy pequeñas y sin dispersión.

**Trazabilidad breve:** necesidad «leer libro traducido» → criterio «EPUB español íntegro, fiel y cómodo» → riesgos «ZIP inválido, cifras perdidas, español malo» → SYN-TR-02 + CAL-01 → paquete y centinelas PASS, naturalidad de muestra FAIL → EPUB y render sintéticos → hallazgo H1. Necesidad «no perder trabajo» → criterio «reapertura recupera» → riesgo «cola perdida» → UI-01 → pendiente recuperado PASS, fase media no ejecutada → H3 sin cerrar. Necesidad «hacerlo sin entender opciones técnicas» → criterio «T1 sin ayuda» → riesgo «elección inicial errónea» → observación UI-01 → Markdown/sin traducir → H2.

## 5. Hallazgos priorizados

Gravedad indica efecto; prioridad indica qué conviene atender primero. Las categorías y conclusiones no convierten hipótesis en defectos probados.

| ID / prioridad / clase | Qué pasa y dónde | Consecuencia / evidencia / confianza | Propuesta, esfuerzo y aceptación |
|---|---|---|---|
| **H1 · P1 para validar · incertidumbre de producto** | No está demostrada la calidad de **un libro largo traducido**; la muestra corta contiene español poco natural aun con 0 avisos. [`processing.py`](../../src/parsezen/processing.py), política de traducción y SYN-TR-02 | Puede producir un libro formalmente válido que Jaime no quiera leer. Observación directa en 2 páginas; **alta confianza en la limitación de la señal**, baja extrapolación a libros extensos | Validar pasajes representativos y un capítulo completo; comparar sentido, omisiones, terminología y fatiga de lectura. Esfuerzo medio, sin cambiar datos. Aceptación: Jaime prefiere leerlo y detecta/repara dudas con esfuerzo tolerable |
| **H2 · P2 · problema de uso/planteamiento** | Para PDF, la configuración inicial usa Markdown y traducción vacía; [`main_window.py:2979`](../../src/parsezen/presentation/main_window.py), [`jobs.py:183`](../../src/parsezen/domain/jobs.py), observación UI-01 | El recorrido central obliga a elegir opciones que contradicen el resultado deseado; se puede iniciar un trabajo incorrecto. **Confianza alta** sobre el valor inicial; impacto personal aún por observar | Prototipar entrada principal «Libro en español» y conservar «Otras conversiones». Esfuerzo medio, riesgo de cambiar hábitos/automatismos; aceptación: Jaime inicia T1 sin ayuda y puede comprobar el resultado previsto antes de ejecutar |
| **H3 · P1 para pilotar · riesgo de datos/progreso no cerrado** | Solo se comprobó recuperación de una fila pendiente, no cierre durante traducción, revisión ni publicación de un libro. [`state_store.py:120`](../../src/parsezen/infrastructure/state_store.py), [`epub_checkpoints.py`](../../src/parsezen/epub_checkpoints.py), UI-01 | Un libro largo puede exigir horas; perder progreso o publicar parcialmente afectaría su uso. **No es fallo demostrado.** Implementación y tests existentes reducen el riesgo, pero no cubren esta sesión real | Ensayo aislado con archivo representativo autorizado; interrumpir en un punto acordado, reabrir, comparar EPUB final y huellas. Esfuerzo medio; no tocar datos personales; aceptación: recuperación visible, sin duplicar ni corromper |
| **H4 · P2 · barrera de distribución local** | En `dist/` solo se observó rueda/fuente `parsezen-1.0.0`, mientras el proyecto declara 1.2.0; no se verificó instalador/ejecutable actual | La suite del entorno de desarrollo no acredita que Jaime pueda abrir la versión vigente con comodidad. Confianza alta en inspección del directorio, **no** se descarta un instalador en otra ubicación | Preparar y probar un paquete local en perfil de prueba cuando se apruebe la ruta de producto. Esfuerzo medio; aceptación: iniciar, cerrar, reabrir y recuperar sin terminal. No instalar sobre entorno habitual |
| **H5 · P2 · privacidad/mantenimiento conocido** | Historial guarda rutas y metadatos en SQLite; EPUB final y originales son archivos normales. [`SECURITY.md`](../../SECURITY.md), [`state_store.py`](../../src/parsezen/infrastructure/state_store.py) | Una copia o acceso a la cuenta puede revelar nombres/rutas; perder cuenta/PC puede perder el libro. No es una vulnerabilidad demostrada ni justifica añadir cuentas/nube | Explicar respaldo y restauración del PDF/EPUB; comprobar una restauración con copia sintética. Esfuerzo bajo; aceptación: Jaime puede recuperar libro y entiende dónde está |
| **H6 · P3 · pregunta tecnológica** | Ollama está preparado y funcionó localmente; no hay motor GGUF directo comparado. [`local_ai_client.py`](../../src/parsezen/local_ai_client.py), [`component_readiness.py`](../../src/parsezen/component_readiness.py) | Sustituirlo ahora podría añadir carga de instalación/GPU/cancelación sin mejorar calidad. Confianza alta sobre estado actual, beneficio alternativo desconocido | Mantener Ollama provisionalmente y medir un prototipo directo solo si la fricción real lo justifica. Esfuerzo medio/alto; aceptación: misma calidad y guardas, menor fricción o coste medido |

No se encontró una pérdida de datos, exposición remota ni corrupción grave reproducible que justifique P0. La prioridad H1/H3 se debe a su capacidad de invalidar el piloto, no a un fallo general ya probado.

## 6. Cobertura por área

| Área | Estado | Alcance real y límite |
|---|---|---|
| A Producto | **PARCIAL** | Necesidad confirmada, ruta actual, supuestos y alternativas; no uso observado ni libro propio |
| B Funcionalidad | **PARCIAL** | PDF→EPUB sintético y traducción local, cola pendiente; otras funciones, cancelación y editor no recorridos manualmente |
| C Arquitectura | **REVISADA estáticamente** | Capas, estado, motor local, contratos y suite; no se demostró cada ruta en ejecución |
| D Datos e integridad | **PARCIAL** | Originales sintéticos intactos, ZIP sano, SQLite íntegro y cola reabierta; import/export, restauración completa y publicación interrumpida pendientes |
| E UX | **PARCIAL** | Valores por defecto, ventana inicial y configuración vistos; comprensión de Jaime pendiente |
| F Interfaz visual | **PARCIAL** | Página de configuración observada, renders Calibre sintéticos y tokens; tamaños, diálogos, estados de error y lectores reales pendientes |
| G Marca/lenguaje | **PARCIAL** | Nombre y mensajes revisados frente a tarea principal; preferencia estética y comprensión del nombre no observadas |
| H Accesibilidad | **PARCIAL** | Nombres accesibles visibles en árbol y contrastes de tokens; teclado integral, lector de pantalla, escalado y foco en todos los diálogos pendientes |
| I Escritorio | **PARCIAL** | Arranque con perfil aislado, ventana y reapertura de cola; multi-monitor, minimización, DPI, suspensión y bloqueo de archivos pendientes |
| J Rendimiento | **PARCIAL** | 0,94 s y 8,659 s en ejecuciones únicas sintéticas; no extrapolables a libro largo ni comparables entre motores |
| K Seguridad/privacidad | **PARCIAL** | Fuente local y loopback, secretos/logs/paquetes inspeccionados de forma selectiva; sin auditoría dinámica de red ni escáner actualizado completo |
| L Robustez/mantenimiento | **PARCIAL** | Suite, recuperación inicial, paquete actual ausente de `dist/`; uso sin desarrollo y copia/restauración pendientes |
| M Pruebas | **REVISADA parcialmente** | 2.449 PASS/3 omisiones, contratos y límites; muchas aserciones prueban la implementación, no la aceptación lingüística y editorial de Jaime |

### Observaciones técnicas por riesgo

- **Arquitectura y responsabilidad:** la interfaz PySide6 delega procesamiento, el plan de ejecución representa tareas y `state_store.py` usa SQLite con WAL y `synchronous=FULL`. Hay controles para archivos temporales, reanudación y publicación. Esta estructura **no exige reescritura inmediata**. Su complejidad sí puede quedar expuesta al usuario en demasiadas decisiones. La suite extensa reduce regresiones técnicas, pero no certifica el objetivo personal. [`activity_view.py`](../../src/parsezen/presentation/activity_view.py) ya distingue las señales automáticas de la cobertura de revisión semántica; en esta auditoría no se completó el recorrido visual de ese resumen y no se afirma que oculte el límite.
- **Salida EPUB:** en los archivos sintéticos el contenedor ZIP abrió, el índice existe y Calibre lo representó. Los renders de 20 páginas muestran texto sin recortes en tres puntos; la última página queda mayormente vacía por repaginación. La muestra traducida detectó 10 capítulos y la misma sin traducir 7: podría deberse a decisiones de estructura distintas y requiere cotejo, sin etiquetar todavía «mejor» ni «peor». No se ha evaluado tipografía, imágenes, tablas, notas o navegación de un libro real. EPUBCheck externo no se ejecutó en esta sesión.
- **Privacidad y seguridad:** Ollama escuchaba en loopback; el cliente fija API local y desactiva proxies/redirecciones en su transporte. Los componentes del catálogo se verifican por modelo/digest. El proyecto conserva metadatos en claro y usa DPAPI para determinados artefactos de revisión, ligado a la cuenta Windows. El 26/09/2026 se comprobó que las versiones instaladas, Stanza 1.10.1 y Accelerate 1.14.0, están en los rangos de los avisos públicos [Stanza](https://github.com/stanfordnlp/stanza/security/advisories/GHSA-v5jw-96jm-7h2c) y [Accelerate](https://osv.dev/vulnerability/PYSEC-2026-3804). [`dependency-security.md`](../../docs/dependency-security.md) documenta una ruta Stanza desactivada y una guarda local contra los cargadores afectados de Accelerate, con regresiones incluidas en la suite; no se forzó una actualización incompatible. **No se ejecutó una auditoría actualizada de todas las dependencias ni se afirma ausencia de vulnerabilidades**.
- **Accesibilidad:** las proporciones medidas son solo combinaciones de tokens. La guía [Qt para widgets accesibles](https://doc.qt.io/qt-6/accessible-qwidget.html) y las [prácticas de accesibilidad de Windows](https://learn.microsoft.com/en-us/windows/win32/winauto/accessibility-best-practices) orientan la prueba posterior. No se afirma cumplimiento formal ni prueba con lector de pantalla.

## 7. Reparar, rediseñar parcialmente o reconstruir

| Alternativa | Beneficio esperado y reutilización | Incertidumbre, esfuerzo, riesgo, datos y validación |
|---|---|---|
| **A. Reparar y simplificar sobre la interfaz actual** | Pequeños textos y valor por defecto; conserva todo el código y navegación | Esfuerzo bajo, pero puede mantener una lógica de «conversor general» que haga difícil T1. Cambiar el valor inicial sin aclarar el objetivo puede lanzar IA sin expectativa del usuario. Sin migración de datos. Validar T1 sin ayuda |
| **B. Rediseñar el flujo principal, conservar la base técnica** **(recomendada provisionalmente)** | Entrada principal de libro español, preparación y revisión orientadas al resultado; reutiliza extracción, IA local, cola, EPUB, guardas y datos | Esfuerzo medio; riesgo de ocultar funciones útiles y de introducir errores en navegación. No requiere alterar formatos persistidos si se hace como presentación gradual. Prototipo pequeño y tarea T1 observada antes de implantar |
| **C. Replantear y reconstruir todo** | Solo tendría ventaja si el modelo de procesamiento o datos impide fidelidad/recuperación, o si el coste de sostenerlo supera el de sustituirlo | Esfuerzo y riesgo altos; migración y compatibilidad de proyectos/datos. La evidencia actual no demuestra tal bloqueo. Validar primero un capítulo complejo y una recuperación real; entonces comparar módulos concretos |

**Segunda revisión crítica de esta elección:** es posible que la interfaz actual ya sea suficientemente clara para Jaime después de una sola sesión, o que un libro real revele problemas profundos de extracción que ninguna reorganización visual arregle. Cambiaría de recomendación si T1 se completara sin ayuda y de manera repetible (favorecer A), o si el libro representativo mostrara corrupción/orden incorrecto sistemático que no se pueda corregir de manera acotada (considerar C). Una maqueta agradable no resuelve el fondo.

## 8. Propuesta concreta de flujo y del papel de Ollama

### Porción de producto que conviene validar

```text
Inicio:  Crear libro en español                [Otras conversiones]
  1. Elegir PDF → muestra páginas, idioma probable y dudas de extracción.
  2. Preparar libro → «Español», EPUB, carpeta destino y modelo local visible.
     Opciones avanzadas: rango, OCR forzado, glosario y ajustes excepcionales.
  3. Procesar → progreso por etapas, pausa/cancelación clara y trabajo recuperable.
  4. Revisar → índice, extractos originales frente al español y avisos explicados.
  5. Abrir EPUB → Calibre/lector del PC, corregir si hace falta y guardar copia final.
```

No hace falta eliminar Markdown, Argos, editor EPUB ni cola para ensayar este camino. La hipótesis de simplificación es **mostrar primero el resultado que Jaime pidió** y desplazar las opciones técnicas a «Avanzado». Un texto de estado más inmediato para el flujo principal podría decir: «El libro aún no está listo: revisa el capítulo 2 y confirma el EPUB». El resumen actual ya separa incidencias automáticas y revisión semántica; habría que observar si Jaime entiende esa distinción antes de cambiarlo. Estas frases son propuestas, no cambios aplicados.

La dirección visual propuesta es una ventana de lectura y decisión: jerarquía tipográfica clara, columna de pasos, vista previa amplia de una página y avisos junto al fragmento afectado. Mantener foco visible, contraste medido y texto escalable. Evitar multiplicar tarjetas y animaciones: un libro necesita espacio para leer. Conservar provisionalmente el nombre/branding, porque no se ha observado que Jaime lo malinterprete; reconsiderarlo solo si la prueba de primer uso muestra expectativas equivocadas.

**Modelo y datos:** conservar `DocumentSource → JobConfiguration → ExecutionPlan → artefactos → EPUB` mientras las pruebas lo sostengan. En lenguaje cotidiano: el PDF original queda intacto; se guarda qué se pidió y qué etapas pasaron; solo se publica el libro al final. Si se cambia la interfaz, evitar migrar la base de datos en la primera fase. Verificar que los trabajos existentes sigan abriendo y que se pueda volver a la interfaz anterior si el prototipo falla. La aceptación incluye abrir una copia previa y una nueva, reanudar, y comparar integridad del EPUB.

**Ollama frente a modelo directo.** Ollama no es obligatorio para la idea de traducir localmente: [llama.cpp](https://github.com/ggml-org/llama.cpp/blob/master/README.md) permite ejecutar modelos GGUF de forma directa o mediante servidor local. Sin embargo, el [API nativo de Ollama](https://docs.ollama.com/api/generate) ya proporciona a Parsezen carga de modelos, generación, streaming y gestión de sesiones, y en este PC el catálogo está preparado. Si se retira, Parsezen tendrá que asumir carga de GGUF, GPU/CPU, memoria, arranque, cancelación, errores y distribución; la dificultad de compilar CUDA en Windows depende del motor elegido. **No se midió** una alternativa directa ni se halló un ejecutable `llama-cli`/`llama-server` en PATH. La calidad de las frases observadas no se puede atribuir a Ollama sin comparación con el **mismo** modelo y parámetros.

Recomendación: conservar Ollama durante el piloto. Si su manejo resulta una molestia real, comparar en una carpeta separada Ollama y ejecución directa con el mismo archivo de modelo, entradas sintéticas equivalentes, parámetros idénticos, tres arranques fríos y calientes, memoria/VRAM, cancelación, reapertura y revisión ciega de calidad. Pasar a motor directo solo si aporta una ventaja perceptible sin perder garantías. **Cambiar el runtime, formatos, identidad o eliminar capacidades requiere aprobación informada de Jaime antes de tocar la app.**

### Etapas y trabajos que conviene detener

1. **Ahora:** observar T1 con un PDF autorizado o una copia representativa; comprobar extracción y traducción de páginas difíciles antes de generar todo el libro.
2. **Después:** prototipo aislado del flujo anterior con una porción funcional completa: PDF sintético → español → EPUB → Calibre → reapertura. No publicar ni migrar.
3. **Si supera la prueba:** aplicar UI de forma incremental, con pruebas de configuración anterior, datos previos y recuperación. La reversión sería restaurar la interfaz previa sin tocar archivos publicados; cualquier cambio de datos necesita copia y plan de reversión propio.

Conviene detener el pulido de conversiones secundarias y los experimentos de otro runtime hasta conocer si la calidad editorial de T1 alcanza el objetivo. No se eliminan integraciones útiles por inercia.

## 9. Preparación del piloto y guía de prueba personal

| Dimensión | Estado |
|---|---|
| Fiabilidad técnica | Sólida en contratos y casos sintéticos ejecutados; recuperación a mitad de libro y paquete cotidiano pendientes |
| Adecuación del producto | **No validada**: el resultado personal es más específico que la entrada actual de «conversor» |
| Experiencia para Jaime | **No observada**: faltan tareas neutrales sin instrucciones de clics |
| Seguridad de datos | Original sintético intacto y estado aislado íntegro; respaldo/restauración de un libro y privacidad del destino final pendientes |

**Estado: PREPARACIÓN NO DETERMINADA.** Para poder declarar «APTA PARA PILOTO LOCAL CONTROLADO» faltan T1 y T3 sobre una **copia autorizada**, ausencia de fallos graves en esa ruta, salida/reapertura del EPUB comprobadas y lugar de respaldo entendido. No equivale a declarar la app lista para uso cotidiano.

Guía sencilla para la futura prueba con Jaime:

1. Guardar fuera de la carpeta de trabajo una copia del PDF de prueba. Empezar con un libro o capítulo que Jaime conozca y pueda comparar; no usar el único original. Elegir una carpeta de salida nueva.
2. Dar esta instrucción neutral, sin explicar los clics: «Convierte este PDF en un EPUB en español que querrías leer en Kindle; dime cuándo confías en el resultado». Observar si completa la tarea, qué duda le frena, qué ayuda pide y si entiende cuándo está guardado.
3. Leer en Calibre el principio, un pasaje del medio, uno con cifras/nombres, una página con imagen/tabla si existe y el final; abrir el índice. Comparar con el PDF. Anotar **pasaje y tipo de problema** en una hoja privada, sin enviar texto del libro a servicios externos.
4. Cerrar y reabrir Parsezen durante una prueba acordada; comprobar que el trabajo sigue allí, continuar y volver a abrir el EPUB. Detenerse si falta texto, cambian cifras, hay orden ilegible, se anuncia éxito sin archivo o se modifica el PDF original.
5. Cuando el EPUB de PC sea satisfactorio, probar **una copia** en Kindle, con el método preferido por Jaime. El método de envío puede implicar servicios de Amazon; esa decisión de privacidad corresponde a Jaime. Abrir índice, varias páginas y último capítulo. Si falla, conservar PDF y EPUB previos y no borrar el trabajo de prueba.

No se han medido tiempos de Jaime ni se ha programado seguimiento. Para un primer piloto se registrarían fecha, versión, documento de prueba (sin texto sensible), tareas, resultado, ayuda necesaria y anomalías observadas.

## 10. Cambios, reversión y anexos reproducibles

**Cambios realizados:** solo se añadió esta carpeta de auditoría con cinco guiones de diagnóstico, dos PDF sintéticos, EPUB y renders sintéticos, perfiles aislados y este informe. No se editó la aplicación, sus tests, su documentación existente ni sus datos habituales. No se hizo copia de seguridad del checkout porque no se cambió ningún archivo existente; antes de cualquier lote de código habría que conservar una copia recuperable. Los guiones se dejaron con Ruff check y format PASS; una corrección de formato afectó únicamente a ellos.

**Reversión:** la app está como antes de la auditoría. Si se quisiera retirar el material de auditoría, se puede archivar o quitar **solo** `audits/2026-09-26-integral-01` después de conservar este informe; no se ha ejecutado limpieza ni borrado. La carpeta contiene únicamente material sintético y estado temporal de prueba.

**Comandos reproducibles sobre el checkout actual:**

```powershell
.venv\Scripts\python.exe -m ruff check . --no-cache
.venv\Scripts\python.exe -m ruff format --check . --no-cache
.venv\Scripts\python.exe -m mypy src/parsezen --no-incremental
.venv\Scripts\python.exe -m pip check
.venv\Scripts\python.exe -m pytest -q -p no:cacheprovider
git diff --check
```

Los guiones de síntesis rehúsan sobrescribir sus archivos existentes: para reproducirlos se necesita **una nueva carpeta de auditoría** con copia de esos guiones, sin borrar los originales. `run_isolated_ui.py --synthetic --check-reopen` comprueba la recuperación de la cola del perfil aislado. Calibre se ejecutó con `CALIBRE_CONFIG_DIRECTORY` y `CALIBRE_TEMP_DIR` redirigidos a esta carpeta; convirtió únicamente los EPUB sintéticos a PDF A5 para inspección.

**Criterio para reabrir la decisión:** un libro representativo revela error grave de extracción/traducción/recuperación; Jaime no puede completar T1 sin ayuda; Ollama causa una fricción observable; o una comparación justa demuestra que otro runtime ofrece la misma fidelidad con menos mantenimiento. En ausencia de esa evidencia, la recomendación sigue siendo probar el recorrido central antes de sustituir tecnología.
