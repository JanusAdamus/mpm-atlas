# Ronda 5 · diagnóstico de calidad: nombres, conexiones y explicaciones

19 de octubre de 2026 · diagnóstico de solo lectura, sin cambios al registro, al snapshot ni al producto.  Lo escribió el agente de evaluación (`data-scientist`).  Todos los juicios de «correcto», «útil» o «fuera de dominio» son **juicio de un agente, provisional**: un solo juez, sin acuerdo entre jueces y sin revisión humana.

## 0. Qué se midió y con qué

| Objeto | Versión |
|---|---|
| Registro | `data/atlas.db`, abierto con `sqlite3.connect('file:data/atlas.db?mode=ro', uri=True)`; `source_event_seq` 3,874,451 |
| Snapshot | `data/snapshot/CURRENT` → `atlas-a2e90658285b7ca5.snap` (proyección conceptual v9, §14.8 de STATUS) |
| Layout | `649b68d3fc5a6cca` (el `data/layout/CURRENT` vigente; no es el `88141d97…` de §14.2) |
| API | servidor ya en marcha en `127.0.0.1:8765` (arrancó 2026-10-19T15:59:02Z, no lo inicié yo); solo peticiones GET |

**Capa general** = todos los nodos únicos de las teselas `layer=structure`, z0–z2 (21 teselas): **3,229 nodos**.  Por tipo: concept 2,640, area 340, statement 207, model 21, algorithm 15, conjecture 6.  De los 2,640 `concept`, **1,157 son conceptos derivados** (`concept:Mathlib/…`) y 1,483 son ítems de Wikidata.  §14.5 contó 3,227 con el layout anterior; la diferencia de 2 viene de la v9.

Las consultas están en `docs/plans/r5/consultas/` y se corren en este orden, con `python -I`: `r5_fetch_tiles.py` (teselas → `r5-general.json`), `r5_load.py` (entidades y afirmaciones de la capa → `r5-load.pkl`), `r5_names.py`, `r5_allconcepts.py`, `r5_edges.py`, `r5_sample20.py`, `r5_text.py`, `r5_ambiguous.py`.  Cada cifra de abajo lleva entre corchetes la consulta que la produjo: **[Qn]**, definidas en el apéndice A.  Los archivos intermedios quedan junto a los scripts; no se escribe nada en `data/`.

Estados vigentes («current»): `extracted`, `model_proposed`, `editorially_reviewed`, `formally_linked`, `disputed` (`atlas/model.py::CURRENT_STATES`).

---

## 1. Nombres raros

### 1.1 Resumen por clase

| Clase | En la capa general (3,229) | En todo el registro | Fuente que la causa | Consulta |
|---|---:|---:|---|---|
| Sin etiqueta ES (el mapa en español muestra el inglés) | **732** (concept 484, statement 129, area 105, algorithm 7) | 8,773 concept/area de Wikidata | Wikidata no tiene etiqueta `es`; `wd-map/2` la copia tal cual y `Entity.label` cae a `en` | [Q2], [Q3] |
| Sin etiqueta EN | 0 | 1,277 (1,216 `mathlib-dir:` y 61 de Wikidata) | Wikidata solo en ES; directorios de Mathlib sin etiqueta por idioma | [Q3] |
| Etiqueta derivada «x › y (Mathlib, derivado)» | **1,157** (1,134 con «›» y 23 raíces «algebra (Mathlib, derivado)») | 1,481 `concept:` | `concept-projection` (versiones de entidad 1, 2, 3, 5, 6, 7 y 8) crea un concepto por zona sin ítem de Wikidata; la etiqueta son los dos últimos segmentos de la ruta de Mathlib partidos por mayúsculas, en inglés | [Q2] |
| Nombre de Lean crudo como etiqueta | 0 como etiqueta propia; los 1,157 derivados son trozos de nombres de módulo («triv sq zero ext», «zsqrtd», «icc», «d finsupp») | 241,983 declaraciones de Mathlib con etiqueta `und` = nombre Lean | `atlas.ingest.mathlib` `mathlib/1+lean-src/1`: la etiqueta es el nombre de la declaración | [Q2], [Q9] |
| Palabra suelta | 420; **72 ambiguas o no matemáticas** (juicio de agente sobre la lista completa) | no medido | Wikidata (etiqueta corta correcta en su contexto, ambigua fuera de él) | [Q2], [Q4] |
| Fuera de dominio (ajedrez, países, física, química, informática) | ≈ 60 (ajedrez 11, países 5, física/química/ciencias de la Tierra ≈ 30 de 47 coincidencias, informática 17 coincidencias) | 615 con el filtro estrecho; 1,760 etiquetas numéricas (fechas, números naturales, monedas) | Semillas de `wd-map/2` + asignaciones homónimas de `concept-projection` y `resolve/2` que convierten el ítem en «concepto» | [Q5], [Q3] |
| Duplicados por nombre normalizado | **35 grupos, 71 nodos** (20 grupos de homónimos de Wikidata, 15 choques de etiquetas derivadas) | no medido | Wikidata separa sentidos con la misma etiqueta ES; el rótulo derivado usa solo 2 segmentos de ruta | [Q2] |
| Etiqueta truncada o con «…» | 0 | 2 (nombres de series, legítimos) | — | [Q2], [Q3] |
| Mayúsculas inconsistentes | 65 ES con mayúscula inicial y EN en minúscula; 6 en mayúsculas (clases de complejidad y funciones hash, legítimas) | no medido | Etiqueta ES de Wikidata copiada del título del artículo | [Q2] |
| Sin ninguna descripción | 165 | no medido | Wikidata sin descripción | [Q2] |

