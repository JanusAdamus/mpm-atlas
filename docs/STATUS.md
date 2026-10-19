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

## 14. Ronda 3 · correcciones de la capa conceptual (2026-10-19, integrador)

Plan: `docs/plans/2026-10-18-ronda-3-correcciones.md`.  Las varas no se movieron.  Cifras del integrador con su comando; las de los trabajadores se marcan «(trabajador)».  No se hizo commit.

### 14.1 Qué se hizo

- Asignación (trabajador): proyector v8 y `resolve/3`; filtro de no matemáticos más fino; nombres de 1–2 caracteres sin señal; secundarias sin señal independiente retiradas como `superseded` con historial; respaldo por zona rotulado.  Reporte `data/cache/reports/concepts-run8.json`.
- Búsqueda (trabajador): carril `concept` para nombres cotidianos y escaneo semántico exacto por defecto (`ATLAS_DENSE=hnsw` lo regresa).
- Mapa, servidor y web (trabajador): presupuesto por tesela (400 nodos generales, 384 aristas), cierre de conexión que espera al cliente (`linger.rs`), ruta por conceptos con «Bajar a declaraciones», CLI `atlas path --concepts --drill-down`.
- Integrador: recompiló, midió y juzgó.  No cambió código.

### 14.2 Build y pruebas

| Comando | Resultado |
|---|---|
| `cargo build --release -p atlas-server` (`.cargo-target/atlas`) | 3 min 20 s |
| `python -m atlas build --skip-ingest --no-cargo --json` (`data/cache/reports/build-logs/build8.json`) | `compile_wall_s` 97.07 s (índice 64.56 s, layout 2.63 s).  Snapshot `4058b8ab7b3d946c…` (405,607,212 bytes; 5,715 conceptos, 75,051 aristas conceptuales, 237,640 declaraciones con concepto), 320,214 nodos |
| `atlas compile --all --json` repetido (`build-logs/compile-r3-2.json`) | 86.5 s; mismo `content_hash` 4058b8ab7b3d946c3f62680640e5a02d5c03d0ad8da1b64e30b61dceb15f3082 |
| `python -m pytest -q` | 166 passed en 38.63 s |
| `cargo test --workspace --release` (`build-logs/cargo-test-r3.txt`) | 147 passed, 0 failed |
| `cargo clippy --workspace --all-targets --release` | 0 advertencias |
| `npm test` / `npx tsc --noEmit` / `npm run build` (en `web/`) | 17 de 17 / sin errores / verde en 792 ms, JS principal 101.09 kB (33.50 kB gzip) |

Nota: el id del layout cambió entre las dos compilaciones (`3ca35abe…` y `88141d97…`) porque el layout arranca en caliente desde el anterior y su id depende de eso.  El snapshot, que es lo que fija la regla, es idéntico.  `data/layout/CURRENT` quedó en `88141d9755376d46`.

### 14.3 AtlasBench v3 test (HTTP) y las 20 consultas

`ATLAS_BIN=…/atlas/release/atlas.exe python -m bench run --split test --serve --skip-cli --out bench/runs/2026-10-19-r3-test-integrated` (179.45 s; log `build-logs/bench-r3.txt`).

| Métrica (HTTP, test) | Ronda 2 (§13.3) | Ahora |
|---|---|---|
| acierto en respondibles | 260/261 | 260/261 = 0.996 [0.979, 0.999] |
| abstención en respondibles | 1/261 | 1/261 = 0.004 [0.001, 0.021] |
| adversariales sin conclusión prohibida | 188/188 | 188/188 [0.980, 1.000] |
| estructura inválida | 0 | 0 de 264 respuestas |
| retrieval en/text nDCG@10 | 0.771 [0.735, 0.807] | **0.816 [0.783, 0.847]** (n = 420) |
| retrieval es/text nDCG@10 | 0.861 (trabajador, ronda 2) | 0.889 [0.832, 0.937] (n = 41) |
| `http.tiles.load_c1` p95 | 8.78 ms [7.63, 15.19] | **22.61 ms [19.71, 27.45]** |
| `http.search.load_c1` p95 / c8 | 11.75 / 62.24 ms | 28.04 / 92.82 ms |
| `http.path` p95 | 3.24 ms | 9.93 ms |

Veredictos del bench: retrieval PROVISIONAL-FAIL (regla del bench sin cambio; ver §13.3), routes.structure PASS, routes.adversarial PROVISIONAL-PASS, provenance PASS, las 5 latencias PASS contra sus propios límites.  `routes.null_model_adversarial` PROVISIONAL-FAIL: el control (BFS del registro) sobreafirma 5/188 (ronda 2: 2/188).  Todas las latencias subieron frente a la ronda 2.  Un `/api/health` da p50 2.52 ms y p95 3.55 ms (300 peticiones) en esta máquina; no se pudo medir la ronda 2 al mismo tiempo, así que la comparación de latencia no es limpia.

