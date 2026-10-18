# Estado del proyecto · 2026-10-16

Ronda 1 (arranque), cerrada por el integrador.  Las mediciones se tomaron entre el 2026-10-16 y el 2026-10-18 (UTC) en esta máquina: Windows 11 Pro 10.0.26200, Intel family 6 model 197 (16 hilos lógicos), 32 GB de RAM, GPU Intel Arc Pro 140T.  Cada cifra lleva el comando que la produjo.  Las cifras marcadas «(trabajador)» las midió el dueño de ese componente con el comando que se indica; el integrador no las repitió.  Hubo otros procesos pesados en paralelo durante casi toda la ronda, así que las latencias tienen ruido (los trabajadores midieron hasta ±30 % y, en un caso, p50 3 veces distintos).  Los contadores de trabajo (expansiones, tamaños, conteos) son deterministas.

Las metas del informe (20,000 ítems, >99 % de precisión de fusiones, p95 <100 ms) siguen siendo objetivos.  Aquí solo se reporta lo medido.

## 1. Qué existe

| Parte | Lenguaje | Estado |
|---|---|---|
| `atlas/model.py`, `atlas/store.py` | Python | Contrato ejecutable: 18 tipos de entidad, 17 relaciones, 7 estados de evidencia, procedencia obligatoria, eventos de solo agregar (triggers), fusiones como redirecciones reversibles, FTS5, migraciones, exportación JSON-LD con validador.  `existing_claim_ids()` agregado en esta ronda a pedido de ingesta. |
| `atlas/ingest/` | Python | Adaptadores Wikidata (SPARQL), Mathlib (tarball de un commit fijo, parser Lean), Wikipedia EN/ES (API, caché HTTP, modo offline). |
| `atlas/resolve.py` | Python | Candidatos de formalización y de área, cola de revisión; fusiones solo por decisión registrada. |
| `atlas/cli.py` | Python | `python -m atlas ingest|build|serve|search|path|stats`.  `build` corre la ingesta offline (reusa `data/atlas.db` y `data/cache`), la resolución, `cargo build --release` y `atlas compile --all`; deja un reporte JSON en `data/cache/reports/build-*.json`. |
| `core/atlas-core` | Rust | Snapshot binario CSR mapeado en memoria, compilado desde SQLite con verificación por hash. |
| `core/atlas-search` | Rust | Vía léxica tantivy (analizadores ES/EN/Lean), índice de fórmulas tipo Tangent, codificador multilingual-e5-small (ONNX) con HNSW y escaneo exacto int8, RRF ponderado con router, diagnóstico por vía, filtros `type:`, `assume:`, `lane:`. |
| `core/atlas-paths` | Rust | Álgebra de composición v2 (`shapes/composition.json`), premisas AND/OR, A* ALT bidireccional sobre (nodo, tipo de conclusión), 6 objetivos, k rutas, justificación por paso, abstención con motivo. |
| `core/atlas-layout` | Rust | Leiden multinivel, ForceAtlas2 multinivel con Barnes-Hut en paralelo, arranque en caliente estable, teselas z/x/y binarias y JSON, etiquetas de región. |
| `core/atlas-server` | Rust | Binario `atlas` (compile, index, layout, serve, search, path, stats; salida `--json`).  API axum con ETag, gzip/brotli, pool acotado. |
| `web/` | TypeScript + Vite + WebGL2 | Mapa multiescala, panel con insignias de evidencia, búsqueda instantánea, rutas, vista textual navegable con teclado, ES/EN.  Compila a `web/dist`. |
| `bench/` | Python | AtlasBench v3: 17,653 ítems silver más ítems escritos a mano, particiones dev/test/sealed, oráculo de composición, reporte por clase con intervalos. |

## 2. Escala de los datos reales

Fuente: `python -m atlas build --json` del 2026-10-18 00:30 UTC (reporte `data/cache/reports/build-20261018T003437Z.json`, salida en `data/cache/reports/build-logs/build6.json`).

- Snapshot `atlas-03c61399814c97c4`: 318,733 nodos, 1,727,728 afirmaciones, 1,940,995 aristas, 1,727,728 filas de procedencia, 5 fuentes, 2,950 fusiones activas, event_seq 2,049,417.  257,445,955 bytes.
- Nodos por tipo: statement 190,407; definition 49,987; source_fragment 31,976; concept 20,594; theory 8,886; person 4,623; resource 4,343; area 1,823; algorithm 1,657; application 678; model 524; conjecture 285.
- Afirmaciones por relación: cites 1,176,414; part_of_area 260,731; depends_on_logically 215,764 (429,031 aristas, por las premisas); appears_in 35,524; special_case_of 28,830; named_after 5,555; authored_by 1,696; formalized_as 1,198; applied_in 812; influenced 451; prerequisite_for 334; equivalent_to 311; generalizes 108.
- Por estado: model_proposed 1,177,168; extracted 305,627; formally_linked 244,933; editorially_reviewed 0.  Todas las aristas de Wikipedia son `cites` en `model_proposed`; ninguna es prerrequisito.
- Desconocidas (sin afirmaciones): person 1,208; concept 194; statement 115; area 20; application 17; conjecture 5.  Se reportan como «desconocida», nunca como «aislada».
- Disco (`du -sh`): `data/atlas.db` 3.4 GB; `data/snapshot` 246 MB; `data/index` 2.1 GB (incluye dos índices viejos, `0dd04e66745eaceb` y `237352f48cdee67a`, que se pueden borrar porque son derivados); `data/layout` 295 MB; `data/cache` 97 MB.

## 3. Build de punta a punta

`python -m atlas build --json` (2026-10-18, binario recién compilado): 278.34 s en total (4 min 38.8 s de reloj).

