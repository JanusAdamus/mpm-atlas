# Ronda 5 · Obras y papers: de las tablillas a arXiv

Fecha de consulta: 2026-10-19.  Investigación de escritorio para AJ.  Extiende `fuentes-candidatas.md` (fuentes de conexiones y explicaciones) y `diagnostico-calidad.md` (calidad del registro actual); no repite lo que ya está ahí.  Método: páginas de licencia y documentación, la API de GitHub y peticiones pequeñas a WDQS (grafo principal y grafo académico), OpenAlex, Crossref, OpenCitations, zbMATH Open (REST y OAI-PMH), arXiv, CDLI, Internet Archive y HathiTrust.  Lectura del registro en solo lectura (`file:data/atlas.db?mode=ro`).  No se descargó ningún dump, no se instaló nada y no se tocaron `atlas/model.py` ni `docs/SPEC.md`.

Convenciones: **V** verificado hoy en la fuente primaria; **S** solo en fuente secundaria; **NV** no verificado.  Las cifras con fecha son conteos de hoy y cambian con la fuente.

## 0. Lo que ya hay en el registro y sirve de ancla

| Medida | Valor | Cómo |
|---|---:|---|
| Personas (`person`) | 4,623, todas con QID | `entities` y `external_ids` |
| Obras (`publication`) | **0** | `entities` agrupado por tipo |
| `authored_by` | 1,696, todas de concepto/enunciado → persona (P61, P1318); ninguna de obra → persona | `claims` + `claim_participants` |
| `appears_in` | 35,524, todas hacia `source_fragment` (extractos de Wikipedia) | ídem |
| `historical_date` en afirmaciones | 0 de 2,828,345 | `json_extract(data,'$.historical_date')` |
| Fechas de personas | ninguna guardada en el registro | muestra wd:Q8747 (Euclides): solo etiquetas, alias e ids |

Puentes que Wikidata ya ofrece para las 4,623 personas del registro (WDQS, lotes de 500 QID, hoy): fecha de nacimiento P569 en 4,257; Mathematics Genealogy P549 en 3,163; zbMATH author P1556 en 3,650; MacTutor P1563 en 1,338.  De los 25,895 conceptos, enunciados, teorías, definiciones, algoritmos y áreas con QID: 2,961 son tema principal (P921) de alguna obra, 1,247 tienen descubridor (P61), 582 tienen fecha de descubrimiento (P575) y 2,604 tienen «descrito por la fuente» (P1343; casi siempre una enciclopedia, no la obra original).

Consecuencia: la base de personas ya existe y está bien enlazada.  Falta la capa de obras y la de fechas.

## 1. Tabla comparativa

