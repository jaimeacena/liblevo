# Historial de decisiones y evaluaciones

Archivo conservado íntegramente al separar el plan vivo el 22 de septiembre de 2026.
No aprueba el checkout actual. El gate vigente y las decisiones activas están en
[el plan de trabajo](work-plan.md). Las rutas privadas identifican evidencia histórica.

## Traducción: evidencia cerrada y mantenimiento seguro

Objetivo del incremento: entender por qué el piloto residual consume revisión y producir una mejora
general que reduzca dudas o aumente propuestas correctas sin tocar bloques limpios.

### Pasos T1 y T2 — CERRADOS: evidencia y cuello de botella

El 31 de agosto de 2026 se validaron los hashes, las decisiones y la cadena de revisiones de los 12
casos que llegaron a tener propuesta. El informe privado, reproducible y sin texto documental ni
notas humanas conserva 19 eventos: las 19 propuestas superaron las guardas mecánicas, pero solo una
fue aprobada. La aceptación fue 1/12 en la primera ronda y 0/7 en rondas posteriores; el estado final
es una aprobación, seis correcciones aún rechazadas y cinco casos sin decisión concluyente.

La conclusión demostrada es que **la generación de propuestas es el cuello de botella observado**.
Las guardas protegen estructura, cifras, idioma y residuos, pero no demuestran calidad semántica. Las
decisiones actuales no proporcionan verdad humana directa suficiente para separar, caso por caso,
fallo de detector y fallo de selección de contexto; esos estados permanecen **NO DEMOSTRADOS** en vez
de forzar una etiqueta. Dos modelos locales clasificaron las 13 notas en una taxonomía cerrada, pero
solo coincidieron de forma completa en una: esas categorías son orientativas y no se usarán como
gate ni como explicación causal.

### Paso T3 — RECHAZADO: descarte barato de parches por consenso

Se probará una sola hipótesis nueva: sustituir la reescritura completa del bloque por parches mínimos
`old → new` propuestos de forma independiente por dos modelos locales. Un candidato solo sobrevive al
piloto si ambos producen exactamente el mismo parche, `old` aparece una sola vez, el cambio queda
acotado a la instrucción humana y vuelven a pasar todas las guardas. El desacuerdo se convierte en
«sin propuesta», nunca en una elección automática.

El descarte se ejecuta solo sobre evidencia ya revisada y controles limpios; no cambia el producto ni
solicita nueva atención humana. Se medirá:

- consenso exacto y tasa de abstención;
- extensión y número de parches;
- conservación de estructura, cifras, enlaces y valores protegidos;
- modificación nula de controles limpios;
- casos donde la nota no puede expresarse como parche inequívoco.

El descarte terminó con 0/13 parches por consenso y cero modificaciones del único control limpio.
Qwen 3.5 produjo nueve respuestas estructuradas de catorce, pero Qwen 4B solo una y LFM ninguna; no
existió una pareja capaz de sostener el contrato dual. El resultado no demuestra que los parches sean
intrínsecamente imposibles, pero sí rechaza implementarlos con los modelos instalados. No se relaja el
consenso, no se prueba otra plantilla y no se abre T4.

### Paso T4 — NO ABIERTO: gate antes de volver a pedir revisión

Solo se crea una nueva tanda humana si:

- todas las guardas duras pasan;
- ningún control limpio cambia de forma material;
- las propuestas son distintas por una causa técnica nueva, no por otra redacción del prompt;
- la evaluación ciega interna muestra una posibilidad razonable de alcanzar los gates de la política;
- la tanda es pequeña, diversa, deduplicada y cómoda desde móvil.

No hubo ninguna propuesta por consenso exacto. El experimento queda cerrado sin trasladar su coste al
usuario.

La aprobación de producción sigue exigiendo la muestra completa definida en
`local-ai-model-policy.md`: precisión mínima del 98 %, cero falsos positivos críticos, modificación de
bloques limpios como máximo del 1 % y recall mínimo del 80 % sobre errores sembrados.

### Salida de traducción — MANTENIMIENTO SEGURO

La fase pasa de frontera activa a mantenimiento bajo este contrato:

- Hy-MT2 conserva sus puertas duras y referencias humanas;
- las señales residuales se clasifican con precisión suficiente o quedan presentadas como revisión
  explícita, sin sobreafirmar cobertura;
- cualquier corrector automático cumple los gates o permanece desactivado;
- un holdout nuevo confirma que las reglas son generales;
- el coste de llamadas y revisión no crece de forma desproporcionada.

Hy-MT2 continúa como traductor base; las puertas duras y las referencias humanas se conservan. Las
señales semánticas no demostrables se presentan como revisión explícita y ningún corrector automático
queda activado. Los tres holdouts del piloto confirman que perseguir otra ronda de generación tendría
peor retorno que validar el recorrido integral. Traducción se reabre solo con un mecanismo o modelo
nuevo, no con otra variante de prompt.

### V1.9 — IMPLEMENTADO y aceptado en los dos fragmentos revisados

Petición explícita de corregir V1.8. Se separan causas reproducidas: (1) las sangrías son pequeñas
y el salto entre párrafos aumenta de unos 6,8 a 9,06–9,65 puntos, señal ignorada antes; (2) la lista
de dos elementos no alcanzaba el mínimo de tres; (3) una continuación de palabra partida, con
negrita parcial, se clasificaba como título y perdía ese énfasis antes de volver a unirse a la prosa.
La vista Markdown reproduce el defecto ya materializado; no lo origina.

Cambios acotados: estimar espaciado con al menos tres continuaciones de la misma página, incluidas
las que regresan al margen desde una primera línea sangrada; reconocer 1–2 solo tras una introducción
próxima terminada en dos puntos; impedir la falsa clasificación de título en la continuación
minúscula compatible, aún partida y sin nivel explícito de índice. Se mantienen las reglas entre
páginas y los umbrales de sangría anteriores. Solo cambia renderizado nativo, no el contenido de
los checkpoints extraídos ni su esquema.

Cinco variantes sintéticas reproducían los fallos antes del cambio. La primera comparación
`tmp/evaluation-transurfing-fix-01/comparison` resolvió lista, negrita y gran parte de la estructura,
pero dos citas aún seguían juntas: faltaban muestras de espaciado al excluir regresos desde sangría.
Se conserva esa salida intermedia; no se entrega ni se declara aprobada. La medición final incluye
esos regresos acotados y añade una regresión específica. **474 pruebas focales y vecinas superadas**;
Ruff, formato y mypy del módulo PDF superados.

La verificación final conserva la selección previa de seis casos conocidos en dos libros; cuatro
son controles humanos que deben quedar idénticos. Solo los dos fragmentos afectados se preparan
para una nueva revisión. No se amplía el corpus ni se usan sustituciones por frase. Límites:
espaciado local no equivale a estructura semántica universal; ante escasa evidencia se conserva la
conducta anterior. Reabrir ante una separación indebida, pérdida de énfasis o deterioro de controles,
con fuente y salida exactas; aceptación humana pendiente.

Verificación final privada `tmp/evaluation-transurfing-fix-01/final-checks.json`: los seis casos
completan conversión e integridad; los cuatro controles permanecen idénticos. En los dos afectados
se conserva el texto al excluir espacios y marcadores de énfasis/título, sin que esa comprobación
equivalga a fidelidad semántica. PDF 21–22 presenta dos ítems ordenados y la frase señalada completa
en negrita; PDF 219–220 presenta 18 párrafos, incluidas las citas antes unidas.
Entrega `tmp/evaluation-transurfing-fix-01/entry.txt`, revisión
`365fb461-6a8e-4593-b4cf-9febab669435`: dos resultados únicos, cuatro intentos, originales y vistas
completas, huellas verificadas y coincidencia exacta con la verificación final. No hay juicios
humanos importados todavía para este paquete. Las salidas rechazadas permanecen intactas.

Cierre técnico: **2346 pruebas superadas, 4 omitidas, cobertura 89,07 %**, 261,08 segundos;
registro `tmp/transurfing-fix-suite.txt`. Omisiones: symlinks no disponibles, EPUBCheck externo
(dos) y Ollama real. Ruff, formato, mypy y diff comprobados. Estado **IMPLEMENTADO**, verificación
acotada a los seis casos y contratos ejecutados; la aprobación de los dos resultados nuevos
corresponde a la revisión humana, no al resultado de la suite.