| Paso | Tiempo | Resultado |
|---|---|---|
| ingest mathlib (offline) | 25.17 s | 0 afirmaciones nuevas; 470,550 ya presentes (idempotente) |
| ingest wikidata (offline) | 9.33 s | **omitido**: la repetición offline necesita un POST SPARQL que no está en `data/cache`; el registro conserva la importación anterior (44,079 afirmaciones) |
| ingest wikipedia (offline) | 31.16 s | 0 nuevas; 1,205,465 ya presentes |
| resolve | 17.37 s | 10,389 candidatos de formalización, 9,496 a la cola de revisión (9,527 ítems); contra 355 enlaces curados: recall@1 0.4282, recall@3 0.5746, precisión sobre el umbral 0.90 de 0.7521 en 117 ítems |
| cargo build --release -p atlas-server | 116.33 s | LTO fat, codegen-units 1 |
| atlas compile --all | 64.35 s de reloj | snapshot 1.51 s; índice de búsqueda 33.28 s (0 textos codificados, 315,783 embeddings tomados de caché; hnsw 18.49 s, carga del codificador 6.92 s, léxico 4.46 s, fórmulas 2.22 s); layout en caliente 1.80 s y 3,304 teselas (68.5 MB) |
| atlas stats --verify | 37.8 ms | verificado; el snapshot abre en 0.27 ms |

Construcción en frío del índice neuronal (trabajador de búsqueda, `atlas.exe index --json`, 286,757 documentos): 2,859.6 s, de ellos 2,810.0 s para codificar (de 350 a 102 textos/s).  Desde entonces los embeddings quedan en `data/index/emb-multilingual-e5-small.cache` y reconstruir no recodifica.

## 4. Pruebas

| Comando | Resultado |
|---|---|
| `python -m pytest -q` | 143 passed en 29.38 s |
| `cargo test --workspace --release` (en `core/`) | 119 passed, 0 failed |
| `cargo clippy --workspace --all-targets --release` | 0 warnings, 0 errores |
| `npm test` (en `web/`) | 8 passed |
| `npm run build` | compila en 578 ms; JS principal 74.43 kB (25.19 kB gzip); KaTeX 259 kB, carga diferida |

## 5. Servidor y web (prueba de humo del integrador)

`python -m atlas serve` sobre el snapshot real; una petición con `curl` por endpoint.  Todos devolvieron 200 salvo la ruta inexistente, que devolvió el 404 JSON esperado.

| Endpoint | Código | Bytes | Tiempo |
|---|---|---|---|
| `/api/health` | 200 | 456 | 2.0 ms |
| `/api/coverage` | 200 | 1,042 | 1.7 ms |
| `/api/search?q=teorema de Pitágoras&lang=es&limit=3` | 200 | 1,020 | 12.1 ms |
| `/api/entity/wd:Q11518` | 200 | 10,250 | 3.2 ms |
| `/api/neighborhood?id=wd:Q11518&depth=1` | 200 | 3,497 | 2.0 ms |
| `/api/path?from=wd:Q274089&to=wd:Q8548` | 200 | 2,364 | 2.9 ms |
| `/api/map/tiles/0/0/0` | 200 | 172,095 | 3.7 ms |
| `/api/map/tiles?z=1&x=0&y=0` | 200 | 73,169 | 3.0 ms |
| `/api/map/locate?id=wd:Q11518` | 200 | 140 | 2.1 ms |
| `/api/nope` | 404 | 65 | 2.0 ms |
| `/` | 200 | 680 | 2.3 ms |

- Los 65 archivos de `web/dist` (sin contar las copias `.br`/`.gz`) devuelven 200.
- Navegador sin interfaz (`node bench/smoke.mjs --q pitagoras`, Edge headless con ANGLE/D3D11): carga en 1,041 ms, 5 teselas con 200, búsqueda en 391 ms (incluye teclear y el debounce), 12 resultados, primer resultado «Pitágoras», panel en 40 ms con 14 afirmaciones y 14 insignias, vista textual con 409 elementos, cambio a inglés reflejado en la URL (`#lang=en&text=1`), 0 errores.
- Al terminar, el árbol del servidor se cerró con `taskkill /T /F` y no quedaron procesos `atlas.exe`, `python.exe` ni Edge headless.

Carga medida por el trabajador del servidor (`node bench/load.mjs … --conns 64 --workers 8 --secs 10 --encoding br`, con `atlas index` ocupando unos 5 núcleos, así que son cotas inferiores): health 19,524 req/s; entity 9,833 req/s (p95 14.10 ms); neighborhood profundidad 1 11,527 req/s, profundidad 2 2,090 req/s; search 2,557 req/s (p95 37.26 ms); path con ruta 4,217 req/s (p95 31.91 ms); tiles binarias 14,231 req/s.  Memoria tras 32 s de carga mixta: 1,111 MB privados y 73 hilos, contra 5,170 MB y 545 hilos antes de acotar el pool.  Cuadros por segundo (trabajador, `node bench/fps.mjs`): 100k puntos y 100k aristas a 107.6 FPS; 1M y 1M a 32.9 FPS con p99 de 416 ms.

## 6. Evidencia: 5 búsquedas y 5 recorridos reales

Comandos: `python -m atlas search "<q>" --lang <l> --limit 5 --json` y `python -m atlas path A B --objective <o> --json`.  Cada llamada a la CLI abre el índice y carga el codificador ONNX: 11.3–11.6 s de reloj por búsqueda y 2.4–2.5 s por recorrido.  El servidor mantiene todo abierto (búsqueda p50 9.3 ms en AtlasBench).

**Búsquedas** (top 3; entre paréntesis, rango en la vía léxica / semántica / de fórmulas):

