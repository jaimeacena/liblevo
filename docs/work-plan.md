# Plan de trabajo y estado de calidad

Este es el único plan vivo de Liblevo. Su función es decir a un agente qué está demostrado, cuál es
la frontera activa y qué condición permite avanzar. No sustituye la arquitectura ni la lista de
aceptación. Se actualiza cuando cambia un estado, un gate o el orden de trabajo; no después de cada
edición menor.

Última actualización: **28 de septiembre de 2026**. Baseline histórica de esta iniciativa: `main` en
`865ef2a`. El estado ejecutable se obtiene siempre del checkout y su diff actual, no de esta línea.

## Norte

Obtener EPUB refluibles desde PDF con la máxima fidelidad práctica:

- texto, cifras, símbolos, énfasis, imágenes, tablas y enlaces conservados o señalados;
- traducción natural y fiel cuando se solicita;
- partes, capítulos y secciones navegables según la evidencia del libro;
- ninguna incertidumbre presentada como certeza;
- originales inmutables, trabajo recuperable y publicación atómica;
- coste local proporcional al riesgo real del documento.

## Lectura del estado

Los estados se usan con el significado de `agent-operating-model.md`:

- **IMPLEMENTADO** no significa universalmente perfecto;
- **VERIFICADO EN CORPUS** siempre está limitado a una muestra;
- **EXPERIMENTAL** no puede modificar el flujo normal ni promocionar cambios;
- **RECHAZADO** solo se reabre con evidencia o mecanismo nuevo.

## Estado del sistema

Las etiquetas de corpus de esta tabla son veredictos históricos anteriores a V1. Su evidencia
requiere revalidación; no constituyen aprobación actual. La implementación se conserva y los
contratos automáticos se comprueban por separado.

| Área | Estado registrado antes de V1 | Evidencia histórica | Límite conocido |
|---|---|---|---|
| contratos, cola, recuperación y publicación | IMPLEMENTADO | suite automatizada, fallos inyectados e integridad final | cualquier cambio transversal exige suite completa |
| extracción PDF/OCR y formato | VERIFICADO EN CORPUS | corpus privado diverso, canarios y reanudación determinista | un PDF nuevo puede introducir otra geometría; no existe «verde universal» |
| traductor Hy-MT2 EN→ES | VERIFICADO EN CORPUS | 41 referencias humanas; 41/41 puertas duras | naturalidad y sentido no quedan demostrados solo por las guardas |
| memoria terminológica acotada | VERIFICADO EN CORPUS | mejora en 9 de 13 casos de dominio sin perder puertas duras | solo sintagmas inequívocos; el glosario humano tiene prioridad |
| reparación semántica residual automática | RECHAZADO con los modelos actuales | 67 señales, 19 decisiones humanas y descarte de parches por consenso | 1/12 propuestas iniciales aprobada; el piloto nuevo produjo 0/13 consensos útiles |
| estructura Parte → Capítulo → Sección | VERIFICADO EN CORPUS | dos holdouts no usados para la regla, 925 páginas, 57 capítulos y 289 destinos equivalentes en ruta directa/editor | evidencia insuficiente conserva jerarquía plana; no se inventan destinos |
| paquete EPUB e integridad | IMPLEMENTADO | validación interna, comparación de payload y EPUBCheck optativo | EPUBCheck no sustituye fidelidad editorial |
| recorrido completo sobre los libros clave | VERIFICADO EN CORPUS para estructura; traducción aún abierta (1/6 centinelas) | *36 Faces*: 317 páginas, 297 bloques, 42 capítulos, 46 imágenes e integridad final; mejor candidato conservado con 25 avisos | quedan cinco libros; dos regeneraciones posteriores con 45 avisos se rechazaron como nueva base editorial |

## Decisiones vigentes

1. **No cambiar ahora el traductor base.** Hy-MT2 sigue siendo la mejor base local evaluada; cambios
   globales de prompt y `repeat_penalty` degradaron o no mejoraron de forma estable el corpus.
2. **No activar reparación semántica automática.** Que LFM esté instalado y pueda generar propuestas
   protegidas no autoriza a aceptarlas sin una persona.
