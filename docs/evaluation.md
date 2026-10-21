# Evaluación

Cómo se mide si un cambio mejora Liblevo: con un **conjunto fijo de PDF públicos y variados** y una
prueba interna adicional que no se distribuye. Un conjunto variado detecta los ajustes que solo
benefician a un documento.

## El conjunto

Solo documentos de dominio público o con licencia abierta. Los PDF no se suben al repositorio:
`eval/manifest.json` guarda para cada uno su origen (URL), licencia, SHA-256, tipo y lo que pone a
prueba, y un script los descarga y comprueba. La prueba interna se guarda fuera del repositorio.

| Grupo | Composición |
|---|---|
| Digitales (~10) | Novela a una columna; *The Gift of the Magi* (portada con fuente rota); libro técnico con tablas y figuras; artículo a dos columnas con notas; libro con notas al pie e índice analítico; documento oficial en español con tablas; libro ilustrado; poesía |
| Escaneados (~8) | Escaneo limpio en inglés; libro antiguo en español; escaneo con tablas; dos columnas; calidad baja o páginas torcidas; notas al pie; ilustraciones |

El conjunto queda fijo una vez acordado. Añadir un documento es una decisión explícita y anotada.
Ningún documento del conjunto se usa para entrenar o ajustar un modelo.

Notas sobre el conjunto:

- *Marianela* (s2) resultó ser una traducción inglesa de 1883, no el original en español. Se
  mantiene como escaneo inglés antiguo; el español escaneado lo cubren s3 y s7.
- s5 (tablas solo como imagen) y s6 (*Scientific American*) todavía no tienen páginas de
  referencia: se miden por estructura, márgenes, tiempo y memoria.

## Uso

```powershell
# Comprobar que los PDF descargados coinciden con el manifiesto
cargo run --release -p liblevo-eval -- verify --manifest eval/manifest.json --pdf-dir <pdf>

# Medir la salida de un candidato: <salida>/<id>/document.md y <salida>/<id>/run.json
cargo run --release -p liblevo-eval -- measure --manifest eval/manifest.json --pdf-dir <pdf> `
  --pdfium <pdfium.dll> --references eval/references --candidate <salida> --out resultados.json

# Tabla comparativa de varios resultados
cargo run --release -p liblevo-eval -- table resultados-a.json resultados-b.json
```

Cada candidato escribe su Markdown con una marca `<!-- page: N -->` al empezar cada página. Sin
esas marcas no se pueden medir el orden por página ni las páginas de referencia de los escaneos.
`scripts/eval_cli_candidate.py` genera esas salidas con `liblevo-convert` (y con otros lectores
para compararlos). `scripts/eval_review.py`, `eval_review_real.py` y `eval_translation.py` miden la
revisión y la traducción.

## Referencias

- **Digitales:** el texto del propio PDF, leído con PDFium, sirve de referencia de contenido.
- **Escaneados:** dos páginas por documento transcritas con su estructura (títulos, párrafos,
  tablas). Salvo la referencia validada indicada en `eval/references/README.md`, son borradores
  pendientes de revisión editorial.

## Métricas

Las calcula la herramienta `eval` con el mismo motor que usa la app:

| Métrica | Qué detecta |
|---|---|
| Texto conservado frente a la referencia (palabras y cifras) | Pérdidas y errores de lectura |
| Cabeceras y pies que se cuelan en el cuerpo | Artefactos repetidos sin eliminar |
| Títulos frente al índice o el marcador del PDF | Estructura de capítulos |
| Tablas e imágenes encontradas, y celdas de tablas en las páginas de referencia | Elementos perdidos o mal reconstruidos |
| Orden de lectura en páginas de varias columnas | Columnas mezcladas |
| Errores de EPUBCheck | EPUB inválido |
| Tiempo y memoria por página | Viabilidad en un PC corriente |

Límite conocido: en los digitales, la referencia es la capa de texto del PDF. Si esa capa está rota
(una portada con una fuente sin mapa Unicode), la referencia contiene basura y un lector que lee lo
impreso parece perder texto o cifras. Ocurre en *Gift of the Magi* y *Heart of Darkness*.

El orden de los digitales se compara con el de esa capa y descuenta también las palabras que faltan,
así que baja sin que haya desorden: tablas grandes que la capa guarda por columnas y Liblevo por
filas (*Docling*), números de línea al margen (*Free Culture*) o tablas con palabras perdidas
(*BERT*). `LIBLEVO_EVAL_PAGINAS=1` muestra, por página, las palabras recuperadas y el orden para
separar una cosa de la otra.

La traducción se mide con `scripts/eval_translation.py` (estructura, cifras, tiempo) y
`scripts/eval_translation_quality.py` (chrF frente a traducciones de referencia de algunos bloques,
guardadas fuera del repositorio). chrF solo compara candidatos sobre las mismas referencias; no es
una nota absoluta.

Cada resultado se da por documento y por métrica. **No hay puntuación global ni «porcentaje de
perfección»**: un número que agrega todo esconde dónde empeora.

## Reglas

1. Un cambio de conversión se mide sobre todo el conjunto y se compara con la medición anterior.
2. **Un cambio que mejora un documento y empeora otros no se acepta** sin evaluar expresamente ese
   intercambio.
3. Nada específico de un documento: ni nombres, ni páginas, ni frases, ni vocabularios, ni
   glosarios de dominio en el código.
4. Cada mecanismo nuevo lleva pruebas con casos inventados: uno que debe corregir y otro que no debe
   tocar.
5. Las cifras son de un equipo concreto; el rendimiento en un PC de 8 GB se declara aparte cuando se
   mida.
6. Ninguna ausencia de avisos certifica fidelidad. Lo no medido se dice expresamente.

## Revisión humana

Las métricas no ven todo: el significado, el orden fino o una tabla sutilmente mal asociada pueden
pasar. Los resultados importantes deben revisarse junto al documento original.