| Fuente | Aporta | Licencia de metadatos | Licencia de texto | Acceso y límites | Periodo real | Enlace con el atlas |
|---|---|---|---|---|---|---|
| **Wikidata, grafo principal** | Obras canónicas con QID, P50 autor, P571/P577 fecha con precisión, P921 tema, P2860 cita, P747/P629 ediciones | CC0 (V, ya en `fuentes-candidatas.md`) | — | WDQS; ya hay adaptador | Desde −1600 (Rhind).  Obras de las 4,623 personas: 14,786, de las que 6,093 no tienen fecha (§2) | QID directo para obra y autor |
| **Wikidata, grafo académico** | Artículos científicos (WikiCite) | CC0 | — | Endpoint aparte `query-scholarly.wikidata.org` desde la división del grafo: 46,009,135 artículos, 2,821 con P921 = matemáticas; en el grafo principal la misma consulta da 0 (V) | Siglos XIX–XXI | QID; DOI P356 |
| **OpenAlex** | Works, autores, `referenced_works`, `cited_by_count`, temas (campo 26 Mathematics) | CC0: «Our data is CC0» (V, openalex.org) | Resúmenes como índice invertido; su licencia no está declarada (NV).  Crossref advierte que «some abstracts … may be subject to copyright» (V) | API sin llave con presupuesto diario compartido (V: aviso en `meta`, US$0.10/día por IP); snapshot en S3 | 4,625,779 works en el campo 26.  Antes de 1700: 154, todos ediciones impresas (V) | P10283 (211,908 ítems con OpenAlex ID); DOI |
| **Crossref** | Metadatos de DOI, referencias depositadas | «almost none of the metadata is subject to copyright, and you may use it for any purpose» (V) | Resúmenes con posible copyright (V) | REST; pool educado con `mailto`: cabecera `x-rate-limit-limit: 3` por segundo (V) | DOIs retroactivos de revistas antiguas; 59,266 works de todas las áreas con fecha hasta 1799 (V) | DOI |
| **OpenCitations Index (antes COCI)** | Citas DOI→DOI | CC0 (S; la página de descargas no lo dice; NV hoy) | — | REST v2 (V); dump de julio 2026: 2,559,460,009 citas, CSV 36 GB comprimido / 250 GB (V, download.opencitations.net) | Lo que tenga DOI.  Ejemplo: Kirchhoff 1850, Crelle 40, recibe 809 citas (V) | DOI, OMID, OpenAlex |
| **zbMATH Open** | Registros con MSC, autores desambiguados, referencias, reseñas; Jahrbuch über die Fortschritte der Mathematik | Términos de la API (junio 2021): «All data are allowed to be used under a CC-BY-SA 4.0 license» y «The API does not contain the complete content of zbMATH Open but only FIZ Karlsruhe's own data and data authorized by the publishers» (V) | Reseñas de terceros vacías: «contents unavailable due to conflicting licenses» (V) | REST `api.zbmath.org/v1` sin llave, «a reasonable rate» (V); OAI-PMH `oai.zbmath.org/v1/` (V, `Identify`) | 1,879 registros antes de 1868; 5,201,602 de 1868 a 2026 (§3) | P1556 autor; DOI; MSC → P3285 |
| **arXiv** | Preprints `math.*` | «Metadata for arXiv articles may be reused in non-commercial and commercial systems» (V, info.arxiv.org/help/oa) | Cada artículo tiene la licencia que eligió su autor (S) | OAI-PMH `oaipmh.arxiv.org/oai`, conjuntos `math:math:AC` … (V); API 1 petición/3 s (V en `fuentes-candidatas.md`) | Desde 1992.  Artículos `math`: 1,064 en 1995, 18,827 en 2010, 46,225 en 2025, más 9,532 cruzados (V, arxiv.org/year/math) | P818; DOI |
| **Semantic Scholar** | — | Descartado en `fuentes-candidatas.md` §5 (licencia de API no sublicenciable) | — | — | — | — |
| **Mathematics Genealogy Project** | Directores y alumnos de tesis | Sin licencia publicada; la FAQ no trata reutilización ni descarga (V, por ausencia) | — | Solo web | 342,527 registros al 19 oct 2026 (V, portada) | P549 en 263,334 ítems; 3,163 personas del registro |
| **MacTutor** | Biografías, cronologías, historia | CC BY-SA 4.0 para el texto; no es dueño de las imágenes ni de muchas páginas `/Extras/` (V) | Igual | HTML sin API (NV) | Antigüedad a hoy, centrado en personas | P1563; 1,338 personas del registro |
| **CDLI** | Tablillas cuneiformes: catálogo, transliteraciones ATF | El texto de las páginas «may be freely copied, aggregated and re-used according to common and fair academic practice», con mención de CDLI (V, términos).  Imágenes: solo uso no comercial (S) | Ídem | JSON en `cdli.earth/search` (V).  El dump `cdli-gh/data` no tiene archivo de licencia y su README dice «Last update was August 2022» (V) | 2,274 resultados con género *Mathematical* (V).  Plimpton 322 = P254790, «Old Babylonian (ca. 1900-1600 BC)» (V) | P2474 en 1,422 ítems; solo 2 tienen P921 matemático (Plimpton 322, YBC 7289) (V) |
| **Perseus / First1KGreek** | Textos griegos (TEI) | — | CC BY-SA 4.0 (V, licencia en GitHub de `canonical-greekLit`, `canonical-latinLit` y `First1KGreek`) | git | Euclides `tlg1799` en ambos; Arquímedes `tlg0552`, Herón `tlg0559` y Ptolomeo `tlg0363` en First1KGreek (V, `__cts__.xml`) | CTS URN; P7041 (Perseus author ID) |
| **Wikisource** | Textos de dominio público y traducciones | — | Aportes y traducciones propias en CC BY-SA 4.0; la obra fuente debe ser de dominio público o compatible (V) | MediaWiki API, dumps | Depende del idioma | sitelinks del QID; P1957 |
| **Project Gutenberg** | Textos de dominio público | Catálogo «granted to the public domain» (V) | Dominio público, con condiciones de marca para redistribuir (NV en detalle) | «intended for human users only»; cosecha masiva solo por espejo o con espera de 2 s (V) | Sobre todo ediciones de los siglos XIX–XX | P2034 |
| **Internet Archive** | Escaneos | NV | Por ítem | `advancedsearch` (V) | 206,709 textos con `subject:mathematics`; 891 fechados hasta 1800 (V).  Etiquetado irregular | P724 |
| **HathiTrust** | Escaneos y registros | NV: la página de términos dio 403 | Por volumen | API bibliográfica responde (V, `/api/volumes/brief/oclc/…`) | NV | P1844; OCLC |
| **Gallica (BnF)** | Escaneos (Fermat, Lagrange, Cauchy…) | NV: la página de condiciones dio 403 | NV (S: libre no comercial; comercial con licencia) | SRU/OAI (S) | NV | P4258 |
| **Euler Archive** | Obras de Euler E1–E866, escaneos y traducciones | NV | Traducción del índice de Eneström bajo GFDL (S) | Web (S: alojado por la MAA y la University of the Pacific) | 1726–1783 | P9978 (Eneström) en solo 4 ítems; Euler (Q7604) tiene 11 obras con P50 en Wikidata (V) |
| **Gauss y Riemann (colecciones)** | Werke, Nachlass | NV | NV | NV | NV | NV |
| **DBLP** | Informática teórica | NV | — | — | Fuera de alcance salvo informática teórica | P2456 |

