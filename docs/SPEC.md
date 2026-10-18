# Especificación ejecutable del atlas · v1

Fecha: 2026-10-16.  Esquema: `SCHEMA_VERSION = 1` (`atlas/model.py`).  Este documento es el contrato que usan ingesta, búsqueda, recorridos, mapa, evaluación e integración.  Lo que aquí se describe está implementado en `atlas/model.py` y `atlas/store.py` y probado en `tests/test_model.py` y `tests/test_store.py`.  Si el código y este texto discrepan, manda el código y se corrige el texto.

Fuentes de diseño: `research/2026-10-16/atlas-investigacion-y-plan.md` (secciones "Arquitectura del atlas" y "Búsqueda y motor de recorridos"), WP01 de `plan-ejecucion.md`, y R01, R02, R50, R51, R52 de `bibliografia-y-evidencia.md` (MMT, Math-in-the-Middle, nanopublicaciones, PROV-O, SHACL).

## 1. Preguntas que el atlas responde

Cada pregunta tiene una operación que la contesta.  Si la operación no existe todavía, se indica el dueño.

| Pregunta | Operación |
|---|---|
| ¿Qué es X y cómo se llama en español e inglés? | `Store.search`, `Store.get_entity`, `/api/entity/{id}` |
| ¿De qué depende lógicamente este enunciado, y con qué premisas *juntas*? | `Store.claims_for(id, roles=[SUBJECT], relations=[DEPENDS_ON_LOGICALLY])` |
| ¿Hay demostraciones alternativas? | `Store.alternatives(group)` |
| ¿Qué necesito saber antes de estudiar X? | relación `prerequisite_for`; rutas en `atlas/paths/` |
| ¿Qué generaliza o especializa a X? | `generalizes` / `special_case_of` |
| ¿Dónde se aplica X y con qué evidencia empírica? | `applied_in`, `models` + procedencia |
| ¿Cómo llego de X a Y y qué autoriza cada salto? | `/api/path` (dueño: recorridos) |
| ¿Está X formalizado? ¿En qué biblioteca, versión y axiomas? | `formalized_as` + `Claim.formal` |
| ¿Quién lo propuso, dónde aparece, a quién influyó? | `named_after`, `appears_in`, `authored_by`, `influenced` |
| ¿Qué fuente sostiene esta conexión, en qué página, quién la extrajo? | `Claim.provenance` |
| ¿Qué cambió y por qué? | `Store.events`, `Store.history` |
| ¿Qué tan cubierta está esta región? | `Store.coverage`, `/api/coverage` |

## 2. Tipos de objeto (`EntityType`)

| Valor | Nombre | Uso |
|---|---|---|
| `concept` | concepto | Idea matemática con nombre (grupo, compacidad). |
| `definition` | definición | Una definición concreta de un concepto; puede haber varias por concepto. |
| `theory` | teoría | Marco con axiomas o lenguaje propio (teoría de grupos, ZFC).  Contexto MMT. |
| `statement` | enunciado | Teorema, lema, proposición, postulado. |
| `proof` | demostración | Un argumento concreto; se enlaza con `proves` y como `via`. |
| `conjecture` | conjetura | Enunciado abierto. |
| `counterexample` | contraejemplo | Objeto que refuta un enunciado bajo supuestos. |
| `algorithm` | algoritmo | Procedimiento efectivo. |
| `model` | modelo | Modelo matemático de algo (SIR, Black-Scholes). |
| `problem` | problema | Pregunta o tarea (problema de los siete puentes). |
| `phenomenon` | fenómeno | Algo del mundo que se modela (difusión, epidemia). |
| `application` | aplicación | Uso documentado fuera de la matemática pura. |
| `publication` | publicación | Artículo, libro, preprint. |
| `source_fragment` | fragmento de fuente | Sección, página o bloque localizado de una publicación. |
| `person` | persona | Autor o figura histórica. |
| `resource` | recurso | Material didáctico, declaración de biblioteca formal, software. |
| `experiment` | experimento | Laboratorio, simulación o estudio empírico. |
| `area` | área | Región de la matemática (por ejemplo, una clase MSC).  Destino de `part_of_area`.  Se agregó a la lista del informe porque `part_of_area` necesita un tipo destino propio. |

Una traducción de un mismo objeto es otra etiqueta del mismo `Entity`.  Dos tradiciones con diferencias conceptuales reales son entidades distintas unidas por una afirmación (`equivalent_to`, `analogous_to`, etc.), nunca una fusión.