Lectura: en el mapa en español, **1,889 de 3,229 nodos (58 %) muestran texto en inglés** (732 sin etiqueta ES + 1,157 derivados, que no se solapan) [Q2].  Es la clase de nombre raro más grande y STATUS no la registra; §14.5 solo nombra las palabras sueltas.

### 1.2 Ejemplos reales por clase (10 o más por clase)

**Sin etiqueta ES** [Q2]: wd:Q8028383 «witness»; wd:Q18207102 «final functor»; wd:Q6824326 «Metric outer measure»; wd:Q4461560 «Poncelet point»; wd:Q120690604 «Right stochastic matrix»; wd:Q3527115 «Lüroth's theorem»; wd:Q123916004 «inverse reinforcement learning»; wd:Q16908668 «equivariant stable homotopy theory»; wd:Q17141967 «geometric complexity theory»; wd:Q118110536 «the hat»; wd:Q141133828 «polynomial term»; wd:Q7391081 «SMASH».

**Sin etiqueta EN (registro)** [Q3]: wd:Q1421993 «Recta fija»; wd:Q38582 «Catorce»; wd:Q43016 «mil»; wd:Q16557296 «Disco»; wd:Q1335089 «adoptar, extender y extinguir»; wd:Q1397596 «arco capaz»; wd:Q1603724 «Elevación»; wd:Q2717852 «Gestor de arranque»; wd:Q2851822 «Año Sacerdotal»; wd:Q2889444 «Duplo».  Ninguno está en la capa general.

**Derivadas** [Q2]: concept:Mathlib/Basic/Finite «basic › finite (Mathlib, derivado)»; …/ModelTheory/Fraisse «model theory › fraisse»; …/NumberTheory/Padics/Measure/Monoid «measure › monoid»; …/Algebra/Module/LocalizedModule «module › localized module»; …/AlgebraicTopology/SimplicialSet/NonDegenerateSimplices «simplicial set › non degenerate simplices»; …/Analysis/CStarAlgebra/Unitary/Maps «unitary › maps»; …/AlgebraicGeometry/Restrict «algebraic geometry › restrict»; …/Geometry/Manifold/Instances/Icc «instances › icc»; …/CategoryTheory/FiberedCategory/Cocartesian «fibered category › cocartesian»; …/NumberTheory/Zsqrtd «number theory › zsqrtd»; …/Analysis/Normed/Lp/MeasurableSpace «lp › measurable space»; …/Algebra/Squarefree «algebra › squarefree».

**Nombres Lean crudos (fuera de z0–z2: z4+, búsqueda, «Bajar a declaraciones», miembros de la ficha de concepto)** [Q9]: `Antitone.tendsto_setIntegral`; `hasFDerivAt_integral_of_dominated_of_fderiv_le''` (el de R4-06); `WittVector.ghostComponentModPPow_teichmuller_coeff`; `Submodule.iSup_eq_range_dfinsupp_lsum`; `FractionalIdeal.isFractional_span_iff`; `Matrix.diag_zero`; `Part.Fix.approxChain`; `jacobiSum_ringHomComp`; `Equiv.Perm.subgroup_eq_top_of_isPreprimitive_of_isSwap_mem`; `mathlib:93e6a6aa47505f1a` (declaración con nombre hash, instancia anónima).

**Palabras sueltas ambiguas** [Q4] (de las 72): wd:Q1096885 «red» (= *lattice*, subgrupo discreto; además existe «retículo»); wd:Q595298 «haz» (*sheaf*); wd:Q5375781 «fin» (*end*, teoría de categorías); wd:Q7595945 «campo» (= *stack*, traducción que choca con «cuerpo/campo»); wd:Q6265342 «más» (signo +); wd:Q1251376 «Doble» (*double-double*); wd:Q1969983 «Diferencial» (*pushforward*); wd:Q1626444 «cut»; wd:Q105225393 y wd:Q140092320 «head»; wd:Q105225381 y wd:Q140092311 «tail»; wd:Q858684 «NaN»; wd:Q6428674 «era»; wd:Q9128 «luz»; wd:Q4652392 «ALL»; wd:Q3064117 «cara»; wd:Q3828144 «carta» (*chart*).

**Fuera de dominio, en la capa general** [Q5]: ajedrez: wd:Q1055911 «táctica», wd:Q1137224 «tablas», wd:Q137 «torre», wd:Q20819848 «jaque mate», wd:Q215599 «promoción», wd:Q2590761 «intercambio», wd:Q842046 «jaque», wd:Q970141 «combinación» (la de ajedrez, que además duplica a wd:Q202805), wd:Q332225 «final», wd:Q3984344 «captura», wd:Q828074 «clavado».  Países: wd:Q810 «Jordania», wd:Q35 «Dinamarca», wd:Q30 «Estados Unidos», wd:Q145 «Reino Unido», wd:Q258 «Sudáfrica».  Física y otras ciencias: wd:Q11379 «energía», wd:Q42213 «trabajo», wd:Q589 «agujero negro», wd:Q209082 «cinética química», wd:Q83371 «sismología», wd:Q9128 «luz», wd:Q4173969 «unidad de presión», wd:Q304479 «vuelta» (unidad de ángulo), wd:Q12142910 «corriente» (de fluido).  Informática: wd:Q186152 «vector» (*array*), wd:Q387015 «Trie», wd:Q858684 «NaN».