Revisión humana del 2026-09-05 importada: los dos fragmentos reciben `no_error_observed`, sin
comentarios adicionales y con inspección de original y resultado confirmada. Son dos unidades
humanas compartidas por cuatro intentos, no cuatro observaciones independientes. Evento validado
`a1e8653f-8022-4d41-a7ca-de3511593515`; informe privado
`tmp/evaluation-transurfing-fix-01/informe-revision-humana-2026-09-05.json`, SHA-256
`b5573b9498e2f5f3342c5a69405251b1d4b6832de05f7f5cca2127cd8bed631a`.
Decisión: aceptar las correcciones en PDF 21–22 y 219–220 y conservar ambos resultados como
referencias humanas acotadas. Se cierra esta depuración; las versiones rechazadas y juicios
anteriores permanecen como controles negativos. No se aprueba el libro completo ni se estima
precisión general. Registro verificado mediante importación, identidad del paquete, informe y diff;
sin cambios de código ni reejecución de pruebas en este cierre.

### V1.8 — REVISADO: prosa rechazada por estructura y énfasis; índice sin errores observados

Tras la aprobación de V1.7, se cierra la depuración de los tres fragmentos conocidos. La siguiente
pregunta es si la extracción conserva estructura y texto en un PDF distinto, sin adaptar más reglas
al mismo libro. Pendiente de que la persona indique nombre y ubicación del documento que conoce.
No se exploran documentos privados arbitrarios ni se importa el banco anterior como referencia.

La persona propone *The Surrender Experiment* en Descargas. La búsqueda acotada encuentra dos EPUB
(original y versión con sufijo de español), ningún PDF con ese nombre. Pendiente de elegir entre
validar el EPUB original como otra capacidad del producto o aportar otro PDF para comprobar la
transferencia de las correcciones nativas. No se ha procesado ninguno ni se considera la versión
en español una referencia fiable. Esta diferencia de formato impide atribuir una validación EPUB
a las reglas geométricas de PDF.

Al recibirlo: comprobar procedencia y huella, seleccionar antes de convertir un máximo de tres
fragmentos breves que cubran prosa, frontera de página y estructura relevante, y congelar alcance y
expectativas. Usar la versión actual y el formulario legible con comentarios; conservar las dudas
para revisión humana. No declarar `new` ni holdout sin procedencia confirmada; no ampliar la selección
después de ver resultados ni cambiar extracción y traducción en la misma prueba.

La persona primero autoriza el EPUB original, pero cambia después a *Transurfing del Ser* en PDF
antes de procesar aquel. Se abandona esa preparación EPUB sin generar conversiones ni valoraciones.
El PDF elegido tiene 438 páginas y 6.556.214 bytes. Procedencia `unverified`: es otro archivo elegido
explícitamente, pero no consta independencia de usos anteriores; no se llama holdout ni se importa
ninguna referencia histórica.

Tras inspeccionar únicamente páginas originales para seleccionar estructura, se congelan tres
casos de riesgo: PDF 6 (índice), 21–22 (título, lista, prosa y diálogo) y 219–220 (título y citas en
cursiva). Las dos fronteras entre páginas cierran frases; no prueban una continuación abierta.
Alcance: PDF→Markdown, sin imágenes, traducción ni EPUB. Se ejecuta la misma versión dos veces y
se agrupan resultados idénticos; no es una comparación de mejoras entre versiones. Plan privado
`tmp/evaluation-transurfing-01/comparison`. La selección precede a las conversiones y no se ampliará
según sus resultados. Aprobación semántica pendiente de revisión humana.

Entrega `tmp/evaluation-transurfing-01/entry.txt`, revisión
`6a3f3211-eb8b-45da-a551-5afe53e09c56`: seis conversiones completas con integridad verificada;
tres resultados únicos porque cada pareja coincide, cinco páginas originales visibles y ninguna
vista truncada. Huellas del paquete y archivos enlazados comprobados; original intacto. Informe
privado `tmp/evaluation-transurfing-01/informe-previo.json` sin juicios humanos. No cambia código ni
se repite la suite: evidencia de ejecuciones reales, paquete y diff documental; no implica aprobación
de los resultados. Reabrir después de la revisión, con una causa nueva por defecto observado.

Revisión humana del 2026-09-05 importada: PDF 21–22 y 219–220 reciben `major`; PDF 6,
`no_error_observed`. La persona señala párrafos, elementos numerados y diálogo unidos indebidamente,
y una frase cuya negrita debería abarcarla completa. El segundo fragmento remite a los mismos
problemas; no se inventan localizaciones más precisas para él. Comentarios completos conservados
privadamente. Son tres unidades de revisión, no seis observaciones independientes.

Evento `bb119d65-9c2c-4726-a003-c4e21a3e054a`; informe
`tmp/evaluation-transurfing-01/informe-revision-humana-2026-09-05.json`, SHA-256
`93480d1993d7c8e7c1fc9d5d67e703dcb3d1ab2af02ddcd15b2b789359a5198a`.
Plan, paquete y vínculos de los juicios validados al importar. Decisión: conservar el índice como
referencia humana acotada y los dos fragmentos rechazados como controles negativos. Las correcciones
validadas en el libro anterior no bastan para estos diseños; no se declara transferencia satisfactoria.

La frontera activa es diagnosticar por separado la segmentación de párrafos/listas/diálogo y la
propagación del énfasis entre líneas, contrastando el Markdown materializado y su vista con la
geometría y fuentes originales. Aún no se atribuye una causa técnica ni se modifica procesamiento.
No basta con reducir otra vez un umbral de sangría: cualquier propuesta debe explicar esta evidencia
y conservar los casos humanos anteriores. Registro e informe verificados y diff documental limpio;
sin cambios de código ni reejecución de la suite.

### V1.7 — IMPLEMENTADO y aceptado por la persona en el fragmento revisado

Petición explícita tras V1.6. El PDF 101 codifica un espacio de aproximadamente 1,57 puntos entre
las dos letras iniciales y otro de 3,91 antes de la palabra siguiente. La segunda letra mide
aproximadamente 6 puntos frente a 7,5 de las vecinas y conserva línea base. El extractor refleja
ese espacio; no es un defecto de la vista de revisión ni lo introdujo la reparación de párrafos.

Hipótesis: una pareja inicial aislada, mayúscula seguida de minúscula de tamaño reducido y con una
separación interna mucho menor que el espacio siguiente, admite recomposición geométrica acotada.
Se implementa al construir la línea nativa, conservando caracteres, énfasis y demás separadores;
no hay diccionario ni sustitución por frase. La identidad de caché PDF pasa a v12 para no recuperar
extracciones anteriores con el defecto. No se elimina la caché antigua.

Reproducción sintética: dos variantes tipográficas fallaban antes del cambio y catorce controles
permanecían intactos; después pasaron **339 pruebas focales y vecinas**. La prueba de separación de
cachés también falló antes de incrementar su identidad. Comparación congelada privada
`tmp/evaluation-initial-fix-01/comparison`: en PDF 100–101 desaparece exactamente un espacio y
ningún otro carácter cambia; índice PDF 6 y control PDF 20–21 quedan idénticos. Se prepara una
revisión del único fragmento afectado, con parejas idénticas agrupadas, sin volver a pedir valorar
los controles intactos. Los originales y juicios anteriores se conservan.

Límite: geometría corroborada en un caso conocido, no detección universal de espacios incorrectos.
No se declara **VERIFICADO EN CORPUS**. Reabrir ante una unión indebida o un patrón tipográfico
distinto, investigando su evidencia antes de ampliar la regla. La aceptación visual sigue pendiente.

Verificación tras cambiar la identidad de reanudación: **425 pruebas de procesamiento, PDF y
contratos de pipeline superadas**, Ruff, formato y mypy de los dos módulos modificados superados.
Entrega privada `tmp/evaluation-initial-fix-01/candidate-entry.txt`, revisión
`558c1bae-c8ad-4dc4-944c-233cada1ee3e`: un resultado único, dos intentos completos, vistas sin
truncar, original intacto, huellas del paquete y coincidencia exacta con el candidato comparado
verificados. No se han importado valoraciones para esta nueva entrega.

Cierre técnico: suite completa con **2336 pruebas superadas, 4 omitidas y cobertura 89,08 %**,
252,19 segundos; registro privado `tmp/initial-fix-suite.txt`. Omisiones: symlinks no disponibles,
EPUBCheck externo (dos) y Ollama real. Cambio **IMPLEMENTADO**, sin otros cambios de texto en los
tres fragmentos comprobados; aprobación humana del resultado nuevo pendiente.