3. **No seguir generando paráfrasis de los mismos rechazos.** Once de doce propuestas revisadas no
   alcanzaron aprobación tras varias rondas. Otra variante sin mecanismo nuevo gastaría atención sin
   aumentar evidencia.
4. **Conservar los 65 casos como conjunto diagnóstico.** No equivalen a 65 errores: mezclan avisos
   reales, posibles falsos positivos y casos que no admiten una propuesta segura.
5. **Promover solo referencias humanas completas.** La única corrección aprobada se conserva en el
   corpus privado; ningún caso dudoso se aplica al producto ni a un libro.
6. **Mantener separación entre detección, propuesta y publicación.** Mejorar una no cambia el estado
   de las otras.
7. **No insistir con parches por consenso en los modelos instalados.** Ni Qwen 4B ni LFM produjeron
   un segundo flujo estructurado capaz de coincidir con Qwen 3.5; se obtuvieron 0/13 candidatos.

El detalle de T1–T4, V1–V1.9, M1–M6 y S1/E1 se conserva en
[el historial de decisiones y evaluaciones](work-plan-history.md). No debe importarse como
aprobación del candidato actual. Las decisiones anteriores siguen vigentes.

## Ahora: estabilización para publicación y evaluación humana pendiente

### Identidad Liblevo — IMPLEMENTADO; aceptación humana y actualización mediante instalador pendientes

La identidad actual completa adopta Liblevo: aplicación, paquete Python `liblevo`, imports,
identificadores propios, recursos, lanzadores, distribución, enlaces y documentación. El repositorio
mantiene su identidad e historial bajo `jaimeacena/liblevo`; la rama de trabajo es
`codex/liblevo-pendientes`. El checkout existente se traslada a `Developments/Liblevo` y el proyecto
registrado en Codex apunta a esa carpeta.

El perfil local y las claves de apariencia también se trasladan a Liblevo. Se recupera el trabajo
existente y se conservan ajustes y preferencias. El GGUF conserva archivo, tamaño y SHA-256 del
catálogo. Se actualiza una identidad de modelo en la configuración de la cola; no se procesa ni
reescribe contenido documental. Los formatos y canales nuevos usan Liblevo. Esta migración local
no acredita todavía una actualización mediante instalador en otros equipos. El instalador conserva
el AppId y usa el destino actual `Programs/Liblevo`.

Los commits, las versiones publicadas y las evidencias anteriores se conservan como historia por
petición expresa de Jaime. Las construcciones antiguas locales quedan en `outputs/history/`,
separadas del paquete actual. Los backups de recuperación contienen los metadatos anteriores.

El logo, icono, Inter y la paleta mantienen el cambio visual autorizado. La corrección de contornos
del nombre y el lema ya cuenta con comparación frente al texto nativo; sus maestros y derivados
reproducibles son los activos actuales. Las pruebas focales del paquete, ajustes y cola pasan
(50 casos), junto con las de los lanzadores (2 casos). Ruff check/formato, Mypy (139 módulos) y
sincronización de versión pasan. La suite final completa pasa con **2.469 pruebas, 3 omisiones y
cobertura 89,04 %**. El paquete 1.3.0 supera el arranque antes y después de su promoción; se
comprueban sus 137 módulos propios, los recursos, Inter y los 9 tamaños del icono. El instalador
local se compila con nombre y metadatos Liblevo 1.3.0. La normalización final de los SVG conserva
byte por byte PNG e ICO; pasan sus 18 pruebas focales y el manifiesto coincide con el paquete.
La aceptación visual humana y la actualización real mediante instalador siguen pendientes;
este cambio no constituye una nueva validación documental.

Registro y condición para reabrir: [verificación de identidad completa](../audits/2026-09-28-liblevo-02/report.md).

### Motor integrado sin Ollama — IMPLEMENTADO; validación lingüística pendiente

