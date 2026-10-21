# Ronda 5 · veredicto del juez A (medición y estadística)

Fecha: 2026-10-20, de 14:42 a 15:00 (−06:00).  Juez A, rol 5, independiente: no leí `docs/design/r5/juez-B/` ni me coordiné con nadie.  Protocolo: `docs/plans/r5/protocolo.md`, sin enmiendas.  Solo escribí en esta carpeta.

## 0. Antes de las reglas: no hay un snapshot final publicado

La tarea dice que use «el snapshot que el integrador acaba de publicar en `data/`».  No existe:

| Qué | Valor | Cómo lo vi |
|---|---|---|
| `data/snapshot/CURRENT` | `atlas-8d4259348f8d6a6d.snap`, del 2026-10-19 13:45 (el del punto de control 1, recién migrado a v4) | `cat`; `GET /api/health` en :8793 |
| `source_event_seq` de ese snapshot | 3,874,452 | `/api/health` |
| `MAX(seq)` del registro | 4,163,538.  El último evento es `atlas.fichas/1` a las 14:37:02 de hoy | `r5_m2_check.py` |
| Eventos que ese snapshot no incluye | 289,086: todo M2, toda la curaduría, las etiquetas ES, los extractos, PlanetMath y las fichas | resta de los dos anteriores |
| Snapshot o checkpoint 9 nuevos | ninguno.  Vigilé `CURRENT` 16 minutos y no cambió.  No había ningún `atlas compile` corriendo | bucle de espera; `tasklist` |

Medir sobre `8d42…` habría sido medir el estado de antes de la ronda, que es justo lo que el protocolo §2 prohíbe.  Por eso compilé un snapshot propio desde el registro actual, solo en el scratchpad y nunca en `data/`:

- Snapshot `87885a72e9c29e5f` (`source_event_seq` 4,163,538, 16 s).
- Layout `da231c7f2f03368c`, arrancado en caliente desde `e4ccedf1…`.
- Servidor en `127.0.0.1:8794`, con el índice de búsqueda viejo.  La búsqueda cae al carril FTS5 y no la uso como evidencia.

**Todas las cifras de abajo que dependen del snapshot son provisionales.**  Miden el código y el registro de hoy, no lo que publique el integrador.  Hay que repetirlas cuando publique.  Si cambia la composición de la capa general (por ejemplo, si saca los derivados), cambian los pools y con ellos la muestra.

## 1. Tabla de reglas (plan, «Reglas de decisión»)

