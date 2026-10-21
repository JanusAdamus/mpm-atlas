# ADR-0005 · Esquema v4 (ronda 5): fechas históricas, obras, curaduría, fichas y escala

Fecha: 2026-10-19.  Autor: arquitecto (dueño de `atlas/model.py`, `atlas/store.py`, `docs/SPEC.md`).  Estado: **aceptado** (2026-10-19; paso 1 implementado con sus pruebas en verde; ver «Estado» al final).  Plan que lo pide: `docs/plans/2026-10-19-ronda-5-calidad-y-fuentes.md` (aprobado por Adrián, con la decisión del 2026-10-19 de ingerir todas las obras).

Numeración: no existen ADR 0001–0004 como archivos.  Las decisiones de los esquemas v1–v3 están en `docs/SPEC.md` §12.  Este ADR usa el número 0005 por indicación del orquestador; los siguientes continúan desde aquí.

Convención de cifras: **medido** cita archivo y fecha; **NV** es una estimación no verificada que el piloto (§8.6) debe medir antes de comprometer la ingesta completa (principio 8).

## Contexto

Lo que el contrato v3 no puede expresar hoy (todas las cifras vienen de `docs/plans/r5/diagnostico-calidad.md` y `papers-historia.md`, 2026-10-19):

- **Fechas.**  `_check_iso` usa `datetime.fromisoformat`, que no acepta años ≤ 0.  `Claim.historical_date` es texto libre sin validar y lo usan 0 de 2,828,345 afirmaciones.  Wikidata numera los años antes de Cristo sin año cero (−300 = 300 a. C.); ISO 8601 y EDTF usan numeración astronómica (−0299 = 300 a. C.).  Copiar el número tal cual equivoca un año en todas las fechas antes de Cristo.
- **Obras.**  Hay 0 entidades `publication` y no se distingue obra, testimonio físico y edición: Plimpton 322, el papiro Rhind (Ahmes figura como P50 en Wikidata, pero es copista) y las *Disquisitiones*, que OpenAlex parte en tres works.
- **Relaciones que se leen más fuertes de lo que son:**
  - 25,786 aristas agregadas concepto→concepto se muestran `extracted` aunque se sostienen en pertenencias `model_proposed`.
  - P460 se ingiere como `equivalent_to` transitivo («grafo ≡ grafo finito»).
  - P31 y P279 se ingieren las dos como `special_case_of`.
  - Hay relaciones registradas en los dos sentidos.
  - 244,933 `part_of_area` están en `formally_linked` solo porque la declaración vive en ese archivo.
- **Curaduría.**  No hay un lugar en el registro para exclusiones, rótulos de presentación ni propuestas de fusión.  El puente que propuso el diagnóstico (`shapes/curation/r5.jsonl`) rompe el principio 2.
- **Fichas.**  Ningún texto de ficha tiene procedencia por frase y 0 afirmaciones están `editorially_reviewed`.
- **Escala.**  El grafo actual tiene 320,214 nodos (`docs/STATUS.md`, build9).  El registro mide 14 GB según el orquestador (STATUS registra 3.4 GB en una ronda anterior; la diferencia no está descompuesta).  El snapshot mide 406,008,820 bytes (build9).  Las obras son unos 5.2 millones en zbMATH Open y 4.6 millones en OpenAlex (campo 26), con solapamiento sin medir.

## Decisión

### 1. Fechas históricas: subconjunto EDTF con tipo, calendario y procedencia por fecha

**Se decide.**  Un tipo nuevo de valor en `model.py`:

```python
class DateKind(StrEnum):
    COMPOSED = "composed"    # composición de la obra (estimada)
    COPIED = "copied"        # producción del testimonio físico (copia o autógrafo: tablilla, papiro, manuscrito)
    PUBLISHED = "published"  # primera publicación
    EDITION = "edition"      # fecha de una edición o traducción concreta
    BORN = "born"; DIED = "died"; FLORUIT = "floruit"   # personas (P569/P570), mismo mecanismo

@dataclass(frozen=True)
class HistoricalDate:
    kind: DateKind
    edtf: str                 # subconjunto EDTF nivel 0/1, numeración ASTRONÓMICA (0000 = 1 a. C.)
    provenance: Provenance    # obligatoria: cada fecha tiene su fuente
    calendar: str = "gregorian"   # "gregorian" | "julian" | "unknown"; nunca se convierte en silencio
    raw: str | None = None    # valor exacto de la fuente, p. ej. Wikidata "-0300-00-00T00:00:00Z/7 Q1985786"
```

`Entity.dates: tuple[HistoricalDate, ...] = ()` (campo nuevo y opcional; `from_json` acepta su ausencia).  `Claim.historical_date` conserva su forma (`str | None`), pero pasa a validarse con el mismo analizador.  Ninguna fila actual lo usa, así que no hay que migrar datos.

**Subconjunto EDTF aceptado**:
- Fecha: `[-]YYYY(-MM(-DD)?)?`, con el año en 4 dígitos y signo solo para negativos.
- Calificador al final: `?` (dudosa), `~` (aproximada) o `%` (las dos).
- Dígitos no especificados `X` (`19XX`, `180X`), solo en años ≥ 0.
- Intervalo `A/B`, con extremos abiertos `../B` y `A/..`.

**Validación sin `datetime`**: expresión regular, mes entre 1 y 12, y día ≤ días del mes.  El año bisiesto se calcula con la regla del calendario declarado: gregoriana proléptica en numeración astronómica (el año 0 es bisiesto) o juliana (`y % 4 == 0`).  En un intervalo se exige `A ≤ B`.  El validador también deriva `lo_year` y `hi_year` (enteros astronómicos inclusivos; `None` si el extremo es abierto), que alimentan la tabla derivada `entity_dates`.

**Conversión centralizada desde Wikidata** (`atlas/ingest/wikidata`: una sola función, `edtf_from_wikidata(time, precision, calendar_qid, qualifiers, channel)`):
- Año histórico negativo `y` → astronómico `y + 1` (−300 → `-0299`; −1 → `0000`).  La salida nunca contiene un año 0 que venga de una entrada histórica.
- Precisión 9 (año), 10 (mes) y 11 (día): se escriben tal cual, con los ceros de mes y día recortados.
- Precisión 8, 7 y 6 (década, siglo, milenio) → intervalo explícito.  Por ejemplo, el siglo III a. C. es `-0299/-0200`.  No se usa `X` en años negativos, porque `-02XX` en numeración astronómica es otro siglo.
- Calificadores: P1480 = circa (Q5727902) → `~`; presumiblemente (Q18122778) → `?`; P1319 y P1326 → extremos de un intervalo.
- `calendar_qid` Q1985786 → `julian`.  No se convierte entre calendarios.
- `channel` ∈ {`json`, `wdqs`}.  Que la exportación RDF de WDQS ya venga en numeración astronómica es **NV**.  El piloto lo verifica con *Elementos* (Q172891) por los dos canales: los dos tienen que dar el mismo EDTF.