**Números, unidades y química en el registro (no en la capa general)** [Q3]: wd:Q556 «hidrógeno» y wd:Q11570 «kilogramo» (ambos `concept`; Q556 tiene 9 pertenencias `retracted`, 2 `applied_in` y 2 `special_case_of` vigentes); wd:Q743 «wolframio»; wd:Q211256 «milla por hora»; wd:Q182429 «metro por segundo»; wd:Q1770822 «Carbono-13»; wd:Q1148503 «uranio-238»; wd:Q101667 «hadrón»; wd:Q277761 «286»; wd:Q3412616 «10^27»; wd:Q2923 «9 de octubre»; wd:Q56276150 «1 sen coin»; wd:Q45 «Portugal»; wd:Q134 «alfil».  En la capa general queda un número: wd:Q4587955 «sextillón» (10³⁶).

**Duplicados por nombre normalizado** [Q2]: «sección» ×3 (wd:Q845080 sección transversal, wd:Q11703678 sección de un fibrado, wd:Q17103180 sección de un morfismo); «topología» ×2 (wd:Q42989 la rama, wd:Q18786106 la estructura); «longitud» ×2 (wd:Q36253 física, wd:Q1761084 longitud de un módulo); «vector» ×2 (wd:Q186152 *array*, wd:Q44528 euclídeo); «cubo» ×2 (wd:Q861555 potencia, wd:Q812880 sólido; el segundo aparece en la regresión 017 de §14.3); «ideal» ×2 (anillo y orden); «vértice» ×2; «arista» ×2; «pendiente» ×2; «pullback» ×2; «morfismo diagonal» ×2 (wd:Q25098585 sin descripción y wd:Q60794278: probable duplicado real en Wikidata); «head» y «tail» ×2 (los Q14009xxxx, sin descripción, parecen duplicados reales de los Q105225xxx); derivados: «algebra › equiv» (…/Algebra/Algebra/Equiv y …/Topology/Algebra/Algebra/Equiv), «group › opposite», «order › category», «module › equiv», «fin › tuple», «monoidal › closed».

**Mayúsculas** [Q2]: wd:Q3117934 «Grupo especial ortogonal»; wd:Q7098616 «Categoría opuesta»; wd:Q1062978 «Clase característica»; wd:Q6086107 «Isomorfismo de categorías»; wd:Q56276831 «Teorema del peso máximo»; wd:Q1781358 «Unión puntual»; wd:Q7233279 «Funcional lineal positiva»; wd:Q332715 «Conjunto absolutamente convexo»; wd:Q837989 «Time t»; wd:Q15704639 «Camino inducido»; wd:Q1886131 «Semilla aleatoria».

### 1.3 Causa verificada de los casos fuera de dominio

No entran al mapa porque Wikidata los llame matemáticos.  Entran porque la proyección los vuelve **objeto de pertenencias**, y la capa general toma los objetos de pertenencia como conceptos [Q6]:

- wd:Q258 «Sudáfrica»: 132 pertenencias `model_proposed`, señal `name_doc` con el alias «r s» (por *RSA*); ejemplos: `AlgCat.restrictScalars`, `RingHom.snd`, `Finsupp.supportedEquivFinsupp`.
- wd:Q810 «Jordania» (*Jordan*): 15 pertenencias por «jordan» (`IsCommJordan`, `Module.End.isNilpotent_isSemisimple_unique`) y un `formalized_as` `model_proposed` hacia `mathlib:IsJordan` (`resolve/2`, `exact_name "Jordan"`).
- wd:Q9128 «luz»: 164 pertenencias; wd:Q842046 «jaque»: 45; wd:Q30 «Estados Unidos»: 72.

Son el mismo error dominante de §13.4 y §14.4 (homónimos de `name_doc`), visto desde el extremo del concepto.  La regla de capa de §14.5 filtra por `kind` y `type`, no por dominio.  Por eso 0 personas o fechas, pero sí países y piezas de ajedrez.

### 1.4 Reglas de depuración propuestas (todas reversibles, ninguna borra)

| Clase | Regla | Mecanismo reversible |
|---|---|---|
| Sin etiqueta ES (732) | Mostrar la etiqueta EN marcada «(en inglés)» y ponerla en una cola de traducción.  Primera fuente: el título del artículo de eswiki si existe (sitelink ya en `external_ids`); si no, traducción editorial | Rótulo de presentación con procedencia (`src:atlas-curation`, `extractor: presentation/1`, estado `model_proposed` → `editorially_reviewed`).  La etiqueta de Wikidata no se toca |
| Derivadas (1,157) | Sacarlas de z0–z2 y mostrarlas desde z3 como «zona de Mathlib: …», nunca como concepto principiante.  Alternativa: rótulo de presentación desde la cabecera `/-! … -/` del módulo, con el nombre de la zona padre completo para romper los 15 choques | Exclusión del mapa registrada como decisión, con regla y versión.  El concepto y sus pertenencias siguen en el registro |
| Lean crudo (241,983 declaraciones) | En vistas de principiante, mostrar el docstring o «resultado técnico de Mathlib» y dejar el nombre Lean como código secundario | Solo presentación |
| Palabras sueltas ambiguas (72) | Calificador desde la descripción: «red (retículo de un espacio vectorial)», «haz (teoría de haces)», «campo (*stack*)», «cubo (potencia)» frente a «cubo (sólido)» | Rótulo de presentación con procedencia hacia la descripción de Wikidata que lo justifica |
| Fuera de dominio (≈ 60 en la capa general) | Exclusión del mapa y de la capa de conceptos.  Además, **retirar las pertenencias homónimas** que los crearon (`superseded` con historial, como en la ronda 3), sin borrar el ítem | Exclusión registrada + transición `model_proposed → retracted` con evento.  Ya existe el patrón (334,699 pertenencias `retracted`) |
| Números, unidades y química (1,760 numéricas, 615 en el filtro estrecho) | Fuera de la búsqueda de conceptos por defecto; siguen accesibles con filtro «otros ítems» | Exclusión de presentación, por regla |
| Duplicados (71) | Homónimos reales: calificador de presentación.  Duplicados probables (morfismo diagonal; head/tail Q14009xxxx): **fusión propuesta**, que exige decisión registrada en `merges` y se puede deshacer (`undone_at`; hoy hay 2,950 fusiones, ninguna deshecha [Q10]) | `merges` ya existe |
| Mayúsculas (65) | Minúscula inicial si la etiqueta EN va en minúscula y la primera palabra ES no es nombre propio | Rótulo de presentación por regla, con lista de excepciones |

