# Ronda 5 · veredicto del juez B, ronda 2 (snapshot nuevo)

21 de octubre de 2026 · juez B (rol 5).  Trabajé solo: no leí `docs/design/r5/juez-A/`.  Uso dos lentes, la de un lector común y la de un matemático: si lo que muestran el mapa y las fichas es verdad, si se entiende y si enseña algo falso.  Todos los juicios son **de un agente y provisionales**.  Los borradores sin revisar no cuentan como respuesta.

## Qué se evaluó

- **Snapshot**: `data/snapshot/CURRENT` = `atlas-19c6f7dc19dab3e9.snap` (2026-10-20 15:43).  Cambió respecto de `8d4259348f8d6a6d`.  Layout `e6ed5eab030b3aec`.
- **Registro**: `data/atlas.db`, `event_seq` 4,188,644.  No cambió entre la toma de evidencia (2026-10-20 16:46–16:54) y la verificación de hoy.
- **Servidor propio**: `atlas serve` en un puerto libre (8823 ayer, 56191 hoy), lanzado sin ventana y apagado al terminar.  Hoy quedó corriendo otro `atlas.exe` (pid 30692, arrancado a las 11:48), que no es mío y no toqué.
- **Reproducibilidad**: volví a correr `r5_protocol_sample.py` hoy contra el servidor nuevo y el resultado es idéntico byte a byte (`cmp`) al de ayer.  Pools: 1,475 directas y 20,359 agregadas.
- **Semilla**: 20261021, la que fijó el orquestador para los dos jueces.  Directo `Random(20261021)`, agregado `Random(20261022)`.  La copia local del script solo cambia la constante `SEED`.  Esta semilla no es la del protocolo §2 (20261020/20261021).  Lo anoto para que quien calcule κ verifique que el juez A usó la misma.

## Tabla de reglas

| Regla | Resultado | Evidencia |
|---|---|---|
| Aristas agregadas | **PASS** | Todas las aristas concepto→concepto salen con `candidate: true`.  Revisé las 29 estadísticas `extracted` contra el registro: 0 son más fuertes que su eslabón más débil y 0 tienen `candidate: false` (`b_weakest.py` → `b-weakest.json`).  `cargo test --workspace --release`: 168 pasan y 0 fallan, incluida `weakest_link_real` (`cargo-test.log`). |
| Equivalencias | **PASS** | Registro: 0 `equivalent_to` vigentes desde P460, 0 `special_case_of` desde P31, 0 `part_of_area` en `formally_linked`.  Hay 8 pares en los dos sentidos y los 8 están en `disputed` (`m2_check.json`).  Lo publicado ya no compone P460: los recorridos que en la ronda 1 concluían «cero negativo ≡ falso» o «distancia euclídea ≡ ortodrómica» ahora se abstienen («alcance bibliográfico… no autoriza una conclusión»), en `path_Q217675_Q5432619.json` y `path_Q847073_Q912925.json`.  En la ficha, Pitágoras ↔ quinto postulado sale como «En disputa / En conflicto». |
| Nombres en español | **FAIL** | Capa z0–z2: 2,041 nodos.  Etiqueta ES del registro en 1,212 (59.4 %); con 116 curadas, 65.1 %.  Faltan unos 300 nodos para el 80 %.  Lo que sí pasa: 0 derivados y 0 «(Mathlib, derivado)» (`b_names.py`, `b_names2.py` → `b-names*.json`).  Algunos nodos visibles siguen en inglés: «graph property», «lattice theory», «simple graph», «object of a category». |
| Fuera de dominio | **FAIL** (por 1) | De las 49 exclusiones `out-of-domain`, 0 siguen en la capa.  La exclusión está registrada y es reversible (`gate.txt`: 49 `exclude_presentation` y 40 `retire_claims`).  Pero de la lista del diagnóstico queda **1** en z0–z2: `wd:Q387015` «trie», una estructura de datos.  La regla pide 0. |
| Muestra nueva de 20 conexiones | **FAIL** | Juez B: **11/20** correctas y útiles, IC de Wilson al 95 % [0.342, 0.742].  Directas 9/10 [0.596, 0.982]; agregadas 2/10 [0.057, 0.510] (`muestra-20.tsv`).  El veredicto conservador exige que pasen los dos jueces, así que queda en ≤ 11 diga lo que diga A.  No llega a 14.  La κ queda para el cierre, que ve a los dos jueces. |
| Fichas | **FAIL** | Hay 0 afirmaciones `editorially_reviewed` en el registro, y las 94 `explains` están en `model_proposed` (`homonimos-check.txt`).  Por eso el piloto de 20 fichas revisadas por Adrián está en 0.  Con extracto completo citado hay 1,324 conceptos de tipo `concept` (meta ≥ 1,500); si se cuentan nodos de cualquier tipo de la capa, son 1,831 (`b-extracts.json`). |
| Prueba de usuario común | **FAIL** | **0 de 7**, con 0 errores de consola (`b_concepts.mjs` → `out-concepts-1-es.json`, `shots/`).  «Qué es» se responde en 7/7 con el extracto de Wikipedia.  En 7/7, «Por qué importa» y «Cómo compruebo» solo muestran «Borrador editorial, sin revisar», y «Qué saber antes» dice «Todavía no hay prerrequisitos registrados». |
| Obras | **FAIL** | Solo existe el piloto (`data/pilot/atlas-pilot-v2.db`, 114,736 obras), y no está en lo publicado.  La ingesta completa espera una decisión de Adrián (`estado.md`, punto de control 8). |
| Escala | **NO EVALUABLE** | No existe la escala real, así que no se puede medir `compile --all` ni la memoria.  A la escala de hoy (`bench-v3.1-test/report.md`): búsqueda c1 p95 17.6 ms; búsqueda c8 p95 **111.1 ms [106.7, 121.7]**, ya por encima de 100 ms; teselas p95 3.3 ms; entidad p95 3.9 ms. |
| Licencias | **FAIL** | La tabla `sources` sí trae licencia: wiktionary CC-BY-SA-4.0, mathlib4-topics Apache-2.0, planetmath CC-BY-SA-3.0.  Pero `docs/STATUS.md` no tiene §16 y no menciona ninguna fuente nueva: `grep -iE "planetmath|wiktionary|zbmath|openalex"` no da nada.  Le toca al cierre. |
| Sin regresión | **PASS** | `pytest` 280 pasan.  `cargo test` 168/0.  `clippy --workspace --all-targets --release`: 0 advertencias (`clippy.log`, hoy).  `npm test` 93/0 y `tsc` sale con 0 (`web-tests.log`).  AtlasBench v3.1 test por HTTP: respondibles 244/245 y adversariales 204/204, igual que la línea base v3.1 (`bench/runs/2026-10-20-r5-v3.1-test-supersede`).  Retrieval da los mismos valores por celda que esa línea base; ya era PROVISIONAL-FAIL desde antes, así que no es regresión.  Procedencia 3,608/3,608.  Diseño, en mi pasada y en los scripts r4 que corrió el integrador sobre este snapshot (`docs/design/r5/integrador/scripts/log-*.txt`): 0 errores de consola, 0 px de desbordamiento de página y 0 jerga en Explorar.  El contraste no falla en ninguna vista.  Se corrigió la regresión B5-01 de la ronda 1. |

