![Parsezen](assets/branding/generated/parsezen-readme.png)

## ***Convierte documentos complejos en contenido útil.***

El nombre *Parsezen* proviene de *parse* (extraer y estructurar información) con *zen* (hacerlo de forma sencilla y fluida).

Parsezen transforma PDFs, documentos de Word y otros archivos con texto, imágenes y tablas en
Markdown limpio o en un EPUB organizado. El procesamiento directo comprueba el resultado y, solo si
encuentra señales concretas, puede proponerte una revisión local de los bloques afectados. También
puedes solicitar de antemano una revisión adicional. La traducción opcional usa por defecto un modelo
local integrado, sin necesitar Ollama, y mantiene Argos como alternativa manual. El contenido del
documento se procesa en el equipo. Los archivos públicos de los modelos pueden descargarse durante
la preparación. Es open source, privado y gratuito.


## Principales características

- **Convierte a Markdown o EPUB.** Extrae texto, imágenes y tablas de PDF, Word y otros documentos.
  Aplica OCR local a páginas escaneadas y selecciona las páginas que necesitas.
- **Traduce en tu equipo.** Usa el modelo local integrado o elige la alternativa sin LLM de
  Argos. Los glosarios ayudan a conservar tus términos y las dudas se muestran para revisión.
- **Prepara libros.** Confirma título, autor, idioma, portada y capítulos; abre el editor cuando
  quieras ajustar estructura o contenido. Los originales se conservan.
- **Revisa con control.** Compara propuestas, acepta o conserva el texto anterior y distingue lo
  comprobado de lo pendiente. Una recomendación nunca ejecuta IA por sí sola.
- **Continúa después.** Trabaja con varios archivos, pausa, reanuda y reintenta la fase que falle.
  Una única ventana por perfil protege la cola frente a aperturas simultáneas.
- **Local y gratuito.** El contenido permanece en tu equipo. No requiere suscripciones ni pagos
  por uso; los componentes y paquetes de idioma se preparan por separado.

Las reglas de conservación, revisión y formatos se explican en la [guía de uso](docs/user-guide.md).
Los límites de lo demostrado y la evaluación pendiente están en el [plan actual](docs/work-plan.md).

## Cómo usar Parsezen