**Varias fechas que no coinciden** (Plimpton 322: CDLI dice «ca. 1900-1600 BC» → `-1899~/-1599~`; Wikidata dice −1799 con precisión de año y sin calificador → `-1798`):
- Se guardan las dos, cada una con su procedencia.
- La vista muestra cada fecha distinta con su fuente.
- La línea de tiempo coloca la obra en la unión de los intervalos.
- Nunca se muestra un año solo como si fuera la fecha.

**Otros calendarios** (OpenAlex da 1403 en un artículo persa, probablemente año hijri solar): se guarda con `?`, `calendar="unknown"` y el valor crudo.  Lo marca un filtro de plausibilidad contra las fechas de vida del autor.  Se cuenta en el informe del lote y no se rechaza: la meta es «0 fechas antiguas rechazadas».

**Alternativas consideradas**:
- **`python-edtf` (PyPI).**  Agrega dos dependencias y trabaja sobre `struct_time`, que maneja mal los años ≤ 0.  Solo hace falta un subconjunto de unas 60 líneas.  Descartada.
- **Fechas como afirmaciones n-arias** (relación `dated`).  Un valor de fecha no es una entidad y esto obligaría a crear millones de nodos.  Descartada.
- **Columnas `year_lo` y `year_hi` en `entities`.**  Pierden la incertidumbre, el tipo y la fuente.  Descartada; se conservan solo como tabla derivada.

**Consecuencias**:
- Rust no necesita analizar EDTF: lee `lo` y `hi` de `entity_dates`.
- La interfaz convierte el año astronómico a la forma de lectura con prueba (`-0299` → «300 a. C.»).
- Hoy `entity_replaced` no guarda la versión anterior, lo que haría perder el historial al corregir una fecha.  Desde v4 su payload lleva `previous` (los datos anteriores completos), y un reemplazo idéntico no hace nada y no deja evento.

### 2. Obra, testimonio y edición: opción mínima con un tipo nuevo, `witness`

**Se decide para esta ronda**:
- `publication` = **obra** (texto abstracto, autor atribuido): *Elementos*, *Disquisitiones*, un artículo de Crelle.
- `witness` (`EntityType.WITNESS`, nuevo, dentro de `DOCS`) = **testimonio físico**: tablilla, papiro, manuscrito.  Se identifica por `external_ids` (`cdli`, `museum`, `wikidata`) y lleva fechas `copied`.  Se agrega ahora porque el plan ingiere las 2,274 tablillas de CDLI, y `papers-historia.md` §5.2 decía que la opción mínima solo bastaba mientras CDLI no entrara más allá de una lista curada.
- **Ediciones y traducciones**: cada registro de fuente es su propia entidad `publication` (`oa:W…`, `zb:…`) con su fecha `edition`, y se **fusiona** con la obra mediante `Store.merge`.  El campo `evidence` va estructurado: `{"kind": "edition_of", "key": "…"}`.  Las citas de las tres *Disquisitiones* de OpenAlex se juntan en la obra y cada fusión se puede deshacer (`undo_merge`).
- No hay relaciones `witness_of` ni `edition_of` en esta ronda.

**Ejemplos.**  Las fechas son ilustrativas: un trabajador las verifica contra la fuente antes de ingerirlas.
- **Plimpton 322**: `witness`, `cdli:P254790`, dos fechas `copied` (CDLI y Wikidata, §1).  Sin `authored_by`.
- **Papiro Rhind**: `witness`; `copied_by` → Ahmes (lista curada con fuente historiográfica); fecha `copied` de alrededor de 1550 a. C. y fecha `composed` aproximada del original (el colofón lo remite a un texto anterior).  El P50 de Wikidata **no** se lee como `authored_by` en un testimonio.
- **Disquisitiones**: obra `wd:Q…` como sobreviviente; los tres works de OpenAlex (1801, 1966, 1986) se fusionan con `edition_of`, y sus fechas `edition` siguen siendo de cada edición.

**Cómo evoluciona**: se agrega `edition_of` (biblio, no encadenable) y, si hace falta, `witness_of`.  La migración es mecánica: por cada fusión con `evidence.kind == "edition_of"` se llama `undo_merge` y se inserta la afirmación `edition_of`; el compilador agrega las citas a través de esa relación.

**Alternativas**:
- **Opción completa ya** (`edition_of` y `witness_of` en v4).  Implica más trabajo en recorridos y en la agregación de citas sin una necesidad medida.  Se pospone.
- **Sin `witness`** (todo como `publication`).  Mostraría una tablilla como «publicación» y no podría expresar al copista con tipos de extremo.  Descartada.

### 3. Roles de autoría: tres relaciones nuevas, todas en la proyección history y ninguna encadenable

| Relación | Sujeto → objeto | `chain_authorizes` |
|---|---|---|
| `authored_by` (ya existe; DOCS crece con `witness`) | doc/matemático → person | atribución de autoría |
| `copied_by` | **witness** → person | «copió este testimonio; no implica autoría ni prioridad» |
| `edited_by` | publication → person | «editó esta edición; no implica autoría» |
| `translated_by` | publication → person | «tradujo esta edición; no implica autoría» |

Las atribuciones en disputa se resuelven con lo que ya existe: `alternative_group`, `disputed` y `disagreements`.  Wikidata: P50 → `authored_by` solo cuando el sujeto es `publication`; P98 → `edited_by`; P655 → `translated_by`.  El copista se cura a mano en esta ronda; no se mapea ninguna propiedad de Wikidata (NV).

**Alternativa**: una sola relación `contributed_by` con un calificador de rol.  Mezcla tipos de relación (principio 4) y obliga a crear un campo nuevo en `Claim`.  Descartada.

### 4. `introduced_in`: fuente historiográfica obligatoria; una cita no autoriza nada matemático