### Campos de `Entity`

`id` (estable; patrón `^[A-Za-z0-9][A-Za-z0-9_.:\-/#]{0,255}$`), `type`, `labels` (una etiqueta preferida por idioma, al menos una), `aliases`, `descriptions`, `statements` (texto del enunciado por idioma), `formula` (LaTeX tal como viene en la fuente), `external_ids` (pares `(esquema, valor)`, por ejemplo `("wikidata", "Q…")`, `("mathlib", "Nat.Prime")`, `("msc", "11A41")`), `provenance` (opcional para entidades, obligatoria para afirmaciones).

Convención de prefijos de id (recomendada, no forzada): `c:` concepto, `d:` definición, `s:` enunciado, `p:` demostración, `a:` área, `per:` persona, `pub:` publicación, `app:` aplicación, `src:` fuente, `g:` grupo de alternativas.  Para ingesta idempotente: `stable_id(prefijo, *partes)`.

## 3. Tipos de relación (`RelationType` y `RELATIONS`)

Toda afirmación se lee **SUJETO —relación→ OBJETO**.  Las propiedades viven en `atlas.model.RELATIONS[rel]` (`RelationSpec`) y el motor de recorridos debe derivar sus reglas de ahí, no de copias.

| Relación | Proyección | Simétrica | Transitiva | Inversa | Premisas conjuntas | Sujeto → objeto | Qué autoriza una cadena |
|---|---|---|---|---|---|---|---|
| `depends_on_logically` | proof | no | sí | — | **sí** | matemático → matemático | Dependencia lógica; los supuestos de cada salto se acumulan (unión). |
| `proves` | proof | no | no | — | no | proof/algorithm → statement/conjecture/problem | No encadena; un salto dice que esta demostración establece ese enunciado. |
| `prerequisite_for` | learning | no | sí | — | no | matemático/doc → matemático/doc | Sugerencia de orden de estudio; nunca necesidad lógica. |
| `generalizes` | structure | no | sí | `special_case_of` | no | matemático → matemático | Generalización. |
| `special_case_of` | structure | no | sí | `generalizes` | no | matemático → matemático | Especialización. |
| `equivalent_to` | structure | sí | sí | — | no | matemático → matemático | Equivalencia bajo la unión de los supuestos de todos los saltos. |
| `analogous_to` | structure | sí | **no** | — | no | cualquiera salvo persona | Solo una hipótesis.  Una cadena de analogías no concluye aplicación ni equivalencia. |
| `applied_in` | application | no | no | — | no | matemático → phenomenon/application/experiment/problem/model | Uso documentado en un salto. |
| `models` | application | no | no | — | no | matemático → mundo | Modelo de un fenómeno; la adecuación empírica requiere su propia evidencia. |
| `cites` | biblio | no | no | — | no | doc → doc | Alcance bibliográfico; no implica dependencia ni acuerdo. |
| `influenced` | history | no | no | — | no | cualquiera | Influencia histórica; cada salto necesita fuente histórica propia. |
| `part_of_area` | structure | no | sí | — | no | cualquiera salvo persona → area | Pertenencia a área (la jerarquía de áreas es transitiva). |
| `formalized_as` | proof | no | no | — | no | matemático → matemático/resource | Traducción a un artefacto formal.  Fidelidad ≠ compilación ≠ prueba (R22). |
| `named_after` | history | no | no | — | no | cualquiera salvo persona → person | Solo nombre; no implica autoría ni prioridad. |
| `counterexample_to` | proof | no | no | — | no | counterexample/model/concept/statement/definition → statement/conjecture | Refuta el destino bajo los supuestos declarados. |
| `appears_in` | biblio | no | no | — | no | cualquiera salvo persona → doc | Localización en la literatura. |
| `authored_by` | history | no | no | — | no | doc/matemático → person | Atribución de autoría. |

"Matemático" = concept, definition, theory, statement, proof, conjecture, counterexample, algorithm, model, problem.  "Doc" = publication, source_fragment, resource.  "Mundo" = phenomenon, application, experiment, problem, model.  El store rechaza afirmaciones cuyos extremos no cumplan estos tipos (tabla `relation_endpoints`, derivada de `RELATIONS`).

### Reglas mínimas de composición para el motor de recorridos

Estas reglas son el piso; `atlas/paths/` puede ser más estricto, nunca más laxo.

