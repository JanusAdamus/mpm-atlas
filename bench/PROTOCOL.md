# AtlasBench · protocolo de evaluación v1

Fecha de congelamiento de las reglas de decisión: **2026-10-16**, antes de correr cualquier componente.  Paquete de trabajo: WP03 (`research/2026-10-16/plan-ejecucion.md`).  Metas de origen: tabla "Evaluación: calidad demostrable" de `research/2026-10-16/atlas-investigacion-y-plan.md`.  Contrato de datos: `docs/SPEC.md`.

Este documento fija qué se mide, con qué datos, cómo se anota y qué resultado hace que un componente cumpla su meta.  Las reglas de la sección 6 se escribieron antes de ver resultados.  Cambiarlas exige una nueva versión del protocolo (`v2`), con fecha, razón y la lista de resultados ya vistos al momento del cambio.  Un umbral nuevo propuesto después de ver resultados es un parámetro ajustado, no una regla.

## 1. Qué decide cada componente

| Componente | Decisión que informa | Unidad de observación | Punto de operación |
|---|---|---|---|
| Resolución de entidades | ¿Se puede fusionar automáticamente por encima de `MERGE_THRESHOLD`? | par de entidades del mismo tipo | el umbral de `atlas/resolve.py` vigente |
| Relaciones | ¿Se publica una familia de relaciones extraídas sin revisión previa? | afirmación (`Claim`) vigente | todas las afirmaciones vigentes de la familia |
| Recuperación | ¿La búsqueda híbrida reemplaza a la vía léxica? | consulta | top-10 (`limit=10`), mismo `lang` y `types` para sistema y baseline |
| Recorridos | ¿Se muestra la ruta seleccionada como conclusión? | consulta de ruta (origen, destino, objetivo) | ruta top-1 devuelta por `atlas path ... --json` o `/api/path` |
| Respuestas con fuentes | ¿Cada afirmación servida dice de dónde viene? | afirmación servida | todas las afirmaciones de la entidad pedida |
| Latencia | ¿El servidor cumple la meta de interfaz? | petición | carga y hardware declarados en el reporte |

## 2. Formato de ítems (`atlasbench/1`)

Un ítem es una línea JSON (`.jsonl`).  Campos comunes:

- `format`: `"atlasbench/1"`.  `id`: estable, único en la batería.  `task`: `er_pair`, `relation`, `retrieval`, `route`, `sourced_answer`.
- `origin`: `silver` (derivado automáticamente del registro), `handwritten` (escrito a mano) o `expert`.
- `label_status`: `silver_auto`, `authored_by_agent_pending_expert_review`, `expert_single`, `expert_adjudicated` o `unlabeled`.
- `author`, `created` (fecha ISO), `strata` (área, idioma, familia, modalidad, dificultad), `split_keys` (`document`, `family`, `time`), `payload` (específico de la tarea), `notes`.
- Las entidades se citan con una referencia: `{"id": "s:pythagoras"}` o `{"ext": ["wikidata", "Q11518"]}` / `{"ext": ["mathlib", "Nat.Prime"]}`.  La referencia se resuelve contra el registro en el momento de evaluar (con fusiones).  Una referencia que no existe en el registro **no** es un error del componente: se reporta como falta de cobertura (`unresolved`).

Payload por tarea:

| Tarea | Payload |
|---|---|
| `er_pair` | `a`, `b` (referencias), `label` ∈ {`same`, `different`, `null`}, `hardness` ∈ {`shared_external_id`, `alias`, `label_collision`, `related_not_same`, `random`}, `evidence`, `circular_with` (si la etiqueta usa la misma evidencia que el resolvedor) |
| `relation` | `claim_id`, `relation`, `family` (proyección), `subject`, `targets`, `source_id`, `locator`, `label` ∈ {`correct`, `incorrect`, `null`}, `error_type`, `corroborated_by` (fuentes independientes que afirman lo mismo) |
| `retrieval` | `query`, `lang`, `modality` ∈ {`text`, `formula`, `lean_name`, `informal_to_formal`}, `types` (opcional), `qrels`: lista de `{ref, grade}` con `grade` 0-3 |
| `route` | `from`, `to` (referencias), `objective`, `answerable` (bool), `allowed` (conclusiones aceptables, claves de `shapes/composition.json`), `forbidden` (conclusiones que no se pueden afirmar), `attack` (tipo de engaño en adversariales), `steps` (ruta de referencia, cada salto con `from`, `relation`, `to`, `direction`, `label` ∈ {`valid`, `invalid`} y `why`) |
| `sourced_answer` | `entity`, `claim_id`, `expected`: lista de `{source_id, locator}` |