20 consultas de concepto: `python -m bench.concept_routes --http http://127.0.0.1:8770 --out bench/runs/2026-10-19-concept-routes-20-r3`.  Respondibles 14/14 [0.785, 1.000]; abstención en respondibles 0/14; estructura 16/16; sobreafirmación frente al oráculo 0/16; adversariales sin conclusión prohibida **9/10 [0.596, 0.982]** (ronda 2: 10/10); abstención en no respondibles 4/6 (ronda 2: 5/6); p50 4.40 ms, p95 10.44 ms (n = 20).  **Regresión nueva:** `hw:concept-route:017:flt-jensen` (Último teorema de Fermat → desigualdad de Jensen) antes se abstenía y ahora devuelve una ruta de 4 pasos con conclusión «apoyo lógico» (`wd:Q132469 → wd:Q812880 (cubo) → wd:Q875399 → wd:Q320357 → wd:Q518131`, evidencia más débil `model_proposed`), marcada como violación prohibida.  La causa aparente es el cambio de asignación, no el motor de recorridos: las declaraciones de `FermatLastTheoremForThreeGen.*` quedan asignadas a «Fermat's Last Theorem» y a «cube».  No se investigó más ni se tocó código.  En las consultas 002 y 007 cambió el concepto intermedio (de `wd:Q122889481` a `wd:Q2672873`; de `wd:Q2000555` a `wd:Q2141130`) y siguen contestando.

### 14.4 Auditoría independiente de la muestra r3 (juicio de agente, provisional)

Archivo: `bench/concepts/audit-sample-200-r3.judged.jsonl` (el original queda intacto).  Lo juzgó el integrador, que no construyó la muestra ni el proyector.  Mismo criterio que §13.4 (correcta si la declaración trata de ese concepto; incorrecta si es otro; dudosa si es generalidad o relación laxa), con etiqueta, nombre Lean, módulo y docstring, sin consultar Wikidata.  Un solo juez, sin acuerdo entre jueces.  La semilla y la proyección son otras, así que no es comparación pareada con §13.4.

| Medida | k / n | Valor [Wilson 95 %] |
|---|---|---|
| correctas (estricto: las dudosas no cuentan) | 70 / 200 | 0.350 [0.287, 0.418] |
| correctas o dudosas (laxo) | 146 / 200 | 0.730 |
| totales | 70 correctas, 54 incorrectas, 76 dudosas | |
| solo primarias (rango 0), estricto | 48 / 126 | 0.381 [0.301, 0.468] (laxo 0.770) |
| rango 1 / rango 2, estricto | 16 / 56 ; 6 / 18 | 0.286 ; 0.333 |
| señal `name_doc` | 47 / 138 | 0.341 (42 incorrectas) |
| `namespace` | 11 / 22 | 0.500 |
| `signature` | 4 / 9 | 0.444 |
| `zone` | 0 / 12 | 0.000 (8 dudosas, 4 incorrectas) |
| `zone_derived` | 8 / 16 | 0.500 (0 incorrectas) |
| `resolve` | 0 / 3 | 0.000 (3 dudosas) |

Lo que sigue fallando: homónimos de palabra suelta de `name_doc` con la etiqueta de un concepto de Wikidata («nat» unidad de información, «unit», «FinSet» para `Set.*`, «A-Integrable», «tombstone» por «Halmos», «well equidistributed long-period linear» por «well», «∂∂̄ lemma» por «lemma», «inverse limit» por `inverse`).  Muchas dudosas son conceptos genéricos («set», «function», «integer») que son verdad y no enseñan nada.  Por eso el estricto es bajo aunque el laxo (0.730) no baja frente a la ronda 2 (0.675).

### 14.5 Mapa, servidor y navegador

- Teselas (servidor `serve --addr 127.0.0.1:8770`, cliente HTTP con keep-alive): z0 en JSON 131,026 bytes (ronda 2: 1,416,991) y en binario 37,880.  La más pesada de las teselas z0–z3 es la 3/3/3 con 270,952 bytes en JSON.  `python web/bench/tiles_mix.py --http http://127.0.0.1:8770 --runs 3`: p95 20.03 y 17.40 ms en las dos primeras corridas.  Una tesela z3 casi vacía (187 bytes) da p50 2.68 ms y p95 3.49 ms; z0 da p50 8.20 y p95 26.62 ms; z3/3/3 da p50 10.09 y p95 30.97 ms (200 peticiones cada una).  El trabajador midió p95 0.64 ms dentro del proceso para generar la tesela y su JSON.  El tiempo crece con los bytes, pero no se aisló cuánto es cliente de Python y cuánto servidor; no se comparó contra el layout de la ronda 2 porque ese layout no carga con el snapshot nuevo.
- Capa general (z0–z2, capa `structure`, todas las teselas): 3,227 nodos únicos (≤ 6,000).  Por `type`: concept 2,639, area 340, statement 207, model 21, algorithm 15, conjecture 5.  0 etiquetas numéricas, 0 con año, «año», «siglo» o «century», 0 «Wikipedia», 0 fuentes, personas o infraestructura por `kind`.  Reserva: hay etiquetas de palabra suelta de dudoso valor educativo («red», «haz», «luz», «era», «fin», «más», «NaN», «cut», «ALL»); la regla no las cubre.  Los 4 nodos de fecha que el trabajador del mapa dejó pendientes ya no están.
- Edge sin interfaz (`node web/bench/r3shots.mjs --base http://127.0.0.1:8770 --from "wd:Q505798|teorema de Lagrange" --to "wd:Q83478|grupo" --out ../docs/design/r3`, salida 0): 2 pasos de concepto, 1 paso al bajar a declaraciones, `aria-expanded` pasa a true y vuelve a false con teclado, 412 elementos en la vista de texto, 0 px de desbordamiento a 390×844, 0 errores de consola, 0 respuestas HTTP ≥ 400 y ningún `p=` en el texto visible (el guion sale con 1 si ocurre cualquiera).  Capturas en `docs/design/r3/` (01 a 05).  Para ese par la API dice «Sin ruta autorizada» a nivel de entidad y a la vez da la ruta por conceptos; la pantalla muestra los dos bloques.
- Búsqueda (`/api/search`, `limit=5`): `integral` 1.º wd:Q80091 en ES y en EN; `derivada` 1.º wd:Q29175; `derivative` 1.º; `Bayes` 2.º «teorema de Bayes» wd:Q182505 en ES y en EN (el 1.º es la persona Thomas Bayes).
- Homónimos en el registro real (`homonyms.py` del trabajador de asignación, solo lectura): `ball` → «snooker ball»: 0 de 110; `M` → «Mandelbrot set»: 0 de 69; `AtPrime` → «prime number»: 0 de 67; `IsIntegral*` → integración: 0 de 202.  `python -m pytest -q tests/test_concepts.py`: 19 passed (13 fallan con el código anterior, según el trabajador).
- Al terminar: mi servidor del puerto 8770 cerrado por PID; no queda `atlas.exe`.

