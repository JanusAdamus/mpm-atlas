# Ronda 5 · veredicto del juez A, ronda 2 (medición y estadística)

Fecha: 2026-10-21, sobre el snapshot que publicó el integrador 2.  Juez A, rol 5, independiente: no leí `docs/design/r5/juez-B/`.  Protocolo: `docs/plans/r5/protocolo.md`.  Solo escribí en esta carpeta.

## 0. Qué se midió

| Qué | Valor | Cómo |
|---|---|---|
| `data/snapshot/CURRENT` | `atlas-19c6f7dc19dab3e9.snap` (2026-10-20 15:43).  **Cambió** respecto de `8d4259348f8d6a6d`. | `cat` |
| `source_event_seq` del snapshot | 4,188,644 = `MAX(seq)` del registro.  El snapshot no está atrasado. | `GET /api/health`; `consultas/m2-check.txt` |
| Layout | `e6ed5eab030b3aec` | teselas z0–z2 |
| Servidor propio | `atlas serve --addr 127.0.0.1:8811`, con `data/snapshot`, `data/index` y `data/layout`.  Lo maté al terminar (PID 30692). | — |
| Capa general (z0–z2) | 2,041 nodos únicos en las teselas `structure`.  `/api/health` dice `general_nodes: 4241`.  Uso 2,041 porque es lo que pintan las teselas. | `consultas/r5-general.json` |

## 1. Tabla de reglas (plan, «Reglas de decisión»)

| Regla | Veredicto | Evidencia |
|---|---|---|
| Aristas agregadas | **PASS** | `ATLAS_CHECK_SNAPSHOT=…19c6f7dc….snap cargo test --release -p atlas-core --test weakest_link_real`: 71,047 aristas agregadas y 0 más fuertes que su eslabón más débil (`logs/weakest-link-real.txt`).  Los 20 ítems agregados de las dos muestras salen con `candidate: true` y `model_proposed`. |
| Equivalencias | **PASS** | Registro (`consultas/m2-check.txt`): 0 `equivalent_to` vigentes desde P460, 311 `said_equivalent_to`, 8 pares en los dos sentidos y 0 de ellos fuera de `disputed`.  Rutas (`consultas/p460-paths-8811.json`): 65 pares a dos saltos por P460 y 130 consultas a `/api/path`, todas 200.  0 rutas encadenan dos `said_equivalent_to`.  Ojo: en 16 respuestas el bloque `exploration` menciona dos o más `said_equivalent_to`.  No son conclusiones; no revisé si la UI lo muestra. |
| Nombres en español | **FAIL** | Lo que ve el lector en z0–z2 (`consultas/names-8811.json`, `reader-labels-8811.json`): 1,329/2,041 = **65.1 % [Wilson 63.0 %, 67.2 %]**, contra una meta de ≥ 80 %.  712 nodos no tienen etiqueta ES en el registro.  La mitad de los derivados sí pasa: 0 en z0–z2 (antes eran 1,157). |
| Fuera de dominio | **PASS** | 0 de los 49 de la lista en z0–z2 (`names-8811.json`).  El retiro está registrado y es reversible (fila de curaduría con `undone_at`): 49 `exclude_presentation/out-of-domain` y 40 `retire_claims/out-of-domain` (`logs/gate.txt`, salida 0). |
| Muestra nueva de 20 conexiones | **FAIL** | Muestra oficial, semilla 20261021: juez A **9/20 [25.8 %, 65.8 %]**.  Directo 8/10 [49.0 %, 94.3 %]; agregado **1/10 [1.8 %, 40.4 %]**.  El veredicto conservador exige que pasen los dos jueces, así que queda en ≤ 9/20 diga lo que diga B: no puede llegar a 14.  κ y la tabla 2×2 los calcula el cierre con el TSV de B.  Detalle en §2. |
| Fichas | **FAIL** | Extractos (`consultas/text-after.txt`): 1,831 de 2,041 nodos de la capa tienen un extracto completo y citado (≥ 1,500).  De tipo `concept` son solo 1,324.  Si «conceptos» quiere decir nodos, esta mitad pasa; si quiere decir `type=concept`, no.  Piloto: 0 afirmaciones `editorially_reviewed` en todo el registro, y las 94 `explains` son `model_proposed`.  Sin la revisión de Adrián, esta mitad no pasa. |
| Prueba de usuario común | **FAIL** | **0/7 [0 %, 35.4 %]** (`scripts/out-concepts-direct-es.json`, capturas `jA-es-ficha-*.png`).  «Qué saber antes» está vacío en 7/7 («Todavía no hay prerrequisitos registrados»), y eso basta para el FAIL.  «Por qué importa» y «Cómo compruebo» solo tienen «Borrador editorial, sin revisar» en 7/7, y los borradores no cuentan.  Bayes muestra además dos usos de Wikidata.  «Qué es» se responde en 7/7 con el extracto. |
| Obras | **FAIL** | Solo existe el piloto: `data/pilot/atlas-pilot-v2.db`, sin cambios desde el 2026-10-20 13:33.  No hay capa de obras completa en `data/`.  El harness la deja fuera de la entrega. |
| Escala | **NO EVALUABLE** | No hay nada medido a la escala real.  Como dato, a la escala de hoy (bench del integrador): p95 de búsqueda c8 = 100.9 ms [93.7, 108.3], ya en el límite de 100 ms; p95 de teselas = 3.2 ms (ronda 4: 9.44). |
| Licencias | **FAIL** | `grep -i "planetmath\|wiktionary\|zbmath\|openalex\|cdli\|coherence" docs/STATUS.md`: 0 resultados.  Las licencias están en la tabla `sources` y en `piloto-obras.md`, pero la regla pide `docs/STATUS.md` (cierre §16). |
| Sin regresión | **PASS** (con reservas) | Pruebas: pytest 280 passed, cargo test 168/0, clippy 0 advertencias, `npm test` 93/0 y `tsc` 0 errores (`logs/`).  AtlasBench: ver «AtlasBench sin regresión», abajo.  Diseño de la ronda 4: ver «Diseño de la ronda 4 sin regresión», abajo.  La reserva es v3.1: ver «Reservas», abajo. |

