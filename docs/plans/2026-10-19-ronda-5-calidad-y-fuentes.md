# Plan: ronda 5 · conexiones de calidad, nombres, explicaciones y obras

Fecha: 2026-10-19.  Dueño: Adrián.  Estado: **borrador para revisión de Adrián**; no se ejecuta hasta su visto bueno.

Fuentes del plan (todas en `docs/plans/r5/`):
- `diagnostico-calidad.md`: nombres, conexiones y explicaciones medidos sobre el snapshot `a2e90658…`.
- `fuentes-candidatas.md`: 14 fuentes nuevas, con licencia.
- `papers-historia.md`: obras desde la Antigüedad.

## Problema (medido)

| Hueco | Cifra | Fuente |
|---|---|---|
| Conexiones concepto→concepto mostradas como `extracted` aunque se sostienen en pertenencias `model_proposed` | 25,786 aristas vistas; 0/10 útiles en la muestra (5 incorrectas por homónimos) | diagnóstico §2 |
| `equivalent_to` transitivo desde Wikidata P460 | casos falsos como «grafo ≡ grafo finito» | diagnóstico §2 |
| Relaciones registradas en los dos sentidos | Pitágoras «generaliza» y «es caso particular de» el quinto postulado | crítica r4, C4-03 |
| Nada revisado por una persona | 0 `editorially_reviewed`, 0 `disputed` | diagnóstico §2 |
| Mapa en español con texto en inglés | 1,889 de 3,229 nodos (58 %): 732 sin etiqueta ES y 1,157 derivados «… (Mathlib, derivado)» | diagnóstico §1 |
| Nodos fuera de dominio por homónimos | unos 60 en la capa general (Sudáfrica con 132 pertenencias por el alias «r s») | diagnóstico §1 |
| Fichas sin explicación | 66 % de los conceptos en español sin texto de 6 palabras o más; 1,889 extractos de Wikipedia guardados que la ficha no usa; 5,625 extractos cortados a media frase | diagnóstico §3 |
| Fichas de la ronda 4 | las cinco preguntas se responden en 0 de 7 conceptos | crítica r4 |
| Obras | 0 `publication`, 0 `historical_date`; 4,623 personas con QID sin obras | papers §1 |

## Qué se hace, en este orden

0. **Protocolo de medición (antes de tocar datos).**  Dos jueces independientes y metas escritas antes de ver resultados.  Muestras con semilla nueva, nunca la mitad test ya usada.
1. **Quitar falsedades sin escribir texto nuevo** (`atlas/concepts.py`, `atlas/ingest/wikidata`, `core/atlas-core`, `core/atlas-paths`):
   - Las aristas agregadas toman el estado de su eslabón más débil y se rotulan «candidata».
   - P460 deja de ser `equivalent_to` transitivo: pasa a «se dice equivalente según Wikidata», sin composición.
   - P31 ya no se lee como «caso particular»: se separa instancia de subclase.
   - Las relaciones en los dos sentidos pasan a `disputed`, con historial.
   - Los `formalized_as` propuestos solo por nombre se ocultan del modo Explorar.
   - `part_of_area` derivado de la ruta de un archivo deja de ser `formally_linked`.
2. **Nombres** (reversible, sin borrar):
   - Exclusión registrada de lo que no es matemática, y retiro con historial de sus pertenencias homónimas.
   - Los conceptos derivados salen de z0–z2.
   - Etiquetas en español desde Wikidata (etiquetas, alias y lexemas) y Wiktionary.
   - Los duplicados casi idénticos se proponen como fusión en `merges`.
3. **Explicaciones con sustento, en markdown**:
   - La ficha muestra el extracto completo de Wikipedia, cortado en límite de oración y con su licencia.
   - Formato de ficha editorial en markdown: una afirmación `explains` por frase, con fuente, localizador y estado.  Solo se publica lo revisado.  Las secciones «Qué saber antes» y «Con qué se conecta» se generan desde afirmaciones existentes, citando su `claim_id`.
   - Piloto: las fichas de los ~20 conceptos de las tres puertas, escritas a partir de fuentes y **revisadas por Adrián**.  Sin su revisión, siguen en «borrador editorial, sin revisar».
4. **Fuentes nuevas**:
   - Listas de temas de Mathlib (`docs/undergrad.yaml`, `docs/overview.yaml`; Apache-2.0).
   - Etiquetas, alias y lexemas de Wikidata (CC0) y Wiktionary (CC BY-SA 4.0).
   - PlanetMath (CC BY-SA 2.5).  Su «relacionado» no equivale a prerrequisito.
   - Math Coherence Map, solo como piloto manual de unas 50 conexiones.
   - ProofWiki queda en espera de un volcado; si no llega, entra zbMATH Open.
