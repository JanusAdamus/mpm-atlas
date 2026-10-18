# Fixtures de ingesta

- `mathlib/`: árbol escrito a mano con el formato de Mathlib4 (no copiado de Mathlib).  Las pruebas lo empaquetan como el tarball de GitHub (`tests/test_ingest_mathlib.py::build_tarball`).
- `cache/wikidata/`: respuestas reales del servicio SPARQL de Wikidata, grabadas por `record_fixtures.py`.  Licencia CC0 1.0.
- `cache/wikipedia/`: respuestas reales de la API de MediaWiki de Wikipedia en inglés y español, grabadas por `record_fixtures.py`.  Texto de los colaboradores de Wikipedia bajo CC BY-SA 4.0; la atribución es el título del artículo y el identificador de revisión que cada respuesta incluye (historial en `https://<lang>.wikipedia.org/w/index.php?title=<título>&action=history`).

Cada archivo `*.gz` es una respuesta: primera línea JSON con `fetched_at`, método, URL y cuerpo de la consulta; después el cuerpo tal como llegó.  Para regrabar (requiere red): `python tests/fixtures/ingest/record_fixtures.py`.