La persona confirma después «Genial, todo bien ahora. Continuemos». Se registra como aceptación
conversacional del único resultado entregado (PDF 100–101), tras verificar de nuevo plan, paquete y
los dos intentos vinculados. Evidencia privada `tmp/evaluation-initial-fix-01/aprobacion-conversacion.json`,
identificador `f53e4995-2cca-423e-9033-f29b2f20d588`. No se fabrica una respuesta de formulario ni se
atribuyen casillas o tiempos de inspección no proporcionados; los agregados de juicios formales no
cambian. Se acepta la corrección en este fragmento y se conservan las referencias anteriores de
índice y continuidad. No equivale a aprobar el libro completo ni la precisión general.
Solo cambia el registro de decisión: huellas y diff comprobados, sin modificación de producto ni
reejecución de pruebas. La frontera activa pasa a V1.8.

### V1.6 — IMPLEMENTADO y revisado: controles sin errores observados; palabra partida pendiente

Petición explícita de corregir los defectos de V1.4. Causas comprobadas en PDF 100–101: las tres
sangrías miden 13,5–14,15 puntos con cuerpo de 7,5; el umbral anterior exigía el 8 % del ancho de
página. El pie cumple las guardas geométricas y se rechazaba únicamente por contener cifras.
Se sustituye ese umbral por el tamaño tipográfico y se admiten cifras internas en etiquetas que
empiezan por letras. Se conservan cierre de frase, guardas de pie y reglas entre páginas.

La reproducción sintética falló en cuatro variantes antes del cambio. Después, **323 pruebas
focales y de contratos vecinos superadas**; Ruff y formato superados. Comparación privada congelada
`tmp/evaluation-paragraph-fix-01/comparison`: índice idéntico; en PDF 100–101 únicamente tres
espacios pasan a separaciones de párrafo y desaparece el pie con su marcado. Ambas versiones
completaron conversión e integridad, sin modificar el original.

El control PDF 20–21 conserva el mismo texto normalizando espacios, pero introduce nueve
separaciones adicionales: no se afirma identidad con la referencia humana previa. Se incorpora
también a la nueva revisión de candidatos, junto al índice y PDF 100–101. Las parejas de esta
entrega son repeticiones de la versión actual y se agrupan; la comparación antes/después permanece
en su paquete separado. No se importan valoraciones ni se aprueba el corpus automáticamente.

Límites: una sangría y una frase cerrada no demuestran toda la estructura editorial; la revisión
humana decidirá sobre las separaciones adicionales. La regla anterior de número y texto aún puede
retirar una nota breve sin puntuación de cierre; este incremento no amplía esa regla y no declara
resuelta esa ambigüedad. Reabrir ante separación indebida, pérdida de contenido o un pie no detectado,
con evidencia geométrica nueva, sin sustituciones por título.

Cierre técnico: **2319 pruebas superadas, 4 omitidas, cobertura 89,08 %**, 259,77 segundos;
registro `tmp/paragraph-fix-suite.txt`. Omisiones: symlinks no disponibles en esta cuenta,
EPUBCheck externo (dos) y Ollama real. La continuidad entre páginas se comprobó conservada;
las nueve sangrías adicionales miden 1,83–1,95 veces el cuerpo. Entrega privada
`tmp/evaluation-paragraph-fix-01/candidate-entry.txt`, revisión
`f1c1e482-d264-4e0a-bed5-65dc62603382`: tres resultados únicos, seis intentos completos,
originales intactos, vistas completas y paquete sellado verificados. **Revisión humana pendiente**;
no se declara verificación general en corpus ni se modifican juicios anteriores.

Revisión humana del 2026-09-05 importada: PDF 20–21 e índice PDF 6,
`no_error_observed`; PDF 100–101, `minor`, con una palabra inicial partida por un espacio.
Los comentarios completos permanecen privados. Son tres unidades humanas, no seis observaciones
independientes. Evento `3ccd85a0-4662-426c-a3bb-844c93d2223f`; informe privado
`tmp/evaluation-paragraph-fix-01/informe-revision-humana-2026-09-05.json`, SHA-256
`75f8422bb5a5e0c40a25d9be755f9680983d882a7c4515a257c40cfa94586e9d`.
Identidad y paquete validados; ambos comentarios compartidos quedaron registrados. Una comprobación
literal acotada confirma que el defecto señalado ya aparece en A y B de la comparación anterior:
no fue introducido por el cambio de sangrías o pies. Su causa técnica aún no está determinada.

Decisión: conservar los dos controles como referencias humanas de este alcance y el caso con
`minor` como pendiente; no hay nuevas objeciones a pies, párrafos o continuidad en esta respuesta.
La siguiente frontera es investigar la separación interna de la palabra sin aplicar sustituciones
por frase ni unir letras indiscriminadamente. Este registro no modifica procesamiento ni reejecuta
la suite; verifica importación, informe, presencia del defecto en ambas versiones y diff documental.

### V1.5 — IMPLEMENTADO: revisión legible y comentarios en el formulario

Petición explícita: sustituir el flujo con código visible y comentarios fuera del formulario por una
revisión sencilla. Hipótesis: original y resultado lado a lado, formato pasivo, comentario junto a
la valoración y una sola confirmación reducen pasos sin perder trazabilidad. Se eliminan el campo
de tiempo, las confirmaciones por panel, instrucciones repetidas y la exposición de código como
alternativa al portapapeles. Si copiar falla, se descarga la revisión para adjuntarla.

Las páginas seleccionadas del PDF se renderizan localmente; Markdown/HTML conserva tablas y énfasis
en una vista de lectura pasiva. Se agrupan salidas solo si coinciden archivo y recursos completos;
el mapping conserva ambos intentos y el informe cuenta las unidades de revisión compartidas.
Los comentarios permanecen privados y no aparecen en agregados. Se conserva la compatibilidad de
respuestas anteriores. No cambia el procesador ni se vuelven a convertir documentos para esta mejora.

Se preparó un paquete nuevo sobre las salidas de V1.4; entrada privada
`tmp/evaluation-transfer-01/simple-entry.txt`. Dos casos, un resultado único por caso, tres páginas
originales visibles y tabla de índice renderizada. Huellas del paquete, enlaces y vistas completas
comprobados. Los paquetes y juicios anteriores permanecen intactos. La respuesta humana del
2026-09-05 llegó con comentarios y se importó correctamente: confirma ese recorrido real de
transmisión y registro, sin implicar una aprobación general de la experiencia de uso.

Verificación: **39 pruebas específicas superadas**, incluyendo HTML pasivo, imágenes locales,
render PDF, comentarios, agrupación por recursos y entrada CLI desde otro directorio. Ruff, formato
y mypy de ambos módulos de evaluación superados. Navegador con datos exclusivamente sintéticos:
tabla visible, comentario editable, navegación, cambio de versión y copia verificados; la copia
conservó comentarios y no confirmó respuestas «Necesito ayuda». A 320 píxeles no hubo desbordamiento
horizontal. La recuperación mediante selector no pudo verificarse por una interrupción de la sesión
de navegador; se comprobó su lógica JavaScript con siete aserciones de recuperación e identidad.
Servidor de prueba cerrado; ningún documento privado se sirvió. No se repitió la suite de producto,
que no cambió; la revisión visual del paquete privado final queda en manos de la persona.

Límites y reapertura: vista de lectura, no reproducción exacta de cualquier lector EPUB. Se muestran
hasta seis páginas PDF y vistas textuales acotadas; los formatos no admitidos requieren lector local.
Reabrir ante dificultad de lectura, pérdida de comentarios o recuperación fallida. La corrección
de esta interfaz no acredita calidad del corpus ni promueve referencias históricas.

### V1.4 — REVISADO: índice sin errores observados; párrafos y pie pendientes

La persona autorizó continuar tras validar el caso anterior. Se eligieron antes de procesar una
página de índice (PDF 6) y una frontera de capítulo (PDF 100–101), previa inspección visual local.
Objetivo: conservar entradas y folios del índice, separar capítulos y retirar pies corrientes.
Se excluyen traducción, imágenes y EPUB; no se presentan estas páginas del mismo libro conocido
como holdout ni se importan referencias históricas. Expectativas pendientes de validación humana.