1. Una cadena solo de una relación transitiva autoriza esa misma relación.  Una no transitiva no encadena.
2. `equivalent_to` puede intercalarse en una cadena de la familia `structure` o `proof` sin cambiar lo que autoriza; los supuestos se acumulan.
3. Cualquier `analogous_to` en la cadena degrada la conclusión a **hipótesis**.
4. `cites`, `appears_in`, `influenced`, `named_after`, `authored_by` nunca producen una conclusión matemática.
5. `prerequisite_for` mezclado con relaciones lógicas produce, como mucho, una ruta de estudio.  Una cadena de `depends_on_logically` tampoco determina el mejor orden de estudio.
6. `B applied_in X` y `A generalizes B` autorizan "A se aplica en X a través de su caso particular B".  `A special_case_of B` y `B applied_in X` no autorizan nada sobre A.
7. Una afirmación conjuntiva (varios `premise`) solo cuenta como salto completo si todas sus premisas están satisfechas.  `Edge.conjunct_count > 1` lo marca; las aristas de una conjunción no son independientes.
8. Las afirmaciones en `superseded` o `retracted` no se usan.  Las `disputed` se usan solo si el usuario lo pide y se muestran como tales.
9. Ningún puntaje de ruta multiplica probabilidades de aristas como si fueran independientes (principio 9).
10. **Conexión histórica como respuesta (decisión de Adrián, 2026-10-17).**  Si entre dos nodos no hay ninguna ruta de fuerza ≥ `min_strength` y sí una cuya conclusión completa es `historical` (fuerza 0, cadenas de `named_after`, `influenced` o `authored_by`), el motor la devuelve como respuesta, no como abstención, con `conclusion.category = "historical"` y la advertencia de que no es dependencia matemática, prerrequisito ni aplicación.  Una ruta de fuerza ≥ 1 siempre va antes: si existe, la histórica no se muestra.  La regla 4 no cambia: una cadena histórica seguida de una relación matemática no autoriza nada.  Las conexiones `bibliographic` y `documentary` (fuerza 0 con citas) siguen siendo abstención `only_weak` con la conexión como exploración.

## 4. Afirmaciones n-arias (`Claim`)

La unidad de verdad del atlas.  Cada afirmación es un nodo con participantes etiquetados por rol.

| Rol | Significado |
|---|---|
| `subject` | De quién se afirma algo.  Exactamente uno. |
| `object` | El otro extremo de una relación binaria.  Exactamente uno, salvo que haya premisas. |
| `premise` | Conjunto conjuntivo: todas las premisas juntas.  Solo en relaciones con `allows_premises` (hoy: `depends_on_logically`).  Incompatible con `object`. |
| `assumption` | Hipótesis que es una entidad del atlas (por ejemplo, el quinto postulado). |
| `context` | Teoría o área en la que se enuncia la afirmación. |
| `via` | La demostración, argumento o método que realiza la relación. |

Campos además de los participantes: `relation`, `provenance` (≥1), `evidence_state`, `assumptions` (texto libre de hipótesis que aún no son entidades), `validity_domain`, `alternative_group`, `supersedes`, `superseded_by` (estado, lo llena el store), `formal` (estado), `confidence` (valor + qué evento estima + cómo se calibró), `historical_date`, `explanation` (texto para el lector, por idioma), `disagreements`.

Reglas de forma (implementadas en `claim_shape_errors`, en el validador JSON-LD y en `shapes/atlas.shacl.ttl`):

- exactamente un `subject`;
- exactamente un `object`, o al menos una `premise` y ningún `object`;
- `premise` solo en relaciones que lo permiten;
- el sujeto no puede ser su propio objeto o premisa; una entidad no se repite en un mismo rol;
- al menos una procedencia completa;
- `formally_linked` exige `formal`; `superseded` exige `superseded_by`;
- `confidence`, si existe, dice qué estima y cómo se calibró.

Reglas que comprueba el store al insertar: las entidades existen; las fuentes de la procedencia están registradas; los tipos de los extremos son válidos para la relación; todas las afirmaciones de un `alternative_group` comparten sujeto y relación; no se inserta directamente en `superseded`/`retracted` ni con `supersedes` (para eso está `supersede`).  Un lote que falla se revierte completo.

### Conjunción y alternativas: ejemplo del fixture

El teorema de Pitágoras tiene dos demostraciones.  La de Euclides (Elementos I.47) necesita LAL **y** la relación de áreas de I.41; la otra necesita semejanza de triángulos.  Son dos afirmaciones del mismo grupo:

