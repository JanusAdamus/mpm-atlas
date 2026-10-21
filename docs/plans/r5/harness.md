# Ronda 5 · harness de trabajo (dynamic workflow)

Fecha: 2026-10-19.  Orquestador: Claude, con dueños por rol.  La herramienta Workflow está apagada en esta sesión, así que el orquestador sigue este harness a mano.  Si se activa, el mismo orden se corre como workflow y cada paso retoma lo que ya está en el registro y en el código.

## Objetivo

- **Se entrega:** los puntos 0–6 del plan `docs/plans/2026-10-19-ronda-5-calidad-y-fuentes.md` (conexiones, nombres, fichas en borrador, fuentes nuevas, canon de obras y piloto de 100,000 obras), con `docs/STATUS.md` §16.
- **No se entrega:**
  - la ingesta completa de unos 5 millones de obras (la decide Adrián con `piloto-obras.md`);
  - fichas marcadas `editorially_reviewed` (las revisa Adrián);
  - ningún commit sin que Adrián lo pida.

## Entradas

- Repo `C:\MPM`.  Registro `data/atlas.db`, ya migrado a v4; respaldo v3 en `data/backups/atlas-v3-2026-10-19.db`.  Copia del piloto en `data/pilot/atlas-pilot.db`.
- Contrato: `docs/adr/0005-esquema-v4-ronda-5.md` (aceptado).
- Sistemas externos, solo lectura: Wikidata, Wiktionary, PlanetMath (GitHub), zbMATH Open, OpenAlex, OpenCitations, Crossref y CDLI.  Siempre con caché local, User-Agent identificable y límite de tasa.
- Credenciales: ninguna.  Nada que requiera token o inicio de sesión.

## Cola (dueños y dependencias)

| # | Rol | Escribe en | Depende de | Estado |
|---|---|---|---|---|
| 2a | Conexiones | `atlas/concepts.py`, `atlas/ingest/wikidata*`, `concepts.rs`, `atlas-paths`, `composition.json`, `atlas-search`; registro (M2) | v4 | en curso |
| 2b | Nombres y fuentes | `atlas/ingest/` (salvo wikidata), `atlas/resolve.py`; registro (curaduría y etiquetas) | v4 | en curso |
| 2c | Obras | `atlas/ingest/works*/`, capa de obras en Rust; **solo** `data/pilot/` | v4 | en curso |
| 3 | Fichas | `atlas/fichas*.py`, `atlas/cli.py` (mínimo), `web/src/panel.ts`, `conceptpanel.ts` | 2b | pendiente |
| 4 | Integrador | `data/snapshot|index|layout`, binario compartido | 2a, 2b, 2c, 3 | pendiente |
| 5 | Jueces A y B (independientes) | `docs/design/r5/juez-*/` | 4 | pendiente |
| 6 | Cierre | `docs/STATUS.md` §16 | 5 | pendiente |

Reglas de conflicto:
- Nadie toca `model.py` ni `store.py`.
- 2a y 2b escriben en el registro con transacciones cortas y nunca borran.
- 2c no escribe en `data/atlas.db`.
- Solo el integrador publica en `data/`.

## Ciclo

1. Revisar el estado actual (git diff y consultas de solo lectura del registro).
2. Hacer el cambio útil más pequeño, con una prueba que falle con el código anterior.
3. Correr las compuertas.
4. Anotar el estado en `docs/plans/r5/estado.md`.
5. Parar si una compuerta falla, si no está claro quién es el dueño, o si la acción externa es insegura (licencia sin verificar, endpoint que pide credenciales).

## Compuertas (eval)

| Compuerta | Comando | Señal de PASA | Dueño si falla |
|---|---|---|---|
| Pruebas | `python -m pytest -q`; `cargo test --workspace --release`; `cargo clippy --workspace --all-targets --release`; en `web/`: `npm test`, `npx tsc --noEmit` | todo en verde, 0 advertencias | el dueño del archivo |
| Registro | `python docs/plans/r5/consultas/r5_gate.py` | `part_of_area` en `formally_linked` = 0; curadurías activas > 0 por regla; lotes del piloto sin filas parciales | 2a / 2b / 2c según la línea |
| Benchmark | AtlasBench v3 test por HTTP | sin regresión contra §15 | 2a |
| Diseño | scripts de `docs/design/r4/final/scripts/` | 0 errores de consola, 0 px de desbordamiento, 0 jerga en Explorar | 3 / integrador |
| Reglas del plan | jueces A y B | PASS/FAIL por regla con evidencia; acuerdo reportado | cierre |

## Entrega al siguiente operador

En `docs/plans/r5/estado.md`: qué está hecho, qué falló y qué necesita una decisión de Adrián.