Plan privado y entrada: `tmp/evaluation-transfer-01/entry.txt`. Ambas ejecuciones usan la versión
actual; no comparan cambios de código. Cuatro conversiones completas, dos casos; cada pareja produce
texto idéntico, lo cual acredita repetibilidad observada, no corrección. Se verificaron original
intacto, archivos enlazados y vistas sin truncar. Informe con evidencia insuficiente, sin juicios
humanos importados. Límite de interfaz: el índice contiene una tabla HTML que la vista segura muestra
como texto; si impide juzgarlo, registrar «Necesito ayuda», nunca inferir aprobación. El segundo caso
permite revisar texto y título, no la ilustración excluida de la conversión.

El estado descrito arriba corresponde a la preparación original; V1.5 sustituyó su presentación.
Revisión humana del 2026-09-05 recibida mediante el paquete
`de8ab776-84d5-4401-b298-c0fb2bdb7c27`: índice PDF 6, `no_error_observed` en ambos intentos;
PDF 100–101, `minor` en ambos. La persona señala tres comienzos de párrafo que deberían separarse
y un pie con el título del libro que permanece. Los comentarios completos se conservan privados.
Son **dos unidades de revisión humana**, no cuatro observaciones independientes: cada pareja es
idéntica y comparte valoración. No demuestra una mejora comparativa entre versiones.

Evento importado `aba3f9a7-476a-4f6c-bd81-831ffe06b85b`; informe privado
`tmp/evaluation-transfer-01/informe-revision-humana-2026-09-05.json`, SHA-256
`dcb07c2b6d22c4f190ad36cb5b8b5f9c421029affcd996d7789576ae59f603e8`.
Sin valoraciones pendientes; ambos comentarios no vacíos quedaron registrados y los agregados no
incluyen texto documental. Decisión: conservar el índice como referencia humana de este alcance y
el otro fragmento como control con defectos pendientes. No se aprueba el libro ni el banco anterior.
La frontera siguiente es diagnosticar la separación de párrafos y el pie restante antes de proponer
otra regla general; no se atribuye aún una causa técnica ni se repiten conversiones sin hipótesis.

No cambió código ni se repitió la suite al registrar la revisión: importación con validación de
identidad, informe y comprobación del diff documental. Las referencias humanas y rechazos anteriores
se conservan sin modificar. Reabrir el caso afectado con un mecanismo que conserve párrafos reales
sin reintroducir cortes artificiales entre páginas.

### V1.3 — IMPLEMENTADO: continuidad confirmada por la persona en el caso revisado

La persona autorizó corregir la frase interrumpida detectada en V1.2. Causa confirmada: la marca de
procedencia era el último bloque y bloqueaba la unión, aunque `_should_join_lines` aceptaba la
geometría. Hipótesis: permitir atravesar únicamente esa marca entre páginas consecutivas restaura
la continuidad sin relajar el criterio lingüístico ni geométrico. El marcador se conserva en línea
y el bloque acumula sus páginas de origen; no se atraviesan anclas, avisos, títulos, listas u otros
bloques. La reparación específica de palabras con guion no cambia.

Plan nuevo congelado, A ejecutado antes del cambio y B después. Entrada privada:
`tmp/evaluation-continuity-fix-01/entry.txt`. Son las mismas dos páginas conocidas, sin traducción,
imágenes ni EPUB. La comparación publicada muestra exactamente un cambio de espacio: cuatro saltos
de línea sustituidos por un espacio; ningún otro carácter cambia y los pies siguen ausentes.
Ambas ejecuciones terminaron; no hay valoraciones importadas para este paquete nuevo. Las revisiones
y salidas anteriores se conservan. No se declara **VERIFICADO EN CORPUS** ni aprobado el libro.

Verificación focal: 300 pruebas de PDF, evaluador y contratos de pipeline superadas, incluidas seis
variantes nuevas de continuidad y fronteras protegidas. Ruff, formato y mypy superados. Decisión
pendiente de revisión humana del candidato. Reabrir ante un corte restante o una unión indebida;
conservar la relación con el original en cualquier corrección posterior.
La suite completa pasó con **2296 pruebas, 4 omitidas y cobertura 89,08 %**, en 264,72 segundos;
registro privado `tmp/continuity-fix-suite.txt`. Las omisiones corresponden a EPUBCheck externo y
Ollama real, no ejecutados en este incremento. La comprobación final confirmó original intacto,
identidad estable de A/B y sustitución exacta de cuatro saltos por un espacio en la salida publicada.

Revisión humana recibida e importada: resultado 1 = B (continuidad corregida), `no_error_observed`;
resultado 2 = A (solo pies corregidos), `minor`. La persona confirma que B elimina ambos problemas y
que A mantiene el salto de página dentro de la frase. Evento validado
`5813b37b-d220-47ce-8c61-0244d4fde144`; informe privado
`tmp/evaluation-continuity-fix-01/informe-revision-humana-2026-09-05.json`, SHA-256
`eafe9e86d408f868513872c0776dd2c8de0c65741532d8fa5317e52a61dda73a`.
Decisión: aceptar la corrección para este caso de dos páginas y conservar B con su revisión como
referencia humana local; A y los rechazos anteriores permanecen como controles. No se promueven
expectativas históricas ni se declara **VERIFICADO EN CORPUS**. Quedan fuera otras páginas, traducción,
imágenes y EPUB. Siguiente paso recomendado: comprobar otros fragmentos con contenido distinto antes
de ampliar conclusiones. Reabrir el caso ante regresión, pérdida de contenido o nueva objeción humana.
Este cierre verifica la importación, vinculación e integridad de las respuestas y el diff documental;
no modifica el motor ni repite la suite registrada arriba.

### V1.2 — IMPLEMENTADO: pies confirmados en el caso; continuidad pendiente

Petición autorizada tras el rechazo de V1.1: corregir los rótulos inferiores sin borrar contenido
legítimo y repetir el mismo fragmento. Se confirmó que dos páginas no aportan repetición suficiente;
con ocho páginas de contexto solo uno de los dos rótulos obtiene consenso. Ambos comparten una fila
inferior aislada con el folio exterior. Hipótesis del cambio: esa geometría permite reconocer el
rótulo sin ampliar I/O ni eliminar palabras por nombre. La regla se comparte entre la extracción
visible y la detección de títulos de referencia. Una protección adicional conserva frases numeradas
terminadas en puntuación, que el patrón anterior podía confundir con un pie.

Se congeló un plan nuevo y se ejecutó A antes de modificar el motor; B usa la corrección sobre las
mismas páginas físicas 20–21. Se conservan originales, salidas rechazadas y valoraciones anteriores.
Entrada privada: `tmp/evaluation-footer-fix-01/entry.txt`. El candidato elimina ambos rótulos
señalados y la secuencia de palabras restante coincide exactamente con A tras excluirlos.
Este control no demuestra igualdad visual, puntuación ni calidad de otras páginas. Ambos intentos
terminaron; la revisión nueva no tiene valoraciones importadas ni aprobación automática.
La comparación posterior carácter a carácter confirmó exactamente dos eliminaciones: los rótulos
señalados y sus marcas Markdown/espaciado, sin otros cambios. El original conserva su huella.

Verificación focal: 292 pruebas de PDF y evaluador superadas, incluidas diez pruebas nuevas de
rótulos y conservación de contenido ambiguo. Ruff, formato y mypy superados.
La suite completa pasó con **2290 pruebas, 4 omitidas y cobertura 89,09 %** en 290,63 segundos;
registro privado `tmp/footer-fix-suite.txt`. Las omisiones incluyen validación externa y Ollama real,
que no se ejecutaron en este incremento. No se afirma
**VERIFICADO EN CORPUS**: es un candidato probado sobre dos páginas de un documento conocido,
sin traducción, imágenes ni salida EPUB. Decisión pendiente de la persona; reabrir ante pérdida de
contenido, pies restantes o una valoración negativa, conservando este caso como control.

Revisión humana recibida: resultado 1 = B, etiqueta `minor`; resultado 2 = A, etiqueta `major`.
La persona confirmó explícitamente que la segunda observación se refería al resultado 2. Confirma
la eliminación de los pies en B, pero señala una frase interrumpida por saltos al cambiar de página;
A conserva ambos problemas. Evento validado `46fd9e35-2f06-43f4-bb17-59d408907820`, informe privado
`tmp/evaluation-footer-fix-01/informe-revision-humana-2026-09-05.json`, SHA-256
`1cc0625d11f90259fb6d6c78fc25d56fdba378495781be5f0a3947cb271509a8`.
No se sustituyen las etiquetas por aprobación ni se amplía el alcance al libro completo.