- Relación nueva `introduced_in`: matemático → `publication | witness`, proyección history, no transitiva, sin premisas.  `chain_authorizes`: «una fuente historiográfica dice que aquí se introdujo; no encadena y no implica demostración ni prerrequisito».
- `Source.kind: str | None` (campo nuevo), con valores `dataset | library | encyclopedia | historiography | editorial`.  **El store rechaza** una afirmación `introduced_in` si ninguna de sus procedencias apunta a una fuente con `kind == "historiography"` (se comprueba en `_check_batch`, con `json_extract(sources.data,'$.kind')`).  Una referencia de Wikidata a un libro de historia se registra con el libro como fuente, el adaptador de Wikidata como extractor y el localizador apuntando a la referencia.
- Las prioridades en disputa (por ejemplo, la del cálculo) se registran como `alternative_group` + `disputed`.
- **Principio 4, en tres capas**:
  1. **Composición.**  En `shapes/composition.json` (dueño: recorridos), `cites`, `appears_in`, `introduced_in`, `authored_by`, `copied_by`, `edited_by` y `translated_by` quedan en las familias biblio/history, sin composición con relaciones matemáticas.  Ninguna ruta que los contenga sale `mathematical` ni `pedagogical`.
  2. **Auditoría al compilar.**  Cada fuente bibliográfica (zbMATH, OpenAlex, OpenCitations, Crossref, CDLI) tiene una lista de relaciones permitidas (`cites`, `authored_by`, `edited_by`, `translated_by`, `copied_by`, `part_of_area`, `appears_in`).  `atlas compile` falla si encuentra otra relación con esa procedencia.
  3. **Tipos de extremo.**  `proves` ya exige sujeto `proof` o `algorithm`, así que una obra no demuestra nada; la demostración es una entidad que `appears_in` la obra.
- `RelationSpec.formal_allowed: bool` (campo nuevo).  `formally_linked` se rechaza en relaciones biblio/history, en `part_of_area` y en `member_of_concept`.  Esto cierra la puerta por la que entraron las 244,933 de §7.

### 5. Curaduría reversible: tabla `curation` y eventos `curation_decided` / `curation_undone`

Sigue el patrón de `merges`: una fila por decisión, un evento al crearla y otro al deshacerla, `undone_at` en lugar de borrar, y actor identificado (no se acepta `""` ni `"system"`; un programa de reglas firma como `curation:<regla>/<versión>`).

| `kind` | Payload | Qué hace | Deshacer |
|---|---|---|---|
| `exclude_presentation` | `{"scopes": ["map:general", "map:all", "search:default", "concept_layer"]}` | `atlas compile` pone bits en la sección `NodeFlags` del snapshot.  El registro no cambia | `undone_at`; la siguiente compilación vuelve a mostrarlo |
| `presentation_label` | `{"lang", "text", "provenance": {...}}` | rótulo de presentación: calificador («red (retículo)»), nombre de una zona derivada o corrección de mayúsculas.  La etiqueta de la fuente no se toca | `undone_at`; cambiar el rótulo = deshacer el anterior y crear uno nuevo |
| `retire_claims` | `{"claims": [ids]}` | pasa cada afirmación a `retracted` con la razón `curation:<id>`, usando `set_evidence_state` | `retracted` sigue siendo terminal.  Se inserta una copia restaurada `stable_id("restored", claim_id, curation_id)` con la procedencia original más la de la curaduría, y el evento `curation_undone` lista las ids nuevas |
| `propose_merge` | `{"survivor", "merged", "evidence"}` | nada, hasta que alguien decide | aceptar = `Store.merge(..., evidence="curation:<id>")`; rechazar = `undone_at` con razón |

Un índice único parcial (`kind, target_id, rule, rule_version` mientras no se haya deshecho) hace idempotente volver a correr una regla.  La exclusión de los ~60 nodos fuera de dominio, la salida de los 1,157 conceptos derivados de z0–z2 y el retiro de las pertenencias homónimas (las 132 de Sudáfrica, por ejemplo) son filas de esta tabla, una por entidad.

**Alternativas**:
- **Un JSONL versionado.**  Rompe el principio 2.  Descartada.
- **Permitir `retracted` → estado anterior.**  Rompe un invariante que ya usan SPEC, Rust y los 334,699 retiros existentes.  Descartada.
- **Suprimir sin cambiar el estado.**  La pertenencia homónima es falsa, así que el estado debe decirlo.  Descartada.

### 6. Fichas: entidad `note` y relación `explains`, una afirmación por frase

- `EntityType.NOTE` (nuevo): una frase, con su texto en `statements` (ES y/o EN).  `Entity.note: NoteInfo | None`, con `NoteInfo(section: FichaSection, ordinal: int)`, y `FichaSection` ∈ `que_es | por_que_importa | compruebo`.  Se exige `note` si y solo si `type == note`.  Las notas no pertenecen a `NON_PERSON` en las relaciones genéricas (no pueden ser sujeto de `part_of_area`, `analogous_to` ni `appears_in`).
- Relación `explains`: `note` → matemático | `area`, proyección learning, no transitiva.  `chain_authorizes`: «texto para el lector; no encadena y no afirma nada matemático más allá de sí mismo».
- **Procedencia por frase** (en la afirmación): `source_id`; `locator` (URL con `oldid` y párrafo, `Archivo.lean:línea` o libro y página); `extractor` (`editorial/1` si escribe una persona, `draft-llm/1` si propone un modelo) y su versión; `license`; `retrieved_at`; y `quote` con el pasaje que sostiene la frase.
- **Estado**: se inserta como `model_proposed`.  Pasa a `editorially_reviewed` solo con `set_evidence_state` y un actor humano (regla que ya existe).
- **Notas inmutables**: `add_entities(on_conflict="replace")` rechaza el tipo `note`.  Cambiar el texto = nota nueva + `supersede` de la afirmación.  Así una revisión nunca queda pegada a un texto distinto del que se revisó.
- Las notas no entran en el índice FTS: un borrador sin revisar no debe salir en la búsqueda.
- **La ficha es una vista derivada**.  `atlas fichas render` escribe `data/fichas/<id>.md`, que es reconstruible y lleva `event_seq` en la cabecera.  La vista pública incluye solo frases `editorially_reviewed`; las demás aparecen bajo «borrador editorial, sin revisar» y solo si se pide.  «Qué saber antes» y «Con qué se conecta» no son notas: se generan desde afirmaciones existentes (`prerequisite_for`, `special_case_of`, `instance_of`…) y citan su `claim_id`.  La licencia de la ficha sale de las procedencias de sus frases: CC BY-SA 4.0 con atribución si alguna parafrasea Wikipedia.
- **Entrada editorial**: el editor escribe markdown en un área de trabajo, y `atlas fichas import` lo parte en notas y afirmaciones.  El markdown de entrada no es fuente de verdad: una vez importado, manda el registro.