## 2. Cobertura por periodo

### 2.1 Cifras medidas hoy

**zbMATH Open**, campo `py` de la API REST (V):

| Periodo | Documentos |
|---|---:|
| 1800–1867 | 1,879 |
| 1868–1899 | 56,307 |
| 1900–1949 | 238,580 |
| 1950–1999 | 1,769,201 |
| 2000–2026 | 3,137,514 |
| **1868–2026** | **5,201,602** |

Los registros del Jahrbuch salen con `database: "ERAM"`, DOI, EuDML y una copia en Zenodo cuando existen.  En la muestra no tienen MSC (Kirchhoff 1850) o lo tienen reclasificado a MSC2020 (Fuchs 1866, `34A30`) (V).  El `earliestDatestamp` del OAI-PMH es 1755 y algunos registros llevan `datestamp: 0001-01-01`: es la fecha del registro, no de la obra.

**OpenAlex**, campo 26 Mathematics, `primary_topic` (V):

| Periodo | Works |
|---|---:|
| antes de 1700 | 154 |
| 1700–1799 | 294 |
| 1800–1849 | 4,853 |
| 1850–1899 | 20,901 |
| 1900–1949 | 86,119 |
| 1950–1999 | 1,140,903 |
| 2000–2026 | 3,350,978 |