```json
{"id": "c:pyth-euclid", "relation": "depends_on_logically",
 "participants": [{"role": "subject", "entity": "s:pythagoras"},
                  {"role": "premise", "entity": "s:sas-congruence"},
                  {"role": "premise", "entity": "s:parallelogram-area"},
                  {"role": "via", "entity": "p:euclid-i47"},
                  {"role": "context", "entity": "a:euclidean-geometry"},
                  {"role": "assumption", "entity": "s:parallel-postulate"}],
 "alternative_group": "g:pythagoras-proofs",
 "provenance": [{"source_id": "src:fixture-hand", "locator": "Elements I.47", "extractor": "hand-written-fixture",
                 "extractor_version": "1", "license": "CC0-1.0", "retrieved_at": "2026-10-16T00:00:00+00:00"}],
 "evidence_state": "extracted"}
```

`c:pyth-similarity` está en el mismo grupo con la premisa única `s:similar-triangles`.  Lectura: Pitágoras se sigue de (LAL ∧ I.41) **o** de (semejanza).  Archivo: `tests/fixtures/mini_atlas.json` (17 entidades, 16 afirmaciones, licencia CC0, escrito a mano para pruebas; sus estados son `extracted` salvo uno `model_proposed`, porque nadie lo ha revisado editorialmente).

## 5. Estados de evidencia (`EvidenceState`)

| Estado | Significado | Cómo se llega |
|---|---|---|
| `extracted` | Extraído automáticamente de una fuente estructurada o de texto. | Inserción. |
| `model_proposed` | Propuesto por un modelo (embedding, LLM, R-GCN). | Inserción. |
| `editorially_reviewed` | Revisado por una persona identificada. | `set_evidence_state(..., actor=<persona>)`; `actor="system"` se rechaza. |
| `formally_linked` | Vinculado a un artefacto formal. | `set_evidence_state(..., formal=FormalArtifact(library, version, declaration, axioms))`. |
| `disputed` | Hay desacuerdo registrado. | `set_evidence_state`. |
| `superseded` | Sustituido por una nueva versión. | Solo `supersede()`. |
| `retracted` | Retirado.  Terminal. | `set_evidence_state`. |

Transiciones (`TRANSITIONS`): entre los cinco estados vigentes (`CURRENT_STATES`) se puede pasar libremente y a `retracted`; `superseded` solo puede pasar a `retracted`; `retracted` no sale.  Todo cambio pide `reason` y deja un evento `evidence_changed` con `from`, `to`, `reason` y el artefacto formal.

"Formal" nombra el artefacto (biblioteca, versión, declaración, axiomas) y no se extiende a la explicación ni a la aplicación.  Traducción, fidelidad y prueba se registran por separado: `formalized_as` con su estado es la traducción; la fidelidad es otra afirmación o una evaluación de `bench/`.

Simplificación conocida: hoy cada afirmación tiene un estado vigente y su historia completa en eventos.  El informe pide "varias evaluaciones simultáneas"; cuando haga falta se agrega una tabla `assessments` con migración v2 sin romper esta API.

## 6. Procedencia y fuentes

`Provenance`: `source_id` (fuente registrada), `locator` (página, sección, proposición, línea, xpath o nombre de declaración), `extractor` y `extractor_version`, `license`, `retrieved_at` (ISO-8601) y `quote` opcional (fragmento justificativo, solo si la licencia lo permite).  Toda afirmación lleva al menos una.  Las entidades pueden llevarla y se recomienda.

`Source` (registro WP02): `id`, `title`, `license` (SPDX o URL; `"unknown"` se rechaza), `uri`, `version`, `retrieved_at`, `terms`.  Los adaptadores registran la fuente con `add_source` antes de insertar datos que la citan.

## 7. Versiones

`supersede(old_id, new_claim, actor, reason)` inserta la nueva versión con `supersedes=old_id`, pone la anterior en `superseded` con `superseded_by` y registra `claim_superseded`.  Solo se sustituye una afirmación vigente.  `history(id)` devuelve todas las versiones, de la más vieja a la más nueva.  Las lecturas por defecto (`claims_for`, `iter_edges`) solo ven estados vigentes; `iter_claims` ve todo salvo que se filtre.

## 8. Registro de eventos

Tabla `events` de solo anexar: dos triggers abortan cualquier `UPDATE` o `DELETE`.  Cada evento tiene `seq`, `ts`, `actor`, `kind`, `target_id` y `payload` JSON.