Diagnóstico inicial de continuidad: se confirmó el corte en B entre las últimas palabras de una
página y las primeras de la siguiente. `_should_join_lines` acepta las líneas nativas de esa
frontera; el resultado final conserva cuatro caracteres de salto de línea en ese tramo. Investigar
el ensamblado de bloques y las marcas internas de procedencia antes de relajar reglas geométricas
o eliminar separadores indiscriminadamente. Conservar párrafos legítimos, títulos, listas, tablas
y asociación a páginas. Estado **PENDIENTE** para continuidad; eliminación de pies confirmada por
la persona únicamente en este caso conocido. Este cierre importa respuestas y registra diagnóstico,
sin modificar código ni repetir la suite; comprobaciones de importación, salida y diff documental.

### V1.1 — IMPLEMENTADO: revisión guiada para una persona no técnica

Primera revisión documental preparada por elección de la persona: dos páginas de un PDF conocido,
con referencias **sin validar**, sin importar expectativas históricas. Plan privado y entrada en
`tmp/evaluation-real-01/entry.txt`. Alcance: páginas físicas 20–21, extracción a Markdown sin
traducción ni imágenes; no evalúa presentación EPUB. Ambos brazos usan la misma versión para
establecer una primera referencia, no para demostrar una mejora. Las dos conversiones terminaron;
se comprobó original intacto, vistas de texto sin truncar y archivos enlazados existentes. Se
inspeccionaron visualmente ambas páginas originales. Estado **PENDIENTE DE REVISIÓN HUMANA**,
informe con evidencia insuficiente. No es una muestra nueva ni una aprobación del libro completo.
No cambió código ni se reejecutó la suite: verificación mediante ejecución real, comprobación del
paquete y diff documental. Reabrir después de recibir las valoraciones; conservar cualquier rechazo
como control antes de proponer correcciones.

Revisión recibida: **RECHAZADO** en ambos brazos por la persona, con etiqueta `major`. Único
problema comunicado: rótulos corrientes del pie de página incorporados al flujo de texto. No se
infiere ausencia absoluta de otros errores. Importación validada en el evento
`3509f680-1a4c-4b2c-9fa0-0b9790c80ee1`; informe privado
`tmp/evaluation-real-01/informe-revision-humana-2026-09-05.json`, SHA-256
`6025b9401eef4df25dd9134db86c3421d54683777b4d3fa95ca2218cf40cce25`.
Se confirmó presencia de ambos rótulos señalados en ambas salidas. Las salidas rechazadas se conservan
sin modificar. El informe requiere decisión humana; la decisión aquí registrada es rechazo de este
caso, sin aprobar el corpus ni promover expectativas históricas.

Localización inicial: `_repeated_margin_lines` necesita presencia en tres páginas; el intervalo
evaluado contiene solo dos. `_omit_margin_line` también reconoce algunos pies aislados, pero su
patrón exige número y texto juntos. Hipótesis pendiente: la falta de contexto entre páginas deja
escapar rótulos alternos sin número. Antes de corregir, confirmar la geometría y comparar con un
intervalo contextual; proteger notas al pie y texto legítimo cercano al margen. No eliminar palabras
por su nombre ni ampliar la zona de borrado sin controles. Este cierre registra evidencia y
diagnóstico inicial; no cambia el motor ni afirma que el fallo esté corregido. Verificación:
importación real, integridad del paquete, presencia en salidas y diff documental.

Intención autorizada: facilitar la validación sin comandos. Hipótesis: presentar un caso cada vez,
explicar las cuatro valoraciones y permitir copiar las respuestas reduce trabajo de preparación sin
convertir dudas o respuestas vacías en aprobaciones. La página indica formato e idioma solicitados,
muestra el original de texto y permite guardar/recuperar borradores; el agente importa las respuestas
con las mismas comprobaciones de identidad. Copiar no registra ni envía decisiones automáticamente.

Evidencia de este incremento: 27 pruebas del evaluador superadas; Ruff, formato y comprobación del
diff sin errores. Sintaxis JavaScript y diez aserciones de estado/serialización comprobadas con Node.
Se preparó una práctica independiente de un caso sintético TXT→Markdown, con dos conversiones reales
completas y estado **EVIDENCIA INSUFICIENTE**, sin importar valoraciones. Entrada local de la práctica:
`tmp/evaluation-guided-practice/entry.txt`. El banco anterior sigue sin incorporarse.

Límites: la vista integrada bloqueó la apertura del archivo local; esta versión de la página no tiene
verificación visual en navegador. La suite completa registrada en V1 corresponde al incremento
anterior; aquí se verificó el evaluador, sin cambiar el procesamiento del producto. No hay todavía
**VERIFICACIÓN EN CORPUS** ni revisión humana con documentos reales. La práctica solo enseña el recorrido.

Revisión humana de la práctica recibida el 5 de septiembre: la persona marcó ambos resultados como
«sin error observado» y confirmó la comparación con el original. Se normalizaron únicamente los
escapes de formato del mensaje; el importador verificó paquete, paneles y resultados sin modificar.
Evento `074aa69f-a2ac-43d0-8083-1c292c4ba61e`; informe privado
`tmp/evaluation-guided-practice/informe-revision-humana-2026-09-05.json`, SHA-256
`ac7e7e15ef0fa20148459fb784d64693989f4c63b8dc69fd64b75c42e319c2a8`.
Quedan cero valoraciones pendientes en ese caso sintético; el informe requiere decisión humana y no
aprueba el producto. Esto acredita la entrega e importación de respuestas de la práctica, no una
auditoría visual completa ni la facilidad de uso de otros formatos. No se modificó código ni se
reejecutó el procesador: se verificó la importación real y el diff documental.

Decisión: entregar la página para apertura manual y preparar la primera comparación real cuando la
persona identifique el documento o elija revisar juntos el banco anterior. Reabrir la interfaz si la
persona no puede completar el recorrido, abrir un formato o recuperar/copiar sus respuestas.

### V1 — IMPLEMENTADO: evaluación mínima autorizada

La persona autorizó sustituir el ciclo centrado en E1 por contratos automáticos, un banco abierto
y muestras nuevas para decisiones importantes. Hipótesis: congelar selección y condiciones antes
de ejecutar, conservar todos los intentos y juzgar ambos resultados frente al original evita
confundir menos avisos o igualdad de salidas con calidad demostrada.

El banco existente y sus referencias se consideran **PENDIENTES DE REVALIDACIÓN**. Las conclusiones
anteriores de este documento se conservan como registro histórico, no como autoridad para aprobar
una nueva versión. No se borran documentos, controles ni decisiones; tampoco se incorporan al
catálogo nuevo sin revisión explícita. Lo complejo o dudoso queda para revisión humana.

E1 deja de ser la frontera activa. `scripts/evaluate_app.py` ofrece catálogo vacío, selección por
grupo y semilla, plan congelado, historial conservador de originales utilizados, ejecución real A/B,
resultados por intento, revisión privada a ciegas e informes agregados JSON/HTML. La página permite
guardar borradores y marcar dudas como no evaluables; cada importación crea un evento independiente
y el informe elige ese evento explícitamente. No se selecciona una versión ni se aplica una
corrección semántica automáticamente. El contrato detallado y sus límites viven en `evaluation.md`.

Se creó `local-benchmarks/evaluation-v1/catalog.json` vacío. El banco previo no fue leído ni importado
en este incremento. Las referencias nuevas comienzan sin validar; declararlas humanas exige
vincular original, expectativas y revisión, sin presentar esa declaración como identidad autenticada.

Calibración: 25 pruebas específicas superadas, incluidas igualdad, mejora y deterioro sintéticos,
omisiones compartidas por guardas, fuentes ya utilizadas, referencias pendientes, modificaciones,
respuestas incompletas, aislamiento local de IA e interrupción entre resultado y recibo de integridad.
Los contratos vecinos pasaron en la primera escalera (44 pruebas). Se corrigió además la resolución
de rutas relativas encontrada por el piloto CLI y quedó protegida por una regresión con el
procesador real.