1. ES `teorema de Pitágoras`: wd:Q11518 «teorema de Pitágoras» (1/2/–); wp:3ad1581f0f7cfa25 «Wikipedia (es): Teorema de Pitágoras» (3/4/–); wp:b31f3638bdf348ea «… Teorema de Pitágoras inverso» (2/7/–).
2. EN `fundamental theorem of calculus`: wd:Q1217677 (1/7/–); wd:Q116450161 «first fundamental theorem of calculus» (2/2/–); wp:5df7af0bce116882 (4/1/–).
3. Fórmula `a^2 + b^2 = c^2` (solo vía de fórmulas): wd:Q11518 «Pythagorean theorem» (–/–/1); wd:Q208225 «Pythagorean triple» (–/–/2); wd:Q17008946 «Eisenstein triple» (–/–/3); 4.º ley de los cosenos; 5.º `mathlib:CharP.sq_add_sq`.
4. Nombre Lean `Nat.Prime.two_le`: `mathlib:Nat.Prime.two_le` (1/1/–); `mathlib:Nat.add_two_le_nth_prime` (2/2/–); `mathlib:Nat.dvd_prime_two_le` (3/10/–).
5. Pregunta entre dominios `¿qué relación hay entre los grupos de simetría y las leyes de conservación?`: wd:Q902019 «grupo de simetría» (2/4/–); wp:d04bfa042eb94d4c «Leyes de conservación (física)» (7/3/–); wd:Q205805 «ley de conservación» (1/8/–).  El teorema de Noether no aparece porque no está en el registro (`Store.search("Noether")` solo devuelve anillos y espacios noetherianos): es un hueco de cobertura, no de ranking.

**Recorridos:**

1. `mathlib:Nat.Prime.emultiplicity_one → mathlib:Monoid` (best_supported): conclusión `logical_dependency`, fuerza 5.  Dos pasos `depends_on_logically` vía `mathlib:emultiplicity`, ambos `extracted` de src:mathlib4, sin premisas faltantes.
2. `wd:Q1000116 → wd:Q36191904` (best_supported): `specialization`, fuerza 4.  Ruta 1 directa (`special_case_of`); ruta 2 alternativa vía wd:Q179692 «axioma de elección».  Ambas `extracted` de Wikidata.
3. `wd:Q274089 → wd:Q8548` «Algoritmo de Dijkstra» (shortest): `study_route`, fuerza 2, «ruta de estudio sugerida; nunca necesidad lógica», vía wd:Q629283 «cola de prioridades».  El motor expandió 29 estados en 917 µs.
4. `wd:Q670235 → mathlib:UniqueFactorizationMonoid` (most_formal): `formalization`, fuerza 4, «formalización (traducción, no fidelidad)», un paso `formalized_as` de src:mathlib4.
5. `wd:Q85769835 «Teorema de Pitágoras inverso» → wd:Q11518` (best_supported): **abstención** `only_weak`.  La única conexión pasa por las páginas de Wikipedia (`appears_in` → `cites` en model_proposed → `appears_in`), que es «alcance bibliográfico» de fuerza 0; el motor la muestra como exploración pero no la presenta como conclusión.

Salidas completas: scratchpad de la sesión (`ev/searches.txt`, `ev/paths.txt`); se reproducen con los comandos de arriba.

## 7. Rendimiento por componente (trabajadores)

**Búsqueda** (`bench_search real`, 200 consultas por clase, datos reales, 2×2 sesiones del codificador), p50/p95/p99 fusionados en ms: nombres 9.57/14.98/18.4; fórmulas 10.89/22.51/35.92; lenguaje natural 10.19/17.45/21.59; entre dominios 14.73/22.84/25.18.  Codificar la consulta cuesta 7.9–12.7 ms del p50 semántico.  Rendimiento: 1 cliente 82 qps; 16 clientes 182 qps con p95 198.49 ms (sobre la meta de 100 ms).  Apertura del índice 3.87 s, 1,559 MiB privados.  HNSW sobre vectores e5 reales: ef96 1.24 ms con recall@10 0.75 (escaneo exacto paralelo 1.68 ms).  Sintético 1M (LSI 128d): 16 clientes 428 qps, p95 72.73 ms; HNSW ef96 con recall@10 0.348.  El arreglo del router subió de 81 a 167 de 200 las consultas de fórmula que llegan a la vía de fórmulas.

**Recorridos** (`bench.exe --snapshot C:/MPM/data/snapshot --queries 200`, k = 3): pares alcanzables p50 0.12–0.15 ms y p95 1.6–4.9 ms en los 6 objetivos, 200/200 encontrados; pares aleatorios p95 0.31–1.24 ms, los 200 abstenidos (167 sin autorización, 27 límite de saltos, 6 desconectados); 6,835 consultas/s en 16 hilos; índice del motor 9.8 MiB.  Sintético 1M nodos / 10M aristas: alcanzables p95 271 ms (shortest) a 3,432 ms (most_diverse); aleatorios p95 3.6–4.6 s y 17.4 s en most_diverse.  Con k = 1, el A* ALT bidireccional da p95 180 ms contra 16,426 ms de Dijkstra.

**Mapa** (`bench_layout --snapshot C:/MPM/data/snapshot --drop 0.01 --det 1`, máquina cargada): construcción en frío 12.72 s (Leiden 2.06, layout 9.22, teselas 1.25), 0.20 GiB pico; con carga ligera, la versión anterior tardó 2.66 s.  Nivel de área: 724 comunidades, la mayor con 64,679 nodos, modularidad 0.780 (antes 199,094 y 0.442).  Estabilidad al agregar 1 % de nodos: desplazamiento medio 0.0035 en caliente contra 0.2329 en frío.  Teselas: p50 8,138 B y p95 19,202 B en el zoom más profundo; `tile()` JSON p95 2.77 ms.  Sintético 1M: 92.4 s en frío con la máquina cargada (34.0 s antes, con carga ligera), 1.62 GiB.  En el build integrado el layout corre en caliente: 1.80 s.

**Ingesta** (trabajador): Wikipedia + resolve 1,306.98 s, 0 reintentos; Mathlib offline 175.2 s, 8,637 archivos sin fallas, 245,656 declaraciones.  Wikipedia va a la tasa anónima de Wikimedia (unos 6.5 s por lote de 50) porque `ATLAS_CONTACT` no está definido.

