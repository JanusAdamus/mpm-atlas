# AtlasBench · reporte 2026-10-20T19:46:27

Protocolo: bench/PROTOCOL.md v1.3 (thresholds frozen 2026-10-16; clarifications 6.1-6.3).  Datos: `{'split': 'test', 'manifest': 'bench/manifest/v3.json'}`.  Registro: `{'path': 'C:/MPM/data/atlas.db', 'sqlite': 'C:/MPM/data/atlas.db', 'bytes': 6346358784, 'mtime_ns': 1792518554766496100, 'schema_version': 4, 'event_seq': 4163350}`.
Hardware: {'platform': 'Windows-11-10.0.26200-SP0', 'processor': 'Intel64 Family 6 Model 197 Stepping 2, GenuineIntel', 'logical_cpus': 16, 'ram_bytes': 33811132416, 'python': '3.14.6'}.  Sistema evaluado: `http`; baseline: `store` (vía léxica FTS5).  Aperturas del conjunto sellado: 0.

Evidencia por tarea: {'retrieval': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'route': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'er_pair': ['silver/silver_auto'], 'sourced_answer': ['silver/silver_auto'], 'relation': ['silver/unlabeled']}.  Ningún ítem es oro de especialistas salvo que diga `expert_adjudicated`; los veredictos sobre ítems silver o escritos por agente llevan `PROVISIONAL-`.

## Veredictos

| Componente | Veredicto | Regla | Detalle |
|---|---|---|---|
| retrieval | PROVISIONAL-FAIL | each lang x modality cell with n >= 30 (trivial silver excluded): delta nDCG@10 >= +0.05 and paired-bootstrap LB >= +0.02 vs lexical baseline | {'cells': {'en/formula': {'value': 0.016556009624328114, 'n': 163, 'clusters': 163, 'ci95': [0.004635707924068904, 0.03287653325202887], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}, 'en/informal_to_formal': {'value': 0.08066618814624495, 'n': 52, 'clusters': 41, 'ci95': [0.03341 |
| routes.structure | PASS | 0 structurally invalid steps in returned paths | {'invalid_paths': 0, 'answered': 252} |
| routes.adversarial | PROVISIONAL-PASS | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 188, 'n': 188, 'value': 1.0, 'ci95': [0.9799758673421861, 1.0], 'interval': 'wilson'}} |
| routes.end_to_end | NOT_EVALUABLE | Wilson LB > 0.90 of expert-valid top-1 routes (n >= 100) | no expert judgments of returned routes yet; route_outputs.jsonl is the queue |
| routes.null_model_adversarial | PROVISIONAL-FAIL | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 5, 'n': 188, 'value': 0.026595744680851064, 'ci95': [0.011412446673968294, 0.060738061906302335], 'interval': 'wilson'}} |
| er | NOT_EVALUABLE | Wilson LB of merge precision >= 0.99 on sampled system merges (n >= 381); strata n >= 50 point >= 0.99 | no labeled sample of system merges (needs experts) |
| relations | NOT_EVALUABLE | each priority family (proof, structure, application): Wilson LB of precision >= 0.97, n >= 125 | no labeled relation items (needs experts) |
| provenance.registry | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3608, 'value': 0.0, 'ci95': [0.0, 0.001063573488618297], 'interval': 'wilson'}} |
| provenance.served | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3608, 'value': 0.0, 'ci95': [0.0, 0.001063573488618297], 'interval': 'wilson'}} |
| latency.search_hot.c1 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 17.70152999379206, 'n': 500, 'clusters': None, 'ci95': [16.759240003011648, 18.157834994781286], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.search_hot.c8 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 111.75275499845152, 'n': 500, 'clusters': None, 'ci95': [99.43762449613132, 118.10990000958554], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.entity | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 2.9062200010230095, 'n': 500, 'clusters': None, 'ci95': [2.1147350038518176, 4.0107600048941086], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.neighborhood | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 1.0101799940457514, 'n': 500, 'clusters': None, 'ci95': [0.6898288818956618, 1.4009849961439607], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.tiles | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 3.2217300009506276, 'n': 500, 'clusters': None, 'ci95': [2.977184995688729, 3.499189999274675], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |

## Componentes ausentes


## Recuperación (nDCG@10, IC 95% bootstrap por familia)

Ítems resueltos: 1593; sin resolver en el registro: 0.

| Grupo | n | triviales | http nDCG@10 | http recall@10 | store nDCG@10 | store recall@10 |
|---|---|---|---|---|---|---|
| cells:en/formula | 163 | 2 | 0.018 [0.005, 0.035] (n=163) | 0.028 [0.009, 0.055] (n=163) | 0.001 [0.000, 0.003] (n=163) | 0.006 [0.000, 0.015] (n=163) |
| cells:en/informal_to_formal | 215 | 163 | 0.425 [0.384, 0.465] (n=215) | 0.605 [0.526, 0.678] (n=215) | 0.308 [0.273, 0.347] (n=215) | 0.465 [0.388, 0.546] (n=215) |
| cells:en/lean_name | 377 | 375 | 0.996 [0.993, 0.999] (n=377) | 0.999 [0.996, 1.000] (n=377) | 0.981 [0.971, 0.989] (n=377) | 0.999 [0.996, 1.000] (n=377) |
| cells:en/text | 420 | 415 | 0.816 [0.783, 0.847] (n=420) | 0.894 [0.863, 0.923] (n=420) | 0.892 [0.869, 0.914] (n=420) | 0.967 [0.949, 0.982] (n=420) |
| cells:es/formula | 197 | 0 | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) |
| cells:es/informal_to_formal | 180 | 1 | 0.176 [0.156, 0.201] (n=180) | 0.122 [0.069, 0.182] (n=180) | 0.132 [0.129, 0.137] (n=180) | 0.006 [0.000, 0.021] (n=180) |
| cells:es/text | 41 | 33 | 0.895 [0.842, 0.942] (n=41) | 0.841 [0.768, 0.915] (n=41) | 0.769 [0.636, 0.882] (n=41) | 0.720 [0.585, 0.841] (n=41) |
| by_generator:handwritten:handwritten | 19 | 0 | 0.452 [0.338, 0.571] (n=19) | 0.553 [0.447, 0.650] (n=19) | 0.000 [0.000, 0.000] (n=19) | 0.000 [0.000, 0.000] (n=19) |
| by_generator:handwritten:handwritten:trivial | 13 | 13 | 0.677 [0.565, 0.775] (n=13) | 0.577 [0.500, 0.679] (n=13) | 0.623 [0.506, 0.755] (n=13) | 0.538 [0.500, 0.636] (n=13) |
| by_generator:silver:docstring:trivial | 260 | 260 | 0.727 [0.681, 0.772] (n=260) | 0.842 [0.796, 0.885] (n=260) | 0.880 [0.849, 0.908] (n=260) | 0.973 [0.950, 0.992] (n=260) |
| by_generator:silver:formula | 352 | 0 | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) |
| by_generator:silver:informal_to_formal | 231 | 0 | 0.183 [0.163, 0.206] (n=231) | 0.143 [0.087, 0.206] (n=231) | 0.129 [0.128, 0.130] (n=231) | 0.000 [0.000, 0.000] (n=231) |
| by_generator:silver:informal_to_formal:trivial | 164 | 164 | 0.492 [0.448, 0.538] (n=164) | 0.726 [0.646, 0.807] (n=164) | 0.368 [0.327, 0.410] (n=164) | 0.616 [0.527, 0.711] (n=164) |
| by_generator:silver:label_exact | 2 | 0 | 1.000 [1.000, 1.000] (n=2) | 1.000 [1.000, 1.000] (n=2) | 1.000 [1.000, 1.000] (n=2) | 1.000 [1.000, 1.000] (n=2) |
| by_generator:silver:label_exact:trivial | 552 | 552 | 0.995 [0.992, 0.999] (n=552) | 1.000 [1.000, 1.000] (n=552) | 0.973 [0.963, 0.982] (n=552) | 0.998 [0.995, 1.000] (n=552) |