El piloto final produjo seis conversiones completas: tres casos por brazo sobre dos originales
sintéticos, con Markdown y EPUB de salida y PDF nativo de entrada. Conservó estado **EVIDENCIA
INSUFICIENTE** al no existir valoración humana. Su informe agregado está en
`tmp/evaluation-final-pilot/report.json`; sus tiempos solo calibran el instrumento, no comparan dos
implementaciones ni acreditan rendimiento. La página se comprobó en navegador, incluida selección
«No evaluable», guardado de respuestas y viewport de 320 px sin desbordamiento horizontal. Los
juicios simulados de las pruebas no entraron en el catálogo privado ni cuentan como revisión humana.

Verificación final en `.venv` canónico: **2 278 pruebas superadas, cuatro omitidas; cobertura
89,06 %**, en 253,57 segundos. Las omisiones corresponden a permisos de symlink, dos comprobaciones
externas con EPUBCheck y el recorrido optativo con Ollama real. Ruff y formato correctos (267
archivos); mypy sin errores en 134 módulos, incluido el ejecutor; sincronización de versión,
`pip check` y `git diff --check` correctos.

Decisión: conservar el instrumento mínimo. No se ha acreditado calidad de OCR o traducción real,
ni se ha construido el instalador. La siguiente decisión es revisar con la persona las fuentes y
referencias que integrarán la primera muestra documental; no continuar E1 por inercia. Reabrir V1
si el instrumento omite intentos, mezcla versiones, acepta evidencia modificada o convierte dudas
en aprobación. Los motores y la revisión de producto conservan su comportamiento.

### M1 — IMPLEMENTADO: conservación y publicación tras la auditoría técnica

Incremento del 5 de septiembre de 2026 sobre `main` en `62d42d3` más el diff local de este cambio.
La hipótesis fue que conservar las particiones del texto, comprobar los archivos materializados y
usar el índice como único punto de publicación impediría aprobar Markdown incompleto; en paralelo,
una respuesta de Ollama sin finalización normal no debía alcanzar una guarda ni un checkpoint.
El diseño resultante y sus límites viven en [`architecture.md`](architecture.md), en publicación e
integridad y en el contrato de streaming local.

La evidencia automática incluye preámbulos antes omitidos, enlaces relativos con y sin imágenes,
metadatos y referencias de página, código y navegación que requieren mantener el archivo único,
índices o capítulos dañados, capítulos ausentes y procesos terminados deliberadamente a ambos lados
del reemplazo del índice. En Ollama se cubren chat y generación raw, EOF prematuro, límite de
generación, errores del servidor, evento terminal y ausencia de checkpoints de respuestas truncadas.
Se añadieron 39 casos de regresión y las respuestas completas simuladas anteriores declaran ahora
`done: true`.

Verificación ejecutada con `.codex-e1-eval-venv/Scripts/python.exe` (Python 3.12.10, pytest 9.1.1,
PySide6/Qt 6.11.2), `PYTHONDONTWRITEBYTECODE=1`, Qt offscreen y cobertura fuera del checkout:

- reproducción y contratos focales: correctos; contratos vecinos: 484 pruebas superadas;
- `pytest -p no:cacheprovider -q --cov=parsezen --cov-report=term:skip-covered --cov-fail-under=88`:
  **2 267 superadas, 4 omitidas; cobertura 88,70 %**, en 243,44 segundos;
- `ruff check . --no-cache`, `ruff format --check . --no-cache`, `sync_version.py --check` y
  `git diff --check`: correctos;
- `mypy src/parsezen`: persisten los mismos **30 errores en siete archivos** de la auditoría,
  ninguno en los tres módulos de producto modificados. La comprobación focal de esos módulos pasó.

Las omisiones corresponden a permisos de symlink, dos comprobaciones con EPUBCheck externo y el
recorrido optativo con Ollama real. El entorno disponible difiere del lock canónico; no se construyó
un instalador ni se ejecutó un nuevo corpus privado. El estado es **IMPLEMENTADO**, con contratos
automáticos comprobados, y no **VERIFICADO EN CORPUS**. El gate completo de publicación sigue abierto.

La decisión es conservar este incremento acotado. Las fases restantes de la auditoría continúan en
los incrementos M2 y siguientes, autorizados expresamente por la persona. Reabrir M1 si un caso real
demuestra pérdida de contenido, cambio de destino, índice incompleto tras una interrupción o rechazo
de una finalización
normal de Ollama. Ampliar la división de enlaces internos o limpiar generaciones huérfanas exige
primero demostrar la navegación del lector y la propiedad de los archivos afectados.

### M2 — IMPLEMENTADO: base reproducible de la auditoría

Incremento del 5 de septiembre de 2026. Hipótesis: las comprobaciones deben poder repetirse sobre
las versiones fijadas, y el inventario auditado debe coincidir con el lock. Se retiró el `uv.lock`
sin consumidores, se declaró Pillow como dependencia directa de producto y se documentó la vía
canónica en `development.md`. La instalación limpia con hashes descubrió que pip-tools 7.6.0 no
podía regenerar con pip 26.2.1; se actualizó únicamente pip-tools a 7.6.1. Ambos locks se regeneraron
correctamente, sin cambiar ninguna otra versión ni los motores del producto.

Evidencia: entorno aislado en `tmp/quality-venv`, Python 3.12.10 y Qt 6.11.1; instalación con hashes,
`pip check` y auditoría real de los 166 paquetes de cada lock correctos. Las únicas variantes son
torch y torchvision `+cpu`, con la misma versión base. La excepción Stanza existente no se amplió.
Las pruebas focales iniciales pasaron (415); la suite canónica inicial pasó con 2 281 pruebas,
cuatro omisiones y 88,68 % de cobertura. Los tipos quedan comprobados junto con M3 sobre 129 módulos.
Al cierre se preparó también `.venv` con el mismo lock y hashes: el directorio anterior no contenía
intérprete ni `pyvenv.cfg`. La comprobación final y la aceptación se ejecutaron desde esta vía
documentada, con instalación editable sin resolver dependencias de nuevo.

Decisión: conservar la vía única y el parche de la herramienta de desarrollo. Reabrir si regenerar
cambia paquetes ajenos al propósito o CI difiere del entorno fijado. No se ha construido el instalador.

### M3 — IMPLEMENTADO: retirada de mantenimiento desconectado

Hipótesis: eliminar código sin consumidores y consolidar reglas idénticas conserva los recorridos
activos. Se verificaron importaciones, exportaciones, configuración, empaquetado y pruebas antes de
retirar las dos pantallas antiguas y su modelo de divisiones EPUB, cinco helpers sin consumidores y
las operaciones genéricas de descarga/eliminación de modelos. La instalación activa sigue recibiendo
capacidades fijas. Se trasladó al generador EPUB la prueba útil de marcadores que estaba entre las
pruebas retiradas; revisión por fases y editor mantienen sus contratos de contenido, decisiones,
recuperación, estructura y accesibilidad.

Se unificaron lectura de portada y detección de daño. Los contratos de pipeline ya no importan motores
al cargarse y la transformación llama firmas explícitas; sus dobles de prueba aceptan los callbacks
y checkpoints reales. La documentación refleja las pantallas y el tema del sistema actuales, y el
paquete excluye los maestros gráficos conservándolos en el repositorio.

Verificación en el entorno canónico: 403 pruebas focales; suite de 2 234 superadas y cuatro omitidas,
cobertura 88,99 %, en 306,73 segundos; Ruff y mypy correctos (129 módulos). La reducción de pruebas
corresponde al código retirado. Decisión: conservar; reabrir si aparece un consumidor real omitido o
una capacidad usada que carezca de equivalente activo. No se retiró compatibilidad de datos guardados.

### M4 — IMPLEMENTADO: instalación y recuperación comprobables

Hipótesis: una espera silenciosa no debe impedir cancelar la descarga, y una referencia repetida no
debe volver a leer el mismo artefacto en una recuperación. El instalador conserva su entrada síncrona
en el trabajador, con una solicitud asíncrona cancelable y líneas de progreso acotadas. Un servidor
local de prueba confirma cancelación antes de cabeceras y con una línea incompleta. Se mantienen
las comprobaciones de identidad y los límites locales; no se ha instalado ningún modelo nuevo.

Los errores seguros de preparación de Ollama llegan a la persona con indicación para reintentar;
el cierre restablece el estado del flujo. La recuperación comparte cada texto solo durante una
carga y lo vuelve a leer en la siguiente. El auditor exige el inventario completo y devuelve un
diagnóstico limitado a paquete, versión e identificador cuando hay vulnerabilidades.

