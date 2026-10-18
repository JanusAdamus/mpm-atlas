# AtlasBench · reporte 2026-10-18T18:32:18

Protocolo: bench/PROTOCOL.md v1.3 (thresholds frozen 2026-10-16; clarifications 6.1-6.3).  Datos: `{'split': 'test', 'manifest': 'bench/manifest/v3.json'}`.  Registro: `{'path': 'C:/MPM/data/atlas.db', 'sqlite': 'C:/MPM/data/atlas.db', 'bytes': 5179146240, 'mtime_ns': 1792338329159387300, 'schema_version': 3, 'event_seq': 2943391}`.
Hardware: {'platform': 'Windows-11-10.0.26200-SP0', 'processor': 'Intel64 Family 6 Model 197 Stepping 2, GenuineIntel', 'logical_cpus': 16, 'ram_bytes': 33811132416, 'python': '3.14.6'}.  Sistema evaluado: `http`; baseline: `store` (vía léxica FTS5).  Aperturas del conjunto sellado: 0.

Evidencia por tarea: {'retrieval': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'route': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'er_pair': ['silver/silver_auto'], 'sourced_answer': ['silver/silver_auto'], 'relation': ['silver/unlabeled']}.  Ningún ítem es oro de especialistas salvo que diga `expert_adjudicated`; los veredictos sobre ítems silver o escritos por agente llevan `PROVISIONAL-`.

## Veredictos

| Componente | Veredicto | Regla | Detalle |
|---|---|---|---|
| retrieval | PROVISIONAL-FAIL | each lang x modality cell with n >= 30 (trivial silver excluded): delta nDCG@10 >= +0.05 and paired-bootstrap LB >= +0.02 vs lexical baseline | {'cells': {'en/formula': {'value': 0.016556009624328114, 'n': 163, 'clusters': 163, 'ci95': [0.004635707924068904, 0.03287653325202887], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}, 'en/informal_to_formal': {'value': 0.034805522702859436, 'n': 57, 'clusters': 46, 'ci95': [0.0080 |
| routes.structure | PASS | 0 structurally invalid steps in returned paths | {'invalid_paths': 0, 'answered': 264} |
| routes.adversarial | PROVISIONAL-PASS | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 188, 'n': 188, 'value': 1.0, 'ci95': [0.9799758673421861, 1.0], 'interval': 'wilson'}} |
| routes.end_to_end | NOT_EVALUABLE | Wilson LB > 0.90 of expert-valid top-1 routes (n >= 100) | no expert judgments of returned routes yet; route_outputs.jsonl is the queue |
| routes.null_model_adversarial | PROVISIONAL-FAIL | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 2, 'n': 188, 'value': 0.010638297872340425, 'ci95': [0.002922269566028632, 0.03795241346076797], 'interval': 'wilson'}} |
| er | NOT_EVALUABLE | Wilson LB of merge precision >= 0.99 on sampled system merges (n >= 381); strata n >= 50 point >= 0.99 | no labeled sample of system merges (needs experts) |
| relations | NOT_EVALUABLE | each priority family (proof, structure, application): Wilson LB of precision >= 0.97, n >= 125 | no labeled relation items (needs experts) |
| provenance.registry | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3608, 'value': 0.0, 'ci95': [0.0, 0.001063573488618297], 'interval': 'wilson'}} |
| provenance.served | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3608, 'value': 0.0, 'ci95': [0.0, 0.001063573488618297], 'interval': 'wilson'}} |
| latency.search_hot.c1 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 11.745979980332772, 'n': 500, 'clusters': None, 'ci95': [11.53081501543056, 12.037299980875105], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.search_hot.c8 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 62.23922497883905, 'n': 500, 'clusters': None, 'ci95': [57.040200015762764, 63.579350025975124], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.entity | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 2.5571349717210974, 'n': 500, 'clusters': None, 'ci95': [1.764989993534982, 3.6165549856377743], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.neighborhood | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 0.9646349935792365, 'n': 500, 'clusters': None, 'ci95': [0.6862349750008419, 1.2266599718714133], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.tiles | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 8.784980003838426, 'n': 500, 'clusters': None, 'ci95': [7.633799975155852, 15.189965011086295], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |

## Componentes ausentes


## Recuperación (nDCG@10, IC 95% bootstrap por familia)

Ítems resueltos: 1593; sin resolver en el registro: 0.