| Regla | Veredicto | Evidencia |
|---|---|---|
| Aristas agregadas | **PASS** | `ATLAS_CHECK_SNAPSHOT=…87885a72….snap cargo test --release -p atlas-core --test weakest_link_real`: 74,598 aristas agregadas, 0 más fuertes que su eslabón más débil, todas candidatas.  En la API, los 10 ítems agregados de la muestra salen con `candidate: true` y `model_proposed`. |
| Equivalencias | **PASS** | Registro: 0 `equivalent_to` vigentes desde P460 y 311 `said_equivalent_to`.  Hay 130 pares a dos saltos por P460; `GET /api/path` da 0 rutas que encadenen dos `said_equivalent_to` (`consultas/p460-paths.json`).  Las pruebas `said_equivalence_is_one_hop_and_never_composes` y `adr0005_said_equivalence…` pasan.  Los 8 pares en los dos sentidos están en `disputed`.  Lo amplié a `applied_in`, `formalized_as`, `instance_of`, `part_of_area` y `prerequisite_for`: 0 pares en los dos sentidos fuera de `disputed`.  La ficha de Pitágoras lo muestra como «En disputa». |
| Nombres en español | **FAIL** | Lo que ve el lector en z0–z2 (`consultas/names-8794-scratch.json`): 1,332/3,202 nodos con etiqueta ES, 41.6 % [Wilson 39.9 %, 43.3 %].  Siguen 1,157 derivados («algebra (Mathlib, derivado)»).  Aun quitando los derivados, el resultado es 1,332/2,045 = 65.1 % < 80 %.  El 64.5 % de 2b aplica las exclusiones de la curaduría en la consulta, pero el layout no las aplica. |
| Fuera de dominio | **FAIL** | De los 49 de la lista, 10 siguen en z0–z2 (gravedad, agujero negro, sismología, Galileo, sextillón…).  En el snapshot publicado están los 49.  El retiro sí está registrado y es reversible: 49 `exclude_presentation` y 40 `retire_claims`, con 2,512 pertenencias retiradas.  Lo que falla es que el mapa todavía no respeta la marca. |
| Muestra nueva de 20 conexiones | **FAIL** (provisional) | Semilla 20261020/20261021 del protocolo, no la mitad test; pools de 1,468 directos y 25,707 agregados.  Juez A: **6/20 correctas y útiles [14.5 %, 51.9 %]**.  Por estrato: directo 6/10 [31.3 %, 83.2 %] y agregado **0/10 [0 %, 27.8 %]**.  El veredicto conservador exige que pasen los dos jueces, así que queda en ≤ 6/20 diga lo que diga B: no puede llegar a 14.  κ no se puede calcular sin B, y el protocolo me impide leerlo.  Archivos: `consultas/r5-protocol-sample.json`, `consultas/r5-cards.json`, `muestra-20-juicio-A.tsv`. |
| Fichas | **FAIL** | Extractos: 1,824 nodos de la capa con un extracto completo y citado (con `oldid` y fin de oración), de 2,035 que quedan tras la curaduría.  De tipo `concept` son solo 1,313.  Si «conceptos» quiere decir nodos, esta mitad pasa; si quiere decir `type=concept`, no.  Piloto: 0 afirmaciones `editorially_reviewed` en todo el registro, y las 94 `explains` son `model_proposed` (borradores importados a las 14:37).  Sin la revisión de Adrián, esta mitad no pasa. |
| Prueba de usuario común | **FAIL** | **0/7 [0 %, 35.4 %]**: abrí los 7 conceptos por id (`scripts/concepts_direct.mjs`, `scripts/out-concepts-direct-es.json`, capturas `jA-es-ficha-*.png`).  «Qué es» se responde en 7/7 con el extracto.  «Por qué importa» está vacío en 7/7; Bayes solo lista dos «usos registrados».  «Qué saber antes» está vacío en 7/7.  «Cómo compruebo» está vacío en 7/7.  Lo único que hay en esas secciones son «Borrador editorial, sin revisar», que no cuentan. |
| Obras | **FAIL** | Solo hay piloto: 114,736 obras en `data/pilot/atlas-pilot-v2.db`, contra 5,201,602 de zbMATH Open y 4,625,779 de OpenAlex (unos 1.2 %).  El harness la deja fuera de la entrega y la decide Adrián.  El solapamiento sí se midió en el piloto: 95.6 % y 36.5 % por DOI (`piloto-obras.md`). |
| Escala | **NO EVALUABLE** | Nada se midió a la escala real (unas 15 veces el grafo); lo de `piloto-obras.md` es extrapolación.  Lo que yo vi a la escala de hoy (16 s de `compile` y 5 s de `layout`, sobre 321,228 nodos) no vale para la regla. |
| Licencias | **FAIL** | `docs/STATUS.md` no menciona ninguna fuente nueva (grep de planetmath, wiktionary, zbmath, openalex, CDLI, coherence: 0 resultados).  Las licencias sí están en la tabla `sources` del registro y en `piloto-obras.md` §7, verificadas el 2026-10-20.  Hay una discrepancia: el plan dice PlanetMath CC BY-SA 2.5 y el registro dice CC-BY-SA-3.0, con la nota de que la página dice 2.5.  Queda pendiente del cierre §16. |
| Sin regresión | **FAIL** | Pruebas en verde: pytest 278, cargo test 166/0, clippy 0, `npm test` 88/0 y `tsc` 0.  AtlasBench no se puede evaluar sobre el snapshot final porque no hay; v3.1 del punto 8 se corrió sobre otro estado.  Diseño: hay una regresión nueva que enseña algo falso.  La ficha de **integración** dice «Según Wikidata, integración instance of integral abeliana», pero la afirmación es al revés (`c:17e2c9f963964ce2`: sujeto `wd:Q318761`, objeto `wd:Q80091`).  En π dice «instance of», en inglés.  `web/src/i18n.ts` no tiene etiqueta ni forma inversa para `instance_of` ni para `said_equivalent_to` (grep: 0), así que `relLabel` cae a «instance of». |

