# Evaluación mínima de Liblevo

La evaluación tiene tres piezas: contratos automáticos, banco abierto para depurar y muestra nueva
para una decisión importante. El ejecutor no decide fidelidad semántica ni modifica los criterios de
revisión del producto. Lo complejo, ambiguo o sin contexto suficiente queda para una persona.

El banco anterior está **pendiente de revalidación**. No se importa automáticamente y sus referencias
no son verdad por haber pasado pruebas, guardas o revisiones históricas. Se conservan para revisarlas
con la persona. Una salida histórica idéntica puede contener el mismo error.

## Contrato y responsabilidades

- Las pruebas normales de producto cubren original intacto, publicación, cancelación, recuperación,
  privacidad y estados. La lista de aceptación sigue siendo el contrato de distribución.
- `scripts/evaluate_app.py` ejecuta el procesador real sobre un plan congelado, separa A y B y conserva
  fallos e intentos incompletos. No crea otro checkout ni cambia de rama automáticamente.
- `scripts/evaluation_review.html` produce una revisión privada a ciegas, sin servidor ni conexiones.
  Cada resultado se contrasta con el original. No preselecciona una aprobación.
- El informe compara garantías, calidad observada y coste; no publica una puntuación global ni una
  estimación de precisión poblacional. El juicio y la decisión de aceptar el cambio son humanos.

## Para la persona que revisa: sin comandos

1. Indica al agente un documento que conozcas bien. El agente prepara la comparación y abre o entrega
   la página local. No necesitas editar catálogos ni ejecutar los comandos de esta guía.
2. Compara el original de la izquierda con el resultado de la derecha. El PDF se muestra como páginas
   y el resultado como texto con títulos, énfasis y tablas. Si hay dos versiones distintas, cambia de
   pestaña para revisar ambas; si son idénticas, basta una valoración.
3. Marca **Lo veo bien**, **Detalle menor**, **Problema importante** o **Necesito ayuda** en cada
   resultado. «Lo veo bien» significa solo que no has observado errores en esa unidad. Escribe lo
   que cambiarías en el comentario situado junto a la valoración; no necesitas explicarlo aparte.
4. Pulsa **Confirmar y copiar revisión** y pégala en la conversación. Ese botón confirma que has
   comparado los resultados evaluables con el original. El agente registra etiquetas y comentarios
   después de comprobar su vinculación. Copiar no los registra ni los envía automáticamente.

Puedes abrir **Guardar o recuperar una revisión** y guardar un borrador con tus comentarios; también
puedes adjuntar únicamente el archivo de respuestas en la conversación. No compartas la carpeta de
revisión: contiene los documentos. Si el navegador no permite copiar, se descarga un archivo para
adjuntarlo en la conversación, sin mostrar código. Las respuestas incluyen identificadores,
valoraciones y tus comentarios; no incluyen automáticamente el texto documental. Evita escribir
información sensible en los comentarios.

El PDF muestra hasta seis páginas del intervalo; una vista parcial lo indica. Los EPUB y DOCX siguen
abriéndose en un lector del dispositivo. Si no puedes abrirlos o no tienes contexto,
marca **Necesito ayuda**. Una práctica marcada como sintética sirve para aprender el recorrido;
no valida la calidad real de Liblevo. El agente prepara después una muestra con documentos reales,
sin aceptar automáticamente el banco anterior.

## Empezar con un catálogo vacío

Desde el entorno canónico:

```powershell
.venv/Scripts/python.exe scripts/evaluate_app.py init local-benchmarks/evaluation-v1
```

El agente prepara el catálogo; la persona revisa el original, las expectativas y los resultados.
`catalog.json` es privado y editable hasta congelarlo. Campos:

```json
{
  "version": 1,
  "question": "Qué propiedad queremos mejorar",
  "acceptance": "Qué mejora es útil y qué deterioro sería inaceptable",
  "cases": [
    {
      "id": "a30b6686-5b77-4d01-a7b2-cd283401cc24",
      "source": "original-local.pdf",
      "source_sha256": "REEMPLAZAR_POR_SHA256_REAL",
      "cohort": "representative",
      "provenance": "unverified",
      "options": {
        "pages": [4, 5],
        "output_format": "epub",
        "translate_to": null,
        "force_ocr": false,
        "include_images": true
      },
      "reference": {"status": "unverified", "contains": []}
    }
  ]
}
```