Necesidad confirmada por Jaime: convertir un PDF largo a EPUB traducido al español para leerlo en
Kindle, con traducción y formato cómodos; no quiere depender de Ollama. Los nuevos trabajos de la
app usan el mismo Hy-MT2 GGUF con el motor integrado Vulkan `llama-cpp-python`. El GGUF se guarda en
la carpeta de modelos de Liblevo y se valida por tamaño y SHA-256. El revisor LFM tiene el mismo
recorrido directo opcional; Argos permanece disponible. Las identidades directas son distintas de
las de Ollama y quedan fijadas en cada trabajo. El catálogo puede incorporar en el futuro otro LLM
con un perfil y una selección activa después de probar su calidad; no se habilita un proveedor
remoto ni un selector técnico en la interfaz. Los trabajos antiguos de Ollama siguen siendo legibles.

La prueba con un PDF sintético produjo EPUB íntegro con red HTTP bloqueada, y la pantalla de IA
local mostró la traducción preparada sin Ollama. La prueba de revisión directa usó una sola frase
sintética. La comprobación auxiliar sin argumentos se orienta ahora a la traducción directa y pasó
con un intervalo sintético de dos páginas. Estas comprobaciones acreditan ejecución e integridad básica, no una traducción buena de
un libro largo. Faltan evaluación humana de lenguaje y formato, lectura en Kindle, recuperación de
un trabajo largo, uso interactivo del paquete y comprobación del instalador final. El arbitraje visual opcional
para PDF ambiguo no está integrado en el motor directo: los casos dudosos permanecen para revisión.
Evidencias y límites: `audits/2026-09-26-direct-runtime-01/RESULTADO-SIN-OLLAMA.md`.

### Paquete local con motor integrado — IMPLEMENTADO; uso final pendiente

El 27 de septiembre se corrigió el script local de construcción: comprueba Python 3.12 y el motor
Vulkan fijado antes de crear salidas, construye un candidato separado y solo lo promueve después de
`--package-smoke`. Conserva el paquete anterior en una carpeta identificada y no instala Inno Setup.
La caché de PyInstaller de futuras ejecuciones queda aislada en la carpeta temporal del candidato.
El paquete local nuevo superó el smoke en Windows 11; contiene las DLL de `llama_cpp` y el anterior
permanece recuperable. La prueba focal del control previo, el análisis del script y el ensayo de
limpieza de temporales sintéticos también pasaron. Evidencia sin contenido documental:
`audits/2026-09-27-local-improvements-01/package-evidence.json`.

Esto no acredita todavía una traducción completa desde el ejecutable, una instalación limpia, la
lectura en Kindle ni la calidad editorial. El workflow remoto no se ejecutó; el alcance de esta
intervención es el paquete local.

### Reanudación de traducción directa — IMPLEMENTADO; interrupción brusca pendiente

Un ensayo aislado con PDF sintético de 20 páginas canceló la traducción en el fragmento 5 de 40:
quedaron 24 archivos intermedios y ningún EPUB prematuro. La reapertura del mismo trabajo reutilizó
4 fragmentos, publicó un EPUB válido con navegación y las 18 cifras de control, y conservó intacto el
PDF. HTTP estuvo bloqueado durante ambas fases. La primera ejecución del guion falló antes de
procesar porque no había creado su carpeta de salida; se registró y corrigió en una carpeta nueva.
`audits/2026-09-27-local-improvements-01/resume-attempts.json` conserva ambos intentos. El tiempo
medido durante la reanudación incluye suspensión de Windows y no sirve para comparar rendimiento.
Este ensayo demuestra cancelación cooperativa y reanudación de ese caso sintético; falta interrumpir
un proceso de forma brusca y comprobar un libro representativo.

### S6 — IMPLEMENTADO; entrada coherente de documentos Markdown

Intención: evitar que la primera acción de una persona falle al añadir un archivo anunciado como
compatible y conservar los documentos válidos de un arrastre mixto. El 23 de septiembre se reprodujo
`ValueError: Unsupported document format: .markdown` en `DocumentFormat.from_path`, mientras el
selector y el conversor ya admitían esa extensión.