Resumen: **4 PASS, 6 FAIL y 1 NO EVALUABLE.**  En la ronda 1 fueron 2 PASS, 8 FAIL y 1 NO EVALUABLE, sobre el snapshot viejo.

### AtlasBench sin regresión

La corrida es la del integrador, `bench/runs/2026-10-20-r5-integrador-v3.1-test`.  Su backend coincide con este snapshot: `content_hash 19c6f7dc…`, `stale: false`.  Yo no la repetí.

- Los veredictos son los mismos que en la ronda 4 (`2026-10-19-r4-test-integrated`).
- Respondibles: 244/245 = 0.996 [0.977, 0.999].
- Adversariales: 204/204.
- Procedencia: 3608/3608.
- Recuperación: igual o mejor en las 7 celdas.  `en/informal_to_formal` pasa de 0.318 a 0.425.

### Diseño de la ronda 4 sin regresión

Corrí los scripts de la ronda 4 contra :8811 (`scripts/`, `logs/design-*.txt`):

- concepts: 7/7 con las cinco secciones y jerga 0.
- doors: 3/3 con jerga 0, desbordamiento de página 0 y errores 0.
- map explore: 0 solapes.
- kb: 0 errores y 0 jerga en la vista textual.

El hallazgo de la ronda 1 que enseñaba algo falso está corregido.  Ahora dice «integración tiene como instancia a integral abeliana»: el verbo se invierte y está en español.

### Reservas

- La comparación de AtlasBench no es sobre los mismos ítems: v3 tenía 261 respondibles y v3.1 tiene 245.  Contra v3, el punto de control 6 midió 217/261.
- La «sin regresión» se apoya en v3.1, y dos decisiones sobre v3.1 siguen pendientes de Adrián: el reetiquetado de sealed y si pasa a ser la versión por omisión.
- Las afirmaciones P279 que se publican como «caso particular» siguen sin revisar (`p279-opciones.md`).  En esta muestra producen los fallos D7 y D3.

## 2. La muestra, en detalle

### Semilla: discrepancia que hay que resolver en el cierre

- La tarea pide la semilla **20261021**.  El protocolo §2 fija la **20261020** (`Random(20261020)` para el estrato directo y `Random(20261021)` para el agregado), y el script la tiene fija en el código.
- Saqué las dos muestras sobre los mismos pools, con una copia del script que recibe la semilla como argumento (`consultas/r5_protocol_sample_seed.py`; el estrato agregado usa la semilla + 1, como el original).  Las dos muestras no comparten ningún ítem.
- `muestra-20.tsv` es la oficial de la tarea (20261021).  `muestra-20-semilla-20261020.tsv` es la del protocolo.  Juzgué las dos para que κ se pueda calcular con la que haya usado B.

### Cadena y pools

- Cadena: `r5_fetch_tiles.py` → `r5_load.py` → `r5_edges.py` (puerto por argumento) → `r5_protocol_sample_seed.py` → `build_cards.py`.  Todo está en `consultas/`.
- Pools:
  - 1,475 afirmaciones directas con los dos extremos `concept` en la capa;
  - 20,359 aristas agregadas.

  En la ronda 1, con el snapshot de scratch, eran 1,468 y 25,707.