Escala de relevancia (`grade`): 3 = es exactamente lo buscado; 2 = la formalización, el enunciado o la definición directamente asociada; 1 = relacionado y útil (generalización, caso particular, área); 0 = no relevante (se anota explícitamente para distinguirlo de "no juzgado").

## 3. Particiones

- **Claves de separación.**  `document` (fuente y archivo o ítem de origen), `family` (resultado matemático o entidad canónica destino; las variantes de un mismo teorema son una familia) y `time` (fecha de la fuente).  Los ítems que comparten documento o familia se unen en un mismo componente (unión-búsqueda); un componente entero cae en una sola partición.  Así ningún documento ni familia aparece en dos particiones.
- **Tiempo.**  Ningún componente con un ítem posterior a la fecha de corte entra en `dev`.  La fecha de corte se declara en el manifiesto.
- **Asignación.**  Por hash estable de la clave mínima del componente: 40% `dev`, 40% `test`, 20% `sealed`.
- **`dev`** se usa para ajustar umbrales, prompts y pesos.  **`test`** se usa para reportar.  **`sealed`** es el conjunto final: no se usa para seleccionar modelos, prompts ni umbrales.  Solo se abre con `python -m bench run --split sealed --open-sealed "<razón>"`; cada apertura queda en `bench/sealed-ledger.jsonl` y todo reporte dice cuántas veces se abrió.
- **Manifiesto.**  `bench/manifest/<versión>.json` guarda, por partición, la cuenta de ítems, el SHA-256 de cada ítem (JSON canónico) y el hash del conjunto.  `python -m bench verify` falla si un ítem cambió sin nueva versión.

## 4. Anotación

### 4.1 Quién anota

Especialistas con formación en el área del ítem (mínimo licenciatura en matemáticas o equivalente; para Mathlib, experiencia leyendo Lean 4).  El agente que escribió ítems (`authored_by_agent_pending_expert_review`) **no** cuenta como anotador.  Los ítems `silver` nunca se convierten en oro sin pasar por esta sección.

### 4.2 Instrucciones por tarea

- **Pares de entidades.**  `same` solo si los dos registros describen el mismo objeto matemático con la misma definición (una traducción es el mismo objeto).  Dos tradiciones con diferencias conceptuales reales, una generalización o un caso particular son `different`.  Un concepto informal y su declaración formal son `different` (el vínculo correcto es `formalized_as`, SPEC §2).  Si no se puede decidir con la información mostrada: `unsure` (cuenta como no anotado y va a adjudicación).
- **Relaciones.**  Leer sujeto, relación, objeto o premisas, supuestos, contexto y la fuente con su localizador.  `correct` si la fuente sostiene exactamente esa relación en esa dirección y con esos supuestos.  `incorrect` con `error_type` ∈ {`wrong_direction`, `wrong_relation`, `wrong_endpoint`, `missing_assumption`, `not_in_source`, `other`}.  Una relación verdadera que la fuente no sostiene es `not_in_source`.
- **Recuperación.**  Para cada consulta se juzgan los 20 primeros resultados de cada sistema comparado (juicio agrupado, *pooling*), más los `qrels` ya presentes.  Se asigna `grade` 0-3 según la escala de la sección 2.  El anotador ve la consulta, el idioma y la modalidad, no el sistema que produjo el resultado ni su posición.
- **Recorridos.**  Se juzga cada salto (`valid` / `invalid` con razón) y la conclusión global: ¿la ruta autoriza exactamente lo que dice `conclusion.authorizes` bajo los supuestos acumulados?  Una ruta con todos los saltos válidos pero conclusión exagerada es inválida.  La abstención en un ítem `answerable=false` es correcta.
- **Respuestas con fuentes.**  Se verifica que la fuente y el localizador servidos existen y contienen lo afirmado (auditoría humana del tipo ALCE, R33).  La parte mecánica (que existan los campos) la mide `bench/` sin anotadores.