## 8. AtlasBench (v3, partición test)

Comando: `python -m bench run --split test --serve --skip-cli --out bench/runs/2026-10-17-v3-test-http-integrated` (2026-10-18 00:39 UTC, 74.5 s de evaluación, 0 reintentos HTTP, servidor vivo al final, registro sin cambios durante la corrida).  Sistema evaluado: HTTP con el binario final.  Baseline: la vía léxica FTS5 del registro.  Partición sellada: 0 aperturas.

**Advertencia:** no hay un solo ítem adjudicado por especialistas.  Todo es silver (generado del propio registro) o escrito por un agente y pendiente de revisión.  Por eso los veredictos son `PROVISIONAL-` o mecánicos, y la mayoría de los ítems léxicos son triviales (label_exact 552 de 552, docstring 260 de 260 en test), así que solo sirven como regresión.

| Componente | Veredicto | Cifra |
|---|---|---|
| retrieval | PROVISIONAL-FAIL | la regla pide Δ nDCG@10 ≥ +0.05 con cota inferior ≥ +0.02 en cada celda con n ≥ 30.  Δ en celdas no triviales: en/informal_to_formal +0.046 [0.013, 0.082]; es/informal_to_formal +0.031 [0.014, 0.052]; en/formula +0.017 [0.005, 0.033]; es/formula 0.000 |
| routes.structure | PASS | 0 rutas inválidas de 224 devueltas |
| routes.adversarial | PROVISIONAL-PASS | 188/188 sin conclusión prohibida [0.980, 1.000]; el modelo nulo saca 1/188 |
| routes.end_to_end | NOT_EVALUABLE | faltan juicios de especialistas (`route_outputs.jsonl` es la cola) |
| er | NOT_EVALUABLE | sin muestra etiquetada de fusiones del sistema |
| relations | NOT_EVALUABLE | sin etiquetas de relaciones |
| provenance (registro y servida) | PASS | 0 fallas de 3,608; cota de Wilson 0.00106 |
| latencia | PASS | search p95 11.86 ms con c=1 y 56.88 ms con c=8; entity 1.10; neighborhood 0.52; tiles 2.57 ms |

Recuperación, nDCG@10 con IC 95 % (sistema vs baseline):

| Celda | n | triviales | híbrida | FTS5 |
|---|---|---|---|---|
| en/lean_name | 377 | 375 | 0.996 [0.993, 0.999] | 0.981 [0.971, 0.989] |
| en/text | 420 | 415 | 0.787 [0.752, 0.822] | 0.892 [0.869, 0.914] |
| es/text | 41 | 33 | 0.878 [0.804, 0.937] | 0.769 [0.636, 0.882] |
| en/informal_to_formal | 215 | 158 | 0.315 [0.279, 0.351] | 0.275 [0.243, 0.311] |
| es/informal_to_formal | 180 | 1 | 0.163 [0.145, 0.182] | 0.132 [0.129, 0.137] |
| en/formula | 163 | 2 | 0.018 [0.005, 0.035] | 0.001 [0.000, 0.003] |
| es/formula | 197 | 0 | 0.000 | 0.000 |
| escritos a mano (no triviales) | 19 | 0 | 0.405 [0.284, 0.527] | 0.000 |

Recorridos (HTTP): acierto en respondibles 221/261 = 0.847 [0.798, 0.885]; abstención en respondibles 40/261; sobreafirmación frente al oráculo 0/224.  El baseline (BFS sobre el registro sin álgebra) sobreafirma en 195/449 y solo pasa 1/188 adversariales.  Resolución de entidades (silver): falsa fusión 1/159 en colisiones de etiqueta y 0 en los otros negativos.  Relaciones corroboradas por otra fuente: prueba 65/217 y 0 en las demás familias.

**Cambio del integrador en esta ronda (búsqueda).**  La primera corrida test del día dejó la híbrida por debajo de FTS5 en en/text (0.643 contra 0.892).  Causa: la vía léxica de tantivy era una disyunción pura, y una consulta hecha con un fragmento de docstring perdía contra etiquetas que coinciden en una sola palabra frecuente; FTS5 exige todas las palabras.  Primero probé subir a 1.0 el peso léxico del router en lenguaje natural, y en dev empeoró (en/text de 0.635 a 0.625), así que lo revertí.  Lo que quedó es `ALL_WORDS_BOOST` en `core/atlas-search/src/lexical.rs`: con consultas de 3 palabras o más, una cláusula conjuntiva sobre descripción y enunciado, con su prueba (`queries_of_three_words_or_more_add_an_all_words_clause_…`, que falla si se quita la cláusula).  El ajuste se hizo en dev (`bench/runs/2026-10-17-v3-dev-http-before` → `…-allwords`):

| Celda dev | antes | después |
|---|---|---|
| en/text | 0.635 [0.596, 0.673] | 0.750 [0.714, 0.785] |
| docstring (trivial) | 0.430 | 0.631 |
| en/informal_to_formal | 0.282 | 0.310 |
| es/text | 0.792 | 0.786 |
| escritos a mano | 0.416 [0.308, 0.529] | 0.388 [0.288, 0.494] |
| search p95 c=8 | 59.60 ms | 60.21 ms |

En test (antes → después): en/text 0.643 → 0.787, docstring 0.451 → 0.680, en/informal_to_formal 0.285 → 0.315.  La corrida test de antes se sobrescribió; esas tres cifras salen de su `report.md`, que leí antes de reemplazarlo.  La híbrida sigue por debajo de FTS5 en en/text: falta calibrar la fusión.

## 9. Dependencias y por qué