### 7. Estados de las conexiones

**a. Aristas agregadas concepto→concepto.**  Se agrega `EVIDENCE_RANK` en `model.py` (`formally_linked` 4 > `editorially_reviewed` 3 > `extracted` 2 > `model_proposed` 1 > `disputed` 0), espejado en `vocab.rs` y cubierto por la prueba de paridad.  El estado de una arista agregada es el mínimo entre la dependencia, la pertenencia del miembro de origen y la del miembro de destino.  `ConceptStatState` pasa a llevar ese mínimo, lo que cambia el significado de la sección y obliga a subir el formato.  Toda arista agregada lleva la marca «candidata»: sale del resumen de principiante y de la «Fuerza».

**b. P460 «se dice que es lo mismo que».**  Relación nueva `said_equivalent_to`: structure, simétrica, **no transitiva**, sin composición ni sustitución.  `chain_authorizes`: «una fuente dice que pueden ser lo mismo; no compone».  Las `equivalent_to` cuya procedencia es P460 (hasta 311) se reemplazan con `supersede` por `said_equivalent_to`, con la razón «ADR-0005 §7b».  Las equivalencias curadas a mano no se tocan.

**c. P31 frente a P279.**  Relación nueva `instance_of`: structure, no transitiva.  `chain_authorizes`: «x es un ejemplo de B».  La única composición permitida es `instance_of ∘ special_case_of*` → `instance_of` (si x es ejemplo de A y A es un tipo de B, x es ejemplo de B).  `instance_of ∘ instance_of` no compone.  Las `special_case_of` que vienen de P31 se reemplazan con `supersede` (casos D5 y D10 del diagnóstico).  P279 sigue siendo `special_case_of`.

**d. Relaciones en los dos sentidos.**  Pares contradictorios en `generalizes`/`special_case_of` (directos o vía la inversa) y en `prerequisite_for`: las dos afirmaciones pasan a `disputed` con `set_evidence_state` (actor `curation:bidirectional/1` y razón con la id de la pareja).  El historial queda en los eventos.  `depends_on_logically` mutuo **no** entra en esta regla, porque las equivalencias demostradas en los dos sentidos son legítimas.

**e. `part_of_area` derivado de la ruta del archivo.**  Las 244,933 pasan de `formally_linked` a `extracted` con `set_evidence_state` y la razón «la ubicación en un archivo no es verificación formal».  El adaptador de Mathlib deja de emitirlas como formales, y `formal_allowed` (§4) impide que vuelva a pasar.

**f.**  Los `formalized_as` en `model_proposed` por coincidencia de nombre se ocultan en el modo Explorar.  Es presentación (compilación y servidor), no esquema.

Todas estas transformaciones de datos pasan por la API del store (`supersede`, `set_evidence_state`, `curation`): dejan eventos, se pueden volver a correr y no borran nada.

### 8. Escala: unos 5 a 10 millones de obras con sus citas internas

**8.1 Identidad y duplicados.**
- Una entidad por registro de fuente, con ids naturales: `zb:<id de documento>`, `oa:W…`, `zba:<código de autor>`, `oaa:A…`, `cdli:P…`, `wd:Q…`.
- Las coincidencias **exactas** de un identificador fuerte (DOI, id de zbMATH, QID, ORCID, puentes P1556/P10283) se fusionan con `merge_many` (API nueva, por lotes y atómica), con actor `resolver:works/1` y `evidence` = la llave.  Sin un identificador fuerte no se fusiona: se crea una `propose_merge`.
- **Regla de registro primario**, para no sumar el solapamiento: `authored_by`, `part_of_area` y `cites` de una obra canónica se emiten solo desde su registro primario (Wikidata, si no zbMATH, si no OpenAlex).  Los demás registros aportan identificadores y cuentas de cobertura.  OpenCitations solo agrega pares entre obras canónicas que todavía no estén.  Las coincidencias de las fuentes secundarias se miden y se reportan en las estadísticas del lote; no se guardan como afirmaciones duplicadas.
- Si se deshace una fusión, se vuelve a correr el lote del registro afectado, que es idempotente y emite sus afirmaciones.
- **Escala de las fusiones**: hoy `_redirects()` carga en un dict todas las entidades fusionadas, y `merged_ids` lo recorre completo en cada llamada.  Con unos 3 a 5.6 millones de fusiones (NV), `resolve` y `merged_ids` pasan a consultas SQL sobre el índice `entities_merged`, con una LRU pequeña.  El dict completo solo se construye en `iter_edges`.

**8.2 ¿Alcanza `u32`?**  Para índices de nodos, afirmaciones, aristas, filas de procedencia y cadenas, **sí**: el peor caso estimado es unos 15 millones de nodos y 90 millones de afirmaciones, contra 4,294,967,295, un margen de 47 veces.  Para **desplazamientos en bytes, no**.  `StrTable` de `core/atlas-core/src/strings.rs` usa desplazamientos `u32` y aborta por encima de 4 GiB, y la tabla de cadenas de la capa de obras se estima en 3.3 GB (4.4 GB en el peor caso).  Por eso el formato v3 del snapshot usa **`StrOffsets` de `u64`** en los dos archivos (cuesta 4 B por cadena; unos 320 MB en obras).  En SQLite los `rowid` son `int64` y el `rowid` de FTS (`entity_rowid × 1024`) cabe holgado (≈1.5·10¹⁰).

**8.3 Tamaño del registro (NV salvo lo marcado)**

| Unidad | Bytes por unidad (estimado desde el DDL) | Cuántas | Total |
|---|---|---|---|
| Afirmación binaria (fila de `claims` con JSON ≈ 530 B + 2 participantes con índice ≈ 235 B + procedencia con índices ≈ 200 B, × 1.15 de páginas; sin evento por afirmación, gracias al evento por lote) | ≈ 1.1 KB | `authored_by` 12–20 M (≈ 2.4 autores por obra) + `part_of_area` 10–20 M (≈ 2.5 MSC por documento de zbMATH + subcampo de OpenAlex) + `cites` internas 15–50 M (2,338,383 obras de OpenAlex con referencias, medido en `papers-historia.md`; ≈ 30 referencias, ≈ 40 % internas, NV) = **37–90 M, central 60 M** | 41–99 GB, central **66 GB** |
| Obra (JSON con título, ids, procedencia y fecha ≈ 1 KB + 3 ids externos con índice ≈ 435 B + procedencia ≈ 300 B + FTS ≈ 300 B + `entity_dates` ≈ 80 B, × 1.15) | ≈ 2.4 KB | 9.83 M registros (5.2 M de zbMATH + 4.6 M de OpenAlex, medido en `papers-historia.md`) | **23.5 GB** |
| Persona | ≈ 1.5 KB | 1.5–4.5 M (NV) | 2.3–6.8 GB |
| Fusión (fila + evento) | ≈ 270 B | 3–5.6 M | 1–1.5 GB |
| Registro actual | | | 14 GB (cifra del orquestador) |
| **Total** | | | **82–145 GB, central ≈ 110 GB** |

