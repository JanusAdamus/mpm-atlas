# AtlasBench · reporte 2026-10-18T00:13:08

Protocolo: bench/PROTOCOL.md v1.3 (thresholds frozen 2026-10-16; clarifications 6.1-6.3).  Datos: `{'split': 'dev', 'manifest': 'bench/manifest/v3.json'}`.  Registro: `{'path': 'C:/MPM/data/atlas.db', 'sqlite': 'C:/MPM/data/atlas.db', 'bytes': 3632160768, 'mtime_ns': 1792279714524870400, 'schema_version': 1, 'event_seq': 2049417}`.
Hardware: {'platform': 'Windows-11-10.0.26200-SP0', 'processor': 'Intel64 Family 6 Model 197 Stepping 2, GenuineIntel', 'logical_cpus': 16, 'ram_bytes': 33811132416, 'python': '3.14.6'}.  Sistema evaluado: `http`; baseline: `store` (vía léxica FTS5).  Aperturas del conjunto sellado: 0.

Evidencia por tarea: {'retrieval': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'route': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'sourced_answer': ['silver/silver_auto'], 'er_pair': ['silver/silver_auto'], 'relation': ['silver/unlabeled']}.  Ningún ítem es oro de especialistas salvo que diga `expert_adjudicated`; los veredictos sobre ítems silver o escritos por agente llevan `PROVISIONAL-`.

## Veredictos

| Componente | Veredicto | Regla | Detalle |
|---|---|---|---|
| retrieval | PROVISIONAL-FAIL | each lang x modality cell with n >= 30 (trivial silver excluded): delta nDCG@10 >= +0.05 and paired-bootstrap LB >= +0.02 vs lexical baseline | {'cells': {'en/formula': {'value': 0.008237034884529733, 'n': 157, 'clusters': 157, 'ci95': [0.0007162462265613938, 0.02062461482147998], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}, 'en/informal_to_formal': {'value': 0.07950375515482284, 'n': 46, 'clusters': 41, 'ci95': [0.0347 |
| routes.structure | PASS | 0 structurally invalid steps in returned paths | {'invalid_paths': 0, 'answered': 247} |
| routes.adversarial | PROVISIONAL-PASS | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 198, 'n': 198, 'value': 1.0, 'ci95': [0.9809679396733518, 1.0], 'interval': 'wilson'}} |
| routes.end_to_end | NOT_EVALUABLE | Wilson LB > 0.90 of expert-valid top-1 routes (n >= 100) | no expert judgments of returned routes yet; route_outputs.jsonl is the queue |
| routes.null_model_adversarial | PROVISIONAL-FAIL | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 6, 'n': 198, 'value': 0.030303030303030304, 'ci95': [0.013960646989789376, 0.06452401574130437], 'interval': 'wilson'}} |
| er | NOT_EVALUABLE | Wilson LB of merge precision >= 0.99 on sampled system merges (n >= 381); strata n >= 50 point >= 0.99 | no labeled sample of system merges (needs experts) |
| relations | NOT_EVALUABLE | each priority family (proof, structure, application): Wilson LB of precision >= 0.97, n >= 125 | no labeled relation items (needs experts) |
| provenance.registry | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3266, 'value': 0.0, 'ci95': [0.0, 0.0011748150083336429], 'interval': 'wilson'}} |
| provenance.served | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3266, 'value': 0.0, 'ci95': [0.0, 0.0011748150083336429], 'interval': 'wilson'}} |
| latency.search_hot.c1 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 12.158554972847924, 'n': 500, 'clusters': None, 'ci95': [11.882362779069808, 12.49980000720825], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.search_hot.c8 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 59.59779000259003, 'n': 500, 'clusters': None, 'ci95': [55.35485089741987, 63.95490001887083], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.entity | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 1.01859000569675, 'n': 500, 'clusters': None, 'ci95': [0.892309664559434, 1.3158000074326992], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.neighborhood | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 0.609049980994314, 'n': 500, 'clusters': None, 'ci95': [0.5819243822770659, 0.6314100290182973], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.tiles | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 2.73348503105808, 'n': 500, 'clusters': None, 'ci95': [2.6392050116555765, 2.832000027410686], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |

## Componentes ausentes


## Recuperación (nDCG@10, IC 95% bootstrap por familia)

Ítems resueltos: 1631; sin resolver en el registro: 0.

| Grupo | n | triviales | http nDCG@10 | http recall@10 | store nDCG@10 | store recall@10 |
|---|---|---|---|---|---|---|
| cells:en/formula | 157 | 0 | 0.008 [0.001, 0.021] (n=157) | 0.013 [0.003, 0.025] (n=157) | 0.000 [0.000, 0.000] (n=157) | 0.000 [0.000, 0.000] (n=157) |
| cells:en/informal_to_formal | 182 | 136 | 0.282 [0.245, 0.325] (n=182) | 0.352 [0.273, 0.435] (n=182) | 0.276 [0.244, 0.314] (n=182) | 0.390 [0.313, 0.472] (n=182) |
| cells:en/lean_name | 465 | 464 | 0.990 [0.985, 0.995] (n=465) | 0.992 [0.987, 0.998] (n=465) | 0.976 [0.966, 0.985] (n=465) | 0.990 [0.983, 0.997] (n=465) |
| cells:en/text | 450 | 442 | 0.635 [0.596, 0.673] (n=450) | 0.743 [0.702, 0.781] (n=450) | 0.883 [0.858, 0.906] (n=450) | 0.948 [0.928, 0.967] (n=450) |
| cells:es/formula | 197 | 0 | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) |
| cells:es/informal_to_formal | 136 | 1 | 0.162 [0.142, 0.184] (n=136) | 0.096 [0.041, 0.159] (n=136) | 0.131 [0.130, 0.131] (n=136) | 0.000 [0.000, 0.000] (n=136) |
| cells:es/text | 44 | 29 | 0.792 [0.696, 0.876] (n=44) | 0.716 [0.614, 0.807] (n=44) | 0.623 [0.476, 0.753] (n=44) | 0.591 [0.443, 0.716] (n=44) |
| by_generator:handwritten:handwritten | 27 | 0 | 0.416 [0.308, 0.529] (n=27) | 0.426 [0.327, 0.520] (n=27) | 0.000 [0.000, 0.000] (n=27) | 0.000 [0.000, 0.000] (n=27) |
| by_generator:handwritten:handwritten:trivial | 18 | 18 | 0.803 [0.757, 0.850] (n=18) | 0.611 [0.526, 0.719] (n=18) | 0.781 [0.714, 0.844] (n=18) | 0.611 [0.500, 0.737] (n=18) |
| by_generator:silver:docstring | 1 | 0 | 0.000 [0.000, 0.000] (n=1) | 0.000 [0.000, 0.000] (n=1) | 0.000 [0.000, 0.000] (n=1) | 0.000 [0.000, 0.000] (n=1) |
| by_generator:silver:docstring:trivial | 282 | 282 | 0.446 [0.395, 0.496] (n=282) | 0.606 [0.550, 0.663] (n=282) | 0.861 [0.828, 0.892] (n=282) | 0.947 [0.922, 0.972] (n=282) |
| by_generator:silver:formula | 349 | 0 | 0.000 [0.000, 0.000] (n=349) | 0.000 [0.000, 0.000] (n=349) | 0.000 [0.000, 0.000] (n=349) | 0.000 [0.000, 0.000] (n=349) |
| by_generator:silver:informal_to_formal | 181 | 0 | 0.175 [0.153, 0.197] (n=181) | 0.127 [0.066, 0.195] (n=181) | 0.131 [0.130, 0.131] (n=181) | 0.000 [0.000, 0.000] (n=181) |
| by_generator:silver:informal_to_formal:trivial | 137 | 137 | 0.305 [0.255, 0.357] (n=137) | 0.394 [0.296, 0.489] (n=137) | 0.324 [0.278, 0.372] (n=137) | 0.518 [0.418, 0.619] (n=137) |
| by_generator:silver:label_exact | 1 | 0 | 1.000 [1.000, 1.000] (n=1) | 1.000 [1.000, 1.000] (n=1) | 1.000 [1.000, 1.000] (n=1) | 1.000 [1.000, 1.000] (n=1) |
| by_generator:silver:label_exact:trivial | 635 | 635 | 0.992 [0.988, 0.997] (n=635) | 1.000 [1.000, 1.000] (n=635) | 0.978 [0.970, 0.985] (n=635) | 0.998 [0.995, 1.000] (n=635) |