1. **[Descarga la última versión](https://github.com/jaimeacena/parsezen/releases/latest).** Necesitas Windows de 64 bits, pero no tienes que instalar Python.

2. **Añade tu documento.** Puedes trabajar con PDF, Word, EPUB, Markdown y archivos de texto.
3. **Configura el resultado.** Dentro de Parsezen, dos tarjetas claras permiten elegir Markdown o
   EPUB; traducción, páginas y OCR usan filas breves `Etiqueta — Valor — ›`, y la revisión adicional con IA un
   único interruptor. Al elegir un idioma aparecen traductor y glosario; un intervalo se resume como
   `25–140`. Cada elección válida se guarda al instante, sin texto técnico permanente, pie de acciones
   ni scroll en el tamaño normal.
4. **Procesa y revisa.** Parsezen extrae y organiza el contenido, conserva las imágenes y tablas
   compatibles y mantiene también la lámina original cuando una página completa contiene una tabla
   OCR, una imagen girada o una figura numerada que no puede separarse con seguridad del escaneo.
   Las portadas sin capa textual útil, las contraportadas y los mosaicos de rótulos permanecen como
   láminas: una lectura OCR parcial o espacialmente falsa nunca los sustituye. Las ilustraciones
   discretas se insertan en su posición de lectura y permanecen unidas a su pie.
   En tablas abiertas de un escaneo puede reconstruir filas y columnas desde la geometría de la capa
   textual, pero solo si conserva todos los caracteres; añade un recorte visual y marca la asociación
   inferida para revisión. Una tabla nativa con varias filas parciales incompatibles se degrada de la
   misma manera; si procede de un escaneo, se publica su recorte visual y se omite la cuadrícula
   textual incierta sin perder la prosa exterior. Cuando la tabla nativa ya es fiable, conserva sus
   filas y saltos internos en vez de sustituirla por una versión OCR más plana. Una grafía aislada
   que siga en disputa recibe el mismo tratamiento localizado: solo su línea pasa a imagen y el
   resto de la página continúa siendo
   refluible. Las llamadas de nota se muestran como superíndices cuando una definición pequeña al pie
   de la misma página confirma su número; una cifra sin ese respaldo no se reinterpreta. Las
   enumeraciones con una secuencia visual
   inequívoca conservan números, sangrías y líneas
   envueltas como listas refluibles; una palabra espaciada de su rótulo solo se recompone si la misma
   grafía completa aparece en la propia página. Un folio pequeño con letras confundidas por cifras se
   retira cuando al menos dos páginas numéricas confirman la misma secuencia o, en un rango aislado,
   cuando su geometría exterior, tamaño y cercanía al número de página solo son compatibles con un
   folio. Cualquier decisión pendiente se muestra antes de publicar.
   Si una cifra imposible dentro de un signo zodiacal sigue sin confirmarse, el OCR se conserva solo
   como evidencia privada: no puede añadir rótulos ni listas al texto del libro. Parsezen mantiene la
   capa nativa, la lámina original y señala la página para compararla, sin adivinar el valor.

> Windows puede mostrar «Editor desconocido» porque Parsezen todavía no utiliza una firma comercial. Asegúrate de descargarlo desde este repositorio.


## Lo que debes saber

- Parsezen transforma el contenido de un PDF; no intenta reproducir exactamente el diseño de cada página.
- Los documentos escaneados, las tablas complejas y las maquetaciones poco habituales pueden requerir una revisión final.
- Los modelos de IA son opcionales y pueden ocupar varios gigabytes.
- Necesitas conexión a Internet para obtener Parsezen y los modelos que no estén ya en este PC.
  Después, los documentos se procesan localmente y Ollama puede permanecer cerrado.


## Ayuda

- [Guía de uso](docs/user-guide.md)
- [Configurar la IA local](docs/local-ai-setup.md)
- [Informar de un problema](https://github.com/jaimeacena/parsezen/issues)
- [Ver todos los cambios](CHANGELOG.md)

## Desarrollo

La aplicación separa dominio, casos de uso, persistencia local, procesadores y presentación Qt.
Los límites de importación se validan automáticamente para que cada módulo pueda evolucionar y
probarse sin arrastrar la interfaz o SQLite.

Para trabajar en el repositorio:

- la [guía de desarrollo](docs/development.md) fija la instalación reproducible y sus comprobaciones;
- el [modelo operativo para agentes](docs/agent-operating-model.md) explica cómo orientarse, decidir,
  verificar y acumular evidencia;
- el [plan de trabajo](docs/work-plan.md) conserva el estado actual y la siguiente frontera;
- la [arquitectura](docs/architecture.md) describe el diseño implementado;
- la [lista de aceptación](docs/acceptance-checklist.md) contiene los gates de publicación.
- la [evaluación mínima](docs/evaluation.md) permite comparar versiones y revisar sus resultados
  localmente; el banco anterior requiere revalidación antes de usarse como referencia fiable.

Cada trabajo compila una única secuencia de ejecución que comparten interfaz, preflight y procesador.
La ventana delega la ejecución de cola, la recuperación y el flujo de revisión en coordinadores sin
widgets. SQLite conserva un historial técnico versionado de transiciones y snapshots. Incluye rutas
de origen y resultado y otros metadatos del trabajo; los textos documentales completos y los prompts
no forman parte de ese historial.

Los benchmarks privados no se versionan. `scripts/benchmark_documents.py profile` mide tiempo total
y por página, RSS, OCR y recursos de un PDF; `scripts/benchmark_runtime.py` usa solo un payload
sintético para medir DPAPI, snapshots, recuperación, arranque en frío, disco temporal y tamaños de la
instalación/instalador indicados. La validación opcional con documentos reales actualiza su informe
atómicamente después de cada caso, de modo que una interrupción conserva las métricas ya obtenidas.
La selección y aprobación de componentes especializados se define en la
[política de modelos de IA local](docs/local-ai-model-policy.md): Hy-MT2 Q4_K_M prepara la
`Traducción IA` y LFM Q6_K prepara la `Revisión IA`. `Preparado` acredita identidad, licencia,
privacidad y contrato local; la revisión sigue produciendo propuestas bajo guardas y confirmación
humana, no una aprobación semántica automática.
La pantalla de IA local muestra únicamente las capacidades fijas `Traducción IA` y `Revisión IA` y
sus estados locales; no ofrece un selector de tags, endpoints ni modelos arbitrarios. El ejecutable
local nuevo incluye el motor de IA; cada modelo se almacena y verifica por separado. Los trabajos
antiguos configurados con Ollama conservan esa dependencia hasta que se vuelvan a configurar.
Para comparar traducción EN→ES de forma optativa y sin descargar modelos, ejecuta
`python scripts/evaluate_translation_models.py --models TAG_A TAG_B --repetitions 2` con tags que ya
aparezcan instalados en Ollama. Usa el corpus sintético versionado del script y genera un informe
atómico de hashes e indicadores agregados, sin contenido, prompts, respuestas ni rutas.

## Licencia

Parsezen es gratuito y se distribuye bajo licencia [MIT](LICENSE).