Evidencia del 5 de septiembre: 33 pruebas focales y 132 vecinas superadas; suite canónica de 2 248
superadas y cuatro omitidas, 89,03 % de cobertura, 275,17 segundos; Ruff y mypy correctos. Decisión:
conservar. Reabrir ante un trabajador que sobreviva a la cancelación o un texto reutilizado entre
generaciones. La cancelación inmediata cubre la descarga; preparar el alias conserva su espera
máxima anterior de 60 segundos. El recorrido con Ollama y documentos reales sigue siendo un gate
separado de publicación.

### M5 — IMPLEMENTADO: escritura proporcional al cambio de la cola

Hipótesis: cambiar un trabajo conserva toda la proyección con una sola escritura, sin renunciar a
transacción, orden o eventos. El almacén compara las filas actuales, retira los trabajos omitidos y
reserva posiciones temporales únicamente para movimientos reales. No mantiene una caché que ignore
escrituras de otra instancia. La comparación y serialización siguen siendo proporcionales a la cola.

Tres repeticiones sintéticas con 500 trabajos pasaron de 500 movimientos y 500 upserts por cambio
a cero movimientos y un upsert. Las medianas observadas fueron 47,898 y 20,289 ms; son mediciones
locales orientativas, no una promesa de latencia de interfaz. Las regresiones comprueban cero cambios
cuando la cola no cambia, una sola fila modificada, reutilización de posiciones borradas, otra
instancia y rollback conjunto de cambios, borrados y eventos. Se conservan las pruebas de orden y
revisiones anteriores.

Verificación: 51 pruebas focales; suite canónica de 2 251 superadas y cuatro omitidas, 89,03 % de
cobertura, 240,59 segundos; Ruff y mypy correctos. Decisión: conservar. Reabrir ante divergencia de
proyección, pérdida de revisión o fallo de atomicidad. Un almacenamiento asíncrono o una caché global
requieren una medición nueva que justifique su complejidad.

### M6 — IMPLEMENTADO: responsabilidades acotadas de PDF y traducción

Hipótesis: separar reglas cerradas permite mantenerlas sin mezclar decisiones de procesamiento ni
cambiar las salidas. Se trasladaron 2 416 líneas de definiciones a `pdf_tables.py`,
`pdf_text_reconciliation.py`, `table_translation.py` y `translation_review_patches.py`. La extracción
comparó los árboles sintácticos de cada definición antes y después, ignorando solo su posición.
Las fachadas conservan los nombres usados; los módulos extraídos no importan sus coordinadores ni
el transporte de IA. No se añadieron interfaces genéricas ni se cambiaron umbrales o prompts.

También se retiraron cinco proyecciones de contratos sin consumidores. Las vistas de origen y
publicación utilizadas por la validación de lotes se conservan; traducción, revisión e informes se
consultan directamente, sin construir objetos de migración. Los datos guardados mantienen su formato.

Verificación focal: 746 pruebas superadas. Tres PDF sintéticos conservan exactamente los hashes de
Markdown, recursos e informe de calidad. Una comparación adicional contra la implementación PDF de
`62d42d3`, usando los mismos colaboradores actuales, confirmó igualdad en seis páginas nativas de dos
documentos del manifest privado, uno de ellos holdout. Se verificaron los hashes de los originales
antes de leerlos. Otras siete páginas seleccionadas de dos documentos necesitaron OCR y quedaron
explícitamente sin ejercitar; no se sustituyó ese recorrido por una aprobación simulada. La muestra
real demuestra invariancia acotada, no calidad editorial ni validación del corpus completo.

Cierre automático del 5 de septiembre sobre `.venv` canónico (Python 3.12.10, Qt 6.11.1):

- suite completa: **2 253 pruebas superadas, cuatro omitidas; cobertura 89,08 %**, 281,35 segundos;
- aceptación: **27 pruebas superadas**, 40,14 segundos;
- Ruff: comprobación y formato correctos, 265 archivos;
- mypy: sin errores en 133 módulos; `pip check`, sincronización de versión y `git diff --check`
  correctos.
- auditoría real de ambos locks: inventario completo de 166 paquetes por lock, sin vulnerabilidades
  adicionales a la excepción Stanza ya documentada; variantes CPU vinculadas a su versión canónica.

Las omisiones siguen siendo permisos de symlink, dos validaciones externas con EPUBCheck y el
recorrido optativo con Ollama real. No se construyó un instalador ni se repitieron OCR y traducción
real sobre el corpus completo. Estos límites siguen siendo gates de publicación, no trabajo
arquitectónico pendiente de esta auditoría.

Decisión: conservar M2–M6 como mantenimiento implementado y comprobado automáticamente. Reabrir M6
ante una divergencia de salida atribuible al traslado o un consumidor omitido. Cualquier cambio de
heurísticas requiere un incremento independiente, sus canarios y la muestra de aceptación completa;
el siguiente trabajo de calidad conserva el orden E1 existente.

### S1 — CERRADO: regresión estructural sobre holdouts no usados

El 31 de agosto de 2026 se ejecutaron dos libros con índices y jerarquías distintas que no se usaron
para diseñar estas correcciones: Tafti 2 (221 páginas) y Ancient Astrology, volumen II (704 páginas).
La comparación descubrió dos incoherencias generales y no específicas de un título:

- el editor eliminaba antes de tiempo la evidencia privada de página/outline y podía volver a partir
  de forma distinta un documento ya planificado;
- el EPUB directo conservaba los archivos correctos, pero aplanaba en su navegación la relación
  superior `Parte → Capítulo` que el editor sí conocía.

Ambas rutas conservan ahora la evidencia únicamente hasta terminar la planificación y construyen la
misma jerarquía mediante niveles de índice demostrados o roles explícitos conservadores. La repetición
final produjo 57 capítulos en ambas rutas, 289 destinos con la misma profundidad, cero capítulos por
debajo de 1 000 bytes, cero saltos de encabezado, cero incidencias PDF bloqueantes y dos EPUB con
integridad verificada. Las secuencias de planificación fueron idénticas y no hubo capítulos añadidos,
eliminados o divididos al pasar por el editor. Los marcadores privados no aparecen en el XHTML.

El informe privado agregado queda en
`local-benchmarks/epub-structure-corpus-v1/structure-holdout-20260831-final-sanitized.json`. El gate
estructural queda cerrado para esta muestra, no como afirmación de perfección universal.

### E1 — HISTÓRICO, PENDIENTE DE REVALIDACIÓN: seis libros clave

El descarte representativo del 1 de septiembre ejecutó 97 páginas de los seis libros, con traducción
EN→ES en cuatro casos. Los seis EPUB superaron integridad de contenedor, conservación de recursos,
ausencia de marcadores privados y jerarquía sin saltos; tampoco hubo incidencias PDF bloqueantes ni
fallos de OCR obligatorio. Esta es evidencia técnica del recorrido acotado, no aprobación editorial
de libros completos.

Los 46 bloques traducidos produjeron nueve señales `SOURCE_TEXT`. La revisión humana privada quedó
completa y ligada por hashes al lote exacto: cinco propuestas se aceptaron y cuatro requieren una
corrección editorial. Las seis señales de *36 Faces* compartían prácticamente la misma forma
mecánica —residuo dentro de énfasis—, pero cuatro fueron aceptables y dos no. Por tanto, cursiva,
longitud o coincidencia léxica no ofrecen una regla general con precisión suficiente. El sistema sí
acotó los cuatro bloques problemáticos, pero el candidato previo a revisión no supera todavía el gate
editorial y no se convertirá ninguna respuesta humana en una sustitución específica por libro.

El informe reproducible sin texto queda en
`local-benchmarks/end-to-end-pilots/e1-representative-20260901-sanitized.json`. Las decisiones y los
fragmentos permanecen en el corpus privado; los casos aceptados son controles y las correcciones son
evidencia diagnóstica, no lógica ejecutable.