| `kind` | `target_id` | `payload` |
|---|---|---|
| `migration` | `schema:<n>` | `{from, to}` |
| `source_added` | fuente | `{license}` |
| `entity_added` | entidad | `{type}` |
| `entity_replaced` | entidad | `{}` |
| `claim_added` | afirmación | `{}` (el contenido está en `claims`) |
| `evidence_changed` | afirmación | `{from, to, reason, formal}` |
| `claim_superseded` | afirmación vieja | `{by, reason, from}` |
| `merge` | entidad fusionada | `{merge_id, into, reason, evidence}` |
| `merge_undone` | entidad fusionada | `{merge_id, from, reason}` |

La fuente de verdad es `claims` + `claim_participants` + `provenance` + `events`.  FTS5, layouts, embeddings y resúmenes son derivados: `rebuild_fts()` reconstruye el índice léxico desde las entidades.

## 9. Fusiones reversibles

`merge(survivor_id, merged_id, actor, reason, evidence)` exige actor identificado, razón, mismo tipo y que ninguna de las dos esté ya fusionada.  No reescribe afirmaciones: marca `entities.merged_into` y las lecturas resuelven (`resolve`, `merged_ids`, `get_entity`, `claims_for`, `iter_edges`, `search`, `export_jsonld`).  `undo_merge(merge_id, actor, reason)` borra la redirección y deja `undone_at`; ambas dejan evento.  Los embeddings proponen candidatos; fusionar es siempre una decisión registrada.  Un lector en otra conexión ve la fusión en cuanto se confirma (se usa `PRAGMA data_version`).

## 10. Idiomas y búsqueda léxica

Español e inglés son de primera clase: etiquetas, alias, descripciones, enunciados y explicaciones se guardan por idioma (`LangString(text, lang)`, etiqueta BCP-47 simple).  `Entity.label(lang)` cae a inglés y luego a la primera etiqueta.

Índice FTS5 (`unicode61 remove_diacritics 2`, prefijos de 2 y 3): una fila por etiqueta, alias, descripción, enunciado e identificador externo.  `search(q, lang, limit, types)` ignora acentos y mayúsculas, trata el último término como prefijo, sanea la consulta (solo tokens `\w+`, entrecomillados), pondera campos (`label` 3, `alias` 2, `external_id` 2, `description` 1, `statement` 1), resuelve fusiones y deduplica por entidad.  Es la vía léxica; la búsqueda híbrida (fórmulas, vectores, RRF) vive en `core/atlas-search` (Rust); esta vía FTS5 es el baseline léxico de AtlasBench y el respaldo del binario cuando falta el índice.

## 11. Cobertura honesta

`coverage()` devuelve conteos por tipo, relación, estado y fuente, fusiones activas y `unknown_by_type`: entidades sin ninguna afirmación.  Se llaman **desconocidas**, no aisladas.  La UI y la API usan esa palabra.

## 12. Almacenamiento

SQLite en modo WAL, `synchronous=NORMAL`, llaves foráneas activas, caché de 256 MiB por conexión.  Tablas: `sources`, `entities`, `external_ids`, `claims`, `claim_participants` (WITHOUT ROWID, llave `(claim_id, position)`, índice por entidad y rol), `provenance`, `events`, `merges`, `relation_endpoints`, `fts`, `meta`.  Versión en `PRAGMA user_version`; `MIGRATIONS[n]` es el DDL que lleva de n-1 a n, se aplica al abrir y registra un evento.  Las migraciones se agregan, nunca se editan una vez publicadas.  Una conexión por hilo; para lectores concurrentes, `Store(path, read_only=True)`.

### Rendimiento medido

Hardware: la máquina de desarrollo de AJ (Windows 11, Python 3.14.6, SQLite 3.50.4).  Datos sintéticos: enunciados con etiqueta ES/EN; un tercio de las afirmaciones son conjunciones de dos premisas y el resto binarias.  Fecha: 2026-10-16.

| Comando | Entidades | Afirmaciones | Inserción | Recorrer aristas | `claims_for` p50/p95 | `search` p50/p95 | Base |
|---|---|---|---|---|---|---|---|
| `python -m pytest -q tests/test_store.py -k bulk` | 20,000 | 100,000 | 4.00 s (25,012/s) | 133,334 en 0.60 s | 0.20 / 0.55 ms | 0.40 / 0.58 ms | 90 MiB |
| `ATLAS_BULK_CLAIMS=1000000 python -m pytest -q tests/test_store.py -k bulk` | 200,000 | 1,000,000 | 65.48 s (15,272/s) | 1,333,334 en 8.00 s | 0.34 / 0.57 ms | 3.39 / 7.06 ms | 920 MiB |