Por subcampo: Applied Mathematics 1,142,936; Statistics and Probability 948,747; Geometry and Topology 746,682; Mathematical Physics 683,537; Modeling and Simulation 287,089; Algebra and Number Theory 255,067; Discrete Mathematics and Combinatorics 209,219; Numerical Analysis 176,585; Theoretical Computer Science 161,873; Computational Mathematics 14,044.  Con resumen: 2,647,274.  Con referencias: 2,338,383.  Las tres obras más citadas del campo son *Random Forests*, el FDR de Benjamini–Hochberg y el análisis de potencia de Cohen.  El campo «Mathematics» de OpenAlex mezcla estadística aplicada y aprendizaje automático, así que no sirve como filtro de matemática pura sin cortar por subcampo.

**Wikidata, obras con P50 = alguna de las 4,623 personas del registro**, por P577 (grafo principal, V):

| Periodo (año de P577) | Obras |
|---|---:|
| antes de la era común | 31 |
| años 0–999 | 42 |
| 1000–1399 | 157 |
| 1400–1499 | 107 |
| 1500–1599 | 119 |
| 1600–1699 | 199 |
| 1700–1799 | 739 |
| 1800–1899 | 2,174 |
| 1900–1999 | 3,505 |
| 2000–2026 | 1,620 |
| sin P577 | 6,093 |
| **total** | **14,786** |

De ellas, 1,849 tienen 3 o más sitelinks y 8,939 al menos uno; 2,486 tienen al menos uno y fecha anterior a 1900.  Solo 53 tienen alguna cita P2860.  Las obras se cuentan por la fecha de publicación de Wikidata, que en obras antiguas suele ser la composición estimada y en ediciones es la de impresión.  1,112 de las 4,623 personas tienen al menos una obra.

Con P921 = matemáticas (Q395) en el grafo principal: 3,608 ítems, de los que 2,340 no tienen fecha.  Es un filtro pobre: las obras suelen apuntar a un tema más específico.

### 2.2 Lectura por periodo

| Periodo | Qué cubre de verdad | Fuente buena | Hueco |
|---|---|---|---|
| Mesopotamia y Egipto (c. 2000–1500 a. C.) | 2,274 textos matemáticos en CDLI; en Wikidata, Rhind, Moscú, Plimpton 322, YBC 7289 y pocos más | CDLI para el corpus, Wikidata para lo canónico | Solo 2 tablillas matemáticas enlazan CDLI con Wikidata; el resto no tiene QID |
| Grecia y Alejandría | Euclides, Arquímedes, Apolonio, Herón y Ptolomeo con QID y texto CC BY-SA en Perseus/First1KGreek | Wikidata + Perseus | Diofanto: no encontré su `Arithmetica` en Wikidata con la búsqueda simple ni su grupo TLG (NV) |
| China, India, mundo islámico | *Nueve capítulos* (Q657057), *Āryabhaṭīya* (Q3518429), *Brāhmasphuṭasiddhānta* (Q1290001), *Shulba Sutras* (Q84052) | Wikidata | El ítem que encontré para el álgebra de al-Juarismi (Q135901116) es la traducción de Rosen de 1830 («version, edition or translation»), no la obra.  Sin medir: proporción por región o idioma (§7) |
| Edad Media europea y Renacimiento | *Liber Abaci* (Q2044496); ediciones impresas en OpenAlex desde 1482 (Euclides con Adelardo) | Wikidata, OpenAlex como ediciones | OpenAlex fecha la edición, no la obra |
| 1600–1867 | Descartes, Newton, Leibniz, Euler, Gauss, Riemann: obras mayores con QID | Wikidata; Euler Archive para Euler | Euler tiene 11 obras con P50 de unas 866 (E1–E866) |
| 1868–1949 | Jahrbuch en zbMATH: 294,887 registros | zbMATH + DOIs retroactivos | Citas escasas: muchos artículos antiguos no tienen referencias depositadas |
| 1950–hoy | 4.9 millones en zbMATH; 4.5 millones en OpenAlex; arXiv desde 1992 | zbMATH, OpenAlex, OpenCitations | Escala: no cabe completo como capa de principiante |