Sobre dónde guardar las decisiones: el principio 2 pide que la fuente de verdad sea el registro.  Propuesta para el arquitecto (dueño de `model.py`/`store.py`): un `EventKind` nuevo `curation_decided` / `curation_undone` con `payload {regla, versión, objetivo, rótulo o exclusión, actor, motivo}`, que `atlas compile` aplica como derivado.  Mientras no exista, un archivo versionado `shapes/curation/r5.jsonl` con las mismas columnas sirve, pero rompe el principio 2.  Es un puente, no el diseño.

---

## 2. Conexiones

### 2.1 Relación × estado de evidencia en todo el registro [Q7]

| Relación | extracted | model_proposed | formally_linked | superseded | retracted |
|---|---:|---:|---:|---:|---:|
| cites | | 1,176,414 | | | |
| member_of_concept | 25,881 | 351,863 | | 388,174 | 334,699 |
| part_of_area | 15,725 | 73 | 244,933 | | |
| depends_on_logically | 215,764 | | | | |
| appears_in | 35,524 | | | | |
| special_case_of | 28,830 | | | | |
| named_after | 5,555 | | | | |
| authored_by | 1,696 | | | | |
| formalized_as | 517 | 621 | | | 60 |
| applied_in | 812 | | | | |
| influenced | 451 | | | | |
| prerequisite_for | 334 | | | | |
| equivalent_to | 311 | | | | |
| generalizes | 108 | | | | |

Total 2,828,345 afirmaciones (coincide con `/api/health`); vigentes 2,105,412.  **`editorially_reviewed`: 0.  `disputed`: 0.**  Ninguna afirmación del atlas tiene revisión editorial; `analogous_to`, `proves`, `models` y `counterexample_to` tienen 0 filas.

### 2.2 Lo que ve un principiante

**En el mapa (z0–z2)** [Q1]: 1,687 aristas distintas: `special_case_of` 853, `part_of_area` 790, `equivalent_to` 33, `generalizes` 11.  **0 `cites`, 0 `appears_in`, 0 `depends_on_logically`.**  Solo 1,279 de 3,229 nodos tienen alguna arista visible; 872 tienen una arista matemática; **0 de los 1,157 derivados** tienen arista visible.

**En la ficha** (`/api/entity`, las primeras 200 afirmaciones vigentes por id, sin las pertenencias donde la entidad es objeto: `core/atlas-server/src/entity.rs`) [Q8], sumando las 3,229 fichas: 22,304 afirmaciones mostradas.

| Categoría | Afirmaciones en fichas | % |
|---|---:|---:|
| `special_case_of` (Wikidata P31 o P279) | 11,968 | 53.7 |
| `part_of_area` | 5,778 | 25.9 |
| `appears_in` (documental) | 2,938 | 13.2 |
| `formalized_as` (342 `extracted`, 311 `model_proposed`) | 653 | 2.9 |
| historia (`named_after`, `authored_by`, `influenced`) | 509 | 2.3 |
| `equivalent_to` 194, `prerequisite_for` 170, `generalizes` 60, `applied_in` 34 | 458 | 2.1 |

Documental frente a matemática: 2,938 `appears_in` frente a 12,426 matemáticas (`special_case_of` + las cuatro de la última fila).  `cites` no aparece en ninguna ficha de la capa: vive entre artículos de Wikipedia y el principiante la encuentra en las rutas documentales (H06 de la auditoría).  Pero el 96 % de lo «matemático» que ve en la ficha es tipificación de Wikidata (`special_case_of`).  Solo 458 afirmaciones en las 3,229 fichas dicen algo más que «es un tipo de» o «pertenece al área».

**Conceptos sin ninguna relación matemática** [Q8]:

- Contando solo afirmaciones directas del registro: **1,304 de 2,640 `concept`** (los 1,157 derivados y 147 de Wikidata), y 1,810 de los 3,229 nodos si se cuentan áreas y enunciados.
- Contando también las aristas agregadas concepto → concepto de `/api/concept` (dependencias de Mathlib entre miembros primarios): solo 41 quedan sin nada (7 de Wikidata).  Pero esas aristas son de baja calidad (§2.3): no sirven como «relación matemática» para principiantes mientras no se revisen.

### 2.3 Veinte conexiones al azar, semilla 20261019 [Q11]