### 14.6 Veredicto contra las reglas del plan

| Regla | Veredicto | Cifra y comentario |
|---|---|---|
| Asignación vs 355 curados (test, segunda lectura) | **FAIL** | top-1 0.7041 [0.637, 0.764] y recall@3 0.7337 [0.668, 0.790], contra 0.75 en los dos (`concepts-run8.json`, trabajador).  Dev: 0.7512 y 0.8104.  La mitad test ya se había usado |
| Auditoría nueva de 200 | **FAIL** | estricta 0.350 [0.287, 0.418] contra 0.60; primarias 0.381 contra 0.70 (§14.4).  Provisional, un juez |
| Homónimos | PASS | pruebas de regresión que fallan con el código anterior y 0 casos en el registro (§14.5).  Costo conocido: `Real.pi` pierde «pi» por la regla de 1–2 caracteres |
| Capa general | PASS | 3,227 ≤ 6,000; 0 fuentes, números, fechas, personas o infraestructura (§14.5).  Reserva: palabras sueltas |
| Cobertura con señal propia | **FAIL** | 191,172 de 237,640 = 80.45 % contra 85 % (ronda 2: 90.15 %).  Se reporta y no se rellena con la zona (20,992 + 25,476 solo con respaldo por zona) |
| Búsqueda | PASS | los tres conceptos en el top 3 (integral 1.º, derivada 1.º, Bayes 2.º); en/text 0.816 ≥ 0.787 |
| Teselas | **FAIL** | z0 131,026 bytes ≤ 250 KB pasa; p95 en AtlasBench 22.61 ms contra ≤ 5.14 ms no pasa.  La máquina está más lenta que en la ronda 2 (§14.3), pero no pasa en cifras absolutas |
| Servidor | PASS | `core/atlas-server/tests/close.rs` (respuesta de 600 KB con cierre) pasa dentro de `cargo test --workspace`; el trabajador midió que falla con el cierre anterior (ECONNRESET a los 18.9 s) y pasa en 0.63 s |
| Recorridos | PASS con reserva | AtlasBench v3 test: abstención 1/261, sobreafirmación 0, adversariales 188/188; 20 consultas 14/14 respondibles.  Reserva: en las 20, adversariales 9/10 por la consulta 017 (§14.3) |
| Web | PASS | ruta por conceptos dibujada y bajada a declaraciones, captura, 0 errores de consola, sin `p=`, 0 px de desbordamiento (§14.5) |
| Reproducibilidad y rendimiento | PASS | mismo `content_hash` en dos compilaciones; `compile --all` 97.07 s y 86.5 s contra tope de 128.7 s (2 × 64.35 s) |

Resultado global: 4 reglas en FAIL (asignación, auditoría, cobertura, teselas) y 7 en PASS (una con reserva).  La capa conceptual queda publicada en `data/` con la confianza baja visible; la asignación sigue **no aprobada**.

### 14.7 Pendiente y riesgos observados

