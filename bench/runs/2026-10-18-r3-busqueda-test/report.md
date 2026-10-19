# AtlasBench · reporte 2026-10-19T02:43:38

Protocolo: bench/PROTOCOL.md v1.3 (thresholds frozen 2026-10-16; clarifications 6.1-6.3).  Datos: `{'split': 'test', 'manifest': 'bench/manifest/v3.json'}`.  Registro: `{'path': 'C:/MPM/data/atlas.db', 'sqlite': 'C:/MPM/data/atlas.db', 'bytes': 6151634944, 'mtime_ns': 1792377559883191400, 'schema_version': 3, 'event_seq': 3854655}`.
Hardware: {'platform': 'Windows-11-10.0.26200-SP0', 'processor': 'Intel64 Family 6 Model 197 Stepping 2, GenuineIntel', 'logical_cpus': 16, 'ram_bytes': 33811132416, 'python': '3.14.6'}.  Sistema evaluado: `http`; baseline: `store` (vía léxica FTS5).  Aperturas del conjunto sellado: 0.

Evidencia por tarea: {'retrieval': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'route': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'er_pair': ['silver/silver_auto'], 'sourced_answer': ['silver/silver_auto'], 'relation': ['silver/unlabeled']}.  Ningún ítem es oro de especialistas salvo que diga `expert_adjudicated`; los veredictos sobre ítems silver o escritos por agente llevan `PROVISIONAL-`.

## Veredictos

| Componente | Veredicto | Regla | Detalle |
|---|---|---|---|
| retrieval | NOT_EVALUABLE | each lang x modality cell with n >= 30 (trivial silver excluded): delta nDCG@10 >= +0.05 and paired-bootstrap LB >= +0.02 vs lexical baseline | snapshot stale vs registry (PROTOCOL §6.1 d); rerun with --compile |
| routes.structure | NOT_EVALUABLE | 0 structurally invalid steps in returned paths | snapshot stale vs registry (PROTOCOL §6.1 d); rerun with --compile |
| routes.adversarial | NOT_EVALUABLE | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | snapshot stale vs registry (PROTOCOL §6.1 d); rerun with --compile |
| routes.end_to_end | NOT_EVALUABLE | Wilson LB > 0.90 of expert-valid top-1 routes (n >= 100) | snapshot stale vs registry (PROTOCOL §6.1 d); rerun with --compile |
| routes.null_model_adversarial | PROVISIONAL-FAIL | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 5, 'n': 188, 'value': 0.026595744680851064, 'ci95': [0.011412446673968294, 0.060738061906302335], 'interval': 'wilson'}} |
| er | NOT_EVALUABLE | Wilson LB of merge precision >= 0.99 on sampled system merges (n >= 381); strata n >= 50 point >= 0.99 | no labeled sample of system merges (needs experts) |
| relations | NOT_EVALUABLE | each priority family (proof, structure, application): Wilson LB of precision >= 0.97, n >= 125 | no labeled relation items (needs experts) |
| provenance.registry | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3608, 'value': 0.0, 'ci95': [0.0, 0.001063573488618297], 'interval': 'wilson'}} |
| provenance.served | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3608, 'value': 0.0, 'ci95': [0.0, 0.001063573488618297], 'interval': 'wilson'}} |
| latency.search_hot.c1 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 21.988660000261007, 'n': 500, 'clusters': None, 'ci95': [21.527787751526823, 22.779136875292302], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.search_hot.c8 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 96.96712499926426, 'n': 500, 'clusters': None, 'ci95': [91.84040000036475, 107.50023499931554], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.entity | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 9.20768999985738, 'n': 500, 'clusters': None, 'ci95': [7.124494999061416, 11.377964999610318], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.neighborhood | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 6.749785000374685, 'n': 500, 'clusters': None, 'ci95': [5.996199999572127, 7.543499999883352], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.tiles | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 33.83638000022982, 'n': 500, 'clusters': None, 'ci95': [31.608805000541896, 37.006749999090964], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |

## Componentes ausentes


## Recuperación (nDCG@10, IC 95% bootstrap por familia)

Ítems resueltos: 1593; sin resolver en el registro: 0.