Diseño: 10 de las 1,470 afirmaciones del registro con dos extremos `concept` distintos en la capa (lo que ve la ficha y el mapa), y 10 de las 25,786 aristas agregadas concepto → concepto de `/api/concept` con los dos extremos en la capa (lo que usan la ficha de concepto y la ruta por conceptos).  Las aristas agregadas se tomaron de las 200 primeras por concepto; 18 conceptos tienen más de 200, así que el muestreo agregado está algo sesgado hacia las aristas más pesadas.  Se juzgó con etiqueta, descripción, la afirmación testigo y la pertenencia de cada declaración, sin consultar fuentes externas.

| # | Conexión | Relación y estado | ¿Correcta? | ¿Útil para aprender? | Nota |
|---|---|---|---|---|---|
| D1 | categoría delgada → categoría localmente pequeña | special_case_of, extracted | sí | poco (avanzado) | |
| D2 | lenguaje de Dyck → lenguaje libre de contexto | special_case_of, extracted | sí | sí | |
| D3 | espacio metrizable → espacio de Hausdorff | special_case_of, extracted | sí | sí | Se lee como implicación; correcto |
| D4 | compacidad local → espacio de Hausdorff | special_case_of, extracted | **dudosa** | — | Solo es cierto si la definición incluye Hausdorff (Bourbaki); sin esa convención enseña algo falso |
| D5 | número áureo → número construible | special_case_of, extracted | sí | sí | Instancia (P31) leída como caso particular |
| D6 | grupo nilpotente → grupo resoluble | special_case_of, extracted | sí | sí | |
| D7 | representación semisimple → módulo semisimple | special_case_of, extracted | sí | medio | |
| D8 | sextillón → número compuesto | special_case_of, extracted | sí | **no** | Un número como concepto de la capa |
| D9 | homotopía → grupo de homotopía | prerequisite_for, extracted | sí | sí | Viene de P2283 «uses», ya rotulado como orden de estudio |
| D10 | grupo trivial → grupo unitario especial | special_case_of, extracted | **engañosa** | no | SU(1) es trivial; «el grupo trivial es caso particular de SU» invierte lo que enseña |
| A1 | cadena → type | depends_on_logically, «extracted» (9) | **no** | no | Homónimo en los dos extremos: `OmegaCompletePartialOrder.Chain` asignada a «cadena» de complejos; `Part` a «type» de teoría de modelos |
| A2 | puerta lógica → espacio topológico | depends_on_logically, «extracted» (1) | **no** | no | `ContMDiff.extend_one` asignada a «puerta lógica» porque la firma menciona `Set.diff` |
| A3 | igualdad → submódulo | depends_on_logically, «extracted» (1) | dudosa | no | `Submodule.decidableEq`: «la igualdad depende de submódulo» es falso como frase |
| A4 | perfectoid › fontaine theta (derivado) → espacio de Teichmüller | depends_on_logically, «extracted» (3) | **no** | no | `WittVector.teichmuller` es el representante de Teichmüller, no el espacio |
| A5 | homomorfismo de anillos → homomorfismo de grupos | depends_on_logically, «extracted» (10) | dudosa | poco | `MulChar` asignado a «homomorfismo de grupos» por la firma `MonoidHom` |
| A6 | submódulo → linear algebra › d finsupp (derivado) | depends_on_logically, «extracted» (4) | sí (declaraciones) | no | El extremo es un rótulo de código |
| A7 | matriz cero → matriz cuadrada | depends_on_logically, «extracted» (1) | dudosa | no | `Matrix.diag` asignada a «matriz cuadrada»; la matriz cero no tiene por qué ser cuadrada |
| A8 | ideal fraccionario → número entero | depends_on_logically, «extracted» (1) | dudosa | no | Vínculo laxo vía premisas genéricas |
| A9 | anillo de los enteros → raíz primitiva módulo n | depends_on_logically, «extracted» (3) | **no** | no | `IsPrimitiveRoot` es raíz primitiva de la unidad; el sujeto `mathlib:93e6a6aa47505f1a` entra por la zona |
| A10 | grupo → red | depends_on_logically, «extracted» (4) | **no** | no | `Lattice` de Mathlib (retículo de orden) asignado por `resolve` a «red» (subgrupo discreto) |

Recuento (juicio de agente, n = 10 por estrato):

- Directas: 7/10 correctas y útiles en algo, Wilson 95 % [0.40, 0.89]; 1 correcta e inútil; 2 dudosas o engañosas.
- Agregadas: **0/10 correctas y útiles [0.00, 0.28]**; 1 correcta e inútil; 4 dudosas; 5 incorrectas.

Con n = 10 los intervalos son anchos.  Lo que sí muestra: la capa de Wikidata es mayormente correcta pero poco instructiva, y la capa agregada no sirve para principiantes.

### 2.4 Patrones de error

1. **Homónimo de palabra suelta** en la pertenencia (`name_doc`: «chain», «type», «jordan», «r s»).  Es la causa de A1, A9, de los países y del ajedrez.
2. **Asignación por firma** (`signature mentions mathlib:Set.diff` → puerta lógica; `MonoidHom` → homomorfismo de grupos).  Una firma menciona tipos auxiliares, no el tema.
3. **`resolve/4 item_candidates` para nombres genéricos de Mathlib** (`Lattice` → red, `teichmuller` → espacio de Teichmüller, `Path` → «ruta» como ruta de archivo).
4. **Tipificación de Wikidata leída como relación**: P31 «instancia de» y P279 «subclase de» van las dos a `special_case_of`, y el motor la trata como transitiva (D8, D10).
5. **Arista con extremo derivado**: correcta pero ilegible (A6); 0 de los derivados tiene arista en el mapa.

