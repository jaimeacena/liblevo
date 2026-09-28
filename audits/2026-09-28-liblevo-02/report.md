# Identidad completa de Liblevo

Fecha: 28 de septiembre de 2026. Versión actual: 1.3.0.
Estado: **IMPLEMENTADO; contratos automáticos y distribución local comprobados**.
No constituye **VERIFICADO EN CORPUS** ni aceptación humana.

## Intención y decisión

Adoptar Liblevo en las superficies actuales del producto, el checkout, el repositorio y el perfil.
La persona confirma conservar los commits, las versiones y las evidencias históricas. La hipótesis
es que una identidad única evita referencias operativas a carpetas o comandos retirados; la
comprobación decisiva es recuperar el perfil y abrir el paquete construido desde el checkout movido.

## Cambios

- Se reutiliza y traslada el checkout existente a `Developments/Liblevo`; no se crea un repositorio.
- GitHub conserva el ID 1317270293 con nombre `jaimeacena/liblevo`. Se renombra la rama de trabajo
  y se actualiza su seguimiento; las referencias históricas locales del archivo anterior se conservan.
- Código, imports, comandos, componentes propios, recursos, avisos, distribución y documentos
  actuales usan Liblevo. El perfil y las claves Qt usan también Liblevo.
- Se reinstala el paquete editable 1.3.0 en los dos entornos locales y se corrigen sus rutas y
  lanzadores tras el traslado. El proyecto guardado en Codex usa la carpeta nueva.
- Hay copias de recuperación de las fuentes, SQLite, JSON, preferencias y metadatos de Codex.
  Las construcciones anteriores se conservan en `outputs/history/`.

## Evidencia y verificaciones

- Git comprueba la integridad del almacén después del traslado. HEAD anterior: `42543e4`.
- SQLite: integridad `ok`; se conserva 1 trabajo, 27 eventos y 24 métricas. No hay revisiones,
  libros ni snapshots cifrados guardados que deban convertirse en este perfil.
- Se actualiza exactamente 1 referencia de modelo en la cola. El código actual recupera el trabajo
  y los ajustes. Los originales y resultados no se modifican.
- El GGUF conserva su mismo archivo y tamaño; SHA-256 coincide con el catálogo verificado.
- Las preferencias de apariencia son idénticas tras normalizar solo los nombres de las claves.
- No hay accesos directos ni instalaciones anteriores de la app registrados en este equipo.
- Pasan 50 pruebas focales de paquete, ajustes, persistencia y runtime, y 2 de lanzadores.
- Pasan Ruff check, Ruff format (300 archivos), Mypy (139 módulos) y sincronización de versión.
- La primera suite detectó una prueba obsoleta que exigía aliases de los lanzadores retirados:
  2.468 casos pasaron, 1 falló y 3 se omitieron. Se corrigió el contrato y pasaron los casos focales;
  la suite final completa pasa con **2.469 pruebas, 3 omisiones y cobertura 89,04 %**
  (237,11 s).
- El paquete Windows 1.3.0 supera el arranque del candidato y el del destino final. El ejecutable
  declara Liblevo; contiene 137 módulos propios con el namespace actual y ninguno anterior.
- Los 12 derivados de marca y el manifiesto empaquetados coinciden con las fuentes; Inter y
  los 9 tamaños del icono incrustado son idénticos. La normalización final de los SVG elimina espacios
  de final de línea sin cambiar PNG ni ICO; pasan sus 18 pruebas focales.
- El instalador local `Liblevo-Setup-1.3.0.exe` se compila correctamente (166,17 s); sus metadatos
  declaran Liblevo 1.3.0. Se genera checksum; no se instala ni se publica una nueva release.
- La búsqueda final en código, pruebas, scripts, recursos, distribución y documentos actuales no
  encuentra el nombre anterior ni filenames con esa identidad. Las referencias históricas se conservan.

## Límites y reapertura

No se ejecuta un corpus documental ni se cambia la calidad lingüística. La aceptación visual humana,
la lectura en Kindle y la actualización de una instalación anterior en otro equipo siguen abiertas.
La migración de este perfil no demuestra compatibilidad de artefactos históricos de otros equipos.
Reabrir si un acceso actual apunta a la carpeta retirada, el perfil deja de recuperarse, reaparecen
cortes en el nombre o una variante de marca pierde legibilidad. Nunca sustituir documentos o modelos
para resolver un fallo de identidad; recuperar primero los metadatos anteriores y diagnosticarlo.

## Huellas de la distribución local

- Ejecutable: `82752e561759fcbc8ee0f1073557c3dd64bc73a481fba00278be14e6f2ee342c`.
- Instalador: `50fcd873cc285d3e2ef5c8291648b99f5556069895d65e9f2abe354ed2ecb734`.