1. Calidad de asignación: los homónimos de palabra suelta de `name_doc` siguen siendo el error dominante (§14.4).  Retirar secundarias y homónimos bajó la cobertura con señal propia a 80.45 %; la regla del plan es no rellenar.  Decisión del dueño: si «pi» merece una excepción a la regla de 1–2 caracteres.  **Decidido el 2026-10-19 (Adrián): sí.**  `resolve/4` agrega `SHORT_LABEL_OK = {"pi"}` con su prueba (`test_resolve_keeps_pi_despite_the_short_label_rule`, falla sin la excepción).  Aplicado al registro el mismo día como proyección v9 (§14.8)
2. Teselas: la meta de 5.14 ms no se alcanza; falta separar cuánto es transferencia (z3 pesa hasta 271 KB) y cuánto servidor, y medir en una máquina sin otros procesos.
3. Consulta 017 (Fermat → Jensen) como regresión de recorridos por la nueva asignación.  Las 20 consultas siguen escritas por un agente.
4. Etiquetas de palabra suelta en la capa general («red», «luz», «era», «NaN»).
5. `null_model_adversarial` del bench pasó de 2/188 a 5/188 (es el control, no una regla).
6. `data/index`, `data/layout` y `data/snapshot` conservan versiones anteriores (derivados, se pueden borrar).
7. Latencias del bench más altas que en la ronda 2 (§14.3); conviene repetir con la máquina libre.
8. No se hizo commit.  Sin versionar: `bench/concepts/audit-sample-200-r3.judged.jsonl`, `bench/runs/2026-10-19-*`, `docs/design/r3/`.

### 14.8 Proyección v9: excepción de «pi» (2026-10-19, orquestador)

`python -m atlas.concepts --sample <scratch>/sample-v9.jsonl --seed r3 --report data/cache/reports/concepts-run9.json`.  La muestra r3 juzgada (`bench/concepts/audit-sample-200-r3.judged.jsonl`) se conserva y no se regeneró.

| Cifra | v8 (§14) | v9 |
|---|---|---|
| top-1 dev | 157/209 = 0.7512 | 158/209 = 0.7560 [0.694, 0.809] |
| recall@3 dev | 171/211 = 0.8104 | 172/211 = 0.8152 [0.757, 0.862] |
| top-1 test (segunda lectura) | 138/196 = 0.7041 | 138/196 = 0.7041 [0.637, 0.764] |
| recall@3 test | 146/199 = 0.7337 | 146/199 = 0.7337 [0.668, 0.790] |
| Pertenencias vigentes | 377,729 | 377,744 |
| Con señal propia | 191,172 (80.45 %) | 191,177 (80.45 %) |
| `mathlib:Real.pi` | sin «pi» | → wd:Q167 «pi» (34 declaraciones en ese concepto) |

Escritura: 28 afirmaciones nuevas, 5,546 reemplazadas (`superseded`, con historial; la explicación de la señal `resolve` nombra `resolve/4`), 2 secundarias retiradas, 11 retractadas por obsoletas.  Los veredictos de §14.6 no cambian: asignación en test y cobertura siguen en FAIL.

`python -m atlas build --skip-ingest --no-cargo --json` (`build-logs/build9.json`, 82.27 s): snapshot `a2e90658285b7ca5…` (406,008,820 bytes; 5,714 conceptos; 75,069 aristas conceptuales; 237,640 declaraciones con concepto), 320,214 nodos.  `python -m pytest -q`: 167 passed.  Una sola compilación: la reproducibilidad de este hash no se repitió, y AtlasBench no se volvió a correr.

## 15. Ronda 4 · diseño para una persona común (2026-10-19, integrador)

Plan: `docs/plans/2026-10-19-ronda-4-diseno.md`.  Las reglas se juzgan tal como quedaron fijadas; no se movió ninguna vara.  Cifras del integrador con su comando o archivo; las de los trabajadores van marcadas «(trabajador)».  El integrador no construyó nada de esta ronda.  No se hizo commit.

### 15.1 Qué se hizo

- **Guía de diseño.**  `docs/design/r4/guia-skills-diseno.md`: lectura de 38 skills de diseño de siete repositorios de terceros (Emil Kowalski, Jakub Krehel, ConardLi, elayadesign, codeswithroh, Owl-Listener, MengTo), destilada en un sistema para el «atlas impreso».  Se leyeron como referencia: no se ejecutó ningún script, no se instaló nada y no se siguió ninguna instrucción de esas skills.  Alertas de seguridad (guía §4): tastemaker pide escribir memoria en `~/.tastemaker/`, fuera del repo, trae 23 scripts Python que salen a la red (openverse, iconify, pixabay con clave), manda instalar CLIs que configuran el agente y servidores MCP con token (`npx @21st-dev/cli init`, `--github-api-key`) y copiar la skill a `~/.claude/skills/`; garden-skills manda descargar video con `yt-dlp`, cargar scripts de CDN sin integridad e instalar skills con `npx` o `curl`, se activa sola en Codex y dice derivar del prompt de sistema de otro producto; MengTo trae un `CLAUDE.md` ajeno que pide commit y `cd` a otra máquina, y una skill que borra temporales y hace commit con un índice alterno; otras piden agregar servidores MCP de navegador, buscar en la web o meter controles de demostración en el producto.  No se encontró texto dirigido a cambiar las instrucciones del agente.
- **Sistema de diseño** (trabajador): `web/src/tokens.ts` (paleta de papel y tinta en claro y oscuro, pares de contraste comprobados en `tokens.test.ts`, estados de evidencia con trazo, glifo y nombre), `web/src/style.css`, `web/src/panels.css`, `web/src/theme.ts`.  Capturas en `docs/design/r4/sistema/`.
- **Mapa** (trabajador): `web/src/map/` (presupuesto de etiquetas por zoom y rejilla de colisión, foco con velo por tinta, tramado de «desconocida», leyenda siempre visible, trazos por estado de evidencia).  Capturas en `docs/design/r4/mapa/`.
- **Recorridos y paneles** (trabajador): inicio con la pregunta, el buscador y tres puertas; modos Explorar y Técnico; ficha de cinco preguntas en orden fijo con estado vacío honesto; ruta paso a paso con teclado; vista de texto.  Capturas en `docs/design/r4/paneles/`.
- **Servidor** (trabajador): `core/atlas-server/src/concept.rs` y `entity.rs` (datos que piden las fichas y las puertas) y `tests/cli.rs`, que ahora lanza `atlas.exe` con `CREATE_NO_WINDOW` para no abrir una ventana por corrida en Windows.
- **Dos vueltas de arreglos.**  Crítica externa `docs/design/r4/critica/2026-10-19-critica-r4.md` (hallazgos C4-01 a C4-19; 6 de 10 reglas en PASS); segunda vuelta de mapa y paneles con capturas antes y después en `docs/design/r4/fix2/` (trabajador).
- **Integrador:** pruebas, AtlasBench, teselas y segunda pasada de los scripts del crítico, copiados a `docs/design/r4/final/scripts/` con la salida apuntando a `final/`.  Añadió `c4.mjs`, que mide los hallazgos que los scripts del crítico no miden, y `veil.mjs`, que mide la tinta de las etiquetas bajo el velo.  No hizo falta ningún arreglo de integración.

