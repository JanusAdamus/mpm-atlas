# AtlasBench · reporte 2026-10-18T00:39:49

Protocolo: bench/PROTOCOL.md v1.3 (thresholds frozen 2026-10-16; clarifications 6.1-6.3).  Datos: `{'split': 'test', 'manifest': 'bench/manifest/v3.json'}`.  Registro: `{'path': 'C:/MPM/data/atlas.db', 'sqlite': 'C:/MPM/data/atlas.db', 'bytes': 3632160768, 'mtime_ns': 1792279714524870400, 'schema_version': 1, 'event_seq': 2049417}`.
Hardware: {'platform': 'Windows-11-10.0.26200-SP0', 'processor': 'Intel64 Family 6 Model 197 Stepping 2, GenuineIntel', 'logical_cpus': 16, 'ram_bytes': 33811132416, 'python': '3.14.6'}.  Sistema evaluado: `http`; baseline: `store` (vía léxica FTS5).  Aperturas del conjunto sellado: 0.

Evidencia por tarea: {'retrieval': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'route': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'er_pair': ['silver/silver_auto'], 'sourced_answer': ['silver/silver_auto'], 'relation': ['silver/unlabeled']}.  Ningún ítem es oro de especialistas salvo que diga `expert_adjudicated`; los veredictos sobre ítems silver o escritos por agente llevan `PROVISIONAL-`.

## Veredictos

| Componente | Veredicto | Regla | Detalle |
|---|---|---|---|
| retrieval | PROVISIONAL-FAIL | each lang x modality cell with n >= 30 (trivial silver excluded): delta nDCG@10 >= +0.05 and paired-bootstrap LB >= +0.02 vs lexical baseline | {'cells': {'en/formula': {'value': 0.016556009624328114, 'n': 163, 'clusters': 163, 'ci95': [0.004635707924068904, 0.03287653325202887], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}, 'en/informal_to_formal': {'value': 0.04577460210660775, 'n': 57, 'clusters': 46, 'ci95': [0.01297 |
| routes.structure | PASS | 0 structurally invalid steps in returned paths | {'invalid_paths': 0, 'answered': 224} |
| routes.adversarial | PROVISIONAL-PASS | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 188, 'n': 188, 'value': 1.0, 'ci95': [0.9799758673421861, 1.0], 'interval': 'wilson'}} |
| routes.end_to_end | NOT_EVALUABLE | Wilson LB > 0.90 of expert-valid top-1 routes (n >= 100) | no expert judgments of returned routes yet; route_outputs.jsonl is the queue |
| routes.null_model_adversarial | PROVISIONAL-FAIL | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 1, 'n': 188, 'value': 0.005319148936170213, 'ci95': [0.0009395783080078223, 0.02950982953429744], 'interval': 'wilson'}} |
| er | NOT_EVALUABLE | Wilson LB of merge precision >= 0.99 on sampled system merges (n >= 381); strata n >= 50 point >= 0.99 | no labeled sample of system merges (needs experts) |
| relations | NOT_EVALUABLE | each priority family (proof, structure, application): Wilson LB of precision >= 0.97, n >= 125 | no labeled relation items (needs experts) |
| provenance.registry | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3608, 'value': 0.0, 'ci95': [0.0, 0.001063573488618297], 'interval': 'wilson'}} |
| provenance.served | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3608, 'value': 0.0, 'ci95': [0.0, 0.001063573488618297], 'interval': 'wilson'}} |
| latency.search_hot.c1 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 11.857720004627481, 'n': 500, 'clusters': None, 'ci95': [11.525520018767564, 12.20852004480548], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.search_hot.c8 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 56.87961994553914, 'n': 500, 'clusters': None, 'ci95': [54.67680502042637, 63.610939998761744], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.entity | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 1.0996400087606115, 'n': 500, 'clusters': None, 'ci95': [0.9088999941013753, 1.7346999957226217], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.neighborhood | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 0.5214500095462423, 'n': 500, 'clusters': None, 'ci95': [0.48308495315723116, 0.5608000210486352], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.tiles | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 2.5697350094560534, 'n': 500, 'clusters': None, 'ci95': [2.509959961753338, 2.6619982605916443], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |

## Componentes ausentes


## Recuperación (nDCG@10, IC 95% bootstrap por familia)

Ítems resueltos: 1593; sin resolver en el registro: 0.

