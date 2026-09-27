# IA local en Parsezen

Los trabajos nuevos usan modelos integrados en Parsezen. No necesitas instalar, abrir ni mantener
Ollama. La app no ofrece proveedores, cuentas ni direcciones configurables. Los documentos se
procesan en el PC; al preparar un componente puede descargarse su archivo público del modelo.

Conversión, EPUB, OCR, traducción y revisión de trabajos nuevos pueden usarse sin Ollama.
La traducción nueva usa IA local por defecto; Argos permanece como elección manual y nunca actúa como
alternativa silenciosa. Parsezen fija Hy-MT2 Q4_K_M para traducir y LFM Q6_K para generar propuestas
de revisión protegidas por guardas y confirmación humana; no pide a la persona elegir un modelo
conversacional.

`IA local` se abre desde Ajustes o desde el aviso contextual de una acción que la necesita. Esa vista
fija muestra exactamente las filas `Traducción IA` y `Revisión IA`, con los estados `Preparado`, `Descargable` o `Equipo
insuficiente`. No ofrece un selector de tags, endpoints, búsqueda, recomendaciones automáticas ni
borrado arbitrario. Los documentos conservan una instantánea del perfil efectivo y los cambios solo
alcanzan trabajos pendientes todavía editables.

## Recorrido guiado

La interfaz muestra una única acción pertinente:

1. Abre **IA local** desde Ajustes.
2. Si **Traducción IA** indica `Descargable`, pulsa **Preparar componente**. Si ya indica
   `Preparado`, puedes traducir. El archivo ocupa unos 4,6 GB.
3. Prepara **Revisión IA** solo si quieres esa revisión adicional; ocupa unos 2,2 GB y muestra
   sus condiciones antes de descargarlo.
4. Elige el PDF, el idioma español y salida EPUB. Revisa las páginas, el índice y los fragmentos
   señalados antes de usar el libro.

El usuario no necesita abrir una consola, una aplicación de chat ni un navegador.

## Componentes fijados

Cada tarjeta representa una capacidad concreta, no un modelo conversacional intercambiable. La
preparación se decide con el catálogo versionado de Parsezen y comprobaciones locales de hardware,
modelo integrado y el SHA-256 del archivo GGUF local. No consulta Ollama ni envía documentos,
prompts ni respuestas.

Un componente directo solo aparece como `Preparado` cuando el motor existe y tamaño y SHA-256 del
archivo coinciden con el perfil fijado. Si falta, la tarjeta puede mostrar `Descargable`; la vista
emite únicamente la capacidad (`translation` o `review`) para que una capa posterior autorizada
gestione la preparación. Nunca acepta un nombre de modelo, URL o endpoint introducido por el usuario.

`Preparado` significa que la instalación y el contrato local son correctos. No significa que toda
traducción sea perfecta ni que una propuesta de revisión pueda aprobarse automáticamente. Hy-MT2 es
el baseline EN→ES verificado; LFM actúa tras una puerta humana y no certifica semánticamente un libro.

Los tags `:cloud` y `-cloud`, las variantes no fijadas y los modelos que no cumplen los requisitos
quedan fuera. La traducción offline con Argos permanece disponible como elección explícita y no es un
reemplazo silencioso de un componente de IA.

## Solo local

El motor directo abre únicamente el modelo GGUF que Parsezen ha verificado en el equipo. No contacta
un servidor de modelos. La preparación puede reutilizar una copia local antigua del mismo archivo o
descargar el modelo público fijado; los documentos nunca forman parte de esa solicitud.
Los trabajos antiguos que guardaron expresamente una identidad de Ollama conservan esa dependencia.
Si no quieres usarla, vuelve a configurar esos trabajos antes de reanudarlos.

## Ventana de contexto

Cada manifest fija 8.192 tokens para su fase, dentro del máximo anunciado por el artefacto. Parsezen
fragmenta los documentos largos y usa la ventana fijada por el perfil; la interfaz no ofrece un
control para elevarla ni permite que una preferencia antigua sustituya el contrato especializado.

## Preparación de componentes

La interfaz no instala ni selecciona modelos arbitrarios. El instalador recibe únicamente la
capacidad del catálogo, importa o descarga el archivo público fijado y comprueba tamaño y SHA-256.
Una descarga incompleta o incorrecta no se publica como modelo preparado. LFM muestra sus condiciones
de licencia y exige confirmación explícita antes de descargar.

## Privacidad de las peticiones

- El motor directo no abre conexiones para procesar documentos.
- Las respuestas llegan en fragmentos para poder cancelar entre ellos.
- No se registra el prompt ni la respuesta.
- El glosario se protege durante la petición y se cifra mientras una revisión sea recuperable.
- Los checkpoints de fragmentos validados se cifran para la cuenta de Windows.

## Diagnóstico sencillo

Si `Traducción IA` no aparece como `Preparado`, abre **IA local** y pulsa **Comprobar de nuevo**.
Si indica que falta el motor integrado, usa el paquete local que lo incluye. Si el archivo no supera
la verificación, consérvalo y consulta el informe técnico antes de intentar repararlo; no se usa
para traducir.

## Prueba técnica opcional y evaluación histórica

`Validar con IA real.cmd` sin argumentos usa el traductor integrado y un PDF sintético; no necesita
Ollama. El recorrido inicial comprueba traducción y EPUB. Puede tardar varios minutos y un resultado
`OK` no demuestra que la prosa de un libro sea agradable de leer. Elige los intervalos y documentos
de prueba de manera explícita si deseas ampliar el ensayo.

La política de modelos de IA local fija los manifests, adaptadores, alcances y gates de los
componentes. La pantalla no muestra candidatos ni permite cambiar tags fuera de esa política.