Cambio: `.md` y `.markdown` comparten el formato de origen Markdown. La entrada por arrastre y
selector omite carpetas y formatos incompatibles, incorpora los archivos válidos y explica en la cola
cuántos elementos se omitieron. El aviso se retira al añadir archivos válidos. No se modificaron
transformación, IA, OCR, persistencia ni publicación.

Verificación: prueba focal del caso, matriz de diseño y límites de arquitectura (78 pruebas),
suite general final tras los últimos ajustes (2.449 superadas, 3 omitidas), Ruff, formato, mypy en
137 módulos, `pip check`, auditoría de dependencias con las excepciones documentadas, versión
sincronizada y render local del aviso en 1100 × 720 y 320 × 520.
Las omisiones son EPUBCheck externo (2) y Ollama real optativo (1).

Límites y decisión: la captura Qt acredita composición, no aceptación humana. Conservar el cambio
local; reabrirlo si un archivo anunciado como compatible no entra, un arrastre mixto pierde un
documento válido o el aviso tapa acciones. V1.10 y la evaluación humana continúan pendientes.

### S5 — IMPLEMENTADO; correcciones de la revisión UX/UI del 22 de septiembre

Intención: aplicar V01–V11 del informe visual autorizado, conservando marca, procesamiento local,
contenido y trabajos previos. Se contrastó el checkout antes de editar y se conservaron copias de
los archivos de presentación para separar esta intervención de los cambios locales anteriores.

Cambios: tema de la confirmación EPUB; formularios y actividad desplazables con acciones estables;
Generar EPUB, destino e idiomas legibles; confirmación de guardado ligada a persistencia; navegación
contextual; selección explícita y decisiones confirmadas; títulos largos y cola compacta; editor con
capítulos compactos, índice real y OCR ampliable; nueva versión con identidad independiente y rollback;
intervalo PDF comprobado en segundo plano; bienvenida y resumen comprensibles. La prueba adicional
a 320 × 520 reprodujo tarjetas comprimidas en Configuración e IA local: también se corrigieron con
desplazamiento vertical condicionado a la altura.

Evidencia: contratos de contraste tras cambiar el tema con la página abierta, lectura hasta el final
de detalles largos, botones separados, etiquetas completas o elipsis deliberada, guardado fallido,
selección sin confirmar, conservación de variantes de idioma, índice equivalente al publicado y
nueva versión que preserva el archivo anterior. La matriz de renders Qt cubre ambos temas, cinco
tamaños desde 320 × 520 hasta 1280 × 760 y escalas 100 %/150 %. El lector real de intervalos comprobó
53 páginas y conservó 4–9; el bucle de interfaz siguió atendiendo eventos durante la consulta.

Validación del candidato final: **2.447 pruebas superadas, 3 omitidas, 143,12 s** en
`pytest-candidato-final.txt`, incluidas 28 regresiones nuevas. Otras **85 pruebas** de interfaz y
edición se repitieron al 150 % y pasaron en 28,99 s. Las omisiones son EPUBCheck externo (2) y Ollama
real optativo (1). Ruff, formato, mypy sobre 137 módulos y diff correctos. La pasada instrumentada
anterior al último pulido del encabezado del índice registró 88,95 % de cobertura y 2.447 pruebas
superadas; el candidato final se volvió a ejecutar completo sin instrumentación.
La evidencia está en `Documents/Codex/2026-09-22/correcciones-ux-ui-liblevo`; el informe local
vincula cada corrección con sus comprobaciones y límites.

Límites: renders y pruebas Qt no equivalen a aceptación humana ni a VERIFICADO EN CORPUS. El control
nativo de Windows devolvió `foreground window did not report a process id` dos veces; no se cuenta
como recorrido real repetido. No se repite aquí traducción/Ollama ni se construye el instalador.
El resaltado visual se limita a corrección/estructura de hasta 4.000 caracteres por versión;
los fragmentos largos mantienen su texto completo. La vista previa de extremos del PDF era opcional
en V10 y queda fuera de esta corrección. Sigue pendiente la prueba con una persona nueva y V1.10.

Decisión: conservar las once correcciones, sin publicar paquete ni enviar cambios a GitHub. Reabrir
ante recorte o superposición reproducible, aviso de guardado incorrecto, discrepancia del índice,
pérdida del resultado previo o bloqueo de interfaz al contar páginas.