## 3. Cómo se identifica una obra antigua

Hay tres cosas distintas que las fuentes confunden:

1. **La obra** (*Elementos*, Q172891): un texto abstracto con autor atribuido y fecha de composición estimada.
2. **El testimonio físico** (la tablilla Plimpton 322, el papiro Rhind, un manuscrito): un objeto con número de museo o de CDLI y fecha de copia.  El papiro Rhind lo copió el escriba Ahmes de un original anterior; Wikidata pone a Ahmes como P50 y la fecha −1600 con calificador «presumiblemente» (V).
3. **La edición o traducción** (Euclides de 1482, Disquisitiones de 1801, 1966, 1986).  OpenAlex tiene *Disquisitiones* como tres works distintos, con 465, 233 y 271 citas (V): las citas de una obra clásica se reparten entre ediciones.  zbMATH tiene 87 documentos con autor «Euclid», todos ediciones o traducciones modernas (V).

Las fechas tienen precisión variable y a veces falsa:

- Wikidata guarda una precisión (7 = siglo, 8 = década, 9 = año) y calificadores: *Elementos* −300 con precisión de siglo y «circa» (P1480 = Q5727902); *Nueve capítulos* −200 «presumiblemente» y otra declaración con «fecha más tardía» 179 (P1326) (V).
- **Plimpton 322 tiene en Wikidata −1799 con precisión de año y sin calificador**, mientras CDLI dice «ca. 1900-1600 BC» (V).  Mostrar «1799 a. C.» sería falsa precisión.
- OpenAlex devuelve un artículo persa con `publication_year` 1403, que parece un año del calendario solar hijri leído como gregoriano (V, muestra de obras «antes de 1700»).
- Convención de años negativos: Wikidata usa numeración histórica (−300 = 300 a. C., sin año cero) (S, documentación de Wikidata, no releída hoy) mientras ISO 8601/EDTF usa numeración astronómica (−0299 = 300 a. C.).  Un adaptador que copie el número tal cual se equivoca en un año en todas las fechas antes de Cristo.  Necesita una prueba.
- `datetime.fromisoformat`, que usa `_check_iso` en `atlas/model.py`, no acepta años ≤ 0.  Ninguna fecha antigua puede pasar por los campos validados de hoy.  `Claim.historical_date` no se valida, es texto libre y nadie lo usa.

## 4. Escala y corte para la ronda 5

### 4.1 Cuántas obras hay

- Literatura indexada: unos **5.2 millones** de documentos en zbMATH (1868–2026) y **4.6 millones** en OpenAlex (campo 26, con estadística y aplicadas).  Se solapan mucho; la unión no está medida (NV).  arXiv añade unos 46 mil artículos `math` al año.
- Antes de 1868: miles, no millones.  zbMATH tiene 1,879; OpenAlex, 5,301 ediciones hasta 1849; Wikidata, 3,568 obras de las personas del registro fechadas antes de 1800.  CDLI suma 2,274 textos cuneiformes.  No hay un catálogo único y ninguna de estas cifras es «todas las obras».
- Orden de magnitud honesto: entre 5 y 6 millones de documentos matemáticos indexados, más un número desconocido de obras premodernas sin catalogar en estas fuentes.

### 4.2 Corte propuesto

