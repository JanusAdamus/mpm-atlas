# Ronda 5 · Fuentes candidatas

Fecha de consulta: 2026-10-19.  Autor: investigación de escritorio para AJ.  Método: páginas oficiales de licencia y documentación, la API de GitHub para ver los repositorios y unas pocas peticiones pequeñas de prueba (WDQS, OpenAlex, zbMATH Open, siteinfo de MediaWiki).  No se descargó ningún dump ni se instaló nada.

Objetivos de la ronda: (a) conexiones de calidad entre conceptos (prerrequisitos, dependencias, generalizaciones, aplicaciones); (b) explicaciones con sustento para público general en ES y EN; (c) mejores nombres y traducciones.

Convenciones:
- **V**: verificado hoy en la fuente primaria (se cita la URL).  **S**: solo en fuentes secundarias.  **NV**: no verificado.
- Esfuerzo del adaptador: **S** ≤1 día, **M** 2–4 días, **L** >1 semana.  Siempre con las reglas del proyecto: caché en `data/cache/` (reusar `atlas/ingest/http.py`, `CachedClient`), modo offline con fixtures, límite de tasa, `User-Agent` identificable y procedencia por afirmación.  Para fuentes en git, commit fijo, como ya se hace con Mathlib.
- "Ya en el registro" se refiere a `research/2026-10-16/bibliografia-y-evidencia.md` (R01–R60).

## 1. Lo que ya existe y cómo se reutiliza

El adaptador de Wikidata (`atlas/ingest/wikidata.py`, `EXTERNAL_IDS`) ya lee los identificadores externos que sirven de puente hacia casi todas las fuentes de abajo: MathWorld P2812, nLab P4215, ProofWiki P6781, EoM P7554, PlanetMath P7726, Metamath P12888, MSC P3285, OEIS P829, MacTutor P1563 y Mathlib P14534.  El adaptador de Mathlib ya lee `docs/100.yaml` y `docs/1000.yaml`, pero no `docs/undergrad.yaml` ni `docs/overview.yaml`.

Ítems de Wikidata con cada identificador, contados hoy con WDQS (`SELECT (COUNT(?item) …) WHERE { ?item wdt:Pxxx [] }`, 2026-10-19):

| Propiedad | Fuente | Ítems |
|---|---|---|
| P2812 | MathWorld | 6,840 |
| P4215 | nLab | 4,348 |
| P1563 | MacTutor (sobre todo personas) | 3,089 |
| P6781 | ProofWiki (incluye páginas `Mathematician:`) | 2,562 |
| P7554 | Encyclopedia of Mathematics | 1,224 |
| P7726 | PlanetMath | 950 |
| P829 | OEIS | 469 |
| P12888 | Metamath | 438 |
| P3285 | MSC | 408 |
| P14534 | Mathlib | 115 |

Estas cifras son cobertura del puente, no cobertura de la fuente.  Una página sin enlace en Wikidata se tiene que resolver por nombre, y eso pasa por `atlas/resolve.py` como candidato, no como fusión.

## 2. Tabla comparativa

