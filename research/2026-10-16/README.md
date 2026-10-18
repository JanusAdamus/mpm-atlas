# Investigación para el atlas de conexiones matemáticas

Fecha de corte: 16 de octubre de 2026.

## Lectura

- `atlas-investigacion.html`: dossier autónomo con navegación, diagramas y bibliografía filtrable. Abrir directamente en un navegador; no requiere instalación ni servicios externos.
- `atlas-investigacion-y-plan.md`: informe fuente editable.
- `../../output/pdf/atlas-investigacion-y-plan.pdf`: informe de 15 páginas, renderizado y revisado visualmente.
- `plan-ejecucion.md` y `plan-ejecucion.json`: 16 paquetes de trabajo con dependencias, entregables y criterios de aceptación.
- `bibliografia-y-evidencia.md`: 60 referencias con técnica, uso propuesto, límites y nivel de lectura.
- `references.json` y `references.bib`: datos bibliográficos reutilizables.

## Trazabilidad

`search-ledger.json` conserva las búsquedas y resultados consultados. `retrieval-ledger.json` registra recuperaciones directas; `sources/` contiene respuestas y metadatos, incluido el lote de 33 registros de arXiv. Los registros web y los de recuperación son complementarios.

Esta es una revisión de alcance para decisiones de diseño. Las 60 entradas incluyen artículos, preprints, estándares y documentación; no representan 60 textos completos leídos ni una revisión sistemática exhaustiva. Cada entrada declara su nivel de consulta. Las metas, recursos y fechas son propuestas de planificación, no resultados obtenidos.

El HTML presenta la investigación; no es una implementación del atlas. La revisión visual se completó para el PDF. La verificación interactiva del HTML en el navegador integrado no pudo completarse debido a un fallo de acceso a localhost.

Los scripts `build_deliverables.py`, `verify_sources.py` y `render_review.py` conservan la construcción y verificación. El primero utiliza ReportLab y pypdf; el último utiliza pypdfium2 y Pillow. Pueden requerir ajustar rutas locales de fuentes o recursos en otro equipo. `delivery-checks.json` contiene los controles estructurales de la entrega.