### 4.3 Doble anotación y adjudicación

- 100% de los ítems de `test` y `sealed` se anotan por dos especialistas independientes que no ven la etiqueta del otro.  En `dev`, al menos 20% (muestra estratificada por área y tarea).
- Desacuerdos (o cualquier `unsure`) van a un tercer especialista que adjudica viendo ambas etiquetas y razones.  La etiqueta adjudicada y las dos originales se conservan; nada se sobrescribe.
- `label_status` pasa a `expert_single` (una anotación) o `expert_adjudicated` (dos anotaciones y, si hubo desacuerdo, adjudicación).

### 4.4 Acuerdo entre anotadores

- Dos anotadores, etiqueta nominal: κ de Cohen.  Dos o más anotadores, con faltantes: α de Krippendorff (nominal para etiquetas, de intervalo para `grade` 0-3).  `python -m bench agreement` calcula ambos.
- Se reporta por tarea y por área, con intervalo bootstrap por ítem (B = 2000).
- Regla: α ≥ 0.80 → las etiquetas se usan como oro.  0.667 ≤ α < 0.80 → solo conclusiones tentativas, y se revisan las instrucciones antes de anotar más.  α < 0.667 → no se usan; se rehacen instrucciones y una ronda piloto.

## 5. Métricas

- **Fusiones:** precisión de fusiones = pares predichos `same` que son `same` / pares predichos `same`.  Tasa de abstención = pares enviados a revisión (cola del resolvedor) / pares evaluados.  Cobertura de fusiones (recall) sobre positivos, reportada pero sin meta.  Estratos: tipo de entidad × dificultad.
- **Relaciones:** precisión por familia (proyección de `RELATIONS`) y por relación.  Nunca un promedio global como resultado.
- **Recuperación:** nDCG@10 (ganancia 2^grade − 1, descuento log2), recall@10 y recall@50 (relevante = grade ≥ 2), MRR@10.  Celdas: idioma × modalidad, y área cuando hay n suficiente.  Siempre junto al baseline en el mismo `k`.
- **Recorridos:** validez estructural (automática: cada salto existe, está vigente, encadena, respeta dirección e inversas, y las conjunciones tienen todas sus premisas cuando se recorren de premisas a sujeto), conformidad con la tabla de composición (automática), validez de extremo a extremo (especialistas), tasa de abstención en ítems con y sin respuesta.
- **Procedencia:** fracción de afirmaciones servidas con `source_id`, `locator`, `extractor`, `extractor_version`, `license`, `retrieved_at` y estado, y coincidencia con lo esperado.
- **Latencia:** p50, p95, p99 por endpoint, con intervalo bootstrap del p95.  Se declaran CPU, núcleos, sistema, tamaño del snapshot, concurrencia y si la caché estaba caliente.
- **Intervalos.**  Proporciones: Wilson 95%.  Métricas por consulta: bootstrap percentil 95% con remuestreo por consulta (B = 2000, semilla 20261016).  Comparaciones sistema − baseline: bootstrap pareado por consulta.  Si los ítems comparten familia, el remuestreo es por familia (bootstrap por conglomerados).  Conteos pequeños se reportan como conteos ("7 de 45").

## 6. Reglas de decisión (congeladas el 2026-10-16)

Niveles de veredicto: `PASS`, `FAIL`, `INSUFFICIENT` (n menor al mínimo), `MISSING` (el componente no existe o responde `not_implemented`), `NOT_EVALUABLE` (faltan etiquetas o el registro).  **Un `PASS` o `FAIL` sobre la meta del informe solo se emite con ítems `expert_adjudicated` de `test` o `sealed`.**  Con ítems `silver` o `authored_by_agent_pending_expert_review` el veredicto lleva el prefijo `PROVISIONAL-` y no cuenta como cumplimiento.