### 15.2 Build y pruebas

Logs en `data/cache/reports/build-logs/`.

| Comando | Resultado |
|---|---|
| `python -m pytest -q` (`pytest-r4.txt`) | 167 passed en 24.67 s |
| `cargo test --workspace --release` en `core/` con `CARGO_TARGET_DIR=C:/Users/AdrianGonzalez/.cargo-target/atlas` (`cargo-test-r4.txt`) | 147 passed, 0 failed, 41.2 s.  Solo se recompiló la prueba `cli`: `compile_stats_layout_and_error_envelope` (`tests/cli.rs`, con `CREATE_NO_WINDOW`) compila y pasa en 6.22 s.  Cargo no tuvo que reemplazar `atlas.exe`, así que el servidor del usuario no se detuvo |
| `cargo clippy --workspace --all-targets --release` (`clippy-r4.txt`) | 0 advertencias |
| `npm test` en `web/` (`npm-test-r4.txt`) | 86 de 86 |
| `npx tsc --noEmit` (`tsc-r4.txt`) | sin errores |
| `npm run build` (`vite-build-r4.txt`) | verde en 636 ms; JS principal 151.34 kB (50.19 kB gzip), CSS 33.38 kB; KaTeX aparte (`math-*.js`, 259.05 kB) |

### 15.3 AtlasBench y teselas

AtlasBench v3 test (HTTP), salida completa en `bench/runs/2026-10-19-r4-test-integrated/` (`report.md`, `report.json`, `route_outputs.jsonl`; reporte de las 17:51:04).  No se volvió a correr.  Snapshot `a2e90658285b7ca5…`, el mismo de §14.8.

| Métrica (HTTP, test) | Ronda 3 (§14.3) | Ronda 4 |
|---|---|---|
| acierto en respondibles | 260/261 | 260/261 = 0.996 [0.979, 0.999] |
| abstención en respondibles | 1/261 | 1/261 |
| adversariales sin conclusión prohibida | 188/188 | 188/188 [0.980, 1.000] |
| estructura inválida | 0 de 264 | 0 de 264 |
| retrieval en/text nDCG@10 | 0.816 | 0.815 [0.783, 0.847] (n = 420) |
| retrieval es/text nDCG@10 | 0.889 | 0.889 [0.832, 0.937] (n = 41) |
| `http.tiles.load_c1` p95 | 22.61 ms [19.71, 27.45] | **9.44 ms [9.12, 9.86]** |
| `http.search.load_c1` p95 / c8 | 28.04 / 92.82 ms | 19.24 / 90.38 ms |
| `http.path` p95 | 9.93 ms | 9.42 ms |

Veredictos del bench sin cambio: retrieval PROVISIONAL-FAIL, routes.structure PASS, routes.adversarial PROVISIONAL-PASS, provenance PASS y las 5 latencias PASS; el control `routes.null_model_adversarial` sigue en 5/188.  La ronda no tocó búsqueda ni recorridos, así que era lo esperado.  La bajada de las latencias frente a la ronda 3 no se atribuye a nada: la máquina estaba menos cargada y no se midieron las dos a la vez.