- **Rust:** rusqlite (bundled, el contrato SQLite), memmap2 (snapshot mmap), rayon (paralelismo), bytemuck (CSR sin copias), blake3 (hash de contenido), ahash, serde/serde_json, thiserror, clap (CLI), axum + tokio + tower/tower-http + serde_urlencoded (servidor, estáticos seguros, gzip/brotli, límites), tantivy 0.26 (BM25), **fastembed 7.1** (ONNX para multilingual-e5-small; es opcional, el respaldo es LSI determinista), **usearch 2.26** sin la opción `numkong`, porque esa opción no enlaza con MSVC (HNSW), memory-stats (benchmarks), criterion (benchmarks).
- **Python:** solo la biblioteca estándar (sqlite3 con FTS5) más numpy, que ya estaba instalado.  Sin dependencias nuevas.
- **Web:** katex 0.19 (fórmulas, carga diferida); vite, typescript y @types/node para compilar; puppeteer-core (solo desarrollo: humo y FPS con el Edge instalado).
- **Herramienta fuera del proyecto:** oha 1.16, instalada en `C:/Users/AdrianGonzalez/.cargo-target/server-oha` para comparar clientes de carga.  No es dependencia.

## 10. Huecos conocidos por paquete de trabajo

| WP | Estado | Hueco |
|---|---|---|
| WP01 Modelo | hecho (v1) | Un solo estado de evidencia vigente por afirmación; las valoraciones simultáneas son una migración v2 (anotado en SPEC).  No se corrió un validador SHACL externo (no hay rdflib/pyshacl).  El dominio w3id es un marcador. |
| WP02 Inventario | parcial | Cada fuente tiene licencia y términos en `sources`; falta el registro formal de acuerdos y cobertura por fuente. |
| WP03 AtlasBench | parcial | 17,653 ítems silver más los escritos a mano; **0 adjudicados por especialistas** (la meta es 20,000).  Los ítems de fórmula no sirven: las 4,398 fórmulas de Wikidata son MathML sin TeX, y la API rechaza 352 de las 360 consultas de test por pasar de 256 caracteres. |
| WP04 Ingesta documental | parcial | HTML de Wikipedia sí; LaTeX/arXiv y PDF no.  Wikidata no se puede repetir offline (falta un POST SPARQL en caché) y `build` lo omite y lo reporta.  Wikipedia está limitada a la tasa anónima mientras no exista `ATLAS_CONTACT`. |
| WP05 Resolución | parcial | 2,950 fusiones activas; la formalización tiene recall@1 0.43 y precisión 0.75 sobre el umbral, contra enlaces curados; cola de 9,527 ítems sin revisar.  La precisión de fusiones no se puede evaluar (no hay etiquetas). |
| WP06 Formal | hecho (Mathlib) | Mathlib en el commit 1e75e9dd (Lean v4.35.0) con enunciados, definiciones e infraestructura separados; 244,933 afirmaciones `formally_linked`.  Falta la evaluación independiente de fidelidad. |
| WP07 Búsqueda | hecho, sin calibrar | La regla de AtlasBench falla (PROVISIONAL-FAIL).  Por debajo de FTS5 en en/text.  Fórmulas casi en 0 por falta de TeX.  p95 de 198 ms con 16 clientes por la codificación ONNX.  Recall de HNSW 0.75 a ef96. |
| WP08 Mapa | hecho (v1) | Las etiquetas de región toman el miembro más central, y eso saca centros espurios como «Wikipedia (en): ISBN» (visto en la captura de humo).  Hay que excluir de la centralidad los `source_fragment` y los `cites` en model_proposed.  No hay evaluación de tareas de orientación. |
| WP09 Síntesis | no empezado | — |
| WP10 Recorridos | hecho (v1) | Validez de extremo a extremo no evaluable sin especialistas.  El caso `historical` de fuerza 0 quedó decidido el 2026-10-17 (§12): cuenta como respuesta rotulada; la abstención en respondibles bajó de 40/261 a 1/261.  El grafo sintético de 1M es lento en pares aleatorios (p95 hasta 17 s). |
| WP11 Aplicaciones | mínimo | 812 `applied_in`, 678 aplicaciones; sin cadenas modelo-método-uso-evidencia; física fuera del alcance de la selección de Wikidata (no está el teorema de Noether). |
| WP12 Aprendizaje | no empezado | 334 `prerequisite_for`, todos de Wikidata. |
| WP13 Puentes | no empezado | — |
| WP14 Contribución | parcial | API JSON y exportación JSON-LD; falta la edición revisable y las réplicas. |
| WP15 Editorial | no empezado | 0 afirmaciones `editorially_reviewed`. |
| WP16 Operación | parcial | Build reproducible con reporte JSON y pruebas en las tres capas.  Falta CI, backups del registro de 3.4 GB y una prueba de reconstrucción desde cero (el índice neuronal en frío tarda unos 47 min). |

## 11. Siguiente ronda, por impacto

1. **Anotación de especialistas** de una primera muestra: rutas devueltas (`route_outputs.jsonl`), relaciones y fusiones.  Sin eso, WP03, WP05 y WP10 siguen en NOT_EVALUABLE y ningún veredicto deja de ser provisional.
2. **TeX de las fórmulas de Wikidata** (P2534 en `Entity.formula`, junto al MathML), después regenerar los ítems silver de fórmula.  Desbloquea la vía de fórmulas y su evaluación (WP07).
3. **Calibrar la fusión de búsqueda en dev** (pesos por ruta, k de RRF, profundidad de candidatos) hasta igualar o superar FTS5 en en/text sin perder lo ganado en informal_to_formal; después, una sola corrida test.
4. ~~Decidir en SPEC el caso `historical` de fuerza 0~~: decidido e implementado el 2026-10-17 (§12).  Falta que la web muestre `conclusion.category` (dueño: web).
5. **Latencia con concurrencia:** caché de embeddings de consulta y 4×2 sesiones (medido: 216 qps con 3.1 GB), o el modelo MiniLM cuantizado; medir de punta a punta contra la meta de p95 <100 ms.
6. **Etiquetas de región del mapa** sin páginas de Wikipedia ni citas propuestas como centros.
7. **Fuentes:** definir `ATLAS_CONTACT` (lo decide Adrián) para que Wikipedia vaya unas 20 veces más rápido; guardar en caché la consulta SPARQL de Wikidata para que `build` pueda repetirla offline; ampliar la selección a física matemática.
8. **Operación:** borrar los índices viejos de `data/index` (620 MB + 695 MB, derivados), definir el backup del registro y una prueba de reconstrucción en frío medida.

