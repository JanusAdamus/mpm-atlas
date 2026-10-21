# Ronda 5 · piloto de obras (punto 5 y 5b)

Fecha: 2026-10-20.  Rol 2c Obras.  Todo se escribió en `data/pilot/`; `data/atlas.db` no se tocó.  Las cifras **medidas** citan su archivo en `data/pilot/logs/`.  Las **extrapoladas** están marcadas así y salen de `data/pilot/extrapolate.py` (lineal en cada unidad).

## 1. Qué es el piloto

- Registro del piloto: `data/pilot/atlas-pilot-v2.db`, copia consistente de `data/atlas.db` hecha el 2026-10-20 con la API de respaldo de SQLite (`python data/pilot/make_pilot.py data/atlas.db data/pilot/atlas-pilot-v2.db`; `make-pilot-v2.json`).
- Muestra (ADR-0005 §8.6): la clase MSC **30 (funciones de una variable compleja) completa**, 99,401 documentos de zbMATH Open de 1868 a 2026, más **10,000 obras al azar** del campo Mathematics de OpenAlex (`sample`, semilla 20261019), más las obras de OpenAlex de 49,798 DOI de la clase 30 (para medir el solapamiento y fusionar), el canon curado y de Wikidata (1,820 obras y testimonios) y las 2,274 tablillas matemáticas de CDLI.
- Resultado: 162,546 registros de obra, 114,736 obras canónicas (tras fusionar), 53,593 personas nuevas y 684,593 afirmaciones de la capa (`cw:`), en 2,781 lotes (`v2-dates-areas.json`, `bytes-per-unit.json`).
- `data/pilot/atlas-pilot.db` es la copia vieja: sus dos lotes del canon se ingirieron con una versión anterior del código que usaba ids `wd:` para las obras, y por eso `atlas compile` del núcleo falla con «merged into unknown wdw:Q8369».  No se usa para ninguna cifra.  No la borré (la acción pedía permiso); se puede borrar o reemplazar por la v2.

## 2. La descarga fallida de zbMATH (2026-10-19)

- Síntoma: `fetch_zb30.log` vacío y la caché con las particiones 1700–2005 nada más.
- Causa: el proceso se murió hacia las 16:56 del 19 de octubre, cuando se pausó la sesión por el límite de uso.  La API respondía bien (probado el 20 con 2006 y 2010).  No fue un error de la API: `ThreadPoolExecutor.map` habría seguido con los años siguientes, y no quedó ninguno después de 2005 en ninguno de los dos hilos.  El log quedó vacío porque el script solo imprimía al final y el stderr no se guardaba.
- Arreglo: una línea de log por partición en `zbmath.fetch_partition` y el script `data/pilot/run_fetch_zb.py`, que ahora se corre con stderr capturado.  La reanudación usó la caché: se bajaron solo 2006–2026.
- Resultado: 99,401 registros; 484 peticiones; 1,762 s; 0 reintentos (`fetch_zb30.log`, `fetch_zb30.err`).  zbMATH indexa 99,401 en `cc:30*` y se ingirieron 99,401 (tabla de cobertura, §5).

## 3. Mediciones del piloto

### 3.1 Ingesta (desde la caché filtrada)

Comando: `PYTHONPATH=. python -m atlas.ingest.works --db data/pilot/atlas-pilot-v2.db --msc 30 canon cdli zb-works oa-works merges zb-cites oa-cites oc-cites cr-cites coverage`.  Los tiempos salen de `finished_at` de los lotes (`v2-ingest-throughput.json`):

| Paso | Lotes | Reloj | Filas |
|---|---|---|---|
| canon + CDLI | 25 | < 1 s | 4,091 entidades, 2,218 afirmaciones |
| zbMATH obras | 327 | 46 s | 99,401 obras (2,161/s), 354,279 afirmaciones (7,702/s) |
| OpenAlex obras | 1,050 | 33 s | 59,185 registros, 47,807 secundarios |
| fusiones | — | ≈ 2 s | 47,807 por DOI, 813 de personas, 2 CDLI, 1 de edición |
| citas zbMATH | 327 | 42 s | 305,165 internas |
| citas OpenAlex | 1,050 | 3 s | 3,698 internas nuevas |
| OpenCitations / Crossref (300 DOI) | 1 + 1 | — | 221 y 6 pares que no estaban |
| cobertura | — | 732 s | 14 celdas; el tiempo se va en los reintentos del 429 de OpenAlex |