| Grupo | n | triviales | http nDCG@10 | http recall@10 | store nDCG@10 | store recall@10 |
|---|---|---|---|---|---|---|
| cells:en/formula | 163 | 2 | 0.018 [0.005, 0.035] (n=163) | 0.028 [0.009, 0.055] (n=163) | 0.001 [0.000, 0.003] (n=163) | 0.006 [0.000, 0.015] (n=163) |
| cells:en/informal_to_formal | 215 | 158 | 0.315 [0.279, 0.351] (n=215) | 0.460 [0.381, 0.536] (n=215) | 0.275 [0.243, 0.311] (n=215) | 0.391 [0.318, 0.471] (n=215) |
| cells:en/lean_name | 377 | 375 | 0.996 [0.993, 0.999] (n=377) | 0.999 [0.996, 1.000] (n=377) | 0.981 [0.971, 0.989] (n=377) | 0.999 [0.996, 1.000] (n=377) |
| cells:en/text | 420 | 415 | 0.787 [0.752, 0.822] (n=420) | 0.861 [0.827, 0.894] (n=420) | 0.892 [0.869, 0.914] (n=420) | 0.967 [0.949, 0.982] (n=420) |
| cells:es/formula | 197 | 0 | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) |
| cells:es/informal_to_formal | 180 | 1 | 0.163 [0.145, 0.182] (n=180) | 0.089 [0.044, 0.140] (n=180) | 0.132 [0.129, 0.137] (n=180) | 0.006 [0.000, 0.021] (n=180) |
| cells:es/text | 41 | 33 | 0.878 [0.804, 0.937] (n=41) | 0.817 [0.732, 0.890] (n=41) | 0.769 [0.636, 0.882] (n=41) | 0.720 [0.585, 0.841] (n=41) |
| by_generator:handwritten:handwritten | 19 | 0 | 0.405 [0.284, 0.527] (n=19) | 0.500 [0.368, 0.625] (n=19) | 0.000 [0.000, 0.000] (n=19) | 0.000 [0.000, 0.000] (n=19) |
| by_generator:handwritten:handwritten:trivial | 13 | 13 | 0.675 [0.563, 0.773] (n=13) | 0.577 [0.500, 0.679] (n=13) | 0.624 [0.507, 0.757] (n=13) | 0.538 [0.500, 0.636] (n=13) |
| by_generator:silver:docstring:trivial | 260 | 260 | 0.680 [0.632, 0.728] (n=260) | 0.788 [0.738, 0.835] (n=260) | 0.880 [0.849, 0.908] (n=260) | 0.973 [0.950, 0.992] (n=260) |
| by_generator:silver:formula | 352 | 0 | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) |
| by_generator:silver:informal_to_formal | 236 | 0 | 0.164 [0.148, 0.184] (n=236) | 0.093 [0.051, 0.145] (n=236) | 0.129 [0.128, 0.130] (n=236) | 0.000 [0.000, 0.000] (n=236) |
| by_generator:silver:informal_to_formal:trivial | 159 | 159 | 0.367 [0.326, 0.411] (n=159) | 0.585 [0.494, 0.673] (n=159) | 0.330 [0.290, 0.374] (n=159) | 0.535 [0.442, 0.632] (n=159) |
| by_generator:silver:label_exact | 2 | 0 | 1.000 [1.000, 1.000] (n=2) | 1.000 [1.000, 1.000] (n=2) | 1.000 [1.000, 1.000] (n=2) | 1.000 [1.000, 1.000] (n=2) |
| by_generator:silver:label_exact:trivial | 552 | 552 | 0.996 [0.993, 0.999] (n=552) | 1.000 [1.000, 1.000] (n=552) | 0.973 [0.963, 0.982] (n=552) | 0.998 [0.995, 1.000] (n=552) |

## Recorridos

Ítems resueltos: 449; sin resolver: 13.

- `http`: estructura válida 1.000 [0.983, 1.000] (224/224); adversariales sin conclusión prohibida 1.000 [0.980, 1.000] (188/188); acierto en respondibles 0.847 [0.798, 0.885] (221/261); abstención en respondibles 0.153 [0.115, 0.202] (40/261); sobreafirmación vs oráculo 0.000 [0.000, 0.017] (0/224).
- `store`: estructura válida 0.824 [0.786, 0.856] (370/449); adversariales sin conclusión prohibida 0.005 [0.001, 0.030] (1/188); acierto en respondibles 0.759 [0.703, 0.807] (198/261); abstención en respondibles 0.000 [0.000, 0.015] (0/261); sobreafirmación vs oráculo 0.434 [0.389, 0.481] (195/449).

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
| http.entity | 3608 | 0.57 | 0.98 | [0.95, 1.02] | 1.73 |
| http.entity.load_c1 | 500 | 0.49 | 1.10 | [0.91, 1.73] | 2.34 |
| http.neighborhood.load_c1 | 500 | 0.33 | 0.52 | [0.48, 0.56] | 0.69 |
| http.path | 449 | 0.70 | 2.46 | [2.06, 3.08] | 4.51 |
| http.search | 1241 | 11.38 | 14.47 | [14.21, 14.62] | 15.82 |
| http.search.load_c1 | 500 | 9.28 | 11.86 | [11.53, 12.21] | 13.25 |
| http.search.load_c8 | 500 | 28.96 | 56.88 | [54.68, 63.61] | 74.42 |
| http.tiles.load_c1 | 500 | 1.19 | 2.57 | [2.51, 2.66] | 3.97 |
| store.entity | 3608 | 0.44 | 0.91 | [0.86, 0.97] | 1.88 |
| store.path | 449 | 0.61 | 9.43 | [5.42, 11.12] | 13.49 |
| store.search | 1593 | 4.00 | 18.35 | [16.71, 21.78] | 84.60 |