## 12. Decisiones del dueño

### 2026-10-17 · Una ruta solo histórica cuenta como respuesta (Adrián)

**Decisión.**  Una ruta cuya conclusión completa es `historical` (fuerza 0: cadenas de `named_after`, `influenced`, `authored_by`) cuenta como respuesta y no como abstención, pero se rotula siempre como conexión histórica, nunca como dependencia matemática, prerrequisito ni aplicación.  Si existe una ruta de fuerza ≥ 1, va primero y la histórica no se muestra.  Las conexiones `bibliographic` y `documentary` siguen siendo abstención `only_weak`.  Regla escrita en `docs/SPEC.md` §3 (regla 10) y §15.

**Implementación** (`core/atlas-paths/src/explain.rs`, `lib.rs`).  Cuando no hay ruta de `min_strength` y la búsqueda relajada a fuerza 0 encuentra una ruta de familia `history`, el motor la devuelve en `paths` con `abstained: false`, `abstention: null` y la advertencia «Conexión histórica: no es dependencia matemática, prerrequisito ni aplicación» como primera de `conclusion.caveats`.  Campo nuevo en cada ruta: `conclusion.category` ∈ `mathematical`, `structural`, `application`, `pedagogical`, `candidate_hypothesis`, `historical`, `documentary`, `none`, derivado de la familia de la conclusión en `shapes/composition.json`.  El álgebra no cambió: una cadena histórica seguida de una relación matemática sigue sin autorizar nada (R4).  Límite conocido: el motor mira una sola ruta relajada; si sale bibliográfica habiendo también una histórica, se abstiene.  En la corrida de abajo no pasó ninguna vez.  Pruebas nuevas en `tests/engine.rs` sobre un grupo nuevo del fixture (`s:x-nm*`, `per:x-namer`): la cadena histórica responde rotulada, una histórica seguida de `depends_on_logically` queda `unauthorized` (R4), la ruta lógica gana a una histórica de igual largo y el alcance bibliográfico sigue abstenido.  `cargo test -p atlas-paths --release`: 37 pruebas verdes; clippy sin avisos.

**Medido** (AtlasBench v3 test, solo los 462 ítems de recorridos, HTTP; mismo registro `data/atlas.db` y snapshot; antes = binario compartido previo, después = binario con el cambio; `bench/runs/2026-10-17-v3-test-routes-historical-before` y `…-after`):

| Métrica (HTTP) | antes | después |
|---|---|---|
| abstención en respondibles | 40/261 = 0.153 [0.115, 0.202] | 1/261 = 0.004 [0.001, 0.021] |
| acierto en respondibles | 221/261 = 0.847 [0.798, 0.885] | 260/261 = 0.996 [0.979, 0.999] |
| estructuralmente válidas | 224/224 | 264/264 = 1.000 [0.986, 1.000] |
| adversariales sin conclusión prohibida | 188/188 | 188/188 |
| sobreafirmación frente al oráculo | 0/224 | 0/264 |
| abstención en no respondibles | 185/188 = 0.984 | 184/188 = 0.979 |

Las 39 respuestas nuevas son todas `historical` sobre ítems cuyo `allowed` es `historical`.  El ítem no respondible que ahora responde (`silver:route:shared:c:dcfbaa1aa8412500<…`) devuelve otra ruta, histórica, que el oráculo también juzga `historical`: no es violación.  La única abstención restante en respondibles (`hw:route:038:charpoly-det`) es un caso de mezcla sin regla, ajeno a esta decisión.  Las etiquetas son silver o escritas por un agente: los veredictos siguen siendo provisionales.


## 13. Ronda 2 · capa conceptual (2026-10-17, cerrada el 2026-10-18 por el integrador)

Plan: `docs/plans/2026-10-17-capa-conceptual.md`.  Las reglas de decisión del plan no se movieron.  Cada cifra lleva su comando.  Las cifras de los tres trabajadores que el integrador no repitió se marcan «(trabajador)» y salen de sus reportes.

### 13.1 Qué se hizo

- Proyección conceptual (esquema v3, relación `member_of_concept`, extractor `concept-projection/5`), 5,941 conceptos y 78,163 aristas agregadas dentro de `atlas compile` (snapshot formato 2, secciones 100–110).
- Mapa: capa general por conceptos, tope de aristas por capa, panel de concepto, `/api/concept` y `/api/concept/edges`.
- Recorridos: búsqueda concepto-primero (`PathResponse.concepts`), `composition.json` v3, `bench/concept_routes.py` y 20 consultas.
- Integrador: `concepts` y `drill_down` expuestos en `/api/path` (`core/atlas-server/src/paths.rs`; el archivo estaba en CRLF y quedó en LF).  La CLI `atlas path` sigue sin esos dos flags.  Se recompiló el binario compartido y se reconstruyó todo.

### 13.2 Build y pruebas

| Comando | Resultado |
|---|---|
| `cargo build --release -p atlas-server` (binario compartido) | 2 min 6 s |
| `python -m atlas build --skip-ingest --no-cargo --json` (reporte `data/cache/reports/build-logs/build7.json`) | 73.47 s en total; `compile --all` 70.42 s (índice 38.69 s, layout 2.39 s).  Snapshot `ad7463ed1d4fcce6` (372,733,708 bytes), layout `846e9337c8a1c382` (11,813 teselas, 105.7 MB, zmax 7) |
| `python -m pytest -q` | 153 passed en 53.18 s |
| `cargo test --workspace --release` (en `core/`) | 142 passed, 0 failed |
| `cargo clippy --workspace --all-targets --release` | 0 advertencias, 0 errores |
| `npm test` (en `web/`) | 14 passed |
| `npm run build` | verde en 553 ms; JS principal 98.05 kB (32.81 kB gzip), KaTeX 259 kB diferido |

