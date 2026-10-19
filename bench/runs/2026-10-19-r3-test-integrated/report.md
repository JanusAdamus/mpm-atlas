# AtlasBench · reporte 2026-10-19T14:53:47

Protocolo: bench/PROTOCOL.md v1.3 (thresholds frozen 2026-10-16; clarifications 6.1-6.3).  Datos: `{'split': 'test', 'manifest': 'bench/manifest/v3.json'}`.  Registro: `{'path': 'C:/MPM/data/atlas.db', 'sqlite': 'C:/MPM/data/atlas.db', 'bytes': 6167932928, 'mtime_ns': 1792378298176709600, 'schema_version': 3, 'event_seq': 3863314}`.
Hardware: {'platform': 'Windows-11-10.0.26200-SP0', 'processor': 'Intel64 Family 6 Model 197 Stepping 2, GenuineIntel', 'logical_cpus': 16, 'ram_bytes': 33811132416, 'python': '3.14.6'}.  Sistema evaluado: `http`; baseline: `store` (vía léxica FTS5).  Aperturas del conjunto sellado: 0.

Evidencia por tarea: {'retrieval': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'route': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'er_pair': ['silver/silver_auto'], 'sourced_answer': ['silver/silver_auto'], 'relation': ['silver/unlabeled']}.  Ningún ítem es oro de especialistas salvo que diga `expert_adjudicated`; los veredictos sobre ítems silver o escritos por agente llevan `PROVISIONAL-`.

## Veredictos

| Componente | Veredicto | Regla | Detalle |
|---|---|---|---|
| retrieval | PROVISIONAL-FAIL | each lang x modality cell with n >= 30 (trivial silver excluded): delta nDCG@10 >= +0.05 and paired-bootstrap LB >= +0.02 vs lexical baseline | {'cells': {'en/formula': {'value': 0.016556009624328114, 'n': 163, 'clusters': 163, 'ci95': [0.004635707924068904, 0.03287653325202887], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}, 'en/informal_to_formal': {'value': 0.05280126505054374, 'n': 57, 'clusters': 46, 'ci95': [0.01899 |
| routes.structure | PASS | 0 structurally invalid steps in returned paths | {'invalid_paths': 0, 'answered': 264} |
| routes.adversarial | PROVISIONAL-PASS | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 188, 'n': 188, 'value': 1.0, 'ci95': [0.9799758673421861, 1.0], 'interval': 'wilson'}} |
| routes.end_to_end | NOT_EVALUABLE | Wilson LB > 0.90 of expert-valid top-1 routes (n >= 100) | no expert judgments of returned routes yet; route_outputs.jsonl is the queue |
| routes.null_model_adversarial | PROVISIONAL-FAIL | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 5, 'n': 188, 'value': 0.026595744680851064, 'ci95': [0.011412446673968294, 0.060738061906302335], 'interval': 'wilson'}} |
| er | NOT_EVALUABLE | Wilson LB of merge precision >= 0.99 on sampled system merges (n >= 381); strata n >= 50 point >= 0.99 | no labeled sample of system merges (needs experts) |
| relations | NOT_EVALUABLE | each priority family (proof, structure, application): Wilson LB of precision >= 0.97, n >= 125 | no labeled relation items (needs experts) |
| provenance.registry | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3608, 'value': 0.0, 'ci95': [0.0, 0.001063573488618297], 'interval': 'wilson'}} |
| provenance.served | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3608, 'value': 0.0, 'ci95': [0.0, 0.001063573488618297], 'interval': 'wilson'}} |
| latency.search_hot.c1 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 28.043250003247515, 'n': 500, 'clusters': None, 'ci95': [26.727624994964568, 29.366965005101495], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.search_hot.c8 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 92.8160100007517, 'n': 500, 'clusters': None, 'ci95': [88.00889999838546, 96.40957999981761], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.entity | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 6.5285549997497565, 'n': 500, 'clusters': None, 'ci95': [5.7623049960966455, 8.328080005230726], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.neighborhood | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 5.617785002687008, 'n': 500, 'clusters': None, 'ci95': [4.4297072538211, 6.2928900038969], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.tiles | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 22.611809995214564, 'n': 500, 'clusters': None, 'ci95': [19.705200000316836, 27.445100000477396], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |

## Componentes ausentes


## Recuperación (nDCG@10, IC 95% bootstrap por familia)

Ítems resueltos: 1593; sin resolver en el registro: 0.

| Grupo | n | triviales | http nDCG@10 | http recall@10 | store nDCG@10 | store recall@10 |
|---|---|---|---|---|---|---|
| cells:en/formula | 163 | 2 | 0.018 [0.005, 0.035] (n=163) | 0.028 [0.009, 0.055] (n=163) | 0.001 [0.000, 0.003] (n=163) | 0.006 [0.000, 0.015] (n=163) |
| cells:en/informal_to_formal | 215 | 158 | 0.318 [0.281, 0.353] (n=215) | 0.456 [0.374, 0.535] (n=215) | 0.275 [0.243, 0.311] (n=215) | 0.391 [0.318, 0.471] (n=215) |
| cells:en/lean_name | 377 | 375 | 0.996 [0.993, 0.999] (n=377) | 0.999 [0.996, 1.000] (n=377) | 0.981 [0.971, 0.989] (n=377) | 0.999 [0.996, 1.000] (n=377) |
| cells:en/text | 420 | 415 | 0.816 [0.783, 0.847] (n=420) | 0.896 [0.866, 0.925] (n=420) | 0.892 [0.869, 0.914] (n=420) | 0.967 [0.949, 0.982] (n=420) |
| cells:es/formula | 197 | 0 | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) |
| cells:es/informal_to_formal | 180 | 1 | 0.165 [0.147, 0.184] (n=180) | 0.094 [0.045, 0.149] (n=180) | 0.132 [0.129, 0.137] (n=180) | 0.006 [0.000, 0.021] (n=180) |
| cells:es/text | 41 | 33 | 0.889 [0.832, 0.937] (n=41) | 0.841 [0.768, 0.915] (n=41) | 0.769 [0.636, 0.882] (n=41) | 0.720 [0.585, 0.841] (n=41) |
| by_generator:handwritten:handwritten | 19 | 0 | 0.428 [0.316, 0.545] (n=19) | 0.553 [0.447, 0.650] (n=19) | 0.000 [0.000, 0.000] (n=19) | 0.000 [0.000, 0.000] (n=19) |
| by_generator:handwritten:handwritten:trivial | 13 | 13 | 0.678 [0.566, 0.775] (n=13) | 0.577 [0.500, 0.679] (n=13) | 0.624 [0.507, 0.757] (n=13) | 0.538 [0.500, 0.636] (n=13) |
| by_generator:silver:docstring:trivial | 260 | 260 | 0.730 [0.684, 0.774] (n=260) | 0.846 [0.804, 0.888] (n=260) | 0.880 [0.849, 0.908] (n=260) | 0.973 [0.950, 0.992] (n=260) |
| by_generator:silver:formula | 352 | 0 | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) |
| by_generator:silver:informal_to_formal | 236 | 0 | 0.168 [0.152, 0.189] (n=236) | 0.106 [0.061, 0.161] (n=236) | 0.129 [0.128, 0.130] (n=236) | 0.000 [0.000, 0.000] (n=236) |
| by_generator:silver:informal_to_formal:trivial | 159 | 159 | 0.367 [0.327, 0.409] (n=159) | 0.566 [0.479, 0.658] (n=159) | 0.330 [0.290, 0.374] (n=159) | 0.535 [0.442, 0.632] (n=159) |
| by_generator:silver:label_exact | 2 | 0 | 1.000 [1.000, 1.000] (n=2) | 1.000 [1.000, 1.000] (n=2) | 1.000 [1.000, 1.000] (n=2) | 1.000 [1.000, 1.000] (n=2) |
| by_generator:silver:label_exact:trivial | 552 | 552 | 0.995 [0.990, 0.999] (n=552) | 1.000 [1.000, 1.000] (n=552) | 0.973 [0.963, 0.982] (n=552) | 0.998 [0.995, 1.000] (n=552) |