### 2.5 Dónde una relación se lee más fuerte de lo que es

| Dónde | Qué dice la pantalla | Qué respalda de verdad | Cifra |
|---|---|---|---|
| Aristas agregadas concepto → concepto | `depends_on_logically` con estado **`extracted`** | Una dependencia Lean entre dos declaraciones cuyas pertenencias son `model_proposed` (primarias: 0.381 estricto en §14.4).  El estado mostrado es el de la dependencia, no el del eslabón más débil | 25,786 de 25,786 aristas muestreadas, todas «extracted» [Q11] |
| Misma arista, en palabras | «A depende lógicamente de B» | «alguna declaración agrupada en A usa alguna agrupada en B».  No dice nada del concepto A | `chain_authorizes` de `depends_on_logically` es para declaraciones |
| Ruta por conceptos | «Fuerza 5/5» (R4-02) | La fuerza ignora la incertidumbre de la pertenencia | auditoría R4-02 |
| `equivalent_to` desde P460 «said to be the same as» | Equivalencia simétrica y **transitiva** | Una afirmación de igualdad que Wikidata marca como posiblemente incierta.  En una muestra de 10 [Q12]: «grafo ≡ finite graph» y «condición necesaria ≡ apódosis» son falsas; «axioma de elección ≡ teorema del buen orden» vale solo sobre ZF | 311 en el registro, 153 tocan la capa |
| `special_case_of` desde P31 | «caso particular de» | «Es un ejemplo de».  Mezclar instancia y subclase rompe la transitividad | 11,968 apariciones en fichas |
| `formalized_as` `model_proposed` por `resolve/2` | «formalizado como» | Coincidencia de nombre.  En 10 al azar [Q12]: «ruta» (de archivo) → `mathlib:Path`, «teorema de multiplicación» → `ofMul_mul`, «complete» → `IsComplete` son homónimos o dudosos; Lusin y Krull sí corresponden | 621 en el registro, 311 en fichas de la capa |
| `part_of_area` `formally_linked` | «vinculada formalmente» | La declaración vive en ese archivo de Mathlib (`formal.axioms: []`).  Es ubicación, no verificación | 244,933 |
| «Intuición» de la ficha | Definición | Descripción de catálogo de Wikidata, que puede estar mal (R4-01: número primo sin «positivos») | ver §3 |

---

## 3. Explicaciones

### 3.1 Qué texto existe hoy para los 3,229 nodos de la capa general [Q13]

| Texto | Nodos con texto | Mediana de palabras | p90 |
|---|---:|---:|---:|
| Descripción de Wikidata ES | 703 | 8 | 17 |
| Descripción de Wikidata EN | 1,907 | 9 | 18 |
| Extracto de Wikipedia ES (entrada, ≤ 1,500 caracteres) | 1,041 | 133 | 245 |
| Extracto de Wikipedia EN | 1,882 | 152.5 | 253 |
| Alguno de los dos extractos | 1,889 | | |
| Docstring de Mathlib de alguna declaración miembro | 2,395 | 42 (el más largo) | 103 |
| Docstring del destino de `formalized_as` | 482 | 23 | 67 |
| Fórmula | 440 | | |

- **Descripción de catálogo de < 6 palabras** (la mejor entre ES y EN): **1,766** de 3,229.  Son 1,157 derivados (solo la plantilla «Concepto derivado: la zona … de Mathlib») y 609 de Wikidata.
- Además, sin extracto de Wikipedia: 1,273 (1,157 derivados + 116 de Wikidata).  Sin docstring tampoco: 582 (557 + 25).
- **Solo en español**: 2,147 nodos (66 %) no tienen ni una descripción ES de ≥ 6 palabras ni un extracto de Wikipedia ES.  Entre los 2,072 de Wikidata son 990.
- Los extractos se cortan a 1,500 caracteres (`MAX_EXTRACT`, `atlas/ingest/wikipedia.py`): **5,625 de 31,976 fragmentos (17.6 %) llegan al tope** y terminan a media frase [Q14].
- La ficha muestra hoy como «Intuición» **solo la descripción de Wikidata** (`web/src/panel.ts`, `localized(e.descriptions)`).  El extracto de Wikipedia existe en el registro como `source_fragment` pero la ficha del concepto no lo usa.
- Los docstrings de miembros describen **declaraciones**, no el concepto, y llegan por pertenencias de precisión 0.38: no sirven como explicación sin revisión.
- Wikipedia tampoco es una fuente segura por sí sola.  El extracto ES de integración (`oldid=175753603`) dice «La integral es la operación inversa al diferencial de una función» (impreciso) y atribuye el primer uso a Descartes, Newton, Leibniz y Barrow (discutible).  Copiarlo a una ficha sin revisión reproduce el problema de R4-01.

### 3.2 Qué hace falta para una ficha de cinco preguntas

