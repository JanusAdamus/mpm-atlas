# AtlasBench · reporte 2026-10-17T01:11:10

Protocolo: bench/PROTOCOL.md v1.1 (rules frozen 2026-10-16).  Datos: `{'items': ['bench/items/handwritten/routes.jsonl'], 'split': 'adhoc', 'manifest': None}`.  Registro: `{'path': 'tests/fixtures/mini_atlas.json', 'sqlite': 'C:/Users/ADRIAN~1/AppData/Local/Temp/atlasbench/mini_atlas-5e368179f613b3f5.db', 'bytes': 19779, 'mtime_ns': 1792173970450716100, 'schema_version': 1, 'event_seq': 35}`.
Hardware: {'platform': 'Windows-11-10.0.26200-SP0', 'processor': 'Intel64 Family 6 Model 197 Stepping 2, GenuineIntel', 'logical_cpus': 16, 'ram_bytes': 33811132416, 'python': '3.14.6'}.  Sistema evaluado: `cli`; baseline: `store` (vía léxica FTS5).  Aperturas del conjunto sellado: 0.

Evidencia por tarea: {'route': ['handwritten/authored_by_agent_pending_expert_review']}.  Ningún ítem es oro de especialistas salvo que diga `expert_adjudicated`; los veredictos sobre ítems silver o escritos por agente llevan `PROVISIONAL-`.

## Veredictos

| Componente | Veredicto | Regla | Detalle |
|---|---|---|---|
| retrieval | NOT_EVALUABLE | each lang x modality cell with n >= 30 (trivial silver excluded): delta nDCG@10 >= +0.05 and paired-bootstrap LB >= +0.02 vs lexical baseline | no non-trivial resolved items for system and baseline |
| routes.structure | PASS | 0 structurally invalid steps in returned paths | {'invalid_paths': 0, 'answered': 12} |
| routes.adversarial | INSUFFICIENT | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 10, 'n': 10, 'value': 1.0, 'ci95': [0.7224672001371107, 1.0], 'interval': 'wilson'}} |
| routes.end_to_end | NOT_EVALUABLE | Wilson LB > 0.90 of expert-valid top-1 routes (n >= 100) | no expert judgments of returned routes yet; route_outputs.jsonl is the queue |
| routes.null_model_adversarial | INSUFFICIENT | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 2, 'n': 10, 'value': 0.2, 'ci95': [0.05668215145437519, 0.5098375284633583], 'interval': 'wilson'}} |
| er | NOT_EVALUABLE | Wilson LB of merge precision >= 0.99 on sampled system merges (n >= 381); strata n >= 50 point >= 0.99 | no labeled sample of system merges (needs experts) |
| relations | NOT_EVALUABLE | each priority family (proof, structure, application): Wilson LB of precision >= 0.97, n >= 125 | no labeled relation items (needs experts) |
| provenance.registry | NOT_EVALUABLE | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | no items |
| provenance.served | MISSING | served provenance | no --http server |
| latency.search_hot.c1 | MISSING | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | no --http server |
| latency.search_hot.c8 | MISSING | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | no --http server |
| latency.interaction_server.entity | MISSING | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | no --http server |
| latency.interaction_server.neighborhood | MISSING | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | no --http server |
| latency.interaction_server.tiles | MISSING | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | no --http server |

## Componentes ausentes


## Recuperación (nDCG@10, IC 95% bootstrap por familia)

Ítems resueltos: 0; sin resolver en el registro: 0.

| Grupo | n | triviales |  |
|---|---|---|

## Recorridos

Ítems resueltos: 20; sin resolver: 22.

- `cli`: estructura válida 1.000 [0.758, 1.000] (12/12); adversariales sin conclusión prohibida 1.000 [0.722, 1.000] (10/10); acierto en respondibles 0.917 [0.646, 0.985] (11/12); abstención en respondibles 0.083 [0.015, 0.354] (1/12); sobreafirmación vs oráculo 0.000 [0.000, 0.242] (0/12).
- `store`: estructura válida 0.800 [0.584, 0.919] (16/20); adversariales sin conclusión prohibida 0.200 [0.057, 0.510] (2/10); acierto en respondibles 0.583 [0.320, 0.807] (7/12); abstención en respondibles 0.000 [0.000, 0.242] (0/12); sobreafirmación vs oráculo 0.450 [0.258, 0.658] (9/20).

## Resolución de entidades

Pares resueltos: 0.  Abstención: —.  Fusiones del sistema muestreadas: 0 (etiquetadas: 0).


## Relaciones

Muestra: 0; etiquetadas: 0; aún vigentes: —.


## Procedencia


## Latencia (ms)

| Serie | n | p50 | p95 | IC p95 | p99 |
|---|---|---|---|---|---|
| cli.path | 20 | 1223.32 | 1858.45 | [1719.41, 1891.15] | 1884.61 |
| store.path | 20 | 0.03 | 0.09 | [0.04, 0.71] | 0.58 |