### S4 — IMPLEMENTADO; recorrido real de una muestra el 22 de septiembre

Intención: ejecutar la aplicación como usuario con un PDF de Descargas, traducir y revisar con
IA local, confirmar y abrir el EPUB. Se utilizaron las páginas 4–9 de un PDF de 53 páginas elegido
para esta petición, mediante el selector de intervalo de la app. Los perfiles de prueba están
aislados de la cola personal; el original conserva su SHA-256. Esto no revalida el corpus histórico.

Fallos reproducidos y correcciones:

- Ollama cerrado se confundía con hardware insuficiente y no ofrecía una salida clara. La vista
  separa disponibilidad, verificación y recursos, permite iniciar la IA y muestra progreso.
- Elegir idioma no llevaba a preparar la IA. Tras prepararla, el editor mantenía una identidad
  antigua; cerrar podía perder las opciones. Ahora refresca la identidad y conserva un borrador
  durable no procesable hasta que la verificación permite continuar.
- Una respuesta cortada de revisión detenía todo el trabajo. Se conserva el fragmento anterior,
  se registra un aviso durable y no se cachea un éxito falso; cancelación y desconexión no se ocultan.
- El presupuesto de LFM cortaba su razonamiento antes de una salida corta. En la comparación local
  de cinco candidatos, 1.238 tokens agotaron el límite; 4.096 permitieron finalizar en 3.935 con cinco
  directivas válidas. La reserva es exclusiva de revisión y está limitada por el contexto; el
  traductor y los prompts no cambian.
- La cola mostraba 100 % al empezar el último fragmento. Mantiene actividad indeterminada hasta
  terminar realmente la fase.
- Una imagen alta provocaba una página vacía en Calibre. El máximo del 85 % de altura visible
  conserva su proporción y evita ese salto, comprobado con un único avance desde el primer capítulo.

Evidencia real: el primer intento falló a los 199,94 s en revisión. El reintento recuperable acabó
en 45,23 s conservando dos respuestas incompletas y mostrando su aviso. Tras corregir la reserva,
una cola nueva repitió las mismas opciones y llegó a publicación en 76,05 s: revisión de contenido
de 1.792 tokens y estructura de 3.935, ambas completas y sin fragmentos de revisión preservados.
Los tiempos posteriores reutilizan extracción/OCR y traducción; no son una comparación de velocidad.
Se confirmaron los avisos, se navegó el editor, se guardó/reabrió el borrador y se publicó desde la UI.
El EPUB se abrió desde «Abrir resultado» en Calibre. La copia de comprobación de la hoja de estilo
solo cambia CSS; texto, índice y recursos son idénticos al EPUB publicado desde la UI.

Verificación final, incluido el ajuste CSS: **2.419 pruebas superadas, 3 omitidas, 89,10 % de
cobertura, 233,51 s**;
las omisiones son EPUBCheck externo (2) y la integración Ollama optativa, complementada aquí por
el recorrido real. El ajuste de imagen supera 91 pruebas vecinas y la comparación visual en Calibre.
La ejecución completa está registrada en `pytest-final-css.txt`, junto al informe local.
Ruff, formato, mypy, dependencias y diff comprobados. La evidencia y los perfiles están fuera del
checkout en `Documents/Codex/2026-09-22/recorrido-real-liblevo`.

Límites: prueba de una muestra, no VERIFICADO EN CORPUS ni garantía del 100 %. La revisión bilingüe
se abstuvo por falta de alineación y la UI mostró 0 de 6 bloques revisados semánticamente; la
revisión monolingüe fue focal. La traducción contiene redacciones mejorables y la jerarquía es
conservadora: dos capítulos principales y encabezados interiores, sin promoción automática de
todos los días al mismo nivel. Las imágenes conservan su orientación original. V1.10 y el instalador
siguen pendientes. Reabrir ante errores de recuperación, revisión descartada sin aviso, truncación
recurrente, navegación insuficiente en una nueva muestra o páginas vacías en otros lectores.