La herramienta genera por defecto un PDF sintético de 20 páginas. Los casos que traducen usan IA
local por defecto; Argos solo se prueba al añadir explícitamente `--translation-engine argos` y
nunca se usa para recuperarse de un fallo del modelo. El informe local
solo contiene fases, tiempos, tamaños, contadores de calidad y revisión, y tipos de error. La
aprobación automática aplica únicamente los cambios que la app clasifica como conservadores. El
informe no se incorpora al repositorio ni contiene texto documental. `OK` exige que no queden
incidencias PDF bloqueantes, fragmentos de traducción conservados ni regresiones estructurales EPUB.
Los avisos PDF y de traducción son señales no bloqueantes por diseño: permanecen cuantificados para
la revisión, pero no se convierten artificialmente en rechazos. En EPUB de entrada, la estructura se
compara con la salida para distinguir defectos heredados de degradaciones nuevas. Una ejecución que
termina y genera un archivo válido pero no supera ese control se presenta como `REVISAR`. Para un
corpus especializado puedes repetir
`--glossary "origen=destino"`; esos términos se usan en la transformación y se omiten del informe.

Los comandos y resultados siguientes pertenecen a la evaluación histórica con Ollama. Se conservan
como referencia, no como validación del motor directo ni como pasos para el uso normal. La misma
herramienta permite pedir expresamente el modelo antiguo para esa comparación:

```powershell
Validar con IA real.cmd --model parsezen/hymt-translation:Q4_K_M --translation-engine local_ai --profile translation --pages 1 4
```

`--profile critical` ejecuta traducción, corrección y estructura; `--profile translation` aísla la
traducción para comparar modelos especializados. `--profile review` compara conversión directa y
revisión semántica sin traducir; `--profile translation-review` compara traducción directa y
revisada conservando la misma traducción base y añadiendo el revisor solo al segundo brazo. Los
informes conservan únicamente recuentos, fases y tiempos, nunca
texto documental. Como referencia histórica —no como opciones actuales—, en la estación objetivo de
8 GB de VRAM la muestra sintética de cuatro páginas del 12 de agosto de 2026 dio estos resultados:

| Modelo | Perfil completo tras carga | Solo traducción en caliente |
|---|---:|---:|
| Qwen 3 4B Instruct Q8 | 20,2 s | 12,5 s |
| Qwen 3.5 9B | 42,3 s | 24,3 s |
| TranslateGemma 4B Q8 | no aplicable | 15,1 s en caliente; 30,1 s con carga |

La conclusión de producto es deliberadamente conservadora: las comparaciones sirven para revisar
manifests y adaptadores locales, no para añadir recomendaciones generales ni otro selector. Un
componente solo queda preparado después de aparecer y verificarse en `/api/tags`.

Una prueba representativa no garantiza una traducción perfecta. Mantén un corpus local privado de
documentos y revisa cualquier actualización de Ollama o de modelo antes de usarla en trabajos
importantes.

El estado vigente y la siguiente frontera se resumen en [`work-plan.md`](work-plan.md). El piloto
residual más reciente aprobó humanamente solo una de doce propuestas protegidas; por ello la
reparación semántica automática permanece experimental y ningún caso dudoso se aplica sin revisión.

### Evaluación de la revisión semántica

El 12 de agosto de 2026 se hicieron cinco comparaciones de recorridos directos y revisados sobre
tres muestras privadas de diez páginas, elegidas en documentos largos por diversidad de texto,
imágenes y tablas. Se
usó el mismo Qwen 3 4B Instruct local y una ejecución por caso. Los informes no conservaron títulos,
rutas ni fragmentos:

| Recorrido | Directo | Revisado | Incidencias de traducción | Propuestas seguras / rechazadas |
|---|---:|---:|---:|---:|
| Conversión, muestra A | 21,9 s | 180,0 s | no aplicable | 2 / 2 |
| Conversión, muestra B | 2,7 s | 138,2 s | no aplicable | 9 / 1 |
| Argos, muestra A | 34,0 s | 171,8 s | 1 → 0 | 10 / 1 |
| Argos, muestra B | 29,6 s | 152,7 s | 2 → 2 | 13 / 0 |
| Traducción con IA, muestra A | 110,9 s | 122,3 s | 0 → 0 | 10 / 0 |

La revisión encontró propuestas conservadoras, pero no demostró una mejora semántica universal. En
conversión multiplicó mucho el tiempo; tras Argos costó alrededor de cinco veces y solo una de las
dos muestras redujo el residuo medible. Con traducción por IA, la corrección se fusionó con la
traducción y el incremento observado fue menor, pero una sola muestra no permite generalizar ni
separar la calidad debida a cada operación. El recuento de propuestas tampoco equivale a calidad.

Por ello, Procesamiento directo sigue siendo el valor recomendado. La revisión semántica permanece
disponible como capa explícita para documentos valiosos, conversiones difíciles o salidas de Argos
que justifiquen el coste. Los informes nuevos separan además propuestas de contenido y de estructura
para que futuras comparaciones no mezclen ambos efectos.

El flujo normal aplica ahora esa conclusión de forma progresiva: las comprobaciones deterministas
pueden recomendar una revisión posterior limitada a los bloques con señales, pero no inician Ollama.
La persona decide si ejecutarla y confirma cualquier cambio. La revisión adicional proactiva sigue
disponible desde la configuración cuando el valor o la
dificultad del documento justifican ampliar la cobertura; Parsezen informa los bloques realmente
revisados y no la presenta como verificación completa por el mero hecho de solicitarla.
