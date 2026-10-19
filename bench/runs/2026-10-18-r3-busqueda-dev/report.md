# AtlasBench · reporte 2026-10-19T02:40:02

Protocolo: bench/PROTOCOL.md v1.3 (thresholds frozen 2026-10-16; clarifications 6.1-6.3).  Datos: `{'split': 'dev', 'manifest': 'bench/manifest/v3.json'}`.  Registro: `{'path': 'C:/MPM/data/atlas.db', 'sqlite': 'C:/MPM/data/atlas.db', 'bytes': 5808304128, 'mtime_ns': 1792377338860113400, 'schema_version': 3, 'event_seq': 3551424}`.
Hardware: {'platform': 'Windows-11-10.0.26200-SP0', 'processor': 'Intel64 Family 6 Model 197 Stepping 2, GenuineIntel', 'logical_cpus': 16, 'ram_bytes': 33811132416, 'python': '3.14.6'}.  Sistema evaluado: `http`; baseline: `store` (vía léxica FTS5).  Aperturas del conjunto sellado: 0.

Evidencia por tarea: {'retrieval': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'route': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'sourced_answer': ['silver/silver_auto'], 'er_pair': ['silver/silver_auto'], 'relation': ['silver/unlabeled']}.  Ningún ítem es oro de especialistas salvo que diga `expert_adjudicated`; los veredictos sobre ítems silver o escritos por agente llevan `PROVISIONAL-`.

## Veredictos

| Componente | Veredicto | Regla | Detalle |
|---|---|---|---|
| retrieval | NOT_EVALUABLE | each lang x modality cell with n >= 30 (trivial silver excluded): delta nDCG@10 >= +0.05 and paired-bootstrap LB >= +0.02 vs lexical baseline | snapshot stale vs registry (PROTOCOL §6.1 d); rerun with --compile |
| routes.structure | NOT_EVALUABLE | 0 structurally invalid steps in returned paths | snapshot stale vs registry (PROTOCOL §6.1 d); rerun with --compile |
| routes.adversarial | NOT_EVALUABLE | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | snapshot stale vs registry (PROTOCOL §6.1 d); rerun with --compile |
| routes.end_to_end | NOT_EVALUABLE | Wilson LB > 0.90 of expert-valid top-1 routes (n >= 100) | snapshot stale vs registry (PROTOCOL §6.1 d); rerun with --compile |
| routes.null_model_adversarial | PROVISIONAL-FAIL | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 8, 'n': 198, 'value': 0.04040404040404041, 'ci95': [0.020613166178095263, 0.07768903068781369], 'interval': 'wilson'}} |
| er | NOT_EVALUABLE | Wilson LB of merge precision >= 0.99 on sampled system merges (n >= 381); strata n >= 50 point >= 0.99 | no labeled sample of system merges (needs experts) |
| relations | NOT_EVALUABLE | each priority family (proof, structure, application): Wilson LB of precision >= 0.97, n >= 125 | no labeled relation items (needs experts) |
| provenance.registry | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3266, 'value': 0.0, 'ci95': [0.0, 0.0011748150083336429], 'interval': 'wilson'}} |
| provenance.served | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3266, 'value': 0.0, 'ci95': [0.0, 0.0011748150083336429], 'interval': 'wilson'}} |
| latency.search_hot.c1 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 23.554770000282584, 'n': 500, 'clusters': None, 'ci95': [22.961424999630246, 24.376530000972696], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.search_hot.c8 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 114.36335000134927, 'n': 500, 'clusters': None, 'ci95': [109.811615000126, 123.94870000025549], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.entity | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 6.7459750004672925, 'n': 500, 'clusters': None, 'ci95': [6.012030000147199, 7.216005001009762], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.neighborhood | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 5.929164999361092, 'n': 500, 'clusters': None, 'ci95': [5.469698751210211, 6.294555000476974], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.tiles | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 37.01796499990451, 'n': 500, 'clusters': None, 'ci95': [32.22678862411383, 39.10458499858577], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |

## Componentes ausentes


## Recuperación (nDCG@10, IC 95% bootstrap por familia)

Ítems resueltos: 1631; sin resolver en el registro: 0.