El `build` no corrió la ingesta (`--skip-ingest`): el registro se compiló tal como estaba.

### 13.3 AtlasBench

`ATLAS_BIN=…/atlas/release/atlas.exe python -m bench run --split test --serve --skip-cli --out bench/runs/2026-10-18-v3-test-concepts-integrated` (102.64 s de evaluación).

| Métrica (HTTP, test) | Antes (§8 y §12) | Ahora |
|---|---|---|
| estructura válida | 264/264 | 264/264 [0.986, 1.000] |
| acierto en respondibles | 260/261 | 260/261 = 0.996 [0.979, 0.999] |
| abstención en respondibles | 1/261 | 1/261 = 0.004 [0.001, 0.021] |
| sobreafirmación frente al oráculo | 0/264 | 0/264 [0.000, 0.014] |
| adversariales sin conclusión prohibida | 188/188 | 188/188 [0.980, 1.000] |
| `http.path` p95 | 5.73 ms (trabajador, mismos 449 ítems) | 3.24 ms [3.01, 3.71] |
| `http.tiles.load_c1` p95 | 2.57 ms [2.51, 2.66] | **8.78 ms [7.63, 15.19]** |
| `http.search.load_c1` p95 / c8 | 11.86 / 56.88 ms | 11.75 / 62.24 ms |
| retrieval en/text nDCG@10 | 0.787 [0.752, 0.822] | 0.771 [0.735, 0.807] |

Veredictos: retrieval PROVISIONAL-FAIL (sin cambio de regla), routes.structure PASS, routes.adversarial PROVISIONAL-PASS, provenance PASS, latencias PASS.  `routes.null_model_adversarial` sigue PROVISIONAL-FAIL, como antes: es el control (BFS del registro) y sobreafirma 2/188 (antes 1/188).

Las 20 consultas concepto→concepto: `python -m bench.concept_routes --http http://127.0.0.1:8769 --out bench/runs/2026-10-18-concept-routes-20-integrated`.  Aciertan 14/14 respondibles [0.785, 1.000]; 0/14 abstenciones en respondibles; 10/10 adversariales sin conclusión prohibida [0.722, 1.000]; 5/6 abstenciones en no respondibles (el homónimo integración→IsIntegral devuelve una dependencia agregada vía «número real», no prohibida); estructura 15/15; sobreafirmación 0/15; p50 1.58 ms, p95 5.55 ms (n = 20).  **Las 20 las escribió un agente** (`authored_by_agent_pending_expert_review`), no una persona a mano como pide el plan; el veredicto es provisional.  `GET /api/path?…&concepts=true&drill_down=true` devuelve la ruta de declaraciones de cada paso (probado con wd:Q1217677 → wd:Q29175: 1 paso, mismo `claim_id` que el testigo); un parámetro desconocido da 400.

### 13.4 Auditoría independiente de la muestra de 200 (juicio de agente, provisional)

Archivo: `bench/concepts/audit-sample-200.judged.jsonl` (veredicto por fila; el original queda intacto).  Lo juzgó el integrador, que no construyó la proyección.  Criterio: correcta si la declaración trata realmente de ese concepto matemático; incorrecta si el concepto es otro (homónimo o asociación sin relación); dudosa si es una generalidad o una relación laxa.  Se juzgó con la etiqueta, el nombre Lean, el módulo y el docstring; no se consultó Wikidata, así que algunos Q poco claros quedaron como dudosos.  Un solo juez, sin acuerdo entre jueces.

| Medida | k / n | Valor [Wilson 95 %] |
|---|---|---|
| correctas (estricto: las dudosas no cuentan) | 84 / 200 | 0.420 [0.354, 0.489] |
| correctas o dudosas (laxo) | 135 / 200 | 0.675 [0.607, 0.736] |
| correctas entre las decididas (sin dudosas) | 84 / 149 | 0.564 [0.484, 0.641] |
| solo asignaciones primarias (rango 0), estricto | 58 / 107 | 0.542 |
| rango 1 / rango 2, estricto | 17 / 55 ; 9 / 38 | 0.309 ; 0.237 |
| señal `name_doc`, estricto | 55 / 148 | 0.372 |
| derivadas (`zone_derived`) | 6 / 6 | 1.000 |

Totales: 84 correctas, 65 incorrectas, 51 dudosas.  La auditoría sale **bastante peor** que la medida contra los 355 curados (top-1 0.714).  Esos ítems son conceptos populares con enlace conocido; la muestra cubre 27 áreas de Mathlib con lemas técnicos.  Los fallos son casi todos homónimos de la señal `name_doc` con la etiqueta de un concepto de Wikidata: «Jacobian matrix» para coordenadas jacobianas, «overtone» para funciones armónicas, «snooker ball» para `ball`, «mobile app» para `app`, «sorting algorithm» para `desc`, «Mandelbrot set» para `M`.  Es una falla de P0 semántico (E01–E04).  Las asignaciones secundarias (rango 1 y 2) casi no sirven y la web las muestra como candidatas.  Ideas, a validar en dev y no en test: ocultar el rango 2, exigir un filtro de dominio por zona.  No se cambió ningún umbral.

### 13.5 Mapa y navegador

`python -m atlas serve` con el binario compartido; Edge sin interfaz (puppeteer-core) con capturas en `docs/design/concepts/`: `01-overview.png`, `02-zoom-concepts.png`, `03-concept-panel.png` («integración»), `04-path.png`, `05-concept-panel-mobile.png` (390×844).  Cero errores de consola, cero respuestas HTTP ≥ 400, ningún `p=` en el texto de las vistas y ningún desbordamiento horizontal.  Al terminar no quedó `atlas.exe` ni Edge sin interfaz.