Teselas: `python web/bench/tiles_mix.py --http http://127.0.0.1:8765 --runs 3` (`tiles-mix-r4.txt`; 85 teselas, 500 peticiones por corrida, 0 errores): p95 12.28 [10.52, 15.61], 11.17 [10.10, 15.70] y 17.54 [14.98, 22.72] ms; p50 6.50, 6.33 y 6.31 ms.  z0 pesa 131,032 bytes en JSON y 37,880 en binario; la más pesada es 3/3/3 con 269,943 bytes.  Tope de la regla: 1.1 × 22.61 = 24.87 ms.  El crítico, con la instancia compartida, midió 14.17, 25.62, 10.57, 11.26, 12.93 y 17.40 ms (`docs/design/r4/critica/tiles_mix*.json`); la corrida de 25.62 ms fue la única por encima del tope.

### 15.4 Segunda vuelta de las reglas del plan

Edge sin interfaz con puppeteer-core contra `http://127.0.0.1:8765` (`web/dist` construido con el mismo código que se probó), 1440×900 y 390×844, claro y oscuro, Explorar y Técnico, con y sin `prefers-reduced-motion`.  Scripts y salidas en `docs/design/r4/final/scripts/` (`run.sh`, `run2.sh` y las corridas sueltas de `contrast`, `kb`, `mobile`, `mobiledoor`, `expand`, `unknown`, `unknown-deep`, `c4` y `veil`).  Capturas en `docs/design/r4/final/`: `p1-*` y `p2-*` repiten las del crítico y `c4-*` son del integrador.