| Grupo | n | triviales | http nDCG@10 | http recall@10 | store nDCG@10 | store recall@10 |
|---|---|---|---|---|---|---|
| cells:en/formula | 157 | 0 | 0.008 [0.001, 0.021] (n=157) | 0.013 [0.003, 0.025] (n=157) | 0.000 [0.000, 0.000] (n=157) | 0.000 [0.000, 0.000] (n=157) |
| cells:en/informal_to_formal | 182 | 136 | 0.317 [0.281, 0.355] (n=182) | 0.445 [0.365, 0.529] (n=182) | 0.276 [0.244, 0.314] (n=182) | 0.390 [0.313, 0.472] (n=182) |
| cells:en/lean_name | 465 | 464 | 0.990 [0.985, 0.995] (n=465) | 0.992 [0.987, 0.998] (n=465) | 0.976 [0.966, 0.985] (n=465) | 0.990 [0.983, 0.997] (n=465) |
| cells:en/text | 450 | 442 | 0.803 [0.772, 0.831] (n=450) | 0.897 [0.869, 0.923] (n=450) | 0.883 [0.858, 0.906] (n=450) | 0.948 [0.928, 0.967] (n=450) |
| cells:es/formula | 197 | 0 | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) |
| cells:es/informal_to_formal | 136 | 1 | 0.164 [0.145, 0.185] (n=136) | 0.103 [0.045, 0.168] (n=136) | 0.131 [0.130, 0.131] (n=136) | 0.000 [0.000, 0.000] (n=136) |
| cells:es/text | 44 | 29 | 0.817 [0.729, 0.895] (n=44) | 0.750 [0.648, 0.841] (n=44) | 0.623 [0.476, 0.753] (n=44) | 0.591 [0.443, 0.716] (n=44) |
| by_generator:handwritten:handwritten | 27 | 0 | 0.451 [0.352, 0.555] (n=27) | 0.500 [0.400, 0.600] (n=27) | 0.000 [0.000, 0.000] (n=27) | 0.000 [0.000, 0.000] (n=27) |
| by_generator:handwritten:handwritten:trivial | 18 | 18 | 0.799 [0.753, 0.847] (n=18) | 0.583 [0.500, 0.676] (n=18) | 0.781 [0.714, 0.844] (n=18) | 0.611 [0.500, 0.737] (n=18) |
| by_generator:silver:docstring | 1 | 0 | 0.000 [0.000, 0.000] (n=1) | 0.000 [0.000, 0.000] (n=1) | 0.000 [0.000, 0.000] (n=1) | 0.000 [0.000, 0.000] (n=1) |
| by_generator:silver:docstring:trivial | 282 | 282 | 0.714 [0.673, 0.758] (n=282) | 0.851 [0.809, 0.894] (n=282) | 0.861 [0.828, 0.892] (n=282) | 0.947 [0.922, 0.972] (n=282) |
| by_generator:silver:formula | 349 | 0 | 0.000 [0.000, 0.000] (n=349) | 0.000 [0.000, 0.000] (n=349) | 0.000 [0.000, 0.000] (n=349) | 0.000 [0.000, 0.000] (n=349) |
| by_generator:silver:informal_to_formal | 181 | 0 | 0.177 [0.155, 0.201] (n=181) | 0.127 [0.066, 0.196] (n=181) | 0.131 [0.130, 0.131] (n=181) | 0.000 [0.000, 0.000] (n=181) |
| by_generator:silver:informal_to_formal:trivial | 137 | 137 | 0.350 [0.305, 0.396] (n=137) | 0.526 [0.425, 0.621] (n=137) | 0.324 [0.278, 0.372] (n=137) | 0.518 [0.418, 0.619] (n=137) |
| by_generator:silver:label_exact | 1 | 0 | 1.000 [1.000, 1.000] (n=1) | 1.000 [1.000, 1.000] (n=1) | 1.000 [1.000, 1.000] (n=1) | 1.000 [1.000, 1.000] (n=1) |
| by_generator:silver:label_exact:trivial | 635 | 635 | 0.992 [0.988, 0.996] (n=635) | 1.000 [1.000, 1.000] (n=635) | 0.978 [0.970, 0.985] (n=635) | 0.998 [0.995, 1.000] (n=635) |

## Recorridos

Ítems resueltos: 471; sin resolver: 3.

- `http`: estructura válida 1.000 [0.987, 1.000] (281/281); adversariales sin conclusión prohibida 1.000 [0.981, 1.000] (198/198); acierto en respondibles 0.985 [0.963, 0.994] (271/275); abstención en respondibles 0.000 [0.000, 0.014] (0/275); sobreafirmación vs oráculo 0.000 [0.000, 0.013] (0/281).
- `store`: estructura válida 0.843 [0.807, 0.873] (397/471); adversariales sin conclusión prohibida 0.040 [0.021, 0.078] (8/198); acierto en respondibles 0.727 [0.672, 0.777] (200/275); abstención en respondibles 0.000 [0.000, 0.014] (0/275); sobreafirmación vs oráculo 0.431 [0.387, 0.476] (203/471).

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
| http.entity | 3266 | 4.64 | 18.13 | [15.88, 19.88] | 45.68 |
| http.entity.load_c1 | 500 | 4.09 | 6.75 | [6.01, 7.22] | 10.65 |
| http.neighborhood.load_c1 | 500 | 3.48 | 5.93 | [5.47, 6.29] | 8.75 |
| http.path | 471 | 7.03 | 16.89 | [15.23, 19.37] | 29.80 |
| http.search | 1282 | 20.05 | 26.10 | [25.68, 27.10] | 32.30 |
| http.search.load_c1 | 500 | 18.31 | 23.55 | [22.96, 24.38] | 26.73 |
| http.search.load_c8 | 500 | 50.15 | 114.36 | [109.81, 123.95] | 140.32 |
| http.tiles.load_c1 | 500 | 12.31 | 37.02 | [32.23, 39.10] | 42.66 |
| store.entity | 3266 | 1.43 | 33.37 | [32.33, 34.48] | 49.63 |
| store.path | 471 | 1.97 | 35.04 | [24.03, 61.06] | 97.08 |
| store.search | 1631 | 7.13 | 84.18 | [76.86, 93.90] | 158.87 |