El primer centinela completo, *36 Faces*, procesó 317 páginas, 297 bloques traducidos, 46 imágenes y
42 capítulos con integridad final, cero incidencias PDF bloqueantes y cero fallos de OCR obligatorio.
La línea base produjo 55 avisos de texto fuente y uno de fidelidad. Dos correcciones humanas del lote
representativo compartían un patrón general inequívoco —una serie `planeta in signo + romano` copiada
en inglés— que no aparecía sin traducir en ninguno de los cinco controles aprobados. La normalización
local de ese patrón, incluidos los casos donde el romano había quedado fuera del énfasis, cambió solo
esos dos casos, mantuvo sus guardas y redujo en el libro completo los residuos de 55 a 42 sin alterar
capítulos ni integridad. El mecanismo queda **IMPLEMENTADO** y verificado en esta muestra privada; no
es una tabla de sustituciones por título ni una corrección general de naturalidad.

El centinela aún conservaba 43 avisos y exigía revisión amplia. Además, el límite de veinte extractos
dejaba inicialmente oculta la única incidencia de fidelidad tras avisos de menor prioridad. El informe
mantiene ahora el mismo límite privado, pero prioriza idioma, alineación y fidelidad antes de residuos
o longitud; el total y los segmentos alineados siguen siendo exhaustivos.

La muestra privada posterior de diez casos quedó **COMPLETA** y ligada a su paquete exacto: cinco
aprobados y cinco con corrección. Las notas separaron un título completamente sin traducir, dos
rótulos/elecciones terminológicas, un romano mal extraído y una falsa alarma sobre un índice ya
español. No se convirtieron en respuestas por libro. Se implementaron cinco mecanismos generales:
consenso I/II/III para glifos astrológicos dañados; localización de rótulos de colocación; memoria
`exaltation`→`exaltación` activable también por decanos/zodiaco; análisis de índices XHTML por celda; y
respaldo Argos, ya instalado y sin descarga, exclusivamente para un título que el reintento de IA deja
intacto.

La cadena de decisiones, fuentes y propuestas pasó sus hashes 10/10. En la muestra, los nuevos patrones
de rótulo coincidieron con tres correcciones y cero aprobaciones; la falsa alarma del índice pasó a cero
incidencias y una prueba real Hy-MT→Argos resolvió el título residual con cero avisos. La extracción
nativa completa de *36 Faces* encontró 74 reparaciones sobre 42 páginas, todas confinadas al patrón
astrológico demostrado; se inspeccionaron visualmente los dos fallos que originaron la regla, no las 74
líneas.

El 2 de septiembre se repitió el centinela completo con esos mecanismos. Las 317 páginas volvieron a
producir 297 bloques traducidos, 46 imágenes y 42 capítulos; el EPUB terminó con integridad, cero
incidencias PDF bloqueantes y cero fallos de OCR obligatorio. Los cinco identificadores que la persona
había marcado para corrección dejaron de aparecer y la incidencia de fidelidad bajó de una a cero, pero
la ausencia del identificador no demostró por sí sola que el defecto hubiese desaparecido: la reparación
de extracción cambió el hash de `SCORPIO IE` a `SCORPIO II`, mientras el mismo encabezado seguía en
inglés bajo dos identificadores nuevos. Los avisos totales bajaron de 43 a 41 y todos los restantes eran
`SOURCE_TEXT`. Por tanto, el candidato quedó **VERIFICADO EN ESTE CENTINELA** como mejora segura, pero
no como traducción editorialmente cerrada.

La primera auditoría residual aisló cinco bloques con rótulos astrológicos donde el modelo había
traducido el signo pero no el planeta ni `in`. La normalización admite ahora esas mezclas parciales.
Coincidió con los cinco bloques y con cero controles aceptados; en el EPUB completo redujo de 26 a cero
las colocaciones parciales visibles y los avisos bajaron de 41 a 25, sin cambiar 317 páginas, 297
bloques, 42 capítulos, 46 imágenes, integridad, OCR obligatorio ni incidencias PDF bloqueantes. El
mecanismo queda **VERIFICADO EN ESTE CENTINELA**.

La auditoría posterior de las veinte formas privadas expuestas separó trece errores probables —doce
defectos distintos porque el encabezado aparece duplicado— y siete falsos positivos o contenidos que
deben conservarse. El encabezado inglés y los residuos de tablas no admiten la misma regla mecánica.
Una ampliación del respaldo de títulos no produjo ninguna mejora medible en el EPUB y se descartó; no
se reescriben celdas ya traducidas sin evidencia semántica. El siguiente gate es una muestra humana
mínima de diez casos que cubra el encabezado, celdas realmente residuales, terminología de tablas y dos
controles bibliográficos/editoriales. Solo si distingue otra causa general separable se abre un
incremento automático; en caso contrario se calibra el informe y se continúa con los otros cinco
libros sin convertir respuestas en lógica por título.

La muestra quedó materializada como la tanda privada 2 del centinela: siete propuestas corregidas y
tres controles sin cambio, todos ligados por hashes a su fuente y candidato. Cuatro formas inicialmente
consideradas se excluyeron porque el recorte privado no permitía validar de manera completa estructura
o idioma; no se pide una decisión que después no pueda aplicarse con seguridad. La primera revisión
humana terminó con cinco aprobaciones y cinco correcciones solicitadas. Las decisiones, fuentes,
candidatos y propuestas pasaron sus hashes 10/10. La ronda correctiva de cinco casos incorpora las
indicaciones sobre mayúsculas editoriales, saltos visuales, puntuación y una celda incompleta. La
segunda revisión terminó con 5/5 aprobaciones y su linaje y hashes volvieron a coincidir. El lote queda
**COMPLETO** con diez referencias aprobadas: siete cumplen las puertas de publicación automáticas y
tres permanecen solo como diagnóstico —dos eliminan saltos visuales de tabla que la guarda estructural
anterior trataba como semánticos y una es una bibliografía breve cuyo idioma no puede decidirse de
forma fiable—. Ninguna respuesta se convirtió en una sustitución por libro.

Las dos observaciones de tabla sí revelaron una causa general de extracción. Una celda PDF une ahora
solo continuaciones visuales inequívocas —inicio en minúscula, puntuación abierta o palabra funcional
de continuación— y conserva frases cerradas, listas y rótulos separados. En el intervalo real de 24
páginas que originó la hipótesis, las tres continuaciones objetivo quedaron unidas y permanecieron 50
saltos internos estructurales. `decan` se añadió además a la memoria astrológica acotada: en una
regeneración completa, los residuos exactos `DECAN` bajaron de 94 a uno y `DECANO` aumentó de 526 a
835. Ambos mecanismos quedan **IMPLEMENTADOS** y verificados en ese alcance, no como aprobación de la
traducción completa.

La misma regeneración completa produjo 45 avisos `SOURCE_TEXT`, frente a 25 del mejor candidato
anterior, concentrados en celdas largas de las páginas 273–294. El EPUB conservó 317 páginas, 297
bloques, 42 capítulos, 46 imágenes, integridad final y cero fallos de OCR obligatorio, pero la pasada
fresca reintrodujo prosa inglesa: no se acepta como nueva base. Un fallback transaccional que dividía
la celda por oraciones redujo una muestra de 22 bloques a cuatro avisos y otro intervalo de 29 bloques
a tres, pero la repetición completa terminó de nuevo con 45. El mecanismo se retiró y queda
**RECHAZADO**: una muestra acotada sirvió para proponerlo, pero el libro completo decidió en contra.

También se rechazó ampliar Argos a rótulos o tablas. El par EN→ES instalado dejó sin traducir varios
rótulos breves y llegó a corromper uno; una frase de prosa aislada correcta no compensa esa precisión
insuficiente. Argos permanece como motor completo elegido por la persona y como respaldo ya instalado
para un único título residual bajo guardas. No se repetirá esta línea sin un mecanismo nuevo.

**Criterio de parada para este centinela:** se conserva el candidato `es-13` como mejor evidencia
editorial conocida, se conservan `es-14`/`es-15` como controles negativos privados y no se generan más
variantes de *36 Faces*. El siguiente incremento es ejecutar otro libro completo de los cinco
restantes y comprobar si los mecanismos generales transfieren; solo un patrón repetido en más de un
libro reabrirá traducción tabular.

Cuando traducción y estructura hayan cerrado sus gates independientes:

1. congelar versiones, opciones y hashes de entrada;
2. ejecutar primero intervalos representativos y después los libros completos;
3. comparar extracción, traducción, estructura y EPUB por separado;
4. revisar únicamente las excepciones que queden;
5. confirmar lectura real en más de un lector EPUB;
6. registrar tiempos, memoria, reintentos y atención humana agregada;
7. conservar los libros como holdouts finales, no como fuente de nuevas reglas específicas.