La velocidad por afirmación baja con el tamaño (25k/s a 15k/s) por el mantenimiento de índices de texto.  Para decenas de millones, el siguiente paso medible es usar llaves enteras en `claim_participants` y `provenance` (migración v2).  Las metas del informe (p95 < 100 ms de interacción local, < 2 s de búsqueda caliente) se refieren al sistema completo, no a esta capa.

## 13. JSON-LD y SHACL

`export_jsonld(entity_ids, depth, current_only)` hace una búsqueda en anchura por afirmaciones hasta `depth` saltos e incluye toda entidad referida, las afirmaciones y sus fuentes.  El contexto (`shapes/context.jsonld`) va embebido.  Vocabulario: `https://w3id.org/mpm-atlas/vocab#` (prefijo `atlas:`); ids: `aid:` + id codificado (`aid:c%3Apyth-euclid`).  El dominio w3id **no está registrado**; es un marcador hasta publicar.  Se usan `skos:prefLabel`/`altLabel` con contenedor de idioma, `prov:hadPrimarySource`, `dct:license`.

`shapes/atlas.shacl.ttl` define `ClaimShape`, `ParticipantShape`, `ProvenanceShape`, `FormalArtifactShape` y `ConfidenceShape` con las reglas de la sección 4.  `validate_jsonld(doc)` es su gemelo en Python puro (sin rdflib ni pyshacl) y devuelve la lista de violaciones.  No se corrió un validador SHACL externo sobre el `.ttl`: no hay rdflib en el entorno y no se agregó la dependencia.

## 14. API de Python estable

Importar desde `atlas`.  Todo es inmutable; nada se muta en sitio.  Errores de contrato: `atlas.ValidationError` (subclase de `ValueError`).

```python
# model
EntityType, RelationType, EvidenceState, Role, Projection, EventKind      # StrEnum
RELATIONS: Mapping[RelationType, RelationSpec]                             # semántica de relaciones
CURRENT_STATES: frozenset[EvidenceState]; TRANSITIONS; SCHEMA_VERSION
LangString(text: str, lang: str)
Source(id, title, license, uri=None, version=None, retrieved_at=None, terms=None)
Provenance(source_id, locator, extractor, extractor_version, license, retrieved_at, quote=None)
Confidence(value: float, estimates: str, calibration: str)
FormalArtifact(library, version, declaration, axioms: tuple[str, ...] = ())
Participant(role: Role, entity_id: str)
Entity(id, type, labels, aliases=(), descriptions=(), statements=(), formula=None, external_ids=(), provenance=())
    .label(lang="es") -> str; .to_json() -> dict; Entity.from_json(d) -> Entity
Claim(id, relation, participants, provenance, evidence_state=EXTRACTED, assumptions=(), validity_domain=None,
      alternative_group=None, supersedes=None, superseded_by=None, formal=None, confidence=None,
      historical_date=None, explanation=(), disagreements=())
    .subject -> str; .targets -> tuple[str, ...]; .is_conjunction -> bool; .ids(role) -> tuple[str, ...]
    .to_json() -> dict; Claim.from_json(d) -> Claim
binary(claim_id, relation, subject, obj, provenance, **kw) -> Claim
conjunction(claim_id, relation, subject, premises, provenance, **kw) -> Claim   # kw "extra": más participantes
claim_shape_errors(claim) -> list[str]
stable_id(prefix: str, *parts: str) -> str
Edge(claim_id, relation, source, target, evidence_state, conjunct_count, alternative_group)
SearchHit(entity_id, field, lang, text, score)          # score: mayor es mejor
Event(seq, ts, actor, kind, target_id, payload);  Merge(id, survivor_id, merged_id, actor, reason, evidence, created_at, undone_at)

# store
Store(path: str | Path = ":memory:", *, read_only: bool = False)   # context manager
  .schema_version -> int; .migrate() -> int; .transaction()  # anidable
  .add_source(source, *, actor="system"); .get_source(id) -> Source | None; .iter_sources()
  .add_entity(entity, *, actor="system", on_conflict="error")
  .add_entities(entities, *, actor="system", on_conflict="error"|"ignore"|"replace", batch_size=20000) -> int
  .get_entity(id, *, resolve=True) -> Entity | None
  .iter_entities(type=None, *, include_merged=False, batch=5000) -> Iterator[Entity]
  .find_by_external_id(scheme, value) -> list[str]
  .add_claim(claim, *, actor="system"); .add_claims(claims, *, actor="system", batch_size=20000) -> int
  .get_claim(id) -> Claim | None; .existing_claim_ids(ids) -> set[str]   # chequeo por lote (ingesta)
  .iter_claims(relation=None, *, states=None, batch=5000) -> Iterator[Claim]
  .claims_for(entity_id, *, roles=None, relations=None, current_only=True) -> list[Claim]
  .alternatives(group) -> list[Claim]
  .iter_edges(*, relations=None, states=CURRENT_STATES, resolve=True) -> Iterator[Edge]
  .set_evidence_state(claim_id, state, *, actor, reason, formal=None) -> Claim
  .supersede(old_id, new_claim, *, actor, reason) -> Claim
  .history(claim_id) -> list[Claim]
  .merge(survivor_id, merged_id, *, actor, reason, evidence=None) -> Merge
  .undo_merge(merge_id, *, actor, reason) -> Merge
  .get_merge(id) -> Merge | None; .merges(*, active_only=False) -> list[Merge]
  .resolve(entity_id) -> str; .merged_ids(entity_id) -> list[str]
  .search(query, *, lang=None, limit=20, types=None) -> list[SearchHit]
  .rebuild_fts() -> int
  .events(target_id=None, *, kinds=None, since_seq=0) -> Iterator[Event]
  .coverage() -> dict
  .export_jsonld(entity_ids, *, depth=1, current_only=True) -> dict
load_bundle(store, {"sources": [...], "entities": [...], "claims": [...]}, *, actor="system") -> dict[str, int]
validate_jsonld(doc) -> list[str]
```