| Grupo | n | triviales | http nDCG@10 | http recall@10 | store nDCG@10 | store recall@10 |
|---|---|---|---|---|---|---|
| cells:en/formula | 163 | 2 | 0.018 [0.005, 0.035] (n=163) | 0.028 [0.009, 0.055] (n=163) | 0.001 [0.000, 0.003] (n=163) | 0.006 [0.000, 0.015] (n=163) |
| cells:en/informal_to_formal | 215 | 158 | 0.317 [0.281, 0.352] (n=215) | 0.460 [0.378, 0.539] (n=215) | 0.275 [0.243, 0.311] (n=215) | 0.391 [0.318, 0.471] (n=215) |
| cells:en/lean_name | 377 | 375 | 0.996 [0.993, 0.999] (n=377) | 0.999 [0.996, 1.000] (n=377) | 0.981 [0.971, 0.989] (n=377) | 0.999 [0.996, 1.000] (n=377) |
| cells:en/text | 420 | 415 | 0.815 [0.783, 0.847] (n=420) | 0.894 [0.863, 0.923] (n=420) | 0.892 [0.869, 0.914] (n=420) | 0.967 [0.949, 0.982] (n=420) |
| cells:es/formula | 197 | 0 | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) |
| cells:es/informal_to_formal | 180 | 1 | 0.165 [0.147, 0.184] (n=180) | 0.094 [0.045, 0.149] (n=180) | 0.132 [0.129, 0.137] (n=180) | 0.006 [0.000, 0.021] (n=180) |
| cells:es/text | 41 | 33 | 0.889 [0.832, 0.937] (n=41) | 0.841 [0.768, 0.915] (n=41) | 0.769 [0.636, 0.882] (n=41) | 0.720 [0.585, 0.841] (n=41) |
| by_generator:handwritten:handwritten | 19 | 0 | 0.428 [0.316, 0.545] (n=19) | 0.553 [0.447, 0.650] (n=19) | 0.000 [0.000, 0.000] (n=19) | 0.000 [0.000, 0.000] (n=19) |
| by_generator:handwritten:handwritten:trivial | 13 | 13 | 0.678 [0.566, 0.775] (n=13) | 0.577 [0.500, 0.679] (n=13) | 0.624 [0.507, 0.757] (n=13) | 0.538 [0.500, 0.636] (n=13) |
| by_generator:silver:docstring:trivial | 260 | 260 | 0.729 [0.683, 0.773] (n=260) | 0.842 [0.796, 0.885] (n=260) | 0.880 [0.849, 0.908] (n=260) | 0.973 [0.950, 0.992] (n=260) |
| by_generator:silver:formula | 352 | 0 | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) |
| by_generator:silver:informal_to_formal | 236 | 0 | 0.168 [0.152, 0.189] (n=236) | 0.106 [0.061, 0.161] (n=236) | 0.129 [0.128, 0.130] (n=236) | 0.000 [0.000, 0.000] (n=236) |
| by_generator:silver:informal_to_formal:trivial | 159 | 159 | 0.366 [0.327, 0.408] (n=159) | 0.572 [0.484, 0.665] (n=159) | 0.330 [0.290, 0.374] (n=159) | 0.535 [0.442, 0.632] (n=159) |
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
| http.entity | 3608 | 4.01 | 8.14 | [7.77, 8.63] | 18.69 |
| http.entity.load_c1 | 500 | 4.01 | 9.21 | [7.12, 11.38] | 26.09 |
| http.neighborhood.load_c1 | 500 | 3.35 | 6.75 | [6.00, 7.54] | 9.60 |
| http.path | 449 | 4.74 | 10.56 | [9.36, 11.25] | 16.40 |
| http.search | 1241 | 21.01 | 29.32 | [28.43, 30.32] | 34.48 |
| http.search.load_c1 | 500 | 16.52 | 21.99 | [21.53, 22.78] | 25.68 |
| http.search.load_c8 | 500 | 50.85 | 96.97 | [91.84, 107.50] | 128.93 |
| http.tiles.load_c1 | 500 | 11.24 | 33.84 | [31.61, 37.01] | 43.95 |
| store.entity | 3608 | 0.83 | 8.94 | [7.11, 10.74] | 69.32 |
| store.path | 449 | 1.58 | 18.54 | [15.92, 21.66] | 37.58 |
| store.search | 1593 | 5.55 | 31.53 | [28.40, 34.40] | 110.03 |