**Presupuesto de disco** (356 GB libres):
- registro R;
- un respaldo R;
- snapshots de obras, dos versiones × ≈ 10 GB;
- índice de obras, ≈ 5 GB × 2;
- caché filtrada de entradas (jsonl.gz), ≈ 10 GB;
- volcado de OpenCitations, 36 GB comprimido y transitorio (medido en `papers-historia.md`);
- margen de 30 GB.

De ahí sale **R ≤ 120 GB**.

**Palancas del registro** (no entran en v4; se activan solo si el piloto las pide, como v5):
- **L1.**  Quitar de `claims.data` y de `entities.data` la copia de participantes y procedencia, que ya están en sus tablas.  Rust ya lee participantes y procedencia de las tablas (`read.rs`); `_claim_from_row` las reconstruiría.  Baja a ≈ 0.75 KB por afirmación y a un total de ≈ 80 GB (106 GB en el peor caso).
- **L2.**  Llaves enteras en `claim_participants` y `provenance` (SPEC §16), unos −17 GB más.

El cálculo central indica que **L1 probablemente hará falta**.  Lo decide la medición del piloto, no esta estimación.

**8.4 Snapshot y layout: las obras son una capa propia**
- **Snapshot central** (`atlas-<hash>.snap`): sin obras, del tamaño de hoy (≈ 406 MB).  Los recorridos, la capa conceptual y el layout de fuerzas no cambian de escala.
- **Snapshot de obras** (`atlas-works-<hash>.snap`, archivo aparte con el mismo formato v3):
  - nodos (`publication`, `witness`, `person` de la capa), ≈ 156 B por nodo con cadenas → ≈ 2.0 GB;
  - afirmaciones con su CSR de entrada y salida, ≈ 125 B por afirmación (localizador internado por registro) → ≈ 7.5 GB (4.6–11.3);
  - tabla hash de afirmaciones, ≈ 0.5–1 GB;
  - fechas (`lo`/`hi` como `i32`) y un índice espacial ordenado por (nivel, llave de tesela), ≈ 0.3 GB;
  - **total ≈ 10–11 GB (7–15)**.
  - Referencia a los nodos del núcleo (concepto o área) por índice central, con `core_content_hash` en la cabecera: el servidor rechaza un par que no coincida.  El orden de compilación es primero el núcleo y después las obras.
- **Búsqueda**: un fragmento propio de tantivy para obras (título, autores, ids), solo léxico.  **Sin embeddings para las obras**: 7 millones × 384 dimensiones × 4 B ≈ 10.8 GB de vectores (la dimensión es la de MiniLM, NV en este registro), y el índice neuronal en frío tarda ≈ 47 min para unos 315,783 textos (`docs/STATUS.md`, WP16 y build-logs), lo que extrapolado da ≈ 17 h.  No cabe en «memoria ≤ 16 GB» ni en «`compile --all` ≤ 2 h».
- **Layout**: las obras **no** entran en Leiden ni en Barnes-Hut.  La posición es determinista: x = tiempo (comprimido hacia la Antigüedad), y = banda del área MSC, con jitter por hash dentro de la banda.  Es O(n).
- **Teselas** `layer=works`: se calculan **al pedirlas** con búsqueda binaria en el índice espacial, sin archivos pregenerados (13 millones de nodos darían ≈ 1.3 GB de teselas JSON).  Se aplica el presupuesto por tesela de la ronda 4 (400 nodos).  El orden dentro de una celda es el grado de entrada interno al corpus y se rotula así; nunca se usa el `cited_by_count` global, que favorece a estadística.
- En Explorar, las obras se ven como «obras sobre este concepto» en la ficha y en la línea de tiempo.
- **`coverage()`** a esta escala (un `COUNT DISTINCT` sobre unos 100 millones de filas de procedencia) se calcula en `compile`, y la API sirve el resultado derivado.

**8.5 Ingesta por lotes, reanudable e idempotente**
- **Llave del lote**:
  - OAI-PMH de zbMATH → ventanas de `from`/`until` por fecha de registro (deterministas y repetibles; los `resumptionToken` caducan);
  - OpenAlex → archivo de partición + rango de líneas;
  - OpenCitations → archivo del volcado + rango.
- **Un lote = una transacción**: entidades, afirmaciones, la fila de `ingest_batches` y **un** evento `batch_ingested` con `{batch_id, source_id, batch_key, stats, rangos de rowid}`.  Las filas llevan `ingest_batch`.  Si el proceso se cae a mitad, no queda nada del lote; al relanzar se saltan los lotes que ya tienen fila.
- **Idempotencia**: ids naturales y deterministas (`stable_id(rel, s, t)` para las afirmaciones), `on_conflict="ignore"` y `existing_claim_ids`.
- **Ahorro**: ≈ 60 millones de eventos `claim_added` (≈ 6.6 GB con su índice).
- **Invariante probado**: toda afirmación tiene `claim_added` o pertenece a un lote con `batch_ingested`.
- **Dos pasadas**: primero todas las obras (para saber qué es interno), después las citas.  Las estadísticas de cada lote cuentan citas totales, internas y externas, fechas marcadas, fusiones y propuestas de fusión.
- **Caché**: solo la entrada filtrada (`data/cache/works/<fuente>/<lote>.jsonl.gz`), nunca el volcado crudo.  Hay fixtures offline para las pruebas, `User-Agent` identificable, ≤ 1 petición por segundo en lo que no tiene volcado, y nada de resúmenes ni reseñas.  Prueba: las entidades de estas fuentes no tienen `descriptions` ni `statements`.
- **Volumen de OpenAlex**: filtrar el campo 26 desde el snapshot de S3 obliga a bajar todo el snapshot de works (tamaño NV).  La alternativa es el cursor de la API, unas 23 mil páginas de 200.  El piloto mide las dos vías y la elección se anota en STATUS.

