# AtlasBench · reporte 2026-10-18T16:59:34

Protocolo: bench/PROTOCOL.md v1.3 (thresholds frozen 2026-10-16; clarifications 6.1-6.3).  Datos: `{'items': ['C:/Users/ADRIAN~1/AppData/Local/Temp/claude/C--Users-AdrianGonzalez-OneDrive---Delos-Advisors-Documentos-ChatGPT-MPM/4e777e56-671e-4d95-a7b5-b0a7fd109ec2/scratchpad/routes-test-v3.jsonl'], 'split': 'adhoc', 'manifest': None}`.  Registro: `{'path': 'C:/MPM/data/atlas.db', 'sqlite': 'C:/MPM/data/atlas.db', 'bytes': 5179146240, 'mtime_ns': 1792338329159387300, 'schema_version': 3, 'event_seq': 2943391}`.
Hardware: {'platform': 'Windows-11-10.0.26200-SP0', 'processor': 'Intel64 Family 6 Model 197 Stepping 2, GenuineIntel', 'logical_cpus': 16, 'ram_bytes': 33811132416, 'python': '3.14.6'}.  Sistema evaluado: `http`; baseline: `store` (vía léxica FTS5).  Aperturas del conjunto sellado: 0.

Evidencia por tarea: {'route': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto']}.  Ningún ítem es oro de especialistas salvo que diga `expert_adjudicated`; los veredictos sobre ítems silver o escritos por agente llevan `PROVISIONAL-`.

## Veredictos

| Componente | Veredicto | Regla | Detalle |
|---|---|---|---|
| retrieval | NOT_EVALUABLE | each lang x modality cell with n >= 30 (trivial silver excluded): delta nDCG@10 >= +0.05 and paired-bootstrap LB >= +0.02 vs lexical baseline | no non-trivial resolved items for system and baseline |
| routes.structure | PASS | 0 structurally invalid steps in returned paths | {'invalid_paths': 0, 'answered': 264} |
| routes.adversarial | PROVISIONAL-PASS | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 188, 'n': 188, 'value': 1.0, 'ci95': [0.9799758673421861, 1.0], 'interval': 'wilson'}} |
| routes.end_to_end | NOT_EVALUABLE | Wilson LB > 0.90 of expert-valid top-1 routes (n >= 100) | no expert judgments of returned routes yet; route_outputs.jsonl is the queue |
| routes.null_model_adversarial | PROVISIONAL-FAIL | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 2, 'n': 188, 'value': 0.010638297872340425, 'ci95': [0.002922269566028632, 0.03795241346076797], 'interval': 'wilson'}} |
| er | NOT_EVALUABLE | Wilson LB of merge precision >= 0.99 on sampled system merges (n >= 381); strata n >= 50 point >= 0.99 | no labeled sample of system merges (needs experts) |
| relations | NOT_EVALUABLE | each priority family (proof, structure, application): Wilson LB of precision >= 0.97, n >= 125 | no labeled relation items (needs experts) |
| provenance.registry | NOT_EVALUABLE | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | no items |
| provenance.served | NOT_EVALUABLE | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | no items |
| latency.search_hot.c1 | NOT_EVALUABLE | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | no HTTP search c=1 samples |
| latency.search_hot.c8 | NOT_EVALUABLE | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | no HTTP search c=8 samples |
| latency.interaction_server.entity | NOT_EVALUABLE | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | no HTTP entity samples |
| latency.interaction_server.neighborhood | NOT_EVALUABLE | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | no HTTP neighborhood samples |
| latency.interaction_server.tiles | NOT_EVALUABLE | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | no HTTP tiles samples |

## Componentes ausentes


## Recuperación (nDCG@10, IC 95% bootstrap por familia)

Ítems resueltos: 0; sin resolver en el registro: 0.

| Grupo | n | triviales |  |
|---|---|---|

## Recorridos

Ítems resueltos: 449; sin resolver: 13.

- `http`: estructura válida 1.000 [0.986, 1.000] (264/264); adversariales sin conclusión prohibida 1.000 [0.980, 1.000] (188/188); acierto en respondibles 0.996 [0.979, 0.999] (260/261); abstención en respondibles 0.004 [0.001, 0.021] (1/261); sobreafirmación vs oráculo 0.000 [0.000, 0.014] (0/264).
- `store`: estructura válida 0.824 [0.786, 0.856] (370/449); adversariales sin conclusión prohibida 0.011 [0.003, 0.038] (2/188); acierto en respondibles 0.759 [0.703, 0.807] (198/261); abstención en respondibles 0.000 [0.000, 0.015] (0/261); sobreafirmación vs oráculo 0.432 [0.387, 0.478] (194/449).

## Resolución de entidades

Pares resueltos: 0.  Abstención: —.  Fusiones del sistema muestreadas: 0 (etiquetadas: 0).


## Relaciones

Muestra: 0; etiquetadas: 0; aún vigentes: —.


## Procedencia


## Latencia (ms)

| Serie | n | p50 | p95 | IC p95 | p99 |
|---|---|---|---|---|---|
| http.path | 449 | 1.20 | 3.88 | [3.51, 4.47] | 7.03 |
| store.path | 449 | 1.37 | 15.27 | [14.22, 19.23] | 30.37 |