Teselas de la capa estructura (cliente HTTP con keep-alive, todas las teselas de cada zoom): z0, z1 y z2 tienen 5,373 nodos únicos: 4,074 concept, 605 area, 561 statement, 58 algorithm, 50 model, 25 conjecture; 0 declaraciones de Mathlib, 0 `mathlib-…` (directorios), 0 `source_fragment`, 0 `resource`.  Aristas: 1,024 en z0.  z3 ya trae 3,619 `mathlib-…` de zona y 0 declaraciones; z4 trae 11,634 declaraciones.  Los 694 nodos que no son concept ni area son entidades de Wikidata (teoremas, algoritmos) que son objeto de pertenencias; 44 etiquetas del z0 son números («10», «1105») o «Wikipedia».

### 13.6 Veredicto contra las reglas del plan

| Regla | Veredicto | Cifra y comentario |
|---|---|---|
| Asignación vs 355 curados | **FAIL** | test: recall@3 0.7638 [0.700, 0.818], pasa 0.75; precisión top-1 0.7143 [0.647, 0.773], no pasa 0.75 (trabajador: `data/cache/reports/concepts-run4.json`).  Dev: 0.8104 y 0.7321.  Se publica igual, con la confianza baja visible |
| Cobertura ≥ 85 % | PASS | 237,640 declaraciones de contenido: 90.15 % por señales propias y 100 % con respaldo de zona (9,202 de baja confianza y 14,201 con concepto derivado); 4,343 de infraestructura sin asignar (trabajador).  Reserva: el respaldo de zona asigna todo, y el plan pedía no forzar |
| Muestra auditada de 200 | PASS (procedimiento) | hecha y reportada en §13.4: 0.420 estricto, 0.675 laxo.  Provisional.  El resultado es malo y es el hallazgo principal de la ronda |
| Mapa | PASS | 5,373 ≤ 6,000 en z0–z2; sin declaraciones ni infraestructura; aristas agregadas con conteos por relación y estado; declaraciones desde z4.  Reserva: 694 nodos de la capa son entidades Wikidata de tipo statement, algorithm, model o conjecture |
| Recorridos | PASS (provisional) | AtlasBench v3 test sin regresión (§13.3).  Las 20 consultas las escribió un agente.  La web **no muestra** `PathResponse.concepts` (no hay rastro en `web/src/pathview.ts`): la ruta por conceptos solo existe en la API |
| Rendimiento | **FAIL** | `compile --all` 70.42 s contra 64.35 s (1.09×, pasa); p95 de recorridos 3.24 ms contra 5.73 ms (pasa en los 449 ítems fijos; con pares aleatorios de `bench.exe` el trabajador midió 1.7–2.9× por el snapshot nuevo); **p95 de teselas en AtlasBench 8.78 ms contra 2.57 ms = 3.4×, no pasa 2×**.  El trabajador del mapa midió 1.3× con `load.mjs` sobre 600 entidades (teselas profundas).  AtlasBench pide solo las 85 teselas z0–z3, y la z0 nueva pesa 523 KB |
| Reproducibilidad | PASS | `build7` dio `ad7463ed1d4fcce6`, el mismo hash de las seis compilaciones del trabajador |
| Integración no asignada ni enlazada a `IsIntegral` | PASS | registro: 0 pertenencias vigentes de declaraciones `IsIntegral*` a wd:Q80091 (de 1,394 vigentes); `formalized_as` hacia `mathlib:IsIntegral*`: solo 1, retractado |
| `integral`, `derivada` y `Bayes` en el top 3 | **FAIL** | `/api/search` en español: derivada, 1.º (wd:Q29175); Bayes, 2.º «teorema de Bayes»; **integral**: 5 declaraciones de Mathlib antes que cualquier concepto, sin concepto en los primeros 5.  En inglés `derivative` queda 5.º.  No se hizo trabajo de búsqueda en esta ronda (B01, B02) |
| Ninguna vista con `p=` | PASS | en las 5 vistas capturadas.  No es una revisión exhaustiva de toda la web |
| Capa general sin fuentes documentales ni infraestructura | PASS (con reservas) | 0 `source_fragment`, 0 `resource`, 0 directorios de Mathlib; quedan 44 etiquetas numéricas o «Wikipedia» |
| Todo verificado con capturas | **FAIL** (parcial) | hay capturas de la vista general, un zoom, el panel de concepto y móvil.  No hay captura de una ruta a nivel de concepto porque la web no la dibuja; `04-path.png` es la ruta de entidades y dice «Sin ruta autorizada» (alcance bibliográfico), aunque en la API los dos conceptos sí tienen ruta |

Resultado global: la capa conceptual se publica.  La asignación **no está aprobada** (precisión top-1 y auditoría).  Tres reglas quedan en FAIL (asignación, rendimiento de teselas, búsqueda) y falta una captura.

### 13.7 Pendiente y riesgos observados

1. Calidad de la asignación: los homónimos de `name_doc` (§13.4).  Nada se tocó en esta ronda.
2. Que la web dibuje `PathResponse.concepts` y use `drill_down`; que `atlas path` tenga `--concepts` y `--drill-down`.
3. Búsqueda: conceptos antes que declaraciones de biblioteca (B01, B02), sin dueño en esta ronda.
4. Teselas: la z0–z2 pesa 523 KB (p95 8.78 ms en AtlasBench).  Opciones: menos nodos en z0 o una tesela general separada.
5. Las respuestas grandes (≥ ~200 KB) se cortan con `ConnectionResetError` si el cliente manda `Connection: close` (`urllib` de Python lo hace siempre; reproducido en la tesela z0).  Con keep-alive, curl y navegador no falla.  Sin diagnosticar.
6. Las 20 consultas de concepto y la muestra de 200 siguen sin juez humano.
7. `data/index` conserva índices de versiones anteriores (derivados, se pueden borrar).
8. No se hizo commit.