## Recorridos

Ítems resueltos: 449; sin resolver: 13.

- `http`: estructura válida 1.000 [0.985, 1.000] (252/252); adversariales sin conclusión prohibida 1.000 [0.980, 1.000] (188/188); acierto en respondibles 0.831 [0.781, 0.872] (217/261); abstención en respondibles 0.046 [0.026, 0.079] (12/261); sobreafirmación vs oráculo 0.000 [0.000, 0.015] (0/252).
- `store`: estructura válida 0.824 [0.786, 0.856] (370/449); adversariales sin conclusión prohibida 0.027 [0.011, 0.061] (5/188); acierto en respondibles 0.598 [0.537, 0.655] (156/261); abstención en respondibles 0.000 [0.000, 0.015] (0/261); sobreafirmación vs oráculo 0.443 [0.398, 0.489] (199/449).

## Resolución de entidades

Pares resueltos: 655.  Abstención: 0.000 [0.000, 0.006] (0/655).  Fusiones del sistema muestreadas: 0 (etiquetadas: 0).

- falsa fusión en negativos `label_collision`: 0.006 [0.001, 0.035] (1/159)
- falsa fusión en negativos `random`: 0.000 [0.000, 0.026] (0/142)
- falsa fusión en negativos `related_not_same`: 0.000 [0.000, 0.026] (0/146)
- cobertura en positivos `alias (circular)`: 1.000 [0.980, 1.000] (185/185)
- cobertura en positivos `shared_external_id`: 0.000 [0.000, 0.143] (0/23)

## Relaciones

Muestra: 1074; etiquetadas: 0; aún vigentes: 0.877 [0.856, 0.895] (942/1074).

- corroboradas por otra fuente, familia `application`: 0.000 [0.000, 0.051] (0/72)
- corroboradas por otra fuente, familia `history`: 0.000 [0.000, 0.012] (0/305)
- corroboradas por otra fuente, familia `learning`: 0.000 [0.000, 0.041] (0/89)
- corroboradas por otra fuente, familia `proof`: 0.300 [0.243, 0.364] (65/217)
- corroboradas por otra fuente, familia `structure`: 0.000 [0.000, 0.010] (0/391)

## Procedencia

- `http`: completa y coincidente 1.000 [0.999, 1.000] (3608/3608); problemas {}; vía sucesora vigente (misma fuente y localizador) {'ok': 338, 'failed': 0}
- `store`: completa y coincidente 1.000 [0.999, 1.000] (3608/3608); problemas {}; vía sucesora vigente (misma fuente y localizador) {'ok': 338, 'failed': 0}

## Latencia (ms)

| Serie | n | p50 | p95 | IC p95 | p99 |
|---|---|---|---|---|---|
| http.entity | 3608 | 0.71 | 2.55 | [2.30, 2.83] | 9.46 |
| http.entity.load_c1 | 500 | 0.74 | 2.91 | [2.11, 4.01] | 10.73 |
| http.neighborhood.load_c1 | 500 | 0.40 | 1.01 | [0.69, 1.40] | 1.98 |
| http.path | 449 | 1.32 | 3.59 | [3.15, 4.24] | 6.81 |
| http.search | 1241 | 13.69 | 18.02 | [17.42, 18.34] | 22.95 |
| http.search.load_c1 | 500 | 12.46 | 17.70 | [16.76, 18.16] | 23.62 |
| http.search.load_c8 | 500 | 43.21 | 111.75 | [99.44, 118.11] | 136.77 |
| http.tiles.load_c1 | 500 | 1.78 | 3.22 | [2.98, 3.50] | 4.08 |
| store.entity | 3608 | 0.34 | 6.37 | [4.51, 7.46] | 53.06 |
| store.path | 449 | 1.36 | 15.21 | [12.62, 19.15] | 30.21 |
| store.search | 1593 | 5.43 | 27.07 | [24.11, 37.51] | 108.55 |