Resumen: 2 PASS, 8 FAIL y 1 NO EVALUABLE.

## 2. La muestra, en detalle

- **Cómo se sacó.** Corrí las copias de `r5_fetch_tiles.py` → `r5_load.py` → `r5_edges.py` → `r5_protocol_sample.py` en `consultas/`.  El único cambio fue el puerto (8794), que en `r5_edges.py` estaba fijo en 8765.  `layout_version` `da231c7f2f03368c`, `event_seq` 4,163,538.  La capa general (z0–z2) tiene 3,202 nodos.
- **Qué vio el juez.** Por ítem: etiquetas y descripciones ES/EN, la relación tal como la rotula la UI, el estado, la marca «candidata» y el testigo con su procedencia (`build_cards.py`), además de la pertenencia de cada declaración al concepto.
- **Directos (6/10).** Fallan:
  - D2 y D5: P31 incorrecto en la fuente.  «isomorfismo de grafos» no es una propiedad de grafos y el supremo no es una función.
  - D3 (dudosa): una extensión de cuerpos no es una especialización de espacio vectorial.
  - D6: es verdad, pero inútil, porque no tiene descripción ni nombre ES.
  - Tres de los cuatro `instance_of` se muestran en inglés.
- **Agregados (0/10).** Son homónimos de la proyección conceptual en 8 de 10:
  - el prefijo `Set.` lleva a «conjunto»;
  - `IsIso` lleva a «isomorfismo de categorías»;
  - `IsPrimitiveRoot` lleva a «raíz primitiva módulo n»;
  - la dimensión inyectiva lleva a «dimensión»;
  - `Nontrivial` lleva a «predicado».

  Los otros 2 son construcciones reales (A6) o lemas de un dominio (A5) que no sostienen una dependencia entre conceptos.  La marca «candidata» es honesta, pero el protocolo pide que la conexión sea correcta, y no lo es.  Dos ítems (A4 y A7) tienen un extremo derivado, cosa que no debería ocurrir si los derivados salieran de z0–z2.
- **Comparación.** El diagnóstico (un juez, semilla 20261019) dio 0/10 útiles en el estrato agregado; aquí sigue en 0/10.  El eslabón más débil corrigió el estado mostrado, no el contenido.
- **Límites.** Fue un solo juez.  n = 20; el intervalo va de 14.5 % a 51.9 %, así que no descarta un ~50 %.  Sí descarta el 70 % que pide la meta.  La conclusión FAIL no depende de B: con el veredicto conservador, B solo puede bajar la cuenta.

## 3. Lo que hay que hacer para que esto sea oficial

1. Que el integrador publique el snapshot final en `data/`, con el layout que respete `NodeFlags` y saque las `note`.  Después, repetir `consultas/` sobre ese servidor: la muestra cambia si cambia la capa.
2. Que B juzgue los mismos 20 ítems del snapshot final.  Entonces se reportan κ y la tabla 2×2.

## 4. Archivos

- `consultas/`: los scripts de la cadena (copias), `build_cards.py`, `names_check.py`, `r5-general.json`, `r5-edges.txt`, `r5-protocol-sample.json`, `r5-cards.json`, `p460-paths.json`, `names-8794-scratch.json` y `names-8793-publicado.json`.
- `muestra-20-juicio-A.tsv`: el juicio por ítem.
- `scripts/`: `concepts_direct.mjs` y `out-concepts-direct-es.json` para la prueba de usuario común.  `concepts.mjs` / `out-concepts-1-es-1440-light.json` vienen de la búsqueda con el índice viejo y no son evidencia.
- Capturas `jA-es-ficha-*.png` y `p1-*.png`.  Las `p1-*` son de esa búsqueda y no son evidencia.
- Los pickles intermedios (`r5-load.pkl`, de 128 MB, y `r5-edges.pkl`) quedaron en el scratchpad, no aquí.