**Resumen**: 3 PASS, 7 FAIL y 1 NO EVALUABLE.  En la ronda 1 fueron 1 PASS, 9 FAIL y 1 NO EVALUABLE.

## Qué se corrigió desde la ronda 1 (visto en pantalla)

- **B5-01**: la ficha de integral ahora dice «integración **tiene como instancia a** integral abeliana».  El sentido es correcto y está en español.
- **B5-02**: ya no hay equivalencias transitivas de P460 en los recorridos.
- **B5-05**: pi ya no «lleva el nombre de Ludolph van Ceulen».  La afirmación está `retracted` (`c:c77bbb29d006dccd`).
- **B5-06**: no quedan derivados en z0–z2 y las 49 exclusiones curadas se aplican.

## Lo que todavía puede enseñar algo falso (prioridad A)

| id | Dónde | Qué se ve | Por qué es falso o confunde |
|---|---|---|---|
| B5-04 (sigue) | Aristas agregadas de la muestra: A2, A4, A5, A6, A7, A9 | «función exponencial usa media-f generalizada», «número ordinal usa predicado», «Noetherian group usa módulo», «object of a category usa isomorfismo» | Son pertenencias homónimas que siguen vigentes: `Differentiable`→media-f (`c:ab6f22d26ac47026`), `Nontrivial`→predicado (`c:553e66d552e7834b`), `IsNoetherian`→grupo noetheriano (`c:1c69991b4feb7911`), `CoalgCat`→isomorfismo (`c:2663105905935a9f`).  El retiro de 25,039 pertenencias no las alcanzó; seguramente están en los 1,093 casos de `homonimos-pendientes.tsv`.  En la muestra, 6 de 10 aristas agregadas enseñan una conexión falsa o al revés. |
| B5-10 (nuevo) | D3, P31 leído como «es una instancia de» | «complemento es una instancia de negación lógica; es un ejemplo de esa clase» | El complemento de un conjunto es el análogo conjuntista de la negación, no un ejemplo de ella.  La negación lógica no es una clase. |
| B5-03 (sigue) | P279 → «es un caso particular de» | 14,408 `special_case_of` vigentes, todas de P279 | En esta muestra las 8 P279 resultaron verdaderas.  D7 («extensión de cuerpos es un caso particular de espacio vectorial») es laxa.  La decisión P279 sigue pendiente (`p279-opciones.md`). |
| B5-11 | Aristas agregadas verdaderas pero vacías: A1, A8 | «localización usa conjunto», «isomorfismo usa affine scheme» | No son falsas, pero no enseñan nada y ocupan el lugar de conexiones útiles. |

Otros (B y C): nodos visibles en inglés (D5 «injective sheaf», D10 «simple graph», «Noetherian group»).  Las fichas de derivada, número primo y matriz se rotulan «ÁREA».  La de número primo abre con «Incluye 194» números sueltos.

## Archivos de este juez (`docs/design/r5/juez-B/ronda-2/`)

- Muestra: `r5-protocol-sample.json`, que es lo que da la semilla, y `muestra-20.tsv` (`item_id`, `correct`, `useful`, `note`).  Lo que ve el lector por ítem está en `b-items.json`.
- Datos y consultas: `r5-general.json`, `r5-edges.txt`, `m2_check.json`, `b-weakest.json`, `b-names*.json`, `b-extracts.json`, `homonimos-check.txt`, `gate.txt` y `path_*.json`.
- Compuertas: `pytest.log`, `cargo-test.log`, `clippy.log`, `web-tests.log` y `bench-v3.1-test/`.
- Usuario común: `out-concepts-1-es.json` y `shots/`.
- Scripts: copias de `docs/plans/r5/consultas/`.  `r5_protocol_sample.py` solo cambia `SEED`, y `r5_edges.py` solo cambia la URL base.  Los míos son `b_*.py`, `b_concepts.mjs` y `b_lib.mjs`.