Pico de memoria de todo el proceso: 443 MB (`v2-ingest.measure.json`).

### 3.2 Reanudación (§8.6)

Maté `zb-works` con `taskkill /T /F` a mitad de corrida (298 de 327 lotes hechos) y lo relancé dos veces (`resume-after-kill.json`, `resume-after-rerun.json`, `resume-after-rerun2.json`, `python data/pilot/check_partial.py <db>`):
- después de matarlo: 0 afirmaciones y 0 entidades de lotes inexistentes, 0 lotes sin evento;
- la 1.ª reanudación ingirió los 29 lotes que faltaban;
- la 2.ª corrida saltó los 327, insertó 0 filas y no dejó ningún evento (3,874,862 eventos antes y después).

**PASA.**  En la v2, 2,781 lotes, 2,781 eventos `batch_ingested` y 0 filas parciales.

### 3.3 Solapamiento zbMATH ∩ OpenAlex, por DOI (`overlap.json`, `python data/pilot/run_overlap.py`)

| Dirección | Muestra | Resultado |
|---|---|---|
| OpenAlex Mathematics → zbMATH | 400 DOI al azar de la muestra (semilla 20261020); el 75.76 % de la muestra tiene DOI | **36.5 % en zbMATH [31.9, 41.3]** |
| zbMATH clase 30 → OpenAlex | 49,798 DOI (los primeros 1,000 bloques en orden de DOI, hasta `10.4064/…`; ver §6) | **95.6 % en OpenAlex [95.4, 95.8]**; de esos, el 93.6 % tiene campo Mathematics |

- Las obras sin DOI no se pueden emparejar: su solapamiento es desconocido.
- 1,357 registros de OpenAlex que llegaron por la búsqueda de DOI de zbMATH quedaron como primarios sin fusionar: OpenAlex devolvió un DOI que zbMATH no tiene escrito igual.  Pueden ser duplicados, y falta revisarlos.

### 3.4 Unidades medidas

| Medida | Valor | Fuente |
|---|---|---|
| Bytes por afirmación (tabla, participantes, índices y toda la procedencia) | 1,040 B | `extrapolation.json`, desde `dbstat` (`core/atlas-core/examples/dbstat.rs`) |
| Resto (FTS, ids, fechas, fusiones, eventos) por entidad | 1,787 B | ídem |
| Crecimiento del registro | 1,098 MB por 162,546 obras, 53,593 personas y 684,593 afirmaciones | `bytes-per-unit.json` |
| Autores por obra de zbMATH | 1.85 (184,335 / 99,401) | `v2-dates-areas.json` |
| Códigos MSC por obra de zbMATH | 2.62; el 9.1 % no tiene un área en el registro (23,586 de 260,106) | ídem |
| Referencias por obra (zbMATH) | 7.58; internas al piloto 40.8 % (307,136 de 753,151) | ídem |
| Referencias por registro (OpenAlex) | 13.69; internas 26.0 % | ídem |
| Lo que añaden OpenCitations y Crossref | 221 y 6 pares de 1,066 y 1,056 internos (el resto ya venía de zbMATH) | `step-oa-merges-cites.json` |
| Estadística o aprendizaje automático (subcampos 2613 y 2611) en la muestra de OpenAlex | 2,674 de 10,000; marcados por `openalex_subfield`, no borrados | `v2-dates-areas.json` |

### 3.5 Fechas

- 0 rechazadas y 0 marcadas en los lotes.
- 311 obras sin fecha.
- Por tipo: 159,450 `published`, 1,967 `copied` (tablillas), 367 `composed` y 255 `edition`.
- La fecha más antigua es `-3199/-2999` (CDLI P006397, Uruk III, astronómico).

### 3.6 Snapshot, índice, teselas y servidor

Comandos:
- `atlas compile --db data/pilot/atlas-pilot-v2.db --out data/pilot/snapshot --json`
- `works compile data/pilot/atlas-pilot-v2.db data/pilot/snapshot data/pilot/works` (el binario `examples/works.exe`), medidos con `data/pilot/measure.ps1`
- servidor: `works serve data/pilot/snapshot data/pilot/atlas-pilot-v2.db data/pilot/works 127.0.0.1:8781`
- latencia: `python data/pilot/run_bench_works.py --http http://127.0.0.1:8781 --db data/pilot/atlas-pilot-v2.db` (cliente de AtlasBench, 500 peticiones con c = 1 y 3 corridas)