## Recorridos

Ítems resueltos: 449; sin resolver: 13.

- `http`: estructura válida 1.000 [0.986, 1.000] (264/264); adversariales sin conclusión prohibida 1.000 [0.980, 1.000] (188/188); acierto en respondibles 0.996 [0.979, 0.999] (260/261); abstención en respondibles 0.004 [0.001, 0.021] (1/261); sobreafirmación vs oráculo 0.000 [0.000, 0.014] (0/264).
- `store`: estructura válida 0.824 [0.786, 0.856] (370/449); adversariales sin conclusión prohibida 0.027 [0.011, 0.061] (5/188); acierto en respondibles 0.759 [0.703, 0.807] (198/261); abstención en respondibles 0.000 [0.000, 0.015] (0/261); sobreafirmación vs oráculo 0.425 [0.380, 0.472] (191/449).

## Resolución de entidades

Pares resueltos: 655.  Abstención: 0.000 [0.000, 0.006] (0/655).  Fusiones del sistema muestreadas: 0 (etiquetadas: 0).

- falsa fusión en negativos `label_collision`: 0.006 [0.001, 0.035] (1/159)
- falsa fusión en negativos `random`: 0.000 [0.000, 0.026] (0/142)
- falsa fusión en negativos `related_not_same`: 0.000 [0.000, 0.026] (0/146)
- cobertura en positivos `alias (circular)`: 1.000 [0.980, 1.000] (185/185)
- cobertura en positivos `shared_external_id`: 0.000 [0.000, 0.143] (0/23)

## Relaciones

Muestra: 1074; etiquetadas: 0; aún vigentes: 1.000 [0.996, 1.000] (1074/1074).

- corroboradas por otra fuente, familia `application`: 0.000 [0.000, 0.051] (0/72)
- corroboradas por otra fuente, familia `history`: 0.000 [0.000, 0.012] (0/305)
- corroboradas por otra fuente, familia `learning`: 0.000 [0.000, 0.041] (0/89)
- corroboradas por otra fuente, familia `proof`: 0.300 [0.243, 0.364] (65/217)
- corroboradas por otra fuente, familia `structure`: 0.000 [0.000, 0.010] (0/391)

## Procedencia

- `http`: completa y coincidente 1.000 [0.999, 1.000] (3608/3608); problemas {}
- `store`: completa y coincidente 1.000 [0.999, 1.000] (3608/3608); problemas {}

## Latencia (ms)

| Serie | n | p50 | p95 | IC p95 | p99 |
|---|---|---|---|---|---|
| http.entity | 3608 | 3.05 | 6.33 | [5.94, 6.79] | 18.04 |
| http.entity.load_c1 | 500 | 2.83 | 6.53 | [5.76, 8.33] | 16.43 |
| http.neighborhood.load_c1 | 500 | 2.52 | 5.62 | [4.43, 6.29] | 8.71 |
| http.path | 449 | 4.29 | 9.93 | [8.45, 10.95] | 14.06 |
| http.search | 1241 | 23.27 | 34.17 | [33.42, 34.88] | 38.08 |
| http.search.load_c1 | 500 | 19.20 | 28.04 | [26.73, 29.37] | 34.39 |
| http.search.load_c8 | 500 | 47.17 | 92.82 | [88.01, 96.41] | 125.37 |
| http.tiles.load_c1 | 500 | 6.77 | 22.61 | [19.71, 27.45] | 32.78 |
| store.entity | 3608 | 0.59 | 7.78 | [6.37, 9.89] | 60.73 |
| store.path | 449 | 1.40 | 20.95 | [15.74, 23.23] | 42.93 |
| store.search | 1593 | 6.81 | 37.08 | [35.69, 43.49] | 124.14 |