| Capa | Criterio explícito | Fuente | Tamaño estimado |
|---|---|---|---:|
| **A. Canon con QID** | Obras de Wikidata con P50 = una persona del registro, ≥ 3 sitelinks y tema matemático (P921 dentro de los conceptos o áreas del registro, o el tipo de obra), más una lista curada a mano de testimonios anónimos (tablillas, papiros, *Shulba Sutras*, manuscrito Bakhshali…) | Wikidata (CC0) | Hasta 1,849 antes del filtro temático (medido); estimo 800–1,500 después (NV).  Testimonios curados: ≤ 100 |
| **B. Literatura más citada** | Top 25 por `cited_by_count` en cada subcampo de OpenAlex (10) y década (1870–2020, 16): 4,000 como máximo.  Se excluyen o se marcan los subcampos de Statistics y Modeling si dominan | OpenAlex (CC0), MSC desde zbMATH por DOI (BY-SA 4.0) | ≤ 4,000 |
| **C. Citas dentro del corte** | Solo `cites` entre obras de A ∪ B, desde `referenced_works` de OpenAlex y OpenCitations | CC0 | NV; decenas de miles como mucho |
| **D. Autores nuevos** | Autores de B que no están entre las 4,623 personas, resueltos por OpenAlex ID → P10283 → QID.  Sin QID: candidato en `resolve.py`, nunca fusión | OpenAlex, Wikidata | ≤ ~10,000 (NV) |

Total de la ronda: unas **6,000 obras**.  Fuera de la ronda: arXiv completo, zbMATH completo y CDLI más allá de lo curado.

**Regla de cobertura honesta.**  Cada adaptador registra un manifiesto de cobertura: fuente, versión, fecha, criterio de corte y, por celda (periodo × área), cuántas obras ingirió y cuántas indexa la fuente en esa celda (las cifras de §2.1).  La interfaz dice «el atlas tiene 25 de 238,580 obras de 1900–1949 indexadas en zbMATH (corte: más citadas); el resto es desconocido», nunca «no hay más».  Una persona sin obras ingeridas muestra «obras: no ingeridas», no «sin obras».  El conteo de citas se muestra como «citas en OpenAlex a esta edición», no como importancia.

## 5. Modelo de datos

### 5.1 Lo que ya encaja sin cambiar el contrato

| Necesidad | Hoy en `atlas/model.py` | Uso |
|---|---|---|
| Obra | `EntityType.PUBLICATION` (en `DOCS`) | *Elementos*, *Liber Abaci*, un artículo de Crelle |
| Autoría | `authored_by`, sujeto `DOCS | MATH`, objeto `PERSON`, proyección history | obra → persona |
| Cita | `cites`, `DOCS → DOCS`, proyección biblio: «bibliographic reach only; implies neither dependency nor agreement» | obra → obra |
| Un concepto o resultado está en una obra | `appears_in`, `NON_PERSON → DOCS`: «location in literature; not a citation of correctness» | concepto → obra, con `locator` = «Elementos I.47» |
| Una obra contiene una demostración | `proves` exige sujeto `PROOF` o `ALGORITHM`: la obra no demuestra nada.  Se modela como en el fixture: `p:euclid-i47` `proves` el enunciado y `p:euclid-i47` `appears_in` *Elementos* | ya existe en `tests/fixtures/mini_atlas.json` |
| Atribuciones en disputa | `alternative_group` (mismo sujeto y relación), estado `disputed`, `disagreements` | Newton frente a Leibniz; autores del *Nueve capítulos* |
| Identificadores | `external_ids`: `wikidata`, `doi`, `openalex`, `zbmath`, `cdli`, `arxiv`, `mgp`, `tlg` | — |

El principio 4 se cumple con estas relaciones si el adaptador no promueve nada: `cites` nunca se convierte en `prerequisite_for` ni en `depends_on_logically`; `appears_in` dice «está en», no «se introdujo en»; el P921 de Wikidata («tema principal») entra como `appears_in` con su procedencia y nunca como `proves`.  La regla 4 de `docs/SPEC.md` §3 ya impide que `cites`, `appears_in` y `authored_by` produzcan conclusiones matemáticas.

### 5.2 Lo que tiene que decidir el arquitecto