| Medida | Piloto | Archivo |
|---|---|---|
| Snapshot del núcleo | 10.3 s de reloj, 3.2 s de compilación, pico 1,225 MB, 429 MB; **2,844,111 afirmaciones, las mismas que la copia: las obras no entran al núcleo** | `compile-core*.json` |
| Capa de obras (snapshot + índice + teselas) | 7.86 s (snapshot 2.8, índice 4.5, teselas 0.16); 18.8 s de reloj; pico 1,072 MB | `compile-works*.json` |
| Tamaños | snapshot 111.2 MB (217,670 nodos, 684,593 afirmaciones); índice 34.7 MB (114,736 documentos); teselas 4.4 MB (113,929 ubicadas, 807 sin fecha) | ídem |
| Teselas de obras z0–z3, p95 | 2.40 / 1.93 / 1.81 ms en las 3 corridas | `bench_works.json` |
| Teselas de obras z5–z10, p95 | 0.50 / 0.47 / 0.56 ms | ídem |
| Búsqueda de obras, p95 (200 consultas de dos palabras de títulos) | 1.60 / 1.27 / 1.22 ms | ídem |
| Servidor con núcleo y obras, después del bench | 174 MB de working set, 22 MB privados (mmap) | `serve-memory.json` |

- Recorridos: no los medí de nuevo.  El motor de recorridos lee solo el snapshot del núcleo, que tiene las mismas 2,844,111 afirmaciones con o sin obras.  El servidor del piloto no cargó `--composition` ni `--layout`, así que ni `/api/path` ni las teselas de estructura se midieron aquí.
- `p95` de `resolve` con 1,000,000 de fusiones: 0.026 ms (prueba `test_store` de `pytest`, 2026-10-20).  Pasa < 1 ms.

## 4. Extrapolación a la ingesta completa (EXTRAPOLACIÓN, no medición)

Supuestos (`data/pilot/extrapolate.py` → `extrapolation.json`):
- Todo escala de forma lineal desde el piloto.
- Se guardan todos los registros de las dos fuentes: 5,203,481 de zbMATH y 4,625,779 de OpenAlex (`papers-historia.md`).
- Las personas crecen en proporción a las obras, lo que sobreestima.
- Las citas van de la densidad interna del piloto (cota baja) a «toda referencia es interna» (cota alta).

| Magnitud | Extrapolado | Regla del plan | ¿Cabe? |
|---|---|---|---|
| Obras canónicas (unión por DOI) | 7.26–8.71 M (no 5 M: OpenAlex solo comparte el 36.5 % con zbMATH) | — | — |
| Afirmaciones de la capa | 42–110 M (citas 20–87 M) | — | — |
| Registro | **74–144 GB** | ≤ 120 GB | en la cota alta **no**: L1 (ADR §8.3) |
| Snapshot de obras | 6.9–17.9 GB | — | — |
| Índice / teselas | 2.2–2.6 GB / 0.3 GB | — | — |
| Compilación de la capa | 8–21 min (× 2 = 16–42 min) | `compile --all` ≤ 2 h | sí, si es lineal |
| Memoria del servidor | mmap de 9–21 GB.  El working set depende de lo que se toque; en el piloto, 174 MB de 579 MB mapeados (30 %) | ≤ 16 GB | no se puede afirmar; hay que medirlo |
| Memoria de `works compile` | pico 1.07 GB en el piloto; lineal daría decenas de GB | — | riesgo: probablemente pide compilar por fragmentos |
| Búsqueda p95 | 1.2–1.6 ms con 115 mil documentos.  Una cota lineal pesimista daría 85–136 ms con unos 70× | ≤ 100 ms | probable (BM25 con poda no es lineal), pero no medido |
| Teselas p95 | las z ≤ 7 tienen la lista precalculada y no dependen de n; las profundas, sí | ≤ 2 × 9.44 ms (ronda 4) | sí en el piloto |
| Descarga de zbMATH por la API REST | 26.9 registros/s → **≈ 54 h** para 5.2 M (2 hilos, 1 petición/s) | ingesta ≤ 48 h | no por REST; OAI-PMH sin medir |
| OpenAlex por la API | **no viable sin llave**: ver §6 | — | — |