| Componente | Regla | Mínimo |
|---|---|---|
| Fusiones | `PASS` si el límite inferior Wilson 95% de la precisión de fusiones ≥ 0.99 en la muestra estratificada, y ningún estrato con n ≥ 50 tiene estimación puntual < 0.99.  `FAIL` si el límite superior < 0.99, o si un estrato con n ≥ 50 tiene límite superior < 0.99.  La tasa de abstención se publica siempre con su intervalo. | 381 fusiones evaluadas (con 0 errores, 381 es el menor n cuyo límite inferior Wilson alcanza 0.99) |
| Relaciones | Por familia prioritaria (`proof`, `structure`, `application`): `PASS` si el límite inferior Wilson 95% ≥ 0.97.  El componente pasa solo si pasan todas las familias prioritarias. | 125 afirmaciones por familia |
| Recuperación | Baseline v1 = vía léxica FTS5 de `Store.search` (SPEC §10), mismo `lang`, `types` y `k`.  `PASS` si en **cada** celda idioma × modalidad con n ≥ 30: Δ nDCG@10 ≥ +0.05 y límite inferior del intervalo bootstrap pareado de Δ ≥ +0.02.  `FAIL` si alguna celda con n ≥ 30 tiene límite superior de Δ < 0 (regresión) o no cumple la regla anterior. | 30 consultas por celda |
| Recorridos, estructura | `FAIL` ante cualquier salto estructuralmente inválido en una ruta devuelta (es un defecto, no un error estadístico). | todas las rutas devueltas |
| Recorridos, adversariales | `PASS` si ninguna ruta devuelta en ítems adversariales afirma una conclusión de `forbidden` y el límite inferior Wilson 95% de "no afirma lo prohibido" ≥ 0.95. | 40 ítems adversariales |
| Recorridos, extremo a extremo | `PASS` si el límite inferior Wilson 95% de rutas top-1 válidas según especialistas > 0.90, **y** la abstención en ítems `answerable=true` ≤ 0.25 (protege contra un motor que gana absteniéndose), **y** la abstención correcta en ítems `answerable=false` tiene límite inferior Wilson ≥ 0.90. | 100 rutas juzgadas; 40 ítems sin respuesta |
| Procedencia | `PASS` si 0 afirmaciones servidas carecen de algún campo de procedencia o no coinciden con el registro.  Se reporta el límite superior Wilson de la tasa de fallas.  Es una verificación mecánica: puede emitir `PASS` sin especialistas. | 1000 afirmaciones (o todas, si hay menos) |
| Latencia, búsqueda caliente | `PASS` si el límite superior del intervalo bootstrap del p95 de `/api/search` < 2000 ms con caché caliente, concurrencia 1 y 8, en el hardware declarado. | 500 peticiones por concurrencia |
| Latencia, interacción local | El informe pide p95 < 100 ms de interacción en la interfaz.  `bench/` mide solo la parte del servidor (`/api/entity`, `/api/neighborhood`, `/api/map/tiles`) con la misma regla y umbral 100 ms; la interfaz completa se mide en la web. | 500 peticiones por endpoint |

Reglas que protegen contra engañarse:

- Un veredicto se calcula sobre `test`.  `sealed` se abre una vez por versión del sistema candidato a publicar, después de congelar todo con `dev` y `test`.
- Se publica el número de configuraciones probadas en `dev`.  No se reporta la mejor semilla: se reportan al menos 3 semillas cuando el sistema tiene aleatoriedad.
- Ítems `silver` de recuperación marcados como triviales por la prueba de fuga (sección 7) se excluyen de toda regla y solo sirven como regresión.
- Las etiquetas de rutas `silver` derivan de `shapes/composition.json`, la misma tabla que carga el motor.  Prueban conformidad con la especificación, no validez matemática.  Nunca sostienen un `PASS` de extremo a extremo.

### 6.1 Aclaraciones v1.1 (2026-10-16, escritas antes de la primera corrida)

No cambian ningún umbral de la tabla anterior; precisan cuándo una regla se puede aplicar.  Al escribirlas no existía ninguna corrida (`bench/runs/` vacío, `sealed-ledger.jsonl` sin entradas).