**8.6 Qué medir en el piloto de 100,000 obras antes de comprometerse**

Muestra: **una clase MSC de primer nivel completa** (todas sus obras y años, del orden de 100 mil; elegirla por su conteo en zbMATH) **más 10,000 obras al azar**.  Una muestra solo al azar de 100 mil tendría ≈ 2 % de las citas internas que tendrá el corpus completo, y la densidad de citas no se podría extrapolar.

| Medida | Cómo | Umbral para seguir (propuesto; Adrián lo confirma antes del piloto) |
|---|---|---|
| Bytes por obra, por persona y por afirmación, por tabla e índice | `dbstat`; también la descomposición de los 14 GB actuales | registro proyectado (cota alta) ≤ 120 GB; si no se cumple, L1 y volver a medir |
| Rendimiento de ingesta (obras/s, afirmaciones/s) y de la descarga | reloj, por fuente | ingesta completa ≤ 48 h de reloj (NV) |
| Reanudación | matar el proceso a mitad de lote y relanzar | 0 filas parciales; la segunda corrida inserta 0 y no deja eventos |
| Solapamiento zbMATH ∩ OpenAlex | DOI y id de zbMATH en la muestra al azar | se reporta con intervalo; no es umbral |
| Autores por obra, MSC por obra, citas internas por obra | estadísticas de los lotes | recalcular §8.3 con los valores medidos |
| Fechas | conteo por tipo; rechazadas | **0 rechazadas**; las marcadas, en el informe |
| Fusiones | cuenta; p95 de `resolve` con N fusiones | p95 de `resolve` < 1 ms |
| Snapshot de obras | tamaño, tiempo de compilación | `compile --all` extrapolado × 2 ≤ 2 h |
| Búsqueda | p95 del fragmento de obras | ≤ 100 ms |
| Teselas `works` | p95 | ≤ 2× el de la ronda 4 |
| Servidor | RSS / working set con los dos snapshots | ≤ 16 GB extrapolado |

Si una medida no pasa, se rediseña esa parte antes de ingerir más (plan, §5b).

### 9. Cobertura honesta: tabla `coverage_cells`, de solo anexar

Por fuente, versión de la fuente, periodo (intervalo EDTF, por ejemplo `1900/1949`), área (id de un área, o `*`) y fecha de medición:
- `indexed`: lo que dice la fuente, con `indexed_method` (por ejemplo «faceta `py` de la API REST de zbMATH, 2026-10-19»); `NULL` si se desconoce;
- `ingested`: lo que cuenta el registro en ese momento;
- `cutoff`: el criterio de selección (`all`, `top25-cited`…).

Las filas no se modifican: medir otra vez es una fila nueva con otro `measured_at`, más un evento `coverage_measured`.

Reglas de presentación:
- Una celda sin fila, o con `indexed` nulo, se muestra **«desconocido»**.
- Una persona sin obras ingeridas muestra «obras: no ingeridas», nunca «sin obras».
- El solapamiento entre fuentes se calcula en `compile` a partir de las fusiones (es derivado) y se muestra junto a las celdas; nunca se suma.
- Las citas se rotulan «citas en OpenAlex a este registro», no como importancia.

## Migración v3 → v4 sin perder historial

1. **Antes de abrir con el código v4**: copiar `data/atlas.db` (el respaldo es obligatorio; los lectores v3, y Rust con `SUPPORTED_SCHEMA = 3`, no abren v4).
2. **M1, DDL automático y solo aditivo** (`MIGRATIONS[4]`, abajo): tablas, columnas e índices nuevos, y `relation_endpoints` regenerado con `INSERT OR IGNORE` desde `RELATIONS` (incluye `witness` en los extremos `DOCS`).  **Ninguna fila existente se reescribe ni se borra**; la tabla `events` queda intacta, más el evento `migration`.
3. **M2, transformaciones de datos**: las hacen los trabajadores, una operación con nombre por cada cosa, con la API del store y eventos:
   - §7b, P460 → `supersede`;
   - §7c, P31 → `supersede`;
   - §7e, `part_of_area` → `set_evidence_state`;
   - §7d, relaciones en los dos sentidos → `disputed`;
   - §5, exclusiones y retiros → `curation`;
   - etiquetas en español → `entity_replaced` con `previous`.
   
   Cada operación se puede volver a correr y deshacer.
4. **Rust**: `SUPPORTED_SCHEMA = 4` y `FORMAT_VERSION = 3` (desplazamientos `u64`, `NodeFlags`, rótulos de presentación, el nuevo significado de `ConceptStatState`, archivo de obras).  Los códigos nuevos de los enums se **agregan al final** de `vocab.rs`: ningún código existente cambia.  Cambia `vocab_hash`, y con él se recompila todo (es esperado).
5. **Docs**: SPEC §2, §3, §8, §11 y §12 (historial: «v4 (fecha) ADR-0005»); `shapes/atlas.shacl.ttl` y `context.jsonld` (fechas, `note` y relaciones nuevas); STATUS, con la ausencia de dependencias nuevas justificada.

## Diff de DDL propuesto (texto; no aplicado)