Decisión: conservar las correcciones y el caso como evidencia de uso; no cambiar modelos ni reglas
editoriales generales a partir de este único libro. No se ha publicado una versión ni subido a GitHub.

### S3 — IMPLEMENTADO y comprobado automáticamente el 22 de septiembre

Intención: aplicar los cinco fallos reproducidos de la segunda auditoría y comprobar la hipótesis
OCR autorizada por la persona. No se añaden capacidades ni se publica un paquete.

- El editor conserva el XHTML original sin editar; zoom y metadatos no lo reescriben. Los elementos
  no representables se mantienen en capítulos de solo lectura, con aviso y guarda de escritura.
- Guardar, publicar y pasar de confirmación a editor requieren persistencia durable antes de
  cerrar la página. Un fallo conserva las ediciones disponibles; cerrar la aplicación respeta el
  editor activo. Renombrar una sección guarda previamente su texto pendiente.
- La finalización persiste estado completado y ruta antes de limpiar material recuperable. Si falla,
  restaura el estado anterior. El cierre comprueba escrituras pendientes aun con trabajos terminados.
- Comparación con extremos iguales recortados y alineación acotada, tanto de párrafos como de
  palabras. Los casos ambiguos conservan las guardas; una sustitución entre párrafos repetidos ya no
  se convierte en dos operaciones imposibles de aceptar.
- OCR forzado usa primero imagen y reserva PDF para resultados vacíos o fallidos. La ruta normal
  mantiene su orden. Los checkpoints v5 reintentan vacíos anteriores y conservan positivos v4.

Las pruebas focales cubren Qt real, SQLite ocupada, reintentos, errores de escritura, recuperación,
EPUB final sin cambios, texto pendiente al renombrar, cancelación y respaldo OCR. La comparación de
8.000 párrafos / 264.000 caracteres pasa de 6,566 s observados a mediana 0,039 s en cinco ejecuciones;
50.000 párrafos / 1.650.000 caracteres: mediana 0,254 s. La propuesta aceptada coincide con el texto
esperado. No es una garantía temporal universal.

Control OCR sintético con modelos locales: una pasada PDF más imagen, 49,232 s y 5,211 GiB RSS;
con imagen primero, 28,840 s y 4,541 GiB. SHA-256 de salida idéntico y cifra 125 conservada. El
trabajador aislado real también produce esa misma huella en 29,113 s. La comparación es una página,
con procesos nuevos y muestreo de memoria; hubo comprobaciones breves concurrentes. Demuestra el
ahorro de una llamada en esa ruta, no fidelidad general ni una mejora aprobada en corpus.

Verificación final de este checkout: **2.408 pruebas superadas, 3 omitidas, 89,09 % de cobertura,
214,54 s**. Las omisiones son las dos comprobaciones EPUBCheck externas y la integración optativa
con Ollama real. Ruff y formato (275 archivos), mypy (137 módulos), `pip check`, sincronización de
versión, enlaces documentales, diff y smoke de arranque/cierre desde código superados. El editor
protegido se renderizó a 640 y 960 px y se inspeccionó la vista compacta. Las 28 regresiones añadidas
no equivalen a aceptación editorial ni a pruebas del instalador.

Decisión: conservar los cambios locales y sus regresiones. La revisión humana V1.10 y las
comprobaciones del instalador exacto siguen pendientes. Reabrir ante contenido perdido
al editar, guardado fallido sin recuperación, estado final no durable, comparación lenta o diferencia
OCR no explicada. Evidencia de esta ejecución: informe de correcciones y registros fuera del checkout,
en `Documents/Codex/2026-09-22/auditoria-liblevo-segunda`.

### S2 — IMPLEMENTADO y comprobado automáticamente el 22 de septiembre

Petición del 22 de septiembre: aplicar las correcciones de la auditoría antes de publicar.
Se conserva el nombre Liblevo, el checkout y los cambios anteriores; no se publica ni se cambia
el modelo base.

- Una instancia de escritorio por perfil y smoke aislado para impedir sobrescrituras de cola.
- Contención SQLite acotada y reintentos espaciados, conservando orden y atomicidad de escrituras.
- Solicitudes Ollama cancelables durante cabeceras o respuestas incompletas, límites antes de
  decodificar y liberación del modelo acotada.