5. **Obras desde la Antigüedad, completas** (decisión de Adrián, 2026-10-19: «todo», unos 5 millones; `papers-historia.md`):
   - Todos los metadatos de zbMATH Open (5,201,602 documentos de 1868 a 2026, más 1,879 anteriores; CC BY-SA 4.0) y las obras del campo Mathematics de OpenAlex (4,625,779; CC0), unificadas por DOI, id de zbMATH y QID.  El solapamiento entre las dos se mide; no se suma.
   - Más una lista curada de testimonios antiguos y no europeos (Plimpton 322, papiro Rhind, Euclides, Arquímedes, Diofanto, Liu Hui, Aryabhata, al-Juarismi…) y las 2,274 tablillas de CDLI, con fechas EDTF.
   - Citas entre obras del corpus (OpenCitations, Crossref, zbMATH Open).  Son miles de millones en total, así que se guardan solo las internas al corpus y se reporta cuántas quedaron fuera.
   - Lo que es estadística o aprendizaje automático dentro del campo Mathematics de OpenAlex se marca por subcampo y se puede filtrar; no se borra.
   - Solo metadatos: título, autores, fecha, lugar de publicación, ids, clasificación y citas.  Sin resúmenes ni texto completo con derechos.
   - Cobertura honesta: cada celda de periodo por área dice cuántas obras se ingirieron frente a cuántas indexa la fuente.  Lo demás es «desconocido».
   - Una obra no «demuestra» nada: la demostración es una entidad que aparece en la obra.  «Introduce el concepto X» exige una fuente historiográfica.
5b. **Escala** (unas 15 veces el grafo de hoy, que tiene 320,214 nodos):
   - Ingesta por volcados y en lotes reanudables, no por API página a página.  Se respetan los límites de tasa para lo que no tenga volcado.  Caché en `data/cache/`.  Disco: hay 356 GB libres; el registro mide hoy 14 GB.
   - Snapshot, índices y layout medidos a la escala real antes de diseñar: tiempo de `compile`, memoria, tamaño, p95 de búsqueda, de recorridos y de teselas.
   - Las obras no entran en la capa general del mapa: van en una capa propia por periodo y área, con presupuesto por tesela.  En el modo Explorar se ven como «obras sobre este concepto» en la ficha y en una línea de tiempo.
   - Si una medición no cabe en los topes, se rediseña la parte que falla (teselas paginadas, índice por fragmentos, layout por niveles) antes de seguir ingiriendo.
6. **Búsqueda**: «Bayes» devuelve primero el teorema, y no a Thomas Bayes, en las consultas de concepto.

## Decisiones del arquitecto (antes de ejecutar)

1. Esquema v4: fechas EDTF con tipo (composición, copia, publicación, edición) y su incertidumbre.  Hoy `datetime` no acepta años antiguos y hay un posible error de un año en las fechas antes de Cristo (Wikidata no tiene año cero).
2. Separar obra, testimonio y edición.  Se recomienda la opción mínima para esta ronda.
3. `introduced_in` opcional, con fuente historiográfica obligatoria.
4. Copista frente a autor (el papiro Rhind tiene como copista a Ahmes).
5. `EventKind curation_*` para exclusiones, rótulos de presentación y fusiones.
6. Afirmación `explains` para las frases de las fichas.

## Reglas de decisión (borrador; se fijan al aprobar el plan)

| Componente | Pasa si |
|---|---|
| Aristas agregadas | 0 aristas concepto→concepto con estado más fuerte que su eslabón más débil (prueba) |
| Equivalencias | 0 composiciones transitivas de P460; las relaciones en los dos sentidos quedan en `disputed` |
| Nombres en español | ≥ 80 % de los nodos de z0–z2 con etiqueta ES y 0 derivados en z0–z2 |
| Fuera de dominio | 0 en z0–z2 según la lista del diagnóstico, con retiro registrado y reversible |
| Muestra nueva de 20 conexiones de la capa general | ≥ 14/20 correctas y útiles, con dos jueces y acuerdo reportado |
| Fichas | ≥ 1,500 conceptos de la capa con un extracto completo citado; piloto de 20 fichas revisadas por Adrián |
| Prueba de usuario común | ≥ 5 de 7 conceptos con las cinco preguntas respondidas (la regla de la ronda 4, que no pasó) |
| Obras | ≥ 95 % de los documentos de zbMATH Open y de las obras Mathematics de OpenAlex ingeridos con procedencia por obra, con el solapamiento medido; 0 fechas antiguas rechazadas; tabla de cobertura por periodo y área |
| Escala | `compile --all` ≤ 2 h; p95 de búsqueda ≤ 100 ms, de recorridos ≤ 2× el de la ronda 3 y de teselas ≤ 2× el de la ronda 4; memoria del servidor ≤ 16 GB; todo medido a la escala real |
| Licencias | cada fuente nueva con licencia verificada en su página y anotada en `docs/STATUS.md` |
| Sin regresión | pruebas en verde, AtlasBench sin regresión y diseño de la ronda 4 sin regresión |

## Ejecución propuesta

1. Arquitecto: esquema v4 y las decisiones de arriba.
2. En paralelo:
   - Calidad de conexiones (puntos 1 y 6).
   - Nombres y fuentes (puntos 2 y 4).
   - Obras (punto 5): primero un piloto de 100,000 obras para medir la escala (punto 5b); luego la ingesta completa por lotes.
3. Fichas (punto 3).
4. Integración, crítica con dos jueces y `docs/STATUS.md` §16.
