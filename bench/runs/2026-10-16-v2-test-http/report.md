# AtlasBench · reporte 2026-10-17T01:03:12

Protocolo: bench/PROTOCOL.md v1.1 (rules frozen 2026-10-16).  Datos: `{'split': 'test', 'manifest': 'bench/manifest/v2.json'}`.  Registro: `{'path': 'C:/Users/AdrianGonzalez/.cargo-target/bench/frozen/atlas-801797.db', 'sqlite': 'C:/Users/AdrianGonzalez/.cargo-target/bench/frozen/atlas-801797.db', 'bytes': 1650872320, 'mtime_ns': 1792197666313402600, 'schema_version': 1, 'event_seq': 801797}`.
Hardware: {'platform': 'Windows-11-10.0.26200-SP0', 'processor': 'Intel64 Family 6 Model 197 Stepping 2, GenuineIntel', 'logical_cpus': 16, 'ram_bytes': 33811132416, 'python': '3.14.6'}.  Sistema evaluado: `http`; baseline: `store` (vía léxica FTS5).  Aperturas del conjunto sellado: 0.

Evidencia por tarea: {'retrieval': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'route': ['handwritten/authored_by_agent_pending_expert_review', 'silver/silver_auto'], 'er_pair': ['silver/silver_auto'], 'sourced_answer': ['silver/silver_auto'], 'relation': ['silver/unlabeled']}.  Ningún ítem es oro de especialistas salvo que diga `expert_adjudicated`; los veredictos sobre ítems silver o escritos por agente llevan `PROVISIONAL-`.

## Veredictos

| Componente | Veredicto | Regla | Detalle |
|---|---|---|---|
| retrieval | MISSING | each lang x modality cell with n >= 30 (trivial silver excluded): delta nDCG@10 >= +0.05 and paired-bootstrap LB >= +0.02 vs lexical baseline | hybrid search absent: the system answered every query with the lexical lane only (registry FTS5 fallback = the baseline) |
| routes.structure | PASS | 0 structurally invalid steps in returned paths | {'invalid_paths': 0, 'answered': 223} |
| routes.adversarial | PROVISIONAL-FAIL | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 225, 'n': 285, 'value': 0.7894736842105263, 'ci95': [0.7384510335874758, 0.8327966005772168], 'interval': 'wilson'}} |
| routes.end_to_end | NOT_EVALUABLE | Wilson LB > 0.90 of expert-valid top-1 routes (n >= 100) | no expert judgments of returned routes yet; route_outputs.jsonl is the queue |
| routes.null_model_adversarial | PROVISIONAL-FAIL | no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40 | {'no_forbidden': {'k': 0, 'n': 285, 'value': 0.0, 'ci95': [0.0, 0.01329954098825824], 'interval': 'wilson'}} |
| er | NOT_EVALUABLE | Wilson LB of merge precision >= 0.99 on sampled system merges (n >= 381); strata n >= 50 point >= 0.99 | no labeled sample of system merges (needs experts) |
| relations | NOT_EVALUABLE | each priority family (proof, structure, application): Wilson LB of precision >= 0.97, n >= 125 | no labeled relation items (needs experts) |
| provenance.registry | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3608, 'value': 0.0, 'ci95': [0.0, 0.001063573488618297], 'interval': 'wilson'}} |
| provenance.served | PASS | 0 served claims without complete, matching provenance; n >= 1000 (mechanical check) | {'failures': {'k': 0, 'n': 3608, 'value': 0.0, 'ci95': [0.0, 0.001063573488618297], 'interval': 'wilson'}} |
| latency.search_hot.c1 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 11.087810003664341, 'n': 500, 'clusters': None, 'ci95': [9.680400020442903, 12.743586125070575], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.search_hot.c8 | PASS | bootstrap 95% upper bound of p95 < 2000 ms, n >= 500 | {'p95': {'value': 14.829044972429967, 'n': 500, 'clusters': None, 'ci95': [12.746599968522787, 16.839774989057332], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.entity | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 1.7312049865722605, 'n': 500, 'clusters': None, 'ci95': [1.4715199678903441, 2.210000006016344], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.neighborhood | PASS | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | {'p95': {'value': 1.1440899776061992, 'n': 500, 'clusters': None, 'ci95': [1.0040299617685375, 1.414965037838539], 'interval': 'bootstrap-percentile', 'B': 2000, 'seed': 20261016}} |
| latency.interaction_server.tiles | MISSING | bootstrap 95% upper bound of p95 < 100 ms, n >= 500 | ComponentMissing: GET /api/map/tiles: HTTP 501 not implemented yet: map tiles: no layout loaded (--layout) |

## Componentes ausentes


## Recuperación (nDCG@10, IC 95% bootstrap por familia)

Ítems resueltos: 1593; sin resolver en el registro: 0.

| Grupo | n | triviales | http nDCG@10 | http recall@10 | store nDCG@10 | store recall@10 |
|---|---|---|---|---|---|---|
| cells:en/formula | 163 | 2 | 0.001 [0.000, 0.003] (n=163) | 0.006 [0.000, 0.015] (n=163) | 0.001 [0.000, 0.003] (n=163) | 0.006 [0.000, 0.015] (n=163) |
| cells:en/informal_to_formal | 215 | 157 | 0.309 [0.273, 0.350] (n=215) | 0.433 [0.360, 0.516] (n=215) | 0.309 [0.273, 0.350] (n=215) | 0.433 [0.360, 0.516] (n=215) |
| cells:en/lean_name | 377 | 377 | 0.983 [0.974, 0.990] (n=377) | 0.999 [0.996, 1.000] (n=377) | 0.983 [0.974, 0.990] (n=377) | 0.999 [0.996, 1.000] (n=377) |
| cells:en/text | 420 | 415 | 0.892 [0.869, 0.915] (n=420) | 0.967 [0.949, 0.982] (n=420) | 0.892 [0.869, 0.915] (n=420) | 0.967 [0.949, 0.982] (n=420) |
| cells:es/formula | 197 | 0 | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) | 0.000 [0.000, 0.000] (n=197) |
| cells:es/informal_to_formal | 180 | 1 | 0.132 [0.129, 0.138] (n=180) | 0.006 [0.000, 0.021] (n=180) | 0.132 [0.129, 0.138] (n=180) | 0.006 [0.000, 0.021] (n=180) |
| cells:es/text | 41 | 33 | 0.769 [0.636, 0.882] (n=41) | 0.720 [0.585, 0.841] (n=41) | 0.769 [0.636, 0.882] (n=41) | 0.720 [0.585, 0.841] (n=41) |
| by_generator:handwritten:handwritten | 19 | 0 | 0.000 [0.000, 0.000] (n=19) | 0.000 [0.000, 0.000] (n=19) | 0.000 [0.000, 0.000] (n=19) | 0.000 [0.000, 0.000] (n=19) |
| by_generator:handwritten:handwritten:trivial | 13 | 13 | 0.626 [0.508, 0.761] (n=13) | 0.538 [0.500, 0.636] (n=13) | 0.626 [0.508, 0.761] (n=13) | 0.538 [0.500, 0.636] (n=13) |
| by_generator:silver:docstring:trivial | 260 | 260 | 0.880 [0.849, 0.908] (n=260) | 0.973 [0.950, 0.992] (n=260) | 0.880 [0.849, 0.908] (n=260) | 0.973 [0.950, 0.992] (n=260) |
| by_generator:silver:formula | 352 | 0 | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) | 0.000 [0.000, 0.000] (n=352) |
| by_generator:silver:informal_to_formal | 237 | 0 | 0.129 [0.128, 0.130] (n=237) | 0.000 [0.000, 0.000] (n=237) | 0.129 [0.128, 0.130] (n=237) | 0.000 [0.000, 0.000] (n=237) |
| by_generator:silver:informal_to_formal:trivial | 158 | 158 | 0.378 [0.334, 0.426] (n=158) | 0.595 [0.506, 0.690] (n=158) | 0.378 [0.334, 0.426] (n=158) | 0.595 [0.506, 0.690] (n=158) |
| by_generator:silver:label_exact:trivial | 554 | 554 | 0.975 [0.965, 0.983] (n=554) | 0.998 [0.995, 1.000] (n=554) | 0.975 [0.965, 0.983] (n=554) | 0.998 [0.995, 1.000] (n=554) |