- **(a) La vía léxica de respaldo no es candidata.**  Si en todas las consultas el sistema evaluado responde solo con la vía `lexical` (`lanes.formula` y `lanes.semantic` nulos en todos los resultados), el sistema es el FTS5 del registro, es decir, el baseline.  Veredicto de recuperación: `MISSING` ("búsqueda híbrida ausente").  Se reporta la fracción de consultas con top-10 idéntico al baseline como prueba de conformidad del respaldo en Rust con `Store.search`, no como resultado de búsqueda.
- **(b) Métricas por vía.**  nDCG@10 y recall@10 por vía (`lexical`, `formula`, `semantic`) se calculan ordenando, dentro del top-50 fusionado, los resultados que cada vía aportó según su propio rango.  Son diagnóstico (la lista de cada vía está truncada por la fusión) y no sostienen ninguna regla.
- **(c) Qrels resueltos parcialmente.**  Las métricas usan solo los `qrels` que se resuelven en el registro evaluado.  Sistema y baseline comparten exactamente esos `qrels`, así que Δ es justo; los valores absolutos no se comparan entre versiones del registro.  El reporte da la fracción de `qrels` resueltos.
- **(d) Snapshot vigente.**  Si el snapshot que usa el binario se compiló desde otro estado del registro (`source_event_seq` ≠ `MAX(events.seq)`), los veredictos de recuperación y recorridos de ese sistema son `NOT_EVALUABLE` (los ids pueden no coincidir).  `--compile` compila uno vigente.
- **(e) Registro de cada ítem de ruta.**  Los recorridos escritos a mano declaran su registro en `strata.registry` (`mathlib` o `mini_atlas`).  Los de `mini_atlas` se evalúan con `--db tests/fixtures/mini_atlas.json`; en el registro real cuentan como no resueltos, no como errores.
- **(g) Zona intermedia y mínimos efectivos.**  Cuando el intervalo contiene la meta (límite inferior < meta ≤ límite superior), la meta no está demostrada ni refutada: el veredicto es `INSUFFICIENT` (hacen falta más ítems), no `FAIL`.  Aplica a fusiones (meta 0.99) y relaciones (0.97).  En recorridos adversariales, cualquier conclusión prohibida es `FAIL` (defecto); sin violaciones, el límite inferior Wilson ≥ 0.95 exige n ≥ 73 (con n = 40 y 0 violaciones el límite es 0.912), así que con 40 ≤ n < 73 y 0 violaciones el veredicto es `INSUFFICIENT`.  El mínimo de 40 de la tabla se mantiene como mínimo para emitir `FAIL`.
- **(h) Conjunciones en la validez estructural.**  Se sigue la regla 7 de `docs/SPEC.md` §3, que no distingue dirección: todo salto sobre una afirmación conjuntiva debe mostrar todas sus premisas (`premises` + `premises_missing`), se recorra de premisas a sujeto o al revés.  La sección 5 decía "de premisas a sujeto"; la SPEC manda.
- **(f) Fuga graduada.**  Además de `trivial` (todo o nada), se reporta la cobertura de tokens: fracción de tokens de la consulta presentes en la unión de campos indexados de la entidad relevante.  Es descriptiva.

### 6.2 Aclaraciones v1.2 (2026-10-16, antes de la primera corrida sobre `test`)

No cambian ningún umbral ni ninguna etiqueta.  Lo que se había visto al escribirlas: el reporte de fuga del conjunto `v1` (sin puntajes de sistemas) y una corrida ad hoc de los 42 recorridos escritos a mano sobre `mini_atlas` (20 resueltos).  Ninguna etiqueta escrita a mano se tocó después de esa corrida.