## 5. Cobertura por periodo y área (`coverage_cells`, medida el 2026-10-20; `v2-coverage.json`)

| Fuente · área | Periodo | Indexa la fuente | Ingerido | Corte |
|---|---|---|---|---|
| zbMATH · MSC 30 (`wd:Q193756`) | 1600–1799 | 0 | 0 | toda la clase |
| | 1800–1867 | 5 | 5 | |
| | 1868–1899 | 1,018 | 1,018 | |
| | 1900–1949 | 1,121 | 1,121 | |
| | 1950–1999 | 40,138 | 40,138 | |
| | 2000–2026 | 57,119 | 57,119 | |
| OpenAlex · Mathematics (*) | 1800–1867 / 1868–1899 / 1900–1949 / 1950–1999 / 2000–2026 | **desconocido** (429, sin presupuesto) | 25 / 26 / 206 / 2,451 / 7,145 | muestra al azar de 10,000 |
| CDLI | antes de 0000 | 2,274 | 2,274 | todo `genre=Mathematical` |
| Wikidata (canon) | hasta 2026 | **desconocido** (no hay catálogo) | 1,817 | P50 de personas del registro con ≥ 3 sitelinks, más la lista curada |
| Las otras 62 clases MSC | todas | — | 0 | **desconocido**, no ingerido |

En la muestra de OpenAlex, por subcampo: 2604 (matemática aplicada) 2,423; 2613 (estadística y probabilidad) 2,068; 2608 (geometría y topología) 1,607; 2610 1,440; 2611 (modelado) 606; 2602 558; 2607 447; 2614 370; 2612 348; 2605 31.

## 6. Hallazgos que cambian el plan

1. **OpenAlex tiene ahora un presupuesto diario por IP sin llave.** Después de unas 1,000 peticiones del 2026-10-20 (bloques de DOI), respondió «Insufficient budget… $0 remaining; resets at midnight UTC» (`fetch_oa_dois.err`).  El cursor de la API (unas 23 mil páginas) no es viable sin llave, y una llave es una credencial.  Las alternativas son el snapshot público de S3 (sin credenciales, tamaño **sin verificar**) o que Adrián dé una llave gratuita.
2. La unión es de unos 7.3–8.7 M de obras, no 5 M.
3. El registro pasa de 120 GB en la cota alta de citas.  L1 probablemente hace falta antes de la ingesta completa.
4. zbMATH por REST tarda unas 54 h: falta medir OAI-PMH, que es lo que pide el ADR para la ingesta completa.

## 7. Licencias, verificadas en su página el 2026-10-20

| Fuente | Texto en la página |
|---|---|
| zbMATH Open | Términos de la API (junio de 2021), `static.zbmath.org/legal/api-terms-and-conditions.html`: «(2) All data are allowed to be used under a CC-BY-SA 4.0 license», con el aviso de copyright incluido en toda extracción y un número razonable de peticiones por segundo.  Solo se guardan metadatos, sin reseñas. |
| OpenAlex | «Our data is CC0» (openalex.org/about) |
| OpenCitations | «available under a Creative Commons public domain dedication (CC0)» (opencitations.net/about) |
| Crossref | «almost none of the metadata is subject to copyright»; los resúmenes son la excepción y no se leen |
| CDLI | «Text in the pages of CDLI may be freely copied, aggregated and re-used according to common and fair academic practice», con mención de la fuente.  El uso general es «non-commercial».  Solo se guardan metadatos del catálogo. |

## 8. Pregunta para Adrián: ¿sigue la ingesta completa?

Mi recomendación es **no seguir todavía**.  Antes de lanzarla, decide:
1. ¿Cómo se baja OpenAlex? Snapshot de S3, sin credenciales pero con un tamaño que hay que medir antes, o una llave gratuita, que es una credencial y la tienes que autorizar tú.
2. ¿Se aplica L1 (ADR §8.3) antes de ingerir, dado que la cota alta del registro es de 144 GB?
3. ¿zbMATH por OAI-PMH? Pide un piloto de un día para medirlo.  Por REST tarda unas 54 h, por encima del tope de 48 h.
4. ¿Se acepta que la meta sean unos 8 M de obras canónicas y no 5 M?
5. ¿Se reemplaza o se borra `data/pilot/atlas-pilot.db` (la copia vieja, con ids `wd:` en el canon), y se apunta `r5_gate.py` a la v2?