1. **Fechas históricas con incertidumbre.**  Hoy `Entity` no tiene fecha y `Claim.historical_date` es texto libre sin validar.  Propuesta: un campo de entidad `dates: tuple[HistoricalDate, ...]`, cada uno con `kind` (`composed`, `copied`, `published`, `edition`), un valor EDTF (ISO 8601-2: `-0299~` circa, `-1899/-1599` intervalo, `1680?` dudoso), la precisión y su procedencia.  Requiere un validador propio, porque `datetime` no acepta años ≤ 0, y la conversión de numeración histórica a astronómica con prueba.  Las fechas de personas (P569/P570, ya disponibles para 4,257) usarían lo mismo.  Esquema v4.
2. **Obra, testimonio y edición.**  Opción mínima (la que recomiendo para la ronda 5): solo obras como `publication`; ediciones y traducciones como `external_ids` y alias de la obra, y el paso de las citas de una edición a su obra queda registrado como decisión reversible, igual que `merges`.  Opción completa: un tipo o subtipo para el testimonio físico (tablilla, papiro) y una relación `edition_of` / `witness_of` en la proyección biblio, no encadenable.  La opción mínima basta mientras no se ingiera CDLI más allá de una lista curada.
3. **«Introduce el concepto X».**  No existe y no debe inferirse de `appears_in` ni de la fecha más antigua.  Si se agrega, propongo `introduced_in` (concepto → obra, proyección history, no transitiva, no encadenable con relaciones matemáticas), con al menos una fuente historiográfica secundaria (MacTutor, P61/P575 de Wikidata con referencia, un libro de historia) y alternativas para las prioridades en disputa.  Sin esa fuente, la afirmación no entra.
4. **Escriba frente a autor.**  `authored_by` sirve para el autor atribuido.  Para el papiro Rhind, Ahmes es copista.  O se agrega un rol o relación de copista, o la ronda 5 no ingiere P50 de testimonios físicos y lo cura a mano.
5. **Licencia por campo.**  zbMATH es BY-SA 4.0 con reseñas de terceros excluidas; OpenAlex y Crossref son CC0, pero los resúmenes no están claros.  Propuesta: en la ronda 5 no se guarda ningún resumen, solo título, identificadores y enlace.  `Provenance.license` por afirmación ya lo permite.

## 6. Recomendación priorizada para la ronda 5

1. **Capa A desde Wikidata.**  Coste bajo: el adaptador existe, es CC0, las personas ya están y sus QID son la llave.  Ingiere `publication`, `authored_by` (obra → persona), `appears_in` desde P921 y fechas con precisión y calificadores.  Requiere antes la decisión 1 (fechas).  Si no llega, la ronda guarda la fecha cruda de Wikidata en la procedencia (`quote`) y no la muestra.  Recordar que el grafo académico de Wikidata está en otro endpoint: las consultas de hoy del adaptador no ven ningún artículo científico.
2. **Lista curada de testimonios antiguos y no europeos**, con CDLI (P254790 para Plimpton 322), Wikidata y MacTutor como fuente de las fechas e intervalos.  Pocas decenas de ítems escritos a mano como `model_proposed` y revisados.  Es lo que corrige el sesgo del corte automático.
3. **Capa B desde OpenAlex**, con DOI como llave, MSC desde zbMATH (BY-SA 4.0) y `cites` solo dentro del corte.  Unas 160 consultas paginadas para la selección, dentro del presupuesto diario sin llave, aunque una llave gratuita da margen (US$1/día).
4. **Puentes de personas**: guardar P549, P1556 y P569/P570 en las 4,623 personas.  Es barato y prepara MGP y zbMATH sin ingerirlos.
5. **Textos de dominio público como enlaces**: Perseus/First1KGreek (CC BY-SA 4.0) y Wikisource para localizadores precisos («Elementos I.47» con CTS URN).  No se copia el texto en la ronda 5.

Segunda ola: arXiv por OAI-PMH (metadatos), zbMATH completo como índice de búsqueda y no como capa del mapa, CDLI completo, Euler Archive.  MGP queda bloqueado mientras no haya licencia o permiso escrito.

## 7. Riesgos