| Regla | Veredicto | Evidencia |
|---|---|---|
| Jerga en modo Explorar | **PASS** | 0 apariciones de la lista del plan en el inicio (4 configuraciones × 2 pasadas, `out-home-1/2.json`), en la búsqueda y la ficha de los 7 conceptos (ES 1440 claro ×2, ES 390, ES oscuro, EN; `out-concepts-*.json`), en las fichas con todos los grupos desplegados (`log-expand.txt`, 0 en 7 de 7), en las tres puertas paso a paso (6 corridas, `out-doors-*.json`) y en la vista de texto (`out-kb-*.json`).  En Técnico aparecen 3 en la ficha de integración, como corresponde |
| Etiquetas | **PASS** | Vista inicial: 30 etiquetas a 1440 y 10 a 390, claro y oscuro, dos pasadas (`out-home-*.json`).  0 solapamientos de cajas en 10 pasos de zoom, con teselas z3 a z7 pedidas, en claro, oscuro, Técnico, movimiento reducido y 390 (`out-map-*.json`).  Reserva: las etiquetas todavía caen sobre puntos (C4-14) |
| Contraste | **PASS** | HTML: 0 nodos de texto bajo 4.5:1 en 48 vistas; mínimos 6.06 (claro) y 5.77 (oscuro) en Explorar, 5.08 y 4.95 en Técnico (`log-contrast.txt`, `out-contrast-1.json`).  Mapa: las etiquetas bajo el velo se pintan ahora con `text-muted`, medido en las llamadas a `fillText`: #655f53 sobre #f4f1e8 = 5.61:1 y #9c9483 sobre #14130f = 6.18:1, en las fichas de Pitágoras e integración y en la ruta derivada → integración (`out-veil.json`).  Puntos velados (`text-veiled`): 3.78:1 y 3.95:1 (≥ 3:1), calculado sobre los tokens como pide la regla y comprobado en `web/src/tokens.test.ts`; no se volvió a medir en píxeles |
| Color | **PASS** | Cada salto se dibuja con su estado, medido en las llamadas a `stroke()` del lienzo (`out-c4-light.json`; en oscuro, el mismo patrón con #8a8374 y #c6bea9 en `out-c4-dark.json`): la puerta de cálculo sale con el punteado gris de «Candidata» (`[1.2, 1.8]`, #857e70) y las de primos y Pitágoras con el continuo de «Documentada» (#48443b) y el punteado de candidata.  Ya no aparece el discontinuo de «Histórica».  La leyenda explica los puntos («Los puntos son de un solo color; la forma dice qué es: ◎ concepto, ◯ región, ● otra idea»).  El dorado solo marca «Validada».  Reserva: «Candidata» y «Retirada» comparten punteado y se distinguen por el glifo (? y ×) |
| Ficha | **PASS** | Las cinco secciones `section.q5` (what, why, before, links, check), en ese orden, en los 7 conceptos, ES y EN, 1440 y 390, claro y oscuro, dos pasadas (`log-c*.txt`).  Las vacías dicen «Todavía no hay una explicación revisada», «…prerrequisitos registrados…» o «…ejercicios revisados…» |
| Puertas guiadas | **PASS** | Solo con teclado (Tab hasta la puerta, Enter, → por los pasos): las tres abren la tarjeta en 172–222 ms, el foco cae en el stepper, paradas 1–2 y 1–4 en el mapa, 1 de 1 y 3 de 3 pasos con →, a 1440 y 390, claro y oscuro, con y sin movimiento reducido (`log-d*.txt`).  En móvil la ruta queda dibujada en el lienzo (`log-md-*.txt`).  Reserva: mientras se lee la tarjeta en móvil, el mapa queda fuera de la pantalla, arriba (y = −376 a −532) |
| Desconocido | **PASS** | Relleno y tramado a 45° con «Desconocida» en la leyenda; 24 regiones a medio paso de zoom en el borde izquierdo, en oscuro (`log-u2.txt`).  Ninguna se rotula «aislada».  Reserva: 0 en la vista inicial (`out-unknown-light.json`, C4-15) |
| Movimiento | **PASS** | Ninguna transición CSS pasa de 300 ms en las 48 vistas (`log-contrast.txt`, `durs>300 0`).  Vuelos: un solo episodio de 300–483 ms al abrir puertas y rutas por formulario o URL (`log-d*.txt`, `log-m*.txt`), bajo 900 ms.  Con movimiento reducido, primos y Pitágoras cambian en 1 cuadro; derivada → integración, en 3–7 cuadros durante 73–177 ms, que coincide con la llegada de teselas y no con un vuelo |
| Prueba de usuario común | **FAIL** | Las 5 tareas se completan, pero las cinco preguntas se responden en **0 de 7** conceptos, y el plan pide ≥ 5 (§15.5).  Juicio de agente, provisional |
| Sin regresión | **PASS** | pytest 167, cargo test 147/0, clippy 0, npm test 86/86, tsc y build en verde (§15.2).  0 errores de consola, `pageerror` ni HTTP ≥ 400 en todas las corridas.  0 px de desbordamiento de página a 390×844, y la barra `.tools` cabe (366 de 366 px, en dos filas; `out-kb-1-light.json`).  Ningún `p=` en el texto guardado.  p95 de teselas en AtlasBench 9.44 ms, con tope de 24.87 |

Resultado: 9 reglas en PASS (cuatro con reserva) y 1 en FAIL (usuario común).  Frente a la crítica pasan ahora contraste, color y sin regresión.

### 15.5 Prueba de usuario común (segunda vuelta)

El integrador no construyó nada de la ronda, pero ya había leído la crítica, así que no es «un agente nuevo» en sentido estricto.  La repetición usa los mismos scripts y pasos que el crítico.

1. **Entender el sitio.**  Sí: pregunta, buscador y tres puertas, y la leyenda ya explica los puntos.  Siguen los dos buscadores.
2. **Buscar los 7 términos.**  El primer resultado es el esperado en 6 de 7, en ES y EN, entre 120 y 194 ms.  «Bayes» sigue dando primero «Thomas Bayes | PERSONA» y en tercer lugar «Bayes estimator», en inglés (C4-19).
3. **Cinco preguntas.**  «Qué es» se responde en los 7; número primo ya trae el extracto de Wikipedia («únicamente dos divisores positivos distintos: él mismo y el 1»).  «Por qué importa», «Qué saber antes» y «Cómo compruebo que lo entendí» siguen vacíos en los 7, con el aviso honesto.  Bayes muestra «Usos registrados en las fuentes: … se usa en estadística bayesiana», que no explica para qué.  «Con qué se conecta» agrupa ahora por destino («3 registros en las fuentes») y marca el conflicto de Pitágoras.  Con las cinco respondidas: 0 de 7.
4. **Rutas y puertas.**  Cálculo: un paso «Candidata» con trazo punteado.  Primos y Pitágoras: tres pasos por artículos, con las paradas rotuladas «Artículo de Wikipedia: …» en el mapa y en la tarjeta, y el aviso «solo exploración, no autoriza conclusión».  La puerta «Primos y criptografía» sigue sin decir por qué los primos importan en criptografía.
5. **Teclado, vista de texto e idioma.**  12 Tab hasta el lienzo.  `T` lleva el foco a «Índice de conceptos», el siguiente Tab llega a «derivada», y al volver el foco regresa al lienzo.  Pasar de EN a ES ya no deja «5 results».  Para llegar a la primera puerta hacen falta 18 Tab en escritorio y 16 en móvil (antes 16 y 14).

### 15.6 Hallazgos de la crítica

| id | Estado | Evidencia |
|---|---|---|
| C4-01 ruta candidata con trazo continuo | **Arreglado** | punteado `[1.2, 1.8]` #857e70 (`out-c4-light.json`, `c4-door_calculus-1440-light.png`) |
| C4-02 rutas documentales con trazo «Histórica» | **Arreglado** | continuo de «Documentada» y punteado de candidata, sin discontinuo largo (`c4-door_primes-1440-light.png`) |
| C4-03 Pitágoras generaliza el quinto postulado y es caso particular de él | **Arreglado** | bloque «Relaciones en conflicto: la fuente registra esta relación en los dos sentidos», con sello «! En conflicto» (`c4-ficha-pitagoras-1440-light.png`).  La causa en el registro (P7719 en los dos ítems) sigue ahí |
| C4-04 definición de número primo | **En parte** | ya hay extracto de Wikipedia correcto; la descripción de Wikidata («solo tiene dos divisores enteros») y la fórmula `τ(p) = 2` siguen a la vista en Explorar |
| C4-05 puertas documentales | **En parte** | paradas rotuladas «Artículo de Wikipedia: …»; la puerta no lleva subtítulo y el recorrido no dice qué tienen que ver primos y criptografía |
| C4-06 «pi lleva el nombre de Ludolph van Ceulen» | **Sigue** | ahora dice «Según Wikidata, pi lleva el nombre de Ludolph van Ceulen»: queda atribuido a la fuente, pero sigue insinuando que π se llama así por él |
| C4-07 «Conexiones establecidas» | **Arreglado** | «Conexiones registradas.  Todas vienen de las fuentes citadas y nadie las ha revisado todavía» |
| C4-08 etiquetas veladas a 3.78:1 | **Arreglado** | 5.61:1 y 6.18:1 (`out-veil.json`) |
| C4-09 «5 results» al volver a ES | **Arreglado** | `englishLeftAfterBack: []`; la región viva dice «4 resultados» (`out-kb-1-light.json`) |
| C4-10 «1 pasos» | **Arreglado** | «Ruta lista: 1 paso.» |
| C4-11 la barra de herramientas desborda a 390 | **Arreglado** | `.tools` mide 366 de 366 px, en dos filas, con el tema a la vista (`c4-movil-inicio-light.png`).  El marcador del buscador de la cabecera sigue cortado en «( /» |
| C4-12 mapa móvil fuera de la vista inicial | **Sigue** | el lienzo empieza en y = 701 y se ven 143 de sus 354 px, menos que antes (207) (`out-mobile-*.json`) |
| C4-13 rótulos en inglés y no matemáticos | **Sigue** | «graph property» y «moneda» en la vista inicial; «revista científica», «objeto físico» y «meridiano» en la ruta (`out-map-*.json`, `out-c4-light.json`) |
| C4-14 etiquetas sobre puntos y paradas encimadas | **Sigue** | etiquetas sobre puntos en la ficha de Pitágoras; en móvil las paradas 1 y 4 de la ruta de Pitágoras siguen a 5 px (306,220 y 301,217; `log-md-door_pythagoras.txt`) |
| C4-15 «desconocida» invisible en el panorama | **Sigue** | 0 regiones en la vista inicial; aparecen a medio paso de zoom (`log-u2.txt`).  Necesita el servidor (§15.7) |
| C4-16 glifo ⧗ ilegible | **Arreglado** | «◷ Histórica» en la leyenda |
| C4-17 conexiones repetidas | **Arreglado** | agrupadas por destino, con «3 registros en las fuentes» (matriz, derivada) |
| C4-18 Tab hasta la primera puerta; objetivos táctiles | **En parte** | en móvil los botones miden 44 px de alto (EN, 38 px de ancho); hasta la primera puerta hacen falta 18 Tab en escritorio y 16 en móvil, dos más que antes |
| C4-19 «Bayes» da la persona primero | **Sigue** | «Thomas Bayes \| PERSONA», «teorema de Bayes», «Bayes estimator» (`log-c*.txt`).  Necesita el ranking del servidor |

Arreglados 10 (C4-01, 02, 03, 07, 08, 09, 10, 11, 16 y 17), en parte 3 (C4-04, 05 y 18) y siguen 6 (C4-06, 12, 13, 14, 15 y 19).  De los siete de prioridad A (lo que puede enseñar algo falso), C4-06 sigue abierto y C4-04 y C4-05 lo están en parte.

### 15.7 Pendiente para el servidor

1. Filtro `relation` en `/api/entity`, para que la ficha pida las relaciones de cada pregunta en vez de recibir 200 conexiones y repartirlas.
2. Teselas: idioma por etiqueta, para no rotular en inglés en Explorar (C4-13), y «sin relaciones extraídas» por elemento, calculado desde el registro y no desde lo que cupo en la tesela (C4-15).
3. Búsqueda de «Bayes»: en Explorar, personas y fragmentos de fuente debajo de conceptos y enunciados (C4-19).

### 15.8 Lo que pasa a la ronda 5

La prueba de usuario común no pasa sin contenido revisado.  Tres de las cinco preguntas («Por qué importa», «Qué saber antes» y «Cómo compruebo que lo entendí») están vacías en los 7 conceptos, y eso no lo resuelve ningún diseño.  La ronda 5 (`docs/plans/2026-10-19-ronda-5-calidad-y-fuentes.md`) tiene que traer la capa editorial (`editorially_reviewed`, que según el plan de esta ronda es trabajo de Adrián), prerrequisitos registrados y ejercicios, y cerrar los hallazgos de contenido abiertos: la descripción de Wikidata de número primo (C4-04), por qué primos y criptografía (C4-05), el epónimo de π (C4-06) y la lista sin orden de «número primo incluye 197».

### 15.9 Notas

- Servidor del usuario: `atlas.exe` (PID 22172) siguió arriba toda la verificación, porque cargo no tuvo que reemplazar el binario.  `/api/health` = 200 al terminar.
- En la raíz del repo hay seis archivos sueltos, `log-d2.txt`, `log-d390.txt`, `log-d390d.txt`, `log-ddark.txt`, `log-drm.txt` y `log-drm2.txt` (10:52): son corridas fallidas del crítico (`Cannot find module 'C:\MPM\doors.mjs'`, lanzadas desde la carpeta equivocada).  No sirven y se pueden borrar.
- No se hizo commit.  Sin versionar: `docs/design/r4/`, `docs/design/codex/r4/`, `bench/runs/2026-10-19-r4-test-integrated/`, los planes de las rondas 4 y 5 y los archivos nuevos de `web/src/` y `web/bench/`.
