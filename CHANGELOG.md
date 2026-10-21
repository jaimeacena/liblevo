# Cambios de Liblevo

## 0.1.0 · versión personal (20 de octubre de 2026)

Primera versión de Liblevo en Rust con interfaz Tauri, para Windows. Las mediciones del conjunto
público están en `eval/resultados/` ([evaluación](docs/evaluation.md)). También se usó una prueba
interna de 317 páginas que no se incluye en el repositorio.

### Conversión

- PDF digitales leídos con Xberg 1.3.10 como programa aparte, con su huella comprobada y atado a la
  app; imágenes extraídas con PDFium y colocadas junto a su pie; los seis digitales del conjunto
  dan EPUB que pasan EPUBCheck 5.3.0 sin errores.
- Pulido general: cabeceras y pies repetidos (también por rachas de páginas), adornos ajenos al
  PDF, palabras partidas, títulos por estilo y marcadores, y rayas repuestas con el texto visible
  del PDF. Libro de 317 páginas en 64–72 s, sin cabeceras coladas.
- DOCX, EPUB, HTML, TXT y Markdown, con las imágenes del propio paquete.
- Reanudación de conversiones interrumpidas con puntos de control atómicos.
- En PDF a dos columnas, los títulos se detectan columna a columna (*BERT*: 43 → 87 % de los
  títulos del índice, sin cambios en el resto del conjunto).

### Escaneos

- Detección automática y OCR; las páginas dudosas (desacuerdo con la capa invisible) y las de
  probable figura se releen con OvisOCR2 dentro de un presupuesto de tiempo, con una guarda que
  conserva el OCR si la relectura no es fiable; las figuras se recortan de la página.
- Tablas escaneadas (*s4*): palabras 89,5 → 97,6 %, cifras 39 → 92 %, 36 tablas.
- Opción por documento de lectura visual completa (mejor en unos escaneos, peor en otros, de 5 a
  15 veces más lenta).

### Revisión y edición

- Lista de avisos por fragmento ordenada por riesgo, con Correcto, Corregir y Después guardados.
  Con 186 errores plantados, el primer 10/20/50 % de la lista recoge el 44/59/72 %; con errores
  reales, el 80 % queda avisado.
- Editor de página del resultado; EPUB ajeno editable en su sitio (guardar sin cambios conserva
  todas las entradas byte a byte); apertura en Sigil o Calibre.

### Traducción

- Hy-MT2 1,8B (predeterminado) o 7B con llama.cpp como programa aparte, bloque a bloque: el modelo
  nunca ve la estructura; tablas celda a celda; párrafos cortados por un salto de página
  traducidos enteros; glosario con la plantilla de terminología del modelo.
- Guardas que avisan sin reescribir (vacía, longitud, sin traducir, formato perdido, glosario,
  cifras que faltan) y reanudación por bloques. En seis documentos (7.237 palabras) la estructura
  queda intacta; 1,8B unos 10 s y 7B unos 31 s por cada 100 palabras.
- Un EPUB ajeno se traduce en su sitio y declara el nuevo idioma.
- El modelo queda cargado durante todo el documento (`llama-server` solo en `127.0.0.1`, puerto al
  azar y clave de un solo uso): mismas traducciones, 17 % menos de tiempo con 1,8B y entre un 27 y un
  37 % menos con 7B. Si el programa no arranca, la traducción se detiene con un mensaje claro.

### Interfaz

- Navegación lateral con Biblioteca (búsqueda, filtros, estado y avisos de cada documento,
  arrastrar y soltar), Actividad, Componentes y Ajustes (tema claro, oscuro o del sistema).
- Espacio de documento con acción principal según el estado y pestañas con el estado de cada
  paso; Leer con índice, zoom, comparación con la traducción y avisos marcados en el texto;
  Revisar en cola con el original al lado y atajos (J, K, C, E, D, L).
- Notificaciones que se apilan y un historial de lo ocurrido.
- EPUB original en una lista compacta: selector de capítulo y teclas ←/→, tipo de cada bloque en
  lenguaje llano, edición con Ctrl+S y Esc que avisa antes de perder un cambio, estado de la copia
  de trabajo visible y explicación para los capítulos sin texto editable.
- Traducir, Exportar y EPUB original centrados en pantallas anchas.

### Exportación, app e instalación

- Markdown con sus imágenes y EPUB 3 con índice, intervalo de páginas, portada con la primera
  página y fórmulas sencillas con subíndices y superíndices.
- Catálogo cerrado de componentes descargables (lectura, relectura visual, traducción rápida y de
  más calidad), con URL, tamaño y SHA-256 fijados.
- Instalador de Windows (NSIS) con PDFium verificado y sus licencias.

### Límites conocidos

- Fórmulas complejas en LaTeX crudo; texto dentro de figuras vectoriales como párrafos sueltos;
  en portadas, líneas cortas centradas pueden salir como títulos; algunas iniciales decoradas se
  leen mal («0 NE» por «ONE»).
- Escaneos sin capa invisible sin señal de páginas dudosas; cursiva y negrita de escaneos sin
  recuperar.
- Bloques de traducción muy cortos pueden quedar sin traducir sin aviso.
- Sin probar en un Windows limpio ni en un equipo de 8 GB.
