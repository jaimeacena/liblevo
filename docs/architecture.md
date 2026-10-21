# Arquitectura de Liblevo

Describe lo que existe hoy. Al cambiar la implementación, este documento se actualiza en el mismo
PR.

## Principios

1. **No perder ni corromper documentos.** El original nunca se modifica; los resultados se
   publican de forma atómica y el trabajo se reanuda tras un cierre.
2. **Todo en local.** Nada sale del equipo; solo se descargan componentes públicos del catálogo,
   verificados por tamaño y SHA-256.
3. **Programas aparte, atados a la app.** Lector y modelos corren como procesos hijos sin red, con
   su huella comprobada antes de cada uso; si la app se cierra, Windows los detiene.
4. **Mecanismos generales.** Nada específico de un documento; cada cambio de conversión se mide con
   el conjunto de [evaluación](evaluation.md).
5. **Lo desconocido, desconocido.** Ninguna guarda reescribe en silencio; avisa o conserva.
6. **Pocas piezas.** Tres crates, la interfaz y la herramienta de medición.

## Piezas

| Pieza | Ubicación | Responsabilidad |
|---|---|---|
| Núcleo | `native/liblevo-core` | Markdown → EPUB 3 (`markdown_epub`), EPUB ajeno editable en su sitio (`epub_package`), fórmulas sencillas (`math`) |
| Motor | `native/liblevo-worker` | Lectura, pulido, señales de duda, relectura visual, revisión y traducción |
| App | `app/` | Backend Tauri: documentos en SQLite, tareas en segundo plano, resultados, catálogo de componentes |
| Interfaz | `ui/` | TypeScript sin framework (Vite), un módulo por sección: biblioteca, leer, revisar, traducir, exportar, EPUB original y paneles |
| Medición | `native/liblevo-eval`, `eval/`, `scripts/` | Conjunto público, métricas, ejecutores e instalador |

## Conversión (`convert`)

1. **Lectura** (`xberg_reader`): Xberg 1.3.10 devuelve Markdown con una marca `<!-- page: N -->`
   por página. En PDF digitales lee sin extraer imágenes (esa opción empeora su lectura del texto);
   en escaneos la usa porque mide mejor, y descarta esas imágenes.
2. **Escaneos** (`scan_detect`, `doubt`, `visual_reader`): un documento con al menos la mitad de
   páginas cubiertas por una imagen y sin texto visible se lee con OCR. Las páginas en las que el
   OCR y la capa invisible coinciden en menos del 80 % de las palabras, o con más tinta de la que
   explican sus palabras, se releen con OvisOCR2 dentro de un presupuesto de 20 s por página
   (mínimo 5 minutos). La relectura sustituye al OCR solo si conserva al menos el 75 % de sus
   palabras y no lo supera en 1,5 veces; sus tablas HTML pasan a Markdown (`html_tables`) y cada
   figura se recorta de la página dibujada.
3. **Estilo y pulido** (`pdf_styles`, `polish`, `cleanup`, `headings`, `dashes`): PDFium da tamaño,
   negrita y posición de cada línea y los marcadores; se quitan cabeceras y pies repetidos (también
   por rachas de 5 páginas), adornos ajenos al PDF y guiones de fin de línea; se marcan títulos por
   estilo y marcadores; se reponen las rayas que el lector perdió cuando el texto de la página las
   muestra sin ambigüedad.
4. **Imágenes** (`pdf_images`, `images`): en PDF digitales, PDFium saca cada imagen y la coloca
   antes del primer texto que tiene debajo en su columna; se descartan las diminutas, repetidas,
   uniformes y fondos. En EPUB y DOCX se copian del propio paquete.
5. **Otros formatos**: DOCX, EPUB, HTML y TXT se leen con Xberg y Markdown tal cual, como una sola
   «página» con el mismo pulido.
6. **Revisión** (`review`): avisos por fragmento ordenados por riesgo: palabras que el texto del
   PDF (o la capa invisible) no contiene, tokens raros, sospechas del vocabulario del propio
   documento y páginas dudosas.
7. **Reanudación** (`checkpoint`): la lectura y cada página releída se guardan de forma atómica,
   ligadas al SHA-256 del original, a las huellas de los componentes y a la versión del proceso.

El resultado es `results/<id>/document.md` (con marcas de página), sus imágenes y `review.json`.

## Traducción (`translate`)

Hy-MT2 (1,8B o 7B) con `llama-server` de llama.cpp, que mantiene el modelo cargado durante todo el
documento (solo en `127.0.0.1`, puerto al azar, clave de un solo uso, contexto de 8.192 piezas
ampliable a 16.384 para un bloque muy largo). Un bloque cada vez, sin historial, con la plantilla de
conversación de cada modelo y el glosario por su plantilla de terminología. Marcas de página,
imágenes y código no pasan por el modelo; títulos, listas y citas conservan su prefijo; las tablas
se traducen celda a celda; un párrafo cortado por un salto de página se traduce entero y se
reparte. Las guardas avisan de traducción vacía, desproporcionada, sin traducir, con el formato
perdido, sin un término del glosario o sin cifras del original. Cada bloque se guarda con la huella
del texto, el idioma, el modelo, el glosario y la versión de las instrucciones, para reanudar. Un
EPUB ajeno se traduce sobre `epub_package` y declara el nuevo idioma.

## App

- **Documentos** en `workspace.sqlite3`, escrito solo por la app; resultados, traducciones,
  puntos de control y copias de trabajo en la carpeta de datos (ver [seguridad](../SECURITY.md)).
- **Tareas** en hilos aparte con cancelación; la interfaz recibe eventos de progreso sin contenido.
- **Publicación atómica**: el resultado nuevo se prepara aparte y sustituye al anterior; al abrir,
  la app devuelve a su sitio un resultado si un cierre llegó a mitad.
- **Componentes** (`components`): catálogo cerrado con URL, tamaño y SHA-256 de cada descarga y de
  cada archivo instalado; se prepara aparte y sustituye a lo anterior solo si todo cuadra.
- **Seguridad de la interfaz**: política de contenido que bloquea todo origen remoto; permisos de
  la ventana limitados a sus comandos y a los diálogos de abrir y guardar.
- **Instalador**: NSIS por usuario con PDFium (`scripts/build_installer.py`).

## Interfaz

- **Navegación lateral**: Biblioteca, Actividad (tareas en curso y avisos recientes), Componentes
  y Ajustes (tema y atajos).
- **Biblioteca**: documentos con su formato, estado, avisos pendientes y traducciones; búsqueda,
  filtros y arrastrar y soltar.
- **Documento**: una acción principal según el estado (convertir, revisar, exportar) y pestañas
  que muestran el estado de cada paso: Leer (original y texto, solo texto o comparar traducción;
  índice, zoom y fragmentos con avisos marcados), Revisar (cola por riesgo con el original al
  lado y atajos de teclado), Traducir, Exportar y EPUB original.
- **Avisos** en notificaciones que se apilan; los errores se quedan hasta cerrarlos.
- Cada sección es un módulo de `ui/src`; las acciones compartidas pasan por `state.ts`, de modo
  que añadir una sección no obliga a tocar las demás.

## Dependencias principales

Tauri 2, PDFium 7947 (pdfium-binaries), Xberg 1.3.10, llama.cpp b10566, OvisOCR2 y Hy-MT2 (GGUF),
SQLite (rusqlite), pulldown-cmark, zip, quick-xml y ureq. Licencias de código, pesos y binarios
revisadas por separado; ninguna GPL/AGPL.