### Qué vio el juez

- Etiquetas y descripciones ES/EN.
- La relación tal como la rotula la UI:
  - «es caso particular de»;
  - «es una instancia de»;
  - en el panel de concepto, «N declaraciones de este concepto: «depende lógicamente de» declaraciones de aquel», con la marca «candidata».
- El estado.
- El testigo, con su procedencia.
- La asignación de cada declaración del testigo a su concepto.

**Criterio:** cuenta como «correcta y útil» si correcta = sí y útil ∈ {sí, medio}.  En el TSV, «dudosa» se registra como `correct = 0`.

### Resultados

| Muestra | Total | Directo | Agregado |
|---|---|---|---|
| 20261021 (oficial) | **9/20 [25.8 %, 65.8 %]** | 8/10 [49.0 %, 94.3 %] | 1/10 [1.8 %, 40.4 %] |
| 20261020 (protocolo) | 11/20 [34.2 %, 74.2 %] | 8/10 [49.0 %, 94.3 %] | 3/10 [10.8 %, 60.3 %] |

### Causas de los fallos

- **Directos** (P279/P31 de Wikidata que se publican sin revisar):
  - el nivel está mal: «complemento es una instancia de negación lógica»; «proyección ortogonal es una instancia de función»;
  - «extensión de cuerpos es caso particular de espacio vectorial», que es dudosa y ya falló en la ronda 1;
  - un caso verdadero pero inútil: un extremo sin etiqueta ni descripción en ES.
- **Agregados, semilla 20261021.**  De los 9 que fallan:
  - 5 son homónimos de la proyección conceptual:
    - `Differentiable` lleva a «media-f generalizada»;
    - `IsNoetherian` lleva a «grupo noetheriano»;
    - `Nontrivial` lleva a «predicado»;
    - `CoalgCat` lleva a «isomorfismo»;
    - «exists_factorization» lleva a «factorización»;
  - 3 son asignaciones dudosas;
  - 1 es cierta pero trivial (depende de «conjunto»).
- **Agregados, semilla 20261020:** 2 homónimos (`IsDomain` lleva a «dominio de definición») o asignaciones erróneas, y 4 triviales.

### Lectura

- El estrato directo mejoró respecto de la ronda 1 (6/10 → 8/10 con la otra semilla), pero n = 10 no separa las dos cifras: los intervalos se solapan casi por completo.
- El estrato agregado sigue siendo el problema (0/10 → 1/10 o 3/10).  La marca «candidata» es honesta, pero el contenido de la arista depende de que la proyección declaración → concepto sea correcta, y no lo es en la mitad de los casos.
- Con 9/20, el límite superior de Wilson (65.8 %) queda por debajo del 70 % que pide la meta.
- La conclusión FAIL no depende de B ni de κ: con el veredicto conservador, B solo puede bajar la cuenta.

## 3. Incidencias de esta corrida

- La primera corrida de `cargo test --workspace` falló con «Access is denied» al reemplazar `atlas.exe`, que mi servidor tenía abierto.  Maté el servidor y la repetí: 168/0.
- Cargo recompiló `libsqlite3-sys` en la prueba `weakest_link_real`, con el `CARGO_TARGET_DIR` compartido.  Pudo abrir ventanas de consola; ya no queda ninguna.  Tampoco queda ningún `atlas.exe` ni ningún `msedge.exe` de puppeteer.
- Los pickles intermedios (`r5-load.pkl`, de 114 MB, y `r5-edges.pkl`) se movieron al scratchpad; no están aquí.

## 4. Archivos

- `muestra-20.tsv`: la muestra oficial (semilla 20261021), con columnas `item_id`, `correct`, `useful` y `note`.
- `muestra-20-semilla-20261020.tsv`: la muestra con la semilla del protocolo.
- `consultas/`:
  - las muestras: `r5-protocol-sample-20261021.json` y `r5-protocol-sample-20261020.json`;
  - las tarjetas: `r5-cards-*.json`;
  - las mediciones: `names-8811.json`, `reader-labels-8811.json`, `integrator-measure-8811.json`, `p460-paths-8811.json`, `m2-check.txt`, `text-after.txt` y `r5-edges.txt`;
  - las copias de los scripts.
- `logs/`: `weakest-link-real.txt`, `pytest.txt`, `cargo-test.txt`, `clippy.txt`, `npm-test.txt`, `tsc.txt`, `gate.txt` y `design-*.txt`.
- `scripts/`: los scripts de diseño de la ronda 4, apuntados a :8811, y `concepts_direct.mjs`, con sus salidas `out-*.json`.
