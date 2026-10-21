# Seguridad y privacidad

## Comunicar una vulnerabilidad

No publiques detalles explotables ni documentos de prueba en una incidencia abierta. Usa
**Security → Report a vulnerability** en el repositorio de GitHub para enviar un aviso privado, con
el impacto, la versión y pasos mínimos para reproducirlo sin contenido documental privado.

## Privacidad

- Los documentos se procesan en el equipo. La app no tiene telemetría, ni servicios remotos, ni
  direcciones configurables.
- Las únicas conexiones son las descargas de componentes que la persona pide en «Componentes»:
  archivos públicos de GitHub y Hugging Face con URL, tamaño y SHA-256 fijados en el código.
- La interfaz no carga recursos remotos: su política de contenido bloquea cualquier origen externo.
- Los registros no incluyen texto de los documentos, instrucciones ni respuestas de los modelos.

## Qué guarda la app y dónde

Todo vive en la carpeta de datos de la app del usuario de Windows
(`%APPDATA%\com.jaimeacena.liblevo`), protegida por los permisos de esa cuenta y **sin cifrar**:

| Ubicación | Contenido |
| --- | --- |
| `workspace.sqlite3` | Lista de documentos (ruta, nombre, páginas, opciones) y decisiones de revisión |
| `results/` | Resultado de cada documento (Markdown, imágenes, lista de revisión) y sus traducciones |
| `work/` | Puntos de control de una conversión interrumpida, para reanudarla |
| `epub/` | Copias de trabajo de los EPUB editados o traducidos en su sitio |
| `components/` | Lector, modelos y programas descargados del catálogo |

Quitar un documento de la lista borra su resultado, sus traducciones, su copia de trabajo y sus
puntos de control; el archivo original nunca se toca. Los Markdown y EPUB exportados son archivos
normales de la persona.

Quedan fuera del alcance un proceso malicioso que ya se ejecute con la misma cuenta, un
administrador del equipo y el acceso físico al disco.