```sql
-- v4 (2026-10-xx): ADR-0005.  Solo aditivo: ninguna fila existente se reescribe.

CREATE TABLE ingest_batches(
  id INTEGER PRIMARY KEY,
  source_id TEXT NOT NULL REFERENCES sources(id),
  run TEXT NOT NULL,                 -- adaptador@versión + versión de la fuente
  batch_key TEXT NOT NULL,           -- ventana OAI, archivo+rango de líneas…
  input_sha256 TEXT NOT NULL,
  stats TEXT NOT NULL,               -- JSON: entidades, afirmaciones, citas totales/internas/externas, fechas marcadas, fusiones
  actor TEXT NOT NULL, finished_at TEXT NOT NULL,
  UNIQUE(source_id, run, batch_key));
ALTER TABLE entities ADD COLUMN ingest_batch INTEGER REFERENCES ingest_batches(id);
ALTER TABLE claims   ADD COLUMN ingest_batch INTEGER REFERENCES ingest_batches(id);
CREATE INDEX claims_batch   ON claims(ingest_batch)   WHERE ingest_batch IS NOT NULL;
CREATE INDEX entities_batch ON entities(ingest_batch) WHERE ingest_batch IS NOT NULL;

CREATE TABLE curation(
  id INTEGER PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('exclude_presentation','presentation_label','retire_claims','propose_merge')),
  target_id TEXT NOT NULL,           -- entidad (en retire_claims: la entidad cuyo retiro motiva la regla)
  rule TEXT NOT NULL, rule_version TEXT NOT NULL,
  payload TEXT NOT NULL,             -- JSON con la forma de §5
  actor TEXT NOT NULL, reason TEXT NOT NULL, created_at TEXT NOT NULL,
  undone_at TEXT, undone_by TEXT, undo_reason TEXT);
CREATE INDEX curation_target ON curation(target_id) WHERE undone_at IS NULL;
CREATE UNIQUE INDEX curation_active ON curation(kind, target_id, rule, rule_version) WHERE undone_at IS NULL;

-- Derivada de entities.data.dates; reconstruible (rebuild_entity_dates)
CREATE TABLE entity_dates(
  entity_id TEXT NOT NULL REFERENCES entities(id), position INTEGER NOT NULL,
  kind TEXT NOT NULL, edtf TEXT NOT NULL,
  lo INTEGER, hi INTEGER,            -- años astronómicos inclusivos; NULL = extremo abierto
  source_id TEXT NOT NULL REFERENCES sources(id),
  PRIMARY KEY(entity_id, position)) WITHOUT ROWID;
CREATE INDEX entity_dates_span ON entity_dates(kind, lo, hi);

-- Hoy el replace hace DELETE ... WHERE entity_id=? recorriendo toda la tabla
CREATE INDEX external_ids_entity ON external_ids(entity_id);

CREATE TABLE coverage_cells(
  source_id TEXT NOT NULL REFERENCES sources(id), source_version TEXT NOT NULL,
  period TEXT NOT NULL, area TEXT NOT NULL,
  indexed INTEGER, indexed_method TEXT NOT NULL,
  ingested INTEGER NOT NULL, cutoff TEXT NOT NULL, measured_at TEXT NOT NULL,
  PRIMARY KEY(source_id, source_version, period, area, measured_at)) WITHOUT ROWID;
CREATE TRIGGER coverage_no_update BEFORE UPDATE ON coverage_cells BEGIN SELECT RAISE(ABORT, 'coverage_cells is append-only'); END;
CREATE TRIGGER coverage_no_delete BEFORE DELETE ON coverage_cells BEGIN SELECT RAISE(ABORT, 'coverage_cells is append-only'); END;

-- relation_endpoints: INSERT OR IGNORE de todas las relaciones, generado desde RELATIONS
-- (said_equivalent_to, instance_of, introduced_in, copied_by, edited_by, translated_by, explains;
--  witness en todo extremo DOCS)
```

Cambios de contrato en Python (sin DDL):
- `EntityType` + `WITNESS`, `NOTE`.
- `RelationType` + `SAID_EQUIVALENT_TO`, `INSTANCE_OF`, `INTRODUCED_IN`, `COPIED_BY`, `EDITED_BY`, `TRANSLATED_BY`, `EXPLAINS`.
- `RelationSpec.formal_allowed`.
- `EventKind` + `CURATION_DECIDED`, `CURATION_UNDONE`, `BATCH_INGESTED`, `COVERAGE_MEASURED`.
- `DateKind`, `HistoricalDate`, `FichaSection`, `NoteInfo` y `EVIDENCE_RANK`.
- `Entity.dates` y `Entity.note`; `Source.kind`.
- Payload de `entity_replaced` con `previous`.
- APIs nuevas: `Store.add_curation`, `undo_curation`, `curations`, `begin_batch` (o el argumento `batch=` en `add_entities`/`add_claims`), `merge_many`, `rebuild_entity_dates`, `add_coverage_cells`.
- `SCHEMA_VERSION = 4`.

## Pruebas que deben existir

**Modelo** (`tests/test_model.py`, arquitecto):
- El analizador EDTF acepta `-0299`, `-0299~`, `-1899~/-1599~`, `0000`, `1801-07-15`, `19XX`, `../1600`, `1680?`, `0000-02-29` (gregoriano proléptico) y `1900-02-29` en calendario juliano.
- Rechaza `1801-13`, `1900-02-29` en calendario gregoriano, `-02XX`, `1600/1500`, `""` y `abc`.
- `lo` y `hi` son correctos, incluidos los extremos abiertos.
- `edtf_from_wikidata`: (−300, 9) → `-0299`; (−300, 7) → `-0299/-0200`; (−1, 9) → `0000`; `+1801-00-00`/9 → `1801`; circa → `~`; presumiblemente → `?`; P1319/P1326 → intervalo; Plimpton −1799/9 → `-1798`; juliano → `calendar="julian"`.
- `HistoricalDate` sin procedencia falla.
- `Claim.historical_date` se valida, y el JSON viejo sin el campo se lee bien.
- Invariantes de `RELATIONS` para las relaciones nuevas: `said_equivalent_to` simétrica y no transitiva; `instance_of` e `introduced_in` no transitivas; `copied_by` solo desde `witness`; `explains` solo `note` → matemático o área.
- `formally_linked` se rechaza en `part_of_area`, `member_of_concept` y las relaciones biblio/history.
- `NoteInfo` se exige si y solo si el tipo es `note`.
- `EVIDENCE_RANK` cubre todos los `CURRENT_STATES`.
- Paridad de vocabulario Python ↔ Rust.

**Store** (`tests/test_store.py`, arquitecto):
- Migración v3 → v4 sobre un fixture v3: `user_version` 4, evento `migration`, y conteos y hash de `claims`, `claim_participants`, `provenance`, `merges` y `events` (sin el evento nuevo) idénticos.
- `introduced_in` sin fuente de historiografía da `ValidationError`; con ella pasa.
- Cada `kind` de curaduría se puede crear y deshacer.  El índice activo da idempotencia.  `"system"` se rechaza.  `retire_claims` → `retracted` y, al deshacer, copias restauradas enlazadas en los eventos.  `propose_merge` aceptada → `merge` con `evidence="curation:<id>"`.
- `entity_replaced` guarda `previous`; un reemplazo idéntico no hace nada.
- `replace` de una `note` falla; las notas no están en FTS.
- Lotes: volver a correr un lote inserta 0 y no deja eventos; una excepción a mitad deja 0 filas y ninguna fila de lote; invariante «toda afirmación tiene `claim_added` o `batch_ingested`».
- `entity_dates` se llena, se reconstruye y responde consultas por rango.
- `resolve` y `merged_ids` funcionan sin cargar toda la tabla (sintético con 1 millón de fusiones, p95 < 1 ms).
- `merge_many` es atómica y se puede deshacer.
- `coverage_cells` no admite cambios (los triggers).
- `EXPLAIN QUERY PLAN` usa `external_ids_entity`.