- Comparación directa de textos idénticos sin alineación cuadrática.
- Portadas verificadas antes de almacenar, lecturas acotadas y límite físico previo de EPUB.
- Cargadores vulnerables no usados de Accelerate desactivados, con excepción versionada comprobable.
- README resumido y evidencias anteriores separadas de este plan, conservando sus decisiones.

La prueba OCR con una página sintética, modelos ya locales y red de modelos desactivada conservó
el texto esperado y la cifra 125 en 30,237 s. Esto comprueba compatibilidad de la protección,
no fidelidad general ni mejora de velocidad del OCR.

Evidencia final de este checkout local, Python 3.12.10 y Qt 6.11.1:

- **2.380 pruebas superadas, 3 omitidas**, 89,02 % de cobertura, 198,04 s. Las omisiones son dos
  comprobaciones externas EPUBCheck y una prueba optativa con Ollama real. Ollama no estaba
  disponible durante la comprobación; la cancelación sí se verificó con conexiones loopback reales.
- Ruff (274 archivos), mypy (137 módulos), `pip check`, versión y diff sin errores.
- Ambos locks superan la auditoría con las dos excepciones justificadas en la política; la de
  Accelerate comprueba versión exacta y rechazo de sus seis alias antes de exceptuar el aviso.
- Comparación idéntica de 4.000 párrafos: mediana 0,009704 s (cinco ejecuciones, 128.000 caracteres).
  Bloqueo SQLite: intento 0,099788 s y posterior guardado correcto tras liberar la base.
- Reapertura sintética con 0/10/50 revisiones: 0,039/0,356/1,651 s desde construcción hasta primera
  proyección de ventana, imports ya cargados, 144.000 caracteres por revisión, artefactos DPAPI y
  recuperación completa. No equivale a arranque frío del ejecutable ni a libros con muchas imágenes.
- OCR real sintético y smoke de arranque desde código superados; no se ha construido un instalador.

Decisión: conservar estas correcciones locales y sus regresiones. Las pruebas no autorizan por sí
solas a publicar ni cambian decisiones editoriales o de traducción.

La revisión humana V1.10 sigue abierta. Quedan fuera de esta implementación el cambio de marca,
la aprobación editorial del corpus y la prueba del instalador exacto en un perfil limpio.
No llamar al candidato VERIFICADO EN CORPUS ni listo para publicar por pasar pruebas automáticas.

Reabrir S2 ante una regresión reproducible de sus contratos, un nuevo cargador OCR o una versión
upstream que permita retirar de forma conjunta la mitigación y su excepción temporal.


### V1.10 — IMPLEMENTADO y pendiente de revisión visual humana

Tras cerrar V1.9, la persona autoriza continuar. Se amplía la cobertura a contenido visual, antes
excluido, sin volver a ajustar prosa ni solicitar las mismas revisiones. Hipótesis de evaluación:
conservar la imagen con sus rótulos y las asociaciones de filas/columnas exige inspección humana
adicional a las guardas de integridad.

Selección fijada tras inspeccionar originales y antes de convertir: PDF 218 de *Transurfing del Ser*
(ilustración con rótulos) y PDF 276 de *36 Faces* (tabla; folio impreso 280). Ambos documentos ya son
conocidos; las referencias del banco anterior no se importan. Plan privado
`tmp/evaluation-visual-01/comparison`, PDF→Markdown con imágenes activadas, sin traducción ni EPUB.
Las dos ejecuciones por caso usan la misma versión y se agrupan solo si coinciden todos los recursos.
No hay modificación de código ni aprobación semántica anticipada. Reabrir después de la revisión
si falta contenido, se altera una asociación o la vista no permite evaluar el resultado.