Los grupos son `representative`, `risk` y `known`; se muestrean por separado. El identificador UUID
es opaco; la huella se calcula sobre el archivo original completo. Se admiten TXT, Markdown, DOCX,
PDF y EPUB locales de hasta 512 MiB; los intervalos se aplican solo a PDF. Para estudiar una unidad
más pequeña de otro formato se prepara un documento independiente y se declara ese alcance.

`provenance` empieza en `unverified`. `new` es una declaración humana de que el documento no se usó
para desarrollar el candidato; `known` indica material ya usado y `synthetic` un caso construido.
El historial registra hashes de originales utilizados o reservados. No puede descubrir por sí solo
otras ediciones, familias similares ni usos realizados fuera de este catálogo: esa comprobación es
humana. Renombrar un caso o elegir otras páginas del mismo archivo no lo convierte en nuevo.

Las expectativas `contains` son comprobaciones literales pequeñas, aplicables al Markdown publicado
de hasta 16 MiB. Su ausencia de fallos no acredita orden, relaciones de tabla, significado ni formato.
El informe distingue comprobaciones solicitadas de ejecutadas; EPUB y vistas no admitidas no cuentan
como comprobados. Por defecto son observaciones de una referencia **no validada**.

Solo después de revisar original y expectativas se puede declarar `human_verified`, acompañándolo
de `source_sha256`, `expectations_sha256` (calculado con `evaluate_app.digest(contains)`) y
`reviewed_at`. Cambiar original o expectativas invalida esa vinculación. Son declaraciones humanas,
no una firma de identidad ni una prueba de que una persona haya leído el documento. `synthetic`
identifica referencias construidas para comprobar el instrumento, no evidencia de calidad real.

## Congelar antes de ver resultados

```powershell
.venv/Scripts/python.exe scripts/evaluate_app.py freeze local-benchmarks/evaluation-v1/catalog.json local-benchmarks/evaluation-v1/comparison-01 --representative 4 --risk 2 --seed 17 --fresh --repetitions 1
```

Las cantidades son un ejemplo operativo, no un tamaño suficiente para demostrar calidad. El muestreo
es uniforme entre casos de cada grupo y se fija con la semilla. Si un libro aporta más casos tendrá
más oportunidades: equilibrar el catálogo por documentos antes de congelar. No llamar representativa
a una selección cuya distribución de uso no conocemos; informar por grupo y número de originales.

`--fresh` exige procedencia `new` y ausencia de todos los originales en `.evaluation-exposure`, junto
al catálogo. Su alcance es ese historial local; copiar el catálogo y perder su historial destruye
la evidencia de independencia. La reserva es conservadora: un plan abandonado también consume la
condición de nuevo. Los casos pueden reutilizarse después como banco abierto, sin `--fresh`.

El plan fija pregunta, aceptación, casos, opciones y de una a tres repeticiones. Sus huellas detectan
modificaciones accidentales. No son una protección criptográfica contra un operador que reescriba
archivos y huellas deliberadamente. No ampliar la muestra después de mirar resultados para conseguir
una conclusión favorable; registrar otra hipótesis y una evaluación nueva si hace falta.

Si se interrumpe una congelación, puede quedar el directorio vacío `.evaluation-exposure/lock`.
Antes de retirarlo manualmente, comprobar que no hay otra congelación activa. No borrar eventos del
historial para recuperar la etiqueta de muestra nueva.

## Ejecutar A y B en el checkout existente

```powershell
.venv/Scripts/python.exe scripts/evaluate_app.py run local-benchmarks/evaluation-v1/comparison-01 --arm a
# Aplicar el cambio acotado y sus pruebas en este mismo checkout.
.venv/Scripts/python.exe scripts/evaluate_app.py run local-benchmarks/evaluation-v1/comparison-01 --arm b
```

No se sobrescribe un brazo existente. Cada caso y repetición tiene salida y checkpoints aislados:
no se reutilizan respuestas entre A y B. El código, dependencias instaladas y modelos se identifican
en cada brazo; cambiar código o identidad de modelo durante la ejecución impide considerarla estable.
Las variantes de entorno deben corresponder al propósito declarado de la comparación.

La traducción inicial admite destinos `es` y `en`, exclusivamente con el componente fijo local de
Liblevo, cuya protección local, manifest y digest se comprueban antes de enviar contenido. No hay
descargas, proveedores alternativos, juez automático ni revisión semántica adicional. La revisión
compleja sigue haciéndose en el flujo humano. Probar otros idiomas o perfiles necesita un incremento
explícito; no se infiere soporte universal de este instrumento inicial.