| Pregunta | Fuente posible hoy | Hueco |
|---|---|---|
| Qué es | Extracto de Wikipedia ES/EN (1,889 nodos), descripción de Wikidata | Redactar 2–3 frases para principiante, con cita por frase; revisión matemática; 1,340 nodos sin extracto |
| Por qué importa | Extracto (a veces), `applied_in` (34 en fichas) | Casi nada estructurado; requiere redacción editorial con fuente |
| Qué saber antes | `prerequisite_for` (170 en fichas), `special_case_of` hacia arriba | Muy escaso; no se debe inferir de `depends_on_logically` agregado |
| Con qué se conecta | `special_case_of`, `generalizes`, `equivalent_to` (con la corrección de §2.5) | Frases que expliquen el sentido de la conexión, no solo el verbo |
| Cómo compruebo que lo entendí | Nada en el registro | Ejercicio con respuesta, de un libro abierto o escrito por un editor, siempre revisado |

### 3.3 Formato propuesto

Una ficha es una **vista derivada en markdown**, reconstruible.  La fuente de verdad son afirmaciones del registro, una por frase (principios 1 y 2):

- Cada frase es una afirmación `explains` (relación nueva, proyección `learning`; requiere esquema v4 y decisión del arquitecto).  Sujeto: una entidad nueva de tipo `note` con el texto en `statements` (ES y EN).  Objeto: el concepto.  Rol `context`: la sección (`que_es`, `por_que_importa`, `antes`, `conexiones`, `compruebo`).
- **Procedencia por frase**: `source_id` (p. ej. `src:wikipedia-es`), `locator` (URL con `oldid` y párrafo, o `Mathlib/…/File.lean:línea`, o libro y página), `extractor` y versión (`editorial/1` si la escribe una persona, `draft-llm/1` si la propone un modelo), `license`, `retrieved_at`, `quote` (el pasaje de la fuente que la sostiene).
- **Estado**: `model_proposed` al redactarla; `editorially_reviewed` solo con evento de revisión (actor, fecha).  La ficha pública muestra únicamente frases revisadas.  Las demás, si se muestran, van bajo «borrador sin revisar».
- Las secciones «qué saber antes» y «con qué se conecta» **no son texto libre**: se generan desde afirmaciones existentes (`prerequisite_for`, `special_case_of`…) y citan su `claim_id`.  Así no aparece una relación nueva sin procedencia.
- Licencia: si una frase parafrasea Wikipedia, la ficha queda bajo CC BY-SA 4.0 con atribución por revisión.

Ejemplo de la vista (estructura real; el texto de la frase 1 es el de Wikipedia EN y tendría que pasar por revisión):

```markdown
---
concepto: wd:Q80091
titulo: integración
version: ficha/1
generada: 2026-10-xx desde el registro (event_seq …)
estado: borrador — 0 de 3 frases revisadas
---
## Qué es
Una integral es el análogo continuo de una suma; sirve para calcular áreas y volúmenes.[^1]

## Qué saber antes
- derivada — `prerequisite_for`, extracted, c:…[^2]

## Cómo compruebo que lo entendí
TBD (sin fuente revisada)

[^1]: Wikipedia (en), «Integral», oldid=1379896500, párrafo 1 · extractor draft-llm/1 · model_proposed · 2026-10-xx
[^2]: Wikidata P2283, https://www.wikidata.org/wiki/Q… · atlas.ingest.wikidata wd-map/2 · extracted · 2026-10-16
```

---

## 4. Diez trabajos para la ronda 5, por prioridad

Impacto = nodos de la capa general (3,229) que mejoran, medido arriba.  Riesgo = riesgo de **enseñar algo falso** que el trabajo quita (A, el criterio 1 del diseño) o que puede introducir.