| Grupo | n | triviales | http nDCG@10 | http recall@10 | store nDCG@10 | store recall@10 |
|---|---|---|---|---|---|---|
| cells:en/formula | 163 | 2 | 0.018 [0.005, 0.035] (n=163) | 0.028 [0.009, 0.055] (n=163) | 0.001 [0.000, 0.003] (n=163) | 0.006 [0.000, 0.015] (n=163) |
| cells:en/informal_to_formal | 215 | 158 | 0.316 [0.279, 0.351] (n=215) | 0.460 [0.380, 0.534] (n=215) | 0.275 [0.243, 0.311] (n=215) | 0.391 [0.318, 0.471] (n=215) |
| cells:en/lean_name | 377 | 375 | 0.996 [0.993, 0.999] (n=377) | 0.999 [0.996, 1.000] (n=377) | 0.981 [0.971, 0.989] (n=377) | 0.999 [0.996, 1.000] (n=377) |
| cells:en/text | 420 | 415 | 0.771 [0.735, 0.807] (n=420) | 0.840 [0.806, 0.875] (n=420) | 0.892 [0.869, 0.914] (n=420) | 0.967 [0.949, 0.982] (n=420) |
| cells:es/formula | 197 | 0 | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) |
| cells:es/informal_to_formal | 180 | 1 | 0.163 [0.145, 0.182] (n=180) | 0.089 [0.044, 0.140] (n=180) | 0.132 [0.129, 0.137] (n=180) | 0.006 [0.000, 0.021] (n=180) |
| cells:es/text | 41 | 33 | 0.861 [0.776, 0.931] (n=41) | 0.817 [0.720, 0.902] (n=41) | 0.769 [0.636, 0.882] (n=41) | 0.720 [0.585, 0.841] (n=41) |
| by_generator:handwritten:handwritten | 19 | 0 | 0.361 [0.256, 0.482] (n=19) | 0.474 [0.353, 0.583] (n=19) | 0.000 [0.000, 0.000] (n=19) | 0.000 [0.000, 0.000] (n=19) |
| by_generator:handwritten:handwritten:trivial | 13 | 13 | 0.680 [0.567, 0.779] (n=13) | 0.577 [0.500, 0.679] (n=13) | 0.624 [0.507, 0.757] (n=13) | 0.538 [0.500, 0.636] (n=13) |
| by_generator:silver:docstring:trivial | 260 | 260 | 0.657 [0.606, 0.707] (n=260) | 0.758 [0.708, 0.808] (n=260) | 0.880 [0.849, 0.908] (n=260) | 0.973 [0.950, 0.992] (n=260) |
| by_generator:silver:formula | 352 | 0 | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) |
| by_generator:silver:informal_to_formal | 236 | 0 | 0.161 [0.146, 0.181] (n=236) | 0.089 [0.047, 0.141] (n=236) | 0.129 [0.128, 0.130] (n=236) | 0.000 [0.000, 0.000] (n=236) |
| by_generator:silver:informal_to_formal:trivial | 159 | 159 | 0.372 [0.329, 0.417] (n=159) | 0.591 [0.500, 0.685] (n=159) | 0.330 [0.290, 0.374] (n=159) | 0.535 [0.442, 0.632] (n=159) |
| by_generator:silver:label_exact | 2 | 0 | 1.000 [1.000, 1.000] (n=2) | 1.000 [1.000, 1.000] (n=2) | 1.000 [1.000, 1.000] (n=2) | 1.000 [1.000, 1.000] (n=2) |
| by_generator:silver:label_exact:trivial | 552 | 552 | 0.995 [0.992, 0.999] (n=552) | 1.000 [1.000, 1.000] (n=552) | 0.973 [0.963, 0.982] (n=552) | 0.998 [0.995, 1.000] (n=552) |

## Recorridos

Ítems resueltos: 449; sin resolver: 13.

- `http`: estructura válida 1.000 [0.986, 1.000] (264/264); adversariales sin conclusión prohibida 1.000 [0.980, 1.000] (188/188); acierto en respondibles 0.996 [0.979, 0.999] (260/261); abstención en respondibles 0.004 [0.001, 0.021] (1/261); sobreafirmación vs oráculo 0.000 [0.000, 0.014] (0/264).
- `store`: estructura válida 0.824 [0.786, 0.856] (370/449); adversariales sin conclusión prohibida 0.011 [0.003, 0.038] (2/188); acierto en respondibles 0.759 [0.703, 0.807] (198/261); abstención en respondibles 0.000 [0.000, 0.015] (0/261); sobreafirmación vs oráculo 0.432 [0.387, 0.478] (194/449).

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
| http.entity | 3608 | 0.65 | 1.80 | [1.58, 2.05] | 7.10 |
| http.entity.load_c1 | 500 | 0.59 | 2.56 | [1.76, 3.62] | 7.12 |
| http.neighborhood.load_c1 | 500 | 0.36 | 0.96 | [0.69, 1.23] | 1.68 |
| http.path | 449 | 1.17 | 3.24 | [3.01, 3.71] | 6.07 |
| http.search | 1241 | 11.69 | 14.46 | [14.31, 14.70] | 16.15 |
| http.search.load_c1 | 500 | 9.23 | 11.75 | [11.53, 12.04] | 12.64 |
| http.search.load_c8 | 500 | 29.16 | 62.24 | [57.04, 63.58] | 69.53 |
| http.tiles.load_c1 | 500 | 2.56 | 8.78 | [7.63, 15.19] | 20.60 |
| store.entity | 3608 | 0.53 | 5.73 | [4.71, 6.70] | 52.21 |
| store.path | 449 | 1.31 | 16.52 | [14.19, 19.52] | 31.56 |
| store.search | 1593 | 3.99 | 18.98 | [17.47, 24.03] | 89.99 |