- **(i) Conjunto de ítems `v2`.**  El silver de `v1` se derivó de un estado anterior del registro (255,037 entidades escaneadas) que aún no tenía afirmaciones `formalized_as` ni fórmulas, así que los generadores `informal_to_formal` y `formula` produjeron 0 ítems.  Se regeneró desde una copia congelada del registro (`event_seq` 801,797: 286,757 entidades, 515,036 afirmaciones) y se particionó como `bench/manifest/v2.json`.  `v1` queda como registro histórico; ningún reporte usó `v1`.  La versión del protocolo sigue siendo v1.x: `v2` nombra el conjunto de ítems, no las reglas.
- **(j) Registro congelado.**  Las corridas se hacen contra una copia de `data/atlas.db` tomada con la API de respaldo de SQLite (lectura consistente) mientras la ingesta sigue escribiendo.  El reporte guarda su `event_seq`, y el snapshot se compila desde esa misma copia (`--compile`).
- **(k) Latencia por concurrencia y por endpoint.**  La regla de búsqueda caliente se evalúa por separado con concurrencia 1 y 8 (pasa solo si pasan las dos).  La de interacción se evalúa por separado para `/api/entity`, `/api/neighborhood` y `/api/map/tiles`, 500 peticiones cada uno con concurrencia 1, tras una pasada de calentamiento de hasta 20 peticiones.  Un endpoint que responde `not_implemented` es `MISSING`.

- **(l) Consultas rechazadas.**  Si el sistema responde con error a una consulta de la batería (por ejemplo, HTTP 400 por una consulta de más de 256 caracteres), esa consulta cuenta como ranking vacío (todas sus métricas valen 0) y se reporta el número de fallas por celda.  Excluirla favorecería al sistema que rechaza las consultas difíciles.  En las pruebas de carga, una respuesta de error no se cronometra y se cuenta aparte.  Se escribió después de ver que la primera corrida abortó en la prueba de carga por un HTTP 400, y antes de ver cualquier métrica de recuperación.

- **(m) Páginas truncadas de `/api/entity`.**  La procedencia servida se pide con el `limit` máximo de la API (1000 afirmaciones; no hay `offset`).  Si la afirmación esperada no viene y la respuesta dice `truncated: true`, la afirmación no se sirvió: no es un defecto de procedencia y se cuenta aparte (`not_served_truncated`).  Si no viene y la respuesta no está truncada, es falla (`claim_missing`).  La prueba de carga de búsqueda usa solo las consultas que el servidor contestó en la pasada por ítem.  Ambas se escribieron después de ver una falla `claim_missing` en `wd:Q41207` (422 afirmaciones, página de 200), y no cambian el umbral.

### 6.3 Aclaraciones v1.3 (2026-10-16, después de la primera corrida completa sobre `test` de `v2`)

Ningún umbral cambia.  Se escribieron **después** de ver los resultados de recorridos de `v2/test` (adversariales: 225 de 285 sin conclusión prohibida); se declaran como correcciones de la batería, no como reglas nuevas, y la corrida sobre `v2` queda guardada en `bench/runs/2026-10-16-v2-test-http/`.

- **(n) Oráculo al día con la tabla.**  `bench/compose.py` implementaba la tabla de composición v1; el motor y `shapes/composition.json` ya están en v2 (monoide de conclusiones con `converse`: recorrer `depends_on_logically` al revés autoriza `logical_support`, R7).  58 de las 60 "violaciones" eran ítems silver `reverse_dependency` etiquetados con el oráculo viejo.  El oráculo ahora es el producto del monoide de la tabla y una prueba verifica que reproduce las 35 entradas `expect` y los 7 `pairs` de la tabla.  Las etiquetas silver de recorridos se regeneraron con él (conjunto `v3`).
- **(o) Alcance de las etiquetas silver de recorridos.**  La lista `forbidden` de un ítem silver se deriva de su ruta de referencia, no de todas las rutas entre los extremos (`forbidden_scope: reference_path`).  Si el motor devuelve otra ruta, la violación se juzga con el oráculo sobre la ruta devuelta (sobreafirmación); si devuelve la ruta de referencia, con la lista.  Los ítems escritos a mano siguen con alcance `route_pair`: ninguna ruta entre los extremos puede concluir lo prohibido.  Las 2 violaciones restantes de `v2` eran rutas alternativas legítimas según la tabla.

### 6.4 Aclaraciones v1.4 (2026-10-20, después de ver `bench/runs/2026-10-20-r5-conexiones-test`)