| # | Trabajo | Impacto (nodos visibles) | Riesgo que quita | Riesgo que introduce | Base |
|---|---|---:|---|---|---|
| 1 | Las aristas agregadas concepto → concepto toman el estado del eslabón más débil (`model_proposed`), se rotulan «candidata» y salen del resumen de principiante y de la «Fuerza».  Solo pertenencias primarias con señal propia | 2,640 fichas de concepto y todas las rutas por concepto | **Alto** (0/10 útiles; R4-02) | Bajo: se ven menos conexiones | §2.3, §2.5 |
| 2 | Sacar de la capa y de los conceptos lo que está fuera de dominio, y retirar con historial las pertenencias homónimas que lo crearon (países, ajedrez, física no matemática, «NaN», «era») | ≈ 60 nodos + sus pertenencias (p. ej. 132 de Sudáfrica) | Alto | Bajo; reversible | §1.3 |
| 3 | Derivados fuera de z0–z2, o con rótulo legible en ES que use la zona completa | 1,157 (36 % de la capa) | Medio (rótulos que parecen conceptos) | Bajo | §1.1 |
| 4 | `equivalent_to` desde P460 → no transitivo y rotulado «Wikidata dice que es lo mismo»; P31 → «es un ejemplo de», separado de P279 «es un tipo de» | 153 + 11,968 apariciones en fichas | Alto (grafo ≡ grafo finito) | Medio: cambia la composición de rutas; hay que volver a correr AtlasBench | §2.5 |
| 5 | `formalized_as` `model_proposed` oculto en fichas de principiante hasta revisión | 311 apariciones | Medio-alto («ruta» → `Path`, Jordania → `IsJordan`) | Bajo | §2.5 |
| 6 | Extracto de Wikipedia en la ficha (ES si hay, si no EN marcado), con atribución y corte en frase completa; quitar el corte a 1,500 caracteres a media frase | 1,889 | Bajo-medio | **Medio**: Wikipedia tiene frases imprecisas (integración); rotular «texto de Wikipedia, sin revisión» | §3.1 |
| 7 | Etiquetas ES: marcar «(en inglés)» las 732 y traducir primero desde el título de eswiki; corregir las 65 mayúsculas; calificar las 72 palabras ambiguas y los 20 grupos de homónimos | 732 + 65 + 72 + 71 | Medio (red/retículo, campo/*stack*, cubo/cubo) | Bajo si cada rótulo cita su fuente | §1 |
| 8 | Revisión editorial de las descripciones de catálogo de los 100 conceptos más visibles (prioridad de tesela) y de los conceptos del bench, empezando por R4-01 | 100 | Alto (definiciones que se muestran como «Intuición») | Bajo | §3.1, R4-01 |
| 9 | Esquema v4 de fichas (`note` + `explains`, eventos `curation_*`) y piloto de 20 fichas completas, revisadas por una persona: integral, derivada, número primo, pi, Pitágoras, Bayes, matriz y el resto de las búsquedas de la auditoría | 20, a fondo | Controlado por la revisión | Medio si se publica sin revisar | §3.3 |
| 10 | Protocolo de medición antes de tocar nada: 200 nombres + 200 conexiones de la capa, dos jueces (al menos uno humano), metas fijadas antes de ver resultados, por clase y con Wilson | 0 directo; es lo que permite decir que algo mejoró | — | — | §2.3 (n = 10 no basta) |

Orden sugerido: 10 primero (fijar metas), después 1, 2 y 5 (quitan falsedades sin escribir texto), 3, 4 y 7 (orientación), y 6, 8 y 9 (explicaciones, que necesitan revisión humana).

---

## Apéndice A · Consultas

| Id | Qué produce | Dónde |
|---|---|---|
| Q1 | Nodos y aristas de la capa general | `r5_fetch_tiles.py`: `GET /api/map/tiles?layer=structure&z={0..2}&x=&y=&format=json`, unión por `id` y por `(source,target,relation,evidence_state)`.  Grados: conteo sobre `edges` |
| Q2 | Clases de nombre en la capa | `r5_names.py` sobre la etiqueta de la tesela y `entities.data` (`labels`, `descriptions`, `provenance[0].extractor_version`).  Duplicados: `casefold` + NFKD sin diacríticos + `[^\w]+ → " "`, agrupados |
| Q3 | Clases en todo el registro | `r5_allconcepts.py`: `SELECT id, type, data FROM entities WHERE type IN ('concept','area') AND merged_into IS NULL` (23,898); `json_extract(data,'$.labels.es') IS NULL` para los 8,773 sin ES |
| Q4 | 72 palabras sueltas ambiguas | `r5_ambiguous.py`: la lista fija de etiquetas (juicio de agente sobre las 420 palabras sueltas de Q2) cruzada con `r5-names.json` |
| Q5 | Fuera de dominio | Expresiones regulares sobre `descriptions` (ajedrez, país, física/química/astro/Tierra, unidades, informática) y revisión a mano de las 81 coincidencias |
| Q6 | Por qué un país es concepto | `SELECT cp.entity_id, p.quote FROM claim_participants cp2 JOIN claims cl ON cl.id=cp2.claim_id AND cl.relation='member_of_concept' AND cl.evidence_state='model_proposed' JOIN claim_participants cp ON cp.claim_id=cl.id AND cp.role='subject' LEFT JOIN provenance p ON p.claim_id=cl.id WHERE cp2.entity_id='wd:Q258' AND cp2.role='object'` |
| Q7 | Relación × estado | `SELECT relation, evidence_state, count(*) FROM claims GROUP BY 1,2` |
| Q8 | Fichas y conceptos sin relación matemática | `r5_load.py` (`claim_participants ⋈ claims` para las 3,229 entidades) + `r5_edges.py`.  La ficha reproduce el filtro de `entity.rs`: vigentes, sin `member_of_concept` con la entidad como objeto, `ORDER BY id LIMIT 200`.  Matemáticas = `depends_on_logically, proves, prerequisite_for, generalizes, special_case_of, equivalent_to, analogous_to, applied_in, models, counterexample_to`.  Agregadas: `GET /api/concept?id=…&limit=1&edge_limit=200` |
| Q9 | Declaraciones de Mathlib | `SELECT substr(id,1,instr(id,':')), type, count(*) FROM entities WHERE merged_into IS NULL GROUP BY 1,2` |
| Q10 | Fusiones | `SELECT count(*), sum(undone_at IS NOT NULL) FROM merges` → 2,950, 0 |
| Q11 | Veinte conexiones | `r5_sample20.py`, semilla 20261019; testigos con `claim_participants` y la cita (`provenance.quote`) de cada pertenencia |
| Q12 | Muestras de `formalized_as` y `equivalent_to` | `random.Random(20261019).sample(...)` de las afirmaciones vigentes de cada relación que tocan la capa (en `r5-load.pkl`) |
| Q13 | Texto disponible | `r5_text.py`: palabras = `\w+`; Wikipedia por `appears_in` hacia `wp:`; docstrings = `descriptions.en` de los sujetos de `member_of_concept` y de los objetos de `formalized_as` |
| Q14 | Extractos cortados | `SELECT data FROM entities WHERE type='source_fragment'`, contar descripciones de ≥ 1,500 caracteres |

Limitaciones: un solo juez de agente para todos los juicios; muestra de 10 por estrato; las clases «ambiguas» y «fuera de dominio» dependen de una lista y de expresiones regulares revisadas a mano, así que son cotas aproximadas, no censos; la API usada es la de un servidor que no arranqué y que pudo tener cambios locales de `concept.rs`/`entity.rs` (el diff en `git status` es de paginación, no cambia los campos usados).