Reglas para los demás módulos: no escribir SQL contra las tablas del store (la API de arriba es el contrato); no guardar la única copia de nada fuera del store; todo adaptador registra su `Source` y llena `Provenance` completa; los identificadores de los extremos de `Edge` ya vienen resueltos.

## 15. Contratos HTTP (JSON)

Servidor: `atlas serve` (crate `core/atlas-server`, axum).  UTF-8, `Content-Type: application/json`.  Éxito: el objeto descrito.  Error: estado HTTP 4xx/5xx y `{"error": {"code": "not_found" | "bad_request" | "internal", "message": "..."}}`.  Toda lista larga lleva `limit` y `truncated`.  `label` se da en el idioma pedido (`lang`, por defecto `es`) con la regla de `Entity.label`.

### `GET /api/search?q=&lang=es&types=statement,concept&limit=20`

```json
{"query": "teorema de pit", "lang": "es", "limit": 20, "truncated": false,
 "hits": [{"id": "s:pythagoras", "type": "statement", "label": "Teorema de Pitágoras",
           "matched": {"field": "label", "lang": "es", "text": "Teorema de Pitágoras"},
           "score": 17.66,
           "lanes": {"lexical": {"rank": 1, "score": 17.66}, "formula": null, "semantic": null}}]}
```

`q` admite de 1 a 256 caracteres; más largo responde 400 `bad_request` (la cadena de consulta completa admite 2,048 bytes).  `lanes` muestra el aporte de cada vía para diagnóstico; `score` es el puntaje fusionado (RRF u otro, lo decide búsqueda y lo documenta en su módulo).

### `GET /api/entity/{id}?lang=es`

```json
{"entity": { "...": "Entity.to_json()" },
 "canonical_id": "s:pythagoras", "merged_from": ["s:pyth-dup"],
 "coverage": "known",
 "counts_by_relation": {"depends_on_logically": 2, "generalizes": 2},
 "claims": [{ "...": "Claim.to_json()", "labels": {"s:sas-congruence": "Criterio de congruencia LAL"} }]}
```

`claims` se trunca en `limit` (200 por defecto, máximo 1000) y no hay `offset`: si `truncated` es `true`, faltan afirmaciones y `counts_by_relation` da los totales.  `coverage` es `"unknown"` cuando la entidad no participa en ninguna afirmación.  Si `{id}` era una entidad fusionada, se responde la sobreviviente y `canonical_id` lo dice.

### `GET /api/neighborhood?id=&depth=1&relations=&states=&limit=200&lang=es`