**Núcleo Rust** (`atlas-core`):
- Formato v3 con desplazamientos `u64` (prueba de tipos y de lectura).
- Arista agregada = eslabón más débil (dependencia `extracted` + pertenencia `model_proposed` → `model_proposed`), siempre marcada candidata.
- `NodeFlags` aplica la curaduría activa e ignora la deshecha.
- El snapshot de obras rechaza un `core_content_hash` que no coincide.
- La auditoría de lista permitida hace fallar `compile`.

**Recorridos** (`atlas-paths`):
- `said_equivalent_to` no compone (A~B, B~C no da A~C).
- `instance_of ∘ instance_of` no compone; `instance_of ∘ special_case_of` → `instance_of`.
- Una ruta con `cites`, `appears_in`, `introduced_in` o cualquier relación de autoría nunca sale `mathematical` ni `pedagogical`.
- `disputed` solo aparece si se pide.

**Conexiones** (sobre el registro, después de M2):
- 0 `equivalent_to` vigentes desde P460.
- 0 `special_case_of` vigentes desde P31.
- 0 `part_of_area` en `formally_linked`.
- Todos los pares en los dos sentidos están en `disputed`.
- Volver a correr no cambia nada.
- AtlasBench sin regresión.

**Nombres**:
- 0 nodos de la lista fuera de dominio en z0–z2, y 0 derivados en z0–z2.
- Deshacer devuelve el nodo a la siguiente compilación.

**Obras** (fixtures offline):
- Sin resúmenes.
- Licencia en cada procedencia.
- Ids naturales.
- Antes de Cristo correcto en *Elementos*, *Nueve capítulos*, Rhind y Plimpton.
- El año 1403 hijri queda marcado y no se rechaza.
- Solo se fusiona por coincidencia exacta de identificador.
- Regla de registro primario: 0 `authored_by` duplicados por par canónico.
- Solo citas internas, con las externas contadas.
- Una celda de cobertura por cada periodo × área de la fuente.
- 0 fechas rechazadas en el fixture.

**Fichas**:
- La vista pública solo contiene frases `editorially_reviewed`.
- Cada nota al pie lleva fuente, localizador, extractor y estado.
- «Antes» y «conexiones» citan `claim_id`.
- Con el mismo `event_seq` salen los mismos bytes.
- La línea de licencia sale de las procedencias.

## Orden de implementación

0. **Requisito**: la ronda 4 cerrada con las pruebas en verde, y el respaldo de `data/atlas.db` hecho.
1. **Arquitecto** (un solo cambio): `model.py`, `store.py` v4, SPEC y shapes con sus pruebas.  **Arquitecto Rust**: `vocab.rs`, `format.rs` v3 (desplazamientos `u64`, `NodeFlags`, cabecera con `core_content_hash`) y `SUPPORTED_SCHEMA = 4`.  Nadie más escribe en el contrato.
2. **En paralelo**:
   - **a. Conexiones** (`atlas/concepts.py`, `atlas/ingest/wikidata`, `concepts.rs`, `composition.json`): M2 de §7b–e; eslabón más débil y marca de candidata; ocultar `formalized_as` propuestos.  Después, AtlasBench.
   - **b. Nombres y fuentes** (`atlas/ingest`, `atlas/resolve.py`): reglas de curaduría (fuera de dominio + retiro de pertenencias; derivados fuera de z0–z2; calificadores); etiquetas ES desde Wikidata y Wiktionary (`entity_replaced` con `previous`); `propose_merge` para duplicados; fuentes nuevas con `Source.kind`.
   - **c. Obras**, en este orden:
     1. Canon de Wikidata y testimonios curados: fechas, `witness`, roles, `introduced_in` solo con historiografía.  Es pequeño y valida §1–§4.
     2. Piloto de §8.6 y decisión de pasa / no pasa (L1 si hace falta), anotada en STATUS.
     3. Snapshot de obras, fragmento de búsqueda y teselas `works` (Rust: núcleo, búsqueda y mapa).
     4. Ingesta completa por lotes: obras y después citas.
     5. OpenCitations, solo pares internos que falten.
     6. `coverage_cells`.
3. **Fichas** (después de que 2b haya corregido los extractos de Wikipedia): `atlas fichas import/render` y piloto de 20 fichas revisadas por Adrián.  Sin su revisión siguen como «borrador editorial, sin revisar».
4. **Integración**: compilación de punta a punta, crítica con dos jueces y `docs/STATUS.md` §16.

## Consecuencias

**Positivas**:
- Las fechas antiguas se pueden guardar sin falsa precisión y sin el error de un año.
- La copia y la autoría son relaciones distintas.
- «Introdujo» exige una fuente de historia.
- Ninguna conexión se muestra más fuerte que su eslabón más débil.
- La curaduría vive en el registro y se puede deshacer.
- Cada frase de una ficha tiene su fuente y su revisión.
- Las obras escalan en una capa aparte sin tocar los recorridos ni el layout del núcleo.

**Negativas**:
- Hay siete relaciones y dos tipos nuevos que propagar por Rust, SHACL, la web y AtlasBench.
- `vocab_hash` cambia y obliga a recompilar todo.
- Las ediciones fusionadas son una simplificación (una edición no es la obra) con una salida mecánica.
- El registro puede pasar de 120 GB sin L1.
- Las obras no tienen búsqueda semántica en esta ronda.

**Riesgos abiertos (NV)**:
- La convención de años de WDQS.
- El tamaño del snapshot de OpenAlex.
- La unión zbMATH ∪ OpenAlex.
- La densidad de citas internas.
- La licencia CC0 de OpenCitations, que la página de descargas de hoy no menciona (`papers-historia.md` §8).

## Estado

Propuesto, 2026-10-19.  **Aceptado, 2026-10-19**: el paso 1 (contrato Python, store v4, SPEC, shapes, `vocab.rs`, `format.rs` v3, `SUPPORTED_SCHEMA = 4`) está implementado y `python -m pytest -q`, `cargo test --workspace --release` y `cargo clippy --workspace --all-targets --release` pasan.  La migración del registro real (M1 sobre `data/atlas.db`) la hace el integrador después del respaldo; M2 la hacen los trabajadores del paso 2.