Ningún umbral cambia.  Se escribieron **después** de ver esa corrida (217/261 en respondibles, 338 `claim_missing`), por decisión de Adrián (opción A, `docs/plans/r5/estado.md`, punto de control 7).

- **(p) Procedencia a través de reemplazos.**  Si la afirmación esperada no se sirve, el chequeo sigue `superseded_by` en el registro hasta la sucesora vigente.  Cuenta como correcta solo si la sucesora se sirve y trae todos los `(source_id, locator)` esperados; si no, es `successor_missing` o `successor_mismatch`.  El reporte cuenta aparte las resueltas por esta vía (`via_successor`).
- **(q) Conjunto `v3.1`.**  `python -m bench.relabel --source v3 --target v3.1` reetiqueta, con la misma regla de `silver.route_labels`, los recorridos silver cuya ruta de referencia pasa por una afirmación reemplazada (se usa la sucesora, con iguales extremos, fuente y localizador) o `disputed` (R8 la excluye por omisión, así que la ruta no concluye nada y el ítem pasa a abstención).  La etiqueta sale del registro y de `shapes/composition.json`, no de la salida del motor.  Misma membresía de particiones que `v3`; `v3` no se toca.  Registro ítem por ítem en `bench/manifest/v3.1-changelog.jsonl`.

## 7. Prueba de fuga léxica

Una consulta `silver` construida desde el texto de la propia entidad mide si el índice encuentra lo que ya contiene, no si entiende la consulta.  Para cada `qrel` con `grade` ≥ 2 se normaliza (NFKD sin acentos, minúsculas, tokens `\w+`) y se marca **trivial** si todos los tokens de la consulta (el último como prefijo, igual que `Store.search`) aparecen en un mismo campo indexado de la entidad relevante (etiqueta, alias, descripción, enunciado, identificador externo; SPEC §10).  También se reporta el Jaccard máximo entre la consulta y los campos.  El reporte de fuga (`python -m bench leakage`) da, por generador y por origen, la fracción trivial con intervalo Wilson.  Un generador con más de 50% trivial es solo de regresión.

## 8. Semilla honesta de la versión 1

- **Silver** (`origin=silver`, `label_status=silver_auto`): derivados automáticamente de `data/atlas.db` por `python -m bench build-silver`.  Son evidencia de conformidad y de regresión, no oro.  Cada generador declara qué supuesto comparte con el sistema evaluado (`notes`, `circular_with`).
- **Escritos a mano** (`origin=handwritten`, `label_status=authored_by_agent_pending_expert_review`): ≥ 60 consultas de recuperación ES+EN (texto, fórmulas y nombres de Lean) y ≥ 40 recorridos con adversariales, en `bench/items/handwritten/`.  Los escribió un agente; las referencias de Mathlib y Wikidata salen de los vínculos curados de Mathlib (`@[wikidata]` y `docs/1000.yaml`, commit registrado en el manifiesto), pero los textos de consulta, los grados y las etiquetas de saltos esperan revisión de especialistas.
- Meta del informe: 20,000 ítems anotados por especialistas.  Esta versión no tiene ninguno.

## 9. Cómo correr

```bash
python -m bench probe                       # qué componentes existen (binario, HTTP, registro)
python -m bench build-silver --db data/atlas.db
python -m bench split --version v3          # particiones + manifiesto (v1, v2: historia)
python -m bench verify                      # hashes del manifiesto
python -m bench leakage --db data/atlas.db
python -m bench probe --db <copia congelada> --compile   # compila un snapshot vigente de esa copia
python -m bench run --db <copia congelada> --split test --serve --skip-cli --bin <atlas.exe> --snapshot <dir del snapshot>
python -m bench run --db data/atlas.db --split test [--bin <atlas.exe>] [--compile] [--http http://127.0.0.1:8765]
python -m bench run --db tests/fixtures/mini_atlas.json --items bench/items/handwritten/routes.jsonl --compile
python -m bench agreement anotador_a.jsonl anotador_b.jsonl
```

El reporte (`bench/runs/<fecha>-<hash>/report.json` y `report.md`) lleva fecha, hash del registro y del snapshot, versión del manifiesto, hardware y, para cada componente, el veredicto con la regla aplicada.