```json
{"center": "s:pythagoras", "depth": 1, "truncated": false,
 "nodes": [{"id": "s:pythagoras", "type": "statement", "label": "Teorema de Pitágoras", "coverage": "known"}],
 "edges": [{"claim_id": "c:pyth-euclid", "relation": "depends_on_logically", "source": "s:pythagoras",
            "target": "s:sas-congruence", "evidence_state": "extracted", "conjunct_count": 2,
            "alternative_group": "g:pythagoras-proofs"}],
 "hidden": {"cites": 120}}
```

Las aristas son `Edge`.  `hidden` cuenta por relación lo que quedó fuera por `limit` o filtros: la agregación conserva tipo y cantidad.

### `GET /api/path?from=&to=&objective=best_supported&max_hops=6&lang=es`

`objective` ∈ `fewest_new`, `best_supported`, `most_intuitive`, `most_formal`, `most_diverse`, `shortest`.

```json
{"from": "c:right-triangle", "to": "s:pythagoras-converse", "objective": "best_supported",
 "weights": {"evidence": 1.0, "length": 0.2},
 "abstained": false, "reason": null,
 "paths": [{"score": 0.0,
   "conclusion": {"authorizes": "study order suggestion", "assumptions": ["Euclidean plane"],
                  "weakest_evidence": "extracted", "is_hypothesis": false,
                  "category": "pedagogical"},
   "steps": [{"claim_id": "c:rt-prereq", "relation": "prerequisite_for", "from": "c:right-triangle",
              "to": "s:pythagoras", "direction": "forward", "evidence_state": "extracted",
              "authorizes": "a study order suggestion; never a logical necessity",
              "assumptions": [], "premises": [], "premises_missing": []}]}]}
```

`conclusion.category` (desde 2026-10-17) dice cómo rotular la ruta ante el lector: `mathematical` (familias proof, formal y structure matemática), `structural` (pertenencia a área), `application`, `pedagogical` (ruta de estudio), `candidate_hypothesis` (analogía), `historical`, `documentary` (bibliográfica o mixta) o `none`.  Se deriva de la familia de la conclusión en `shapes/composition.json`, nunca de los saltos sueltos.  Una respuesta `historical` se presenta como «conexión histórica» (regla 10 de §3).

`abstained: true` con `reason` cuando no hay conexión suficiente; nunca se inventa una ruta.  `direction: "inverse"` cuando se recorre la relación al revés (vía `RelationSpec.inverse` o simetría).  Los pesos son visibles y ajustables.

### `GET /api/map/tiles?layer=structure&z=0&x=0&y=0`

```json
{"layer": "structure", "layout_version": "<hash del layout>", "z": 0, "x": 0, "y": 0,
 "bbox": [0.0, 0.0, 1.0, 1.0],
 "nodes": [{"id": "a:euclidean-geometry", "kind": "cluster", "label": "Geometría euclidiana",
            "x": 0.42, "y": 0.31, "size": 17, "community": "L0:3"}],
 "edges": [{"source": "L0:3", "target": "L0:7", "relation": "generalizes", "count": 12}],
 "aggregates": [{"cluster_id": "L0:3", "members": 17, "hidden_relations": {"cites": 4}}]}
```

`kind` ∈ `entity`, `cluster`.  Las posiciones son estables dentro de un `layout_version`.  Una arista agregada lleva una sola relación y su cuenta: nunca se agrupan relaciones distintas en una sola línea.

### `GET /api/coverage`

Exactamente `Store.coverage()` más `generated_at`:

```json
{"generated_at": "2026-10-16T12:00:00-06:00", "schema_version": 1, "sources": 1,
 "entities_by_type": {"statement": 8}, "claims_by_relation": {"depends_on_logically": 5},
 "claims_by_state": {"extracted": 15, "model_proposed": 1}, "unknown_by_type": {},
 "claims_by_source": {"src:fixture-hand": 16}, "active_merges": 0}
```

## 16. Pendiente y límites conocidos

- Varias evaluaciones simultáneas por afirmación (tabla `assessments`, migración v2).
- Llaves enteras para escalar a decenas de millones de afirmaciones (migración v2, medir antes y después).
- Validación SHACL externa del `.ttl` (requiere rdflib + pyshacl; decidir en `docs/STATUS.md`).
- Registrar el dominio del vocabulario (w3id) antes de publicar exportaciones.
- La búsqueda léxica no indexa las explicaciones de las afirmaciones; si búsqueda lo necesita, se agrega como campo con su propia fila.