Un proceso interrumpido conserva los casos terminados y los restantes se cuentan como incompletos.
Una ejecución fallida nunca desaparece del denominador. Los originales se verifican antes y después.
La herramienta no acredita por sí sola toda la interfaz ni el instalador.

Tiempo: duración observada por intento, con mediana y máximo. Memoria: RSS muestreado del ejecutor y
sus hijos; **excluye Ollama ya iniciado y GPU**, puede perder picos menores de 100 ms y no es memoria
total del sistema. Alternar orden A/B o repetir cuando la caché del sistema, la carga del equipo o la
variabilidad del modelo puedan explicar la diferencia. No presentar una diferencia pequeña como
mejora hasta separarla de esa variación.

## Revisar y conservar decisiones

```powershell
.venv/Scripts/python.exe scripts/evaluate_app.py review local-benchmarks/evaluation-v1/comparison-01
```

Abrir `reviews/<id>/view/index.html` localmente. El paquete incluye copias privadas del original y
los resultados; no debe compartirse ni servirse fuera del equipo. Las vistas pasivas están acotadas
y no reproducen el diseño exacto de todos los lectores. Los archivos completos siguen disponibles.
Las valoraciones de contenido requieren confirmar la inspección al entregar la revisión; si falta
contexto, elegir **Necesito ayuda** (etiqueta interna `not_evaluable`).

Los casos aparecen sin revelar qué brazo es A o B. La comparación humana puede detectar errores
compartidos porque siempre muestra el original y no filtra por avisos o diferencias. La máscara es
de presentación; quien inspeccione los archivos internos puede conocer la correspondencia.

Se puede guardar un borrador y recuperarlo con sus comentarios, sin almacenamiento persistente de
documentos en el navegador. El formulario no solicita tiempo; entrega `review_seconds: null`.
El importador conserva soporte de tiempos declarados en respuestas anteriores. Los comentarios
son opcionales, máximo 2000 caracteres sin NUL; quedan fuera de los informes agregados.
Guardar descarga un JSON privado; no publica ni aplica correcciones a documentos.
Copiar usa el portapapeles por una acción explícita de la persona, sin conexiones; pegar ese bloque
en la conversación permite al agente crear el archivo de respuestas e importarlo por el mismo contrato.

```powershell
.venv/Scripts/python.exe scripts/evaluate_app.py import-review local-benchmarks/evaluation-v1/comparison-01 respuestas.json
.venv/Scripts/python.exe scripts/evaluate_app.py report local-benchmarks/evaluation-v1/comparison-01 informe-01.json --judgment local-benchmarks/evaluation-v1/comparison-01/judgments/ID.json
```

La importación exige paneles completos, identidad del paquete y resultados sin modificar. Acepta
«no evaluable» sin forzar una valoración. Cada corrección produce un evento nuevo; el informe usa
solo el evento seleccionado explícitamente, nunca el último por accidente. Los originales, salidas,
respuestas y anotaciones siguen siendo privados; `informe-01.json` y su HTML compañero contienen
solo agregados e identidades técnicas.

Las salidas idénticas se agrupan solo si coinciden archivo y recursos completos, no solo su vista
previa. El formulario expande un juicio a sus paneles equivalentes y el mapping registra el grupo.
El informe distingue intentos de `human_review_units`; una valoración compartida no es evidencia
humana independiente por cada ejecución. Los paquetes anteriores conservan su formato y validez.

No hay aceptación semántica automática. El estado será fallo técnico/incompleto, evidencia
insuficiente o pendiente de decisión humana. Revisar ambos brazos frente al original precede a
decidir si B es preferible. Registrar en el plan la decisión humana, la huella del informe y su
alcance. La incertidumbre no obliga a generar más variantes ni autoriza aceptar contenido dudoso.

## Calibrar antes de medir producto

Las pruebas del evaluador usan originales sintéticos nuevos: igualdad, omisión deliberada,
referencia no validada, salida alterada, intento incompleto y respuestas humanas inválidas. Estas
pruebas comprueban el instrumento. No validan traducción real ni convierten etiquetas simuladas en
revisiones humanas. La primera auditoría documental requiere seleccionar fuentes y referencias con
la persona; el corpus previo no proporciona esa aprobación automáticamente.