## Recorridos

Ítems resueltos: 471; sin resolver: 3.

- `http`: estructura válida 1.000 [0.985, 1.000] (247/247); adversariales sin conclusión prohibida 1.000 [0.981, 1.000] (198/198); acierto en respondibles 0.865 [0.820, 0.901] (238/275); abstención en respondibles 0.120 [0.087, 0.164] (33/275); sobreafirmación vs oráculo 0.000 [0.000, 0.015] (0/247).
- `store`: estructura válida 0.847 [0.812, 0.877] (399/471); adversariales sin conclusión prohibida 0.030 [0.014, 0.065] (6/198); acierto en respondibles 0.738 [0.683, 0.787] (203/275); abstención en respondibles 0.000 [0.000, 0.014] (0/275); sobreafirmación vs oráculo 0.435 [0.391, 0.480] (205/471).

## Resolución de entidades

Pares resueltos: 654.  Abstención: 0.000 [0.000, 0.006] (0/654).  Fusiones del sistema muestreadas: 0 (etiquetadas: 0).

- falsa fusión en negativos `label_collision`: 0.007 [0.001, 0.037] (1/148)
- falsa fusión en negativos `random`: 0.000 [0.000, 0.021] (0/183)
- falsa fusión en negativos `related_not_same`: 0.000 [0.000, 0.024] (0/158)
- cobertura en positivos `alias (circular)`: 1.000 [0.973, 1.000] (139/139)
- cobertura en positivos `shared_external_id`: 0.000 [0.000, 0.129] (0/26)

## Relaciones

Muestra: 881; etiquetadas: 0; aún vigentes: 1.000 [0.996, 1.000] (881/881).

- corroboradas por otra fuente, familia `application`: 0.000 [0.000, 0.044] (0/83)
- corroboradas por otra fuente, familia `history`: 0.000 [0.000, 0.018] (0/206)
- corroboradas por otra fuente, familia `learning`: 0.000 [0.000, 0.057] (0/64)
- corroboradas por otra fuente, familia `proof`: 0.289 [0.229, 0.356] (56/194)
- corroboradas por otra fuente, familia `structure`: 0.000 [0.000, 0.011] (0/334)

## Procedencia

- `http`: completa y coincidente 1.000 [0.999, 1.000] (3266/3266); problemas {}
- `store`: completa y coincidente 1.000 [0.999, 1.000] (3266/3266); problemas {}

## Latencia (ms)

| Serie | n | p50 | p95 | IC p95 | p99 |
|---|---|---|---|---|---|
| http.entity | 3266 | 0.63 | 0.99 | [0.96, 1.02] | 2.00 |
| http.entity.load_c1 | 500 | 0.52 | 1.02 | [0.89, 1.32] | 2.04 |
| http.neighborhood.load_c1 | 500 | 0.39 | 0.61 | [0.58, 0.63] | 0.75 |
| http.path | 471 | 0.76 | 3.10 | [2.48, 3.33] | 12.95 |
| http.search | 1282 | 11.42 | 14.27 | [14.04, 14.53] | 15.64 |
| http.search.load_c1 | 500 | 9.77 | 12.16 | [11.88, 12.50] | 13.40 |
| http.search.load_c8 | 500 | 30.01 | 59.60 | [55.35, 63.95] | 74.05 |
| http.tiles.load_c1 | 500 | 1.35 | 2.73 | [2.64, 2.83] | 3.18 |
| store.entity | 3266 | 0.45 | 0.80 | [0.75, 0.86] | 2.03 |
| store.path | 471 | 0.65 | 16.68 | [10.39, 28.93] | 57.56 |
| store.search | 1631 | 3.30 | 15.02 | [13.79, 16.03] | 87.37 |