| Fuente | Aporta | Licencia (página) | Reutilizar y redistribuir | Acceso | Idiomas | Cobertura (fuente, fecha) | Mapeo | Esfuerzo |
|---|---|---|---|---|---|---|---|---|
| **Mathlib `docs/undergrad.yaml` y `docs/overview.yaml`** | a (pertenencia a área), c (nombre usual EN → declaración Lean) | Apache-2.0 (V, API de GitHub de `leanprover-community/mathlib4`) | Sí | Ya está en el commit fijo que se ingiere; no hay red | EN | Temario del concurso francés de agregación (comentario en el archivo, V) | Nombres de Lean directos | **S** |
| **Wikidata: etiquetas, alias y lexemas** | c | CC0 para los espacios principal, Property y Lexeme (V, [Wikidata:Copyright](https://www.wikidata.org/wiki/Wikidata:Copyright)) | Sí, sin condiciones | WDQS o dumps; ya hay adaptador | ES, EN y muchos más | De 6,840 ítems con P2812, solo 496 tienen un sentido léxico en inglés y 133 en español enlazado por P5137 (V, WDQS, 2026-10-19) | QID directo | **S** (extensión del adaptador) |
| **Wiktionary** (vía extracción tipo wiktextract/kaikki) | c (traducciones y sinónimos) | CC BY-SA 4.0 + GFDL (V, [en.wiktionary Copyrights](https://en.wiktionary.org/wiki/Wiktionary:Copyrights); la página se marca como desactualizada) | Sí, con atribución y compartir igual | dumps.wikimedia.org; kaikki.org responde, pero no confirmé su licencia ni su formato (NV) | Multilingüe | NV | Por cadena + sentido etiquetado "mathematics"; sin QID | **M** |
| **PlanetMath** | a (`\pmrelated`, `\pmdefines`), b (EN), c (`\pmsynonym`, `\pmtitle`) | CC BY-SA 2.5 (V, [planetmath.org/about](https://planetmath.org/about)) | Sí, con atribución y compartir igual (se puede adaptar bajo una versión posterior de BY-SA) | 64 repositorios en GitHub, uno por sección MSC (V, API de GitHub); los `.tex` llevan los metadatos (V, `03-00-Bijection.tex`) | EN | El último push fue el 2024-02-19 y los archivos revisados datan de 2013 (V); está prácticamente congelado | P7726 (950 ítems) + `\pmcanonicalname` + MSC en `\pmclassification` | **M** |
| **Learning Commons Knowledge Graph** (Math Coherence Map + Learning Components) | a (progresiones pedagógicas K–12) | Progresiones: CC0, de Student Achievement Partners.  Componentes de matemáticas: CC BY 4.0, de Achievement Network.  Estándares estatales: "varía por estado".  Código: MIT (V, [licencia](https://docs.learningcommons.org/knowledge-graph/resources/license.md)) | Sí para progresiones y componentes; los estándares se revisan fila por fila | `nodes.jsonl` y `relationships.jsonl` versionados (v1.11.0) en cdn.learningcommons.org (S, docs); la API está en beta privada (S) | EN | K–12; dataset actualizado el 2025-09-23 (V, [ficha](https://docs.learningcommons.org/knowledge-graph/datasets/math-coherence-map.md)); número de aristas NV | Los nodos son estándares (3.OA.A.1), no conceptos: requiere una capa de mapeo | **L** |
| **ProofWiki** | a (dependencia lógica: cada demostración enlaza definiciones y teoremas), c | CC BY-SA 3.0 salvo páginas de discusión y de usuario (V, [ProofWiki:Copyrights](https://proofwiki.org/wiki/ProofWiki:Copyrights)) | Sí, con atribución y compartir igual | MediaWiki; `api.php` respondió **403 con reto de Cloudflare** (V, prueba de hoy).  No encontré un dump público (NV) | EN | P6781: 2,562 ítems (V, WDQS) | P6781 + título | **M** si hay dump; bloqueado sin él |
| **nLab** | a (secciones "Related concepts", enlaces densos), b (nivel investigación, no público general) | Sin licencia formal: "free and encouraged if you acknowledge the source" y "no consensus on a more formal license statement" (V, [HomePage](https://ncatlab.org/nlab/show/HomePage)).  Parte de las aportaciones es CC BY-SA 3.0 (S, nForum) | Uso con atribución sí; redistribuir texto es ambiguo | Espejo en git `ncatlab/nlab-content`, Markdown + itex, ≈1.08 GB, actualizado hoy, sin archivo de licencia (V, API de GitHub) | EN | P4215: 4,348 ítems (V) | P4215 + nombre de página | **M** |
| **Encyclopedia of Mathematics** | b (definiciones de referencia), a (enlaces) | Texto nuevo: CC BY-SA 3.0 + GFDL.  Artículos originales de Springer: siguen con copyright de Springer (S; la página de copyrights dio 502) | Solo el texto nuevo; hay que separarlo artículo por artículo | MediaWiki; `api.php` dio **502** hoy (V) | EN | P7554: 1,224 ítems (V) | P7554 | **M** |
| **MacTutor** | b (historia, EN), a (influencia histórica) | CC BY-SA 4.0 para el texto; extras e imágenes aparte (V, [copyright](https://mathshistory.st-andrews.ac.uk/Miscellaneous/copyright/)) | Sí, con atribución y compartir igual | HTML; sin API (NV) | EN | P1563: 3,089 ítems, casi todos personas (V) | P1563 | **M** |
| **Wikibooks ES** | b (ES, público general) | CC BY-SA 4.0 (V, siteinfo `rightsinfo` de es.wikibooks) | Sí | MediaWiki API y dumps de Wikimedia | ES | 9,251 artículos de todas las materias (V, siteinfo 2026-10-19); la parte de matemáticas NV | Por título/sitelink de Wikidata | **S–M** (se reusa el adaptador de Wikipedia) |
| **zbMATH Open** | a (MSC con "See also" entre clases), c (títulos MSC EN) | Contenido propio (reseñas, clasificaciones, software, autores): CC BY-SA 4.0 (S, [Zenodo](https://zenodo.org/records/12784173), autores de FIZ).  La API sigue sus propios términos (NV, la página solo muestra "I agree…") | Sí para lo propio; los resúmenes de terceros vienen vacíos ("contents unavailable due to conflicting licenses", V en la respuesta de la API) | REST `api.zbmath.org/v1` sin llave (V: `/document/_search`, `/classification/11F06`); OAI-PMH en oai.zbmath.org (S) | EN | 2,813,563 documentos con DOI en el dataset de 2024 (V, Zenodo) | Código MSC → P3285 | **S–M** |
| **Metamath set.mm** | a (dependencia lógica formal, segunda biblioteca) | `*.mm` en CC0 (V, [copyright](https://us.metamath.org/copyright.html)) | Sí | Repo `metamath/set.mm` (V); un solo archivo de texto | EN (etiquetas) | P12888: 438 ítems (V) | P12888 | **M** |
| **BOE: currículo LOMLOE** | a (prerrequisitos pedagógicos ES), c (terminología escolar ES) | Reutilización comercial y no comercial con cita y enlace, sin desnaturalizar ni sugerir carácter oficial (V, [aviso legal](https://www.boe.es/informacion/aviso_legal/index.php)) | Sí | XML por disposición; RD 217/2022 ESO, BOE-A-2022-4975 (V) | ES | ESO (V); Primaria y Bachillerato NV | Ninguno: curación manual | **L** |
| **SEP México: programas sintéticos** | a (progresión escolar MX), c (terminología MX) | Acuerdo 08/08/23, DOF 15/08/2023 (S).  La LFDA, art. 14 fr. VIII, excluye los textos administrativos (S; no pude abrir el DOF por un error de certificado) | Probablemente sí, apegado al texto oficial (S) | Solo PDF | ES | Fases 2–6 (S) | Ninguno: curación manual | **L** |

## 3. Recomendación para la ronda 5 (en orden)

1. **Mathlib `undergrad.yaml` y `overview.yaml`.**  Cuesta casi nada: ya está en el commit fijo, es Apache-2.0, no usa red y las pruebas offline salen gratis.  Da a cada concepto conocido su nombre usual en inglés junto con la declaración de Lean, y agrupa los conceptos por área del temario.  Hay que tiparlo como `part_of_area` y como alias de nombre, nunca como prerrequisito.  Un detalle: `parse_links_yaml` solo lee un nivel y estos archivos anidan tres, así que hace falta un lector un poco más general (la biblioteca estándar no trae YAML).
2. **Wikidata (etiquetas y alias ES/EN y lexemas) más Wiktionary para traducciones.**  Ataca directo el objetivo (c).  Wikidata es CC0 y el adaptador ya existe.  La medición de hoy dice cuánto falta: de 6,840 ítems con P2812, solo 133 tienen un lexema en español enlazado, así que los lexemas solos no alcanzan.  Wiktionary (BY-SA 4.0) completa las traducciones.  Cada traducción entra como `model_proposed` o `extracted` con su fuente, y la elección del nombre preferido queda registrada como decisión, no como sobrescritura.
3. **PlanetMath.**  Es la fuente estructurada más barata para (a), (b) y (c) al mismo tiempo.  Trae tipo de entrada (Definition, Theorem…), sinónimos, términos que define, relacionados y MSC, todo en metadatos LaTeX.  Es BY-SA y está en git, igual que Mathlib.  La cautela es de tipado: `\pmrelated` es "relacionado", no prerrequisito ni dependencia, y debe entrar como relación sin dirección lógica.  Además está congelada desde ~2013, así que no sirve para nada reciente.
4. **Learning Commons, Math Coherence Map.**  Es la única fuente abierta encontrada con progresiones pedagógicas explícitas (`buildsTowards`, `relatesTo`), con licencia CC0 y descarga versionada.  Responde a "qué debo saber antes" en el nivel escolar.  Dos límites: la propia documentación advierte que las secuencias "do not name definitive pre-reqs", y los nodos son estándares, no conceptos.  Por eso propongo un piloto acotado: mapear a mano unas 50 conexiones de aritmética, fracciones y álgebra inicial y medir cuántas sobreviven a la revisión antes de escalar.  El piloto también probaría el tipo de relación "progresión pedagógica", distinto de prerrequisito lógico.
5. **ProofWiki, condicionado al acceso.**  Es la mejor fuente abierta de dependencias lógicas en lenguaje natural (BY-SA 3.0) y complementa las de Mathlib con demostraciones legibles.  Hoy la API devuelve un reto de Cloudflare, y un adaptador que lo esquive violaría la regla de acceso educado.  La acción para la ronda es pedir a los administradores un dump o acceso para bots.  Si no llega a tiempo, el quinto lugar pasa a **zbMATH Open** (MSC con "See also", API abierta y probada hoy), con la salvedad de licencia del punto 4.

Para (b) en español no hay una fuente nueva de calidad comparable a Wikipedia ES, que ya se ingiere.  Wikibooks ES es compatible (BY-SA 4.0) y sale casi gratis con el adaptador de Wikipedia, pero su cobertura de matemáticas no está medida.  Propongo medirla antes de comprometerla.

## 4. Segunda ola (útiles, no para esta ronda)

- **Metamath set.mm** (CC0): una segunda biblioteca formal para corroborar dependencias de fundamentos, con 438 ítems enlazados vía P12888.  Su alcance es lógica, conjuntos y algo de análisis.
- **nLab**: muy rico en conexiones de nivel investigación.  Usarlo solo para metadatos y enlaces ("la página X enlaza a Y", con atribución y versión) y no redistribuir el texto mientras no haya una licencia formal.
- **MacTutor** (BY-SA 4.0): para las rutas históricas que ya existen; casi todo trata de personas.
- **BOE LOMLOE y SEP**: prerrequisitos escolares en español.  Las licencias son favorables, pero la extracción es manual.  Conviene después del piloto de Learning Commons, reutilizando el mismo tipo de relación.
- **zbMATH Open** (si no entra en el top 5) y **OpenAlex Topics** (CC0, 10 subcampos bajo el campo 26 "Mathematics", V en `/fields/26`): sirven como señal de actividad de investigación, no como conexiones conceptuales.
- **Formales restantes**: Isabelle AFP (BSD o LGPL por entrada, S), Rocq stdlib (LGPL-2.1, V en GitHub), Agda stdlib (archivo LICENCE tipo MIT, leído parcialmente), HOL Light (aviso tipo BSD de Cambridge, leído parcialmente).  Ninguno tiene puente en Wikidata, así que mapearlos cuesta caro.
- **MaRDI / MathModDB**: la ontología es CC BY 4.0 (S, repo `MaRDI4NFDI/MathModDB`) y cubre ~200 modelos (S).  Sirve para "aplicaciones".  La licencia del grafo completo sigue sin verificar.
- **arXiv** (metadatos CC0, 1 petición cada 3 s, V en [términos](https://info.arxiv.org/help/api/tou.html)): 32 categorías `math.*` (V, [taxonomía](https://arxiv.org/category_taxonomy)).  La página de taxonomía no da equivalencias con MSC (V), en contra de lo que se suele suponer.

## 5. Descartes y motivo

| Fuente | Motivo | Evidencia |
|---|---|---|
| **MathWorld** | Prohíbe copiar, espejar o reproducir en bloque sin permiso, y usar robots para copias de archivo.  Solo se usa como enlace saliente vía P2812 | V, [términos de Wolfram](https://www.wolfram.com/legal/terms/mathworld.html) |
| **DLMF** | "Bulk copying, reproduction, or redistribution in any form is not permitted"; el copyright es de NIST | V, [avisos](https://dlmf.nist.gov/about/notices) |
| **MSC2020 (texto de los títulos)** | CC BY-NC-SA: el NC es incompatible con mezclarse con BY-SA en un derivado redistribuible.  Usar solo los códigos como identificadores (P3285) y tomar los títulos de zbMATH bajo BY-SA, verificando antes esa contradicción | V, [msc2020.org](https://msc2020.org/) |
| **Khan Academy** | CC BY-NC-SA con alcance limitado; la página de soporte respondió 403; sin API pública confirmada | S, soporte de Khan citado en buscador |
| **OpenStax** | El centro de ayuda dice hoy que todos los libros son CC BY-NC-SA, aunque libros antiguos se publicaron como CC BY.  Existe una página "An update on OpenStax licensing" que no pude leer.  Los títulos en español quedan NV | V, [ayuda de OpenStax](https://help.openstax.org/s/article/Licensing-information-of-OpenStax-textbooks) |
| **LibreTexts** | La licencia cambia página por página (BY-NC-SA, BY-NC, BY-SA), incluso dentro de un mismo libro.  La procedencia por afirmación sería frágil y abundan los remixes | S, avisos por página |
| **Stack Exchange (Math.SE, MathOverflow)** | El contenido es BY-SA, pero desde 2024 los dumps exigen iniciar sesión y aceptar términos adicionales (sin LLM, uso propio); desde el dump del 2025-06-30 traen marcas de agua y ya no se publican en archive.org.  Las preguntas y respuestas son ruidosas para extraer relaciones.  ARQMath (R25) ya cubre su uso como benchmark | S, [devclass](https://devclass.com/2024/07/30/stack-exchange-restricts-access-to-dump-of-user-contributed-data-as-critics-complain-license-permits-reuse-for-any-purpose), [Archive Team](https://wiki.archiveteam.org/index.php/Stack_Overflow) |
| **Semantic Scholar** | Licencia de API no sublicenciable; datos con licencias que varían, incluido CC BY-NC.  Aporta poco a conexiones entre conceptos | V, [licencia de API](https://www.semanticscholar.org/product/api/license) |
| **LMFDB** | Trata objetos (curvas, formas modulares), no conceptos.  El sitio redirige a una puerta anti-bots (`gate.html`) y no pude leer la licencia | V, prueba de hoy |
| **OEIS** | CC BY-SA 4.0 con repo git `oeis/oeisdata` (V), pero aporta sucesiones, no conexiones conceptuales.  Se aplaza; sirve para nombrar sucesiones famosas | V, LICENSE del repo |
| **Common Core directo** | Licencia limitada a usos que "support the Common Core State Standards Initiative" y exige el aviso "All rights reserved".  Se accede mejor vía Learning Commons, con licencia por fila | V, [licencia pública](https://www.thecorestandards.org/public-license/) |
| **Metacademy** | Repositorio archivado (2023), GPL-3.0 y centrado en aprendizaje automático | V, API de GitHub |
| **SMGloM / MathHub** | Las fuentes encontradas no listan el español; la licencia del contenido NV | S, BARTOC |
| **Lean Blueprints** | Son grafos por proyecto (FLT, PFR…) de nivel investigación, con licencias que varían.  No sirven al público general | NV en detalle |
| **OpenAlex Concepts** | Congelados y marcados como obsoletos; para usos nuevos se recomienda Topics | S, [ayuda de OpenAlex](https://help.openalex.org/data/concepts/) |

## 6. Qué cambió respecto al registro R01–R60

- **R25 (ARQMath, Math.SE)**: ahora los dumps de Stack Exchange tienen restricciones de acceso y marcas de agua (2024–2025).  Si AtlasBench usa ARQMath, conviene fijar una copia ya publicada.
- **R53 (OpenAlex)**: Concepts está obsoleto y lo reemplaza Topics (4 niveles).  La API funciona sin llave con 10 req/s; con llave gratuita sube a 30 req/s, con un presupuesto diario de US$1 y costo informado por respuesta (V, [límites](https://help.openalex.org/how-to-use-the-api/rate-limits-and-authentication)).
- **R54 (MaRDI)**: se encontró la licencia de la ontología MathModDB (CC BY 4.0, S).  La del grafo completo sigue pendiente.
- **R07 (S2ORC)**: los términos de la API de Semantic Scholar descartan su uso como fuente redistribuible.
- **R49 (Wikidata)**: se midió la cobertura léxica, que es baja (sección 2).

## 7. Riesgos transversales y reglas para los adaptadores

- **Licencias por afirmación.**  Cada afirmación nueva lleva `license` además de fuente, localizador, extractor y versión.  Un snapshot publicado lista las licencias que contiene.  No se mezcla texto NC con BY-SA en un mismo artefacto redistribuible.
- **Tipos de relación.**  `\pmrelated` (PlanetMath) y "Related concepts" (nLab) entran como relación no dirigida.  `buildsTowards` entra como progresión pedagógica.  Las dependencias de ProofWiki y Metamath son dependencia lógica.  Ninguna de estas se promueve a prerrequisito lógico sin revisión.
- **Homónimos.**  PlanetMath y Wiktionary resuelven por nombre, así que todo emparejamiento sin QID entra como candidato en `resolve.py` y nunca como fusión automática.
- **Acceso educado.**  Ningún adaptador esquiva retos anti-bots (ProofWiki, LMFDB).  Si no hay dump ni permiso, la fuente queda bloqueada en `docs/STATUS.md`.
- **Fuentes congeladas.**  PlanetMath (~2013) y la extracción de cada commit llevan su fecha en la procedencia, para que el panel muestre la antigüedad.

## 8. No verificado (pendiente)

- La licencia de kaikki.org y el formato de su extracción de Wiktionary.
- La existencia de un dump de ProofWiki y su política para bots.
- La página de copyrights de EoM (dio 502) y la separación entre artículos de Springer y texto nuevo.
- Los términos de la API de zbMATH Open, y la contradicción entre "classifications" en BY-SA (zbMATH) y MSC2020 en BY-NC-SA (msc2020.org).
- El número de nodos y aristas del Math Coherence Map y el contenido del JSONL v1.11.0.
- Los títulos de OpenStax en español y el contenido de "An update on OpenStax licensing".
- La política de reutilización de la SEP y el DOF (el certificado de dof.gob.mx no coincide con `www`); RD 157/2022 (Primaria) y RD 243/2022 (Bachillerato).
- Los términos de uso de Khan Academy sobre acceso automatizado.
- La licencia del grafo completo de MaRDI.
- Las licencias de Agda stdlib y HOL Light (solo leí el encabezado del archivo).