| Riesgo | Ejemplo verificado | Mitigación |
|---|---|---|
| Falsa precisión de fechas | Plimpton 322: −1799 con precisión de año en Wikidata frente a «ca. 1900-1600 BC» en CDLI | Intervalos EDTF; se muestra la fecha menos precisa entre las fuentes, o ambas |
| Error de un año antes de Cristo | Numeración histórica (Wikidata) frente a astronómica (ISO) | Conversión centralizada con prueba |
| Fechas de edición tomadas por fechas de obra | OpenAlex: Euclides «1482»; *Disquisitiones* partido en tres works | `kind` en la fecha; las ediciones colapsan en la obra por decisión registrada |
| Calendarios no gregorianos | Artículo persa con año 1403 en OpenAlex | Filtro de plausibilidad (año de obra frente a fecha de vida del autor, ya disponible para 4,257 personas) |
| Atribuciones disputadas | *Nueve capítulos* con varios autores; Rhind con escriba como P50; prioridad del cálculo | `alternative_group`, `disputed`, nunca una sola atribución sin fuente |
| Homónimos de autores | «Euclid» en zbMATH (87 ediciones); autores sin QID en OpenAlex | Resolución por QID o ID externo; sin ID, candidato en `resolve.py` y nunca fusión.  Mismo patrón de error que `name_doc` en `diagnostico-calidad.md` §2.4 |
| Sesgo eurocéntrico y de idioma | Obras con fecha de las personas del registro: 73 antes del año 1000 frente a 2,174 solo en el siglo XIX; solo 2 tablillas matemáticas con QID enlazado a CDLI | Lista curada (§6.2) y manifiesto de cobertura por región, con «desconocido» explícito |
| Sesgo de citas | Las más citadas del campo 26 de OpenAlex son de estadística y aprendizaje automático; los clásicos acumulan pocas citas por DOI | Corte por subcampo y década, nunca top global; el conteo se rotula como dato de OpenAlex |
| Licencias de resúmenes y reseñas | Crossref advierte copyright en resúmenes; zbMATH vacía reseñas de terceros | No guardar resúmenes en la ronda 5 |
| Fuentes sin licencia | MGP no publica licencia; el dump de CDLI no trae archivo de licencia y está congelado desde agosto de 2022 | MGP bloqueado; CDLI solo por la API y con atribución, según sus términos |
| Cita leída como dependencia | `cites` es la relación más numerosa del registro (1,176,414) | La proyección biblio ya no autoriza nada; el adaptador nunca promueve |

## 8. Lo que no pude verificar

- La licencia CC0 de OpenCitations en la página de hoy (la página de descargas no la menciona) y la unión zbMATH ∪ OpenAlex.
- La licencia de los resúmenes de OpenAlex (índice invertido).
- Los términos de HathiTrust y de Gallica (403 en las dos páginas).
- Las condiciones de reutilización de los escaneos y traducciones del Euler Archive; la licencia GFDL de la traducción del índice de Eneström es de fuente secundaria.
- Las colecciones de Gauss (Werke en el GDZ de Göttingen) y Riemann (transcripciones de D. R. Wilkins): no consulté ninguna fuente primaria.
- Si zbMATH permite ordenar por número de citas en la API; para la capa B uso OpenAlex.
- El ítem de Wikidata de la obra de al-Juarismi (solo encontré la traducción de 1830), el de la *Arithmetica* de Diofanto y el grupo TLG de Diofanto.  Q534087 (*Introductio in analysin infinitorum*) apareció en la búsqueda pero no devolvió datos en WDQS; puede ser una redirección.
- La proporción de obras por región o idioma (P495, P407) dentro del corte A: no la medí.
- El tamaño de la capa A después del filtro temático y el número de citas dentro del corte (capa C).
- La convención de años negativos de Wikidata: la tomo de su documentación sin releerla hoy; necesita una prueba con *Elementos* (−300 en Wikidata).
- DBLP: no lo consulté; queda fuera salvo informática teórica.