## Recorridos

Ítems resueltos: 449; sin resolver: 13.

- `http`: estructura válida 1.000 [0.983, 1.000] (223/223); adversariales sin conclusión prohibida 0.789 [0.738, 0.833] (225/285); acierto en respondibles 0.988 [0.957, 0.997] (162/164); abstención en respondibles 0.012 [0.003, 0.043] (2/164); sobreafirmación vs oráculo 0.269 [0.215, 0.331] (60/223).
- `store`: estructura válida 0.820 [0.781, 0.852] (368/449); adversariales sin conclusión prohibida 0.000 [0.000, 0.013] (0/285); acierto en respondibles 0.982 [0.948, 0.994] (161/164); abstención en respondibles 0.000 [0.000, 0.023] (0/164); sobreafirmación vs oráculo 0.570 [0.524, 0.615] (256/449).

## Resolución de entidades

Pares resueltos: 655.  Abstención: 0.000 [0.000, 0.006] (0/655).  Fusiones del sistema muestreadas: 0 (etiquetadas: 0).

- falsa fusión en negativos `label_collision`: 0.000 [0.000, 0.024] (0/159)
- falsa fusión en negativos `random`: 0.000 [0.000, 0.026] (0/142)
- falsa fusión en negativos `related_not_same`: 0.000 [0.000, 0.026] (0/146)
- cobertura en positivos `alias (circular)`: 0.000 [0.000, 0.020] (0/185)
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
| http.entity | 3608 | 0.89 | 1.56 | [1.49, 1.60] | 2.69 |
| http.entity.load_c1 | 500 | 0.93 | 1.73 | [1.47, 2.21] | 3.33 |
| http.neighborhood.load_c1 | 500 | 0.61 | 1.14 | [1.00, 1.41] | 5.20 |
| http.path | 449 | 1.16 | 8.19 | [6.06, 14.09] | 40.35 |
| http.search | 1241 | 2.72 | 13.38 | [12.86, 14.07] | 56.16 |
| http.search.load_c1 | 500 | 2.07 | 11.09 | [9.68, 12.74] | 53.67 |
| http.search.load_c8 | 500 | 2.70 | 14.83 | [12.75, 16.84] | 77.59 |
| store.entity | 3608 | 0.37 | 0.89 | [0.83, 0.97] | 2.08 |
| store.path | 449 | 0.71 | 11.58 | [7.03, 13.63] | 18.96 |
| store.search | 1593 | 2.90 | 14.35 | [13.35, 15.33] | 35.27 |