La primera conversión (tabla PDF 276) falló tras dos timeouts OCR de una página sin resultado y
unos 368 segundos. El fallo final registrado es `PermissionError` al salir del contexto de fuente
temporal de `ocr_executor._worker_source_path`, referencia diagnóstica `b6d3e9f3`. El timeout y el
error de cierre son observaciones; un bloqueo de archivo por un descendiente sigue siendo hipótesis.
Se detuvo únicamente el árbol de procesos de esta evaluación para evitar repetir el fallo en las
ejecuciones restantes. Resultado contabilizado: un intento fallido y tres incompletos, no descartados
del informe. No se preparó una revisión humana ni se considera evaluada la ilustración.

Evidencia privada `tmp/evaluation-visual-01/interrupcion.json` e informe
`tmp/evaluation-visual-01/informe-interrupcion.json`. Huellas de ambos originales comprobadas intactas.
El diagnóstico reproduce que la tabla dispone de 321 letras seleccionables, calidad nativa 0,965,
una matriz explícita de 23×2 y el mismo volumen al leerla con PDFium; no requiere una segunda lectura
OCR completa. La selección conserva OCR para tablas raster inferidas, texto escaso, calidad baja o
glifos sospechosos. El cierre del proceso privado termina primero el árbol descendiente en Windows y
la limpieza del alias temporal reintenta bloqueos transitorios sin sustituir el error original.

La tanda se repite sin cambiar muestra en `tmp/evaluation-visual-fix-01/comparison`: cuatro unidades
completas, integridad verificada y originales intactos. La tabla tarda 1,844–2,875 s frente a los
368,282 s fallidos del primer intento. La ilustración tarda 29,156–59,797 s y alcanza 5,35–5,38 GiB
RSS en el árbol observado; ese coste queda como señal para el siguiente incremento, no como prueba de
regresión ni como aprobación visual. Revisión privada `500492b0-1aa2-47b9-9c86-8a678c5f92aa` preparada.
Las 336 pruebas focales, Ruff completo y la suite completa (2350 correctas, 4 omitidas por condiciones
de entorno) pasan. Falta la valoración humana de las dos páginas antes de cerrar V1.10; las referencias
anteriores conservan su alcance.
Una reproducción real con deadline de un segundo conserva el error de tiempo máximo, deja cero
directorios temporales nuevos y no mantiene procesos OCR; acredita el cierre en este Windows, no todos
los fallos posibles del motor ni otros sistemas operativos.

### U1 — SIGUIENTE: pulido de experiencia

Solo después del gate integral, revisar si la interfaz expresa con claridad:

- qué hará Liblevo;
- qué está haciendo;
- qué evidencia obtuvo;
- qué quedó sin demostrar;
- cuál es la siguiente acción segura.

No añadir paneles técnicos permanentes. La explicación detallada y el diagnóstico aparecen bajo
demanda; la ruta principal conserva una acción dominante por contexto.

## Aparcado o rechazado

| Idea | Estado | Condición para reabrir |
|---|---|---|
| sustituir Hy-MT2 por otro modelo general | RECHAZADO por ahora | candidato local con procedencia y ventaja clara en corpus humano |
| prompt global con más contexto | RECHAZADO | mecanismo acotado que no repita la degradación de 13/14 casos |
| `repeat_penalty=1.05` global | RECHAZADO | evidencia estable y significativa en holdout |
| razonamiento abreviado de LFM | RECHAZADO | adaptador nuevo que mejore precisión y recall sin degradar referencias |
| corrección automática por longitud/idioma global | RECHAZADO | señal causal demostrable; esos avisos siguen siendo solo revisión |
| aprobar por pasar guardas estructurales | RECHAZADO | nunca: las guardas no demuestran sentido ni naturalidad |
| ejecutar revisión adicional proactiva por defecto | RECHAZADO por ahora | beneficio semántico general y coste aceptable demostrados |

## Registro mínimo al actualizar este plan

Una actualización debe responder:

- qué estado cambió y con qué evidencia fechada;
- qué decisión reemplaza, si alguna;
- qué gate se cerró o abrió;
- cuál es ahora la única frontera activa;
- qué no debe repetirse sin una hipótesis nueva.

Las métricas detalladas y el contenido permanecen en informes privados. Este plan conserva solo la
conclusión necesaria para dirigir el siguiente trabajo.
