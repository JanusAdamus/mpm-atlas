# Ronda 5 · veredicto del juez B

20 de octubre de 2026 · juez B (rol 5), independiente: no leí `docs/design/r5/juez-A/` ni me coordiné con nadie.  Miro con dos lentes, la de un lector común y la de un matemático: si lo que muestran el mapa y las fichas es verdad, si se lee y si enseña algo falso.  Todos los juicios son **de un agente, provisionales**.

## Qué se evaluó

- **Servidor propio**: `atlas serve --addr 127.0.0.1:8811`, lanzado sin ventana sobre lo que está publicado en `data/` y apagado al terminar.  No quedó ningún proceso mío corriendo.
- **Snapshot publicado**: `data/snapshot/CURRENT` = `atlas-8d4259348f8d6a6d.snap`, del 2026-10-19 13:45.  Su `source_event_seq` es 3,874,452.  Layout `e4ccedf1b8b46087`.
- **Registro** (`data/atlas.db`): `event_seq` 4,163,538.  La última escritura es la importación de 94 frases de fichas en borrador (`atlas.fichas/1`, 14:37).
- **Hallazgo de fondo: en `data/` no se publicó un snapshot nuevo.**
  - Esperé unos 5 minutos a que cambiara `CURRENT` y no cambió.  Tampoco había ningún `compile` corriendo.
  - El snapshot que se sirve es anterior a M2 y a toda la curaduría: tiene 311 `equivalent_to` y 244,933 `part_of_area` en `formally_linked`.
  - El lector ve una mezcla.  El mapa, las conexiones entre conceptos y los recorridos salen del snapshot viejo.  Las fichas (`/api/entity`) leen el registro vivo.
  - El propio AtlasBench lo detecta: «snapshot stale vs registry».
  - Juzgué lo que está publicado, que es lo que pide el protocolo.  Varias de las fallas de abajo desaparecerían con un `compile` nuevo, y lo señalo en cada una.  Otras no desaparecerían.

## Tabla de reglas

| Regla | Resultado | Evidencia |
|---|---|---|
| Aristas agregadas | **PASS** | En el snapshot publicado, las aristas de salida de los 2,639 conceptos de la capa (`edge_limit=200`) suman 355,827 en `model_proposed` y 1,469 en `extracted`, todas con `candidate: true`.  Revisé las 242 estadísticas `extracted` contra el registro: 0 son más fuertes que su eslabón más débil (`b_weakest.py` → `b-weakest.json`).  `cargo test --workspace --release` da 166 pasan y 0 fallan, incluida `weakest_link_real`. |
| Equivalencias | **FAIL** | En el registro se cumple: 0 `equivalent_to` vigentes desde P460, 0 `special_case_of` desde P31, 8 pares en los dos sentidos, los 8 en `disputed`, y R13 en `composition.json` (`m2_check.json`).  Pero en lo publicado `/api/path` **compone dos saltos de P460** y concluye «equivalencia bajo supuestos», con `mathematical: true`.  Ejemplos: «cero negativo ≡ falso», «distancia euclídea ≡ ortodrómica», «grafo no dirigido ≡ grafo de intersección» (`path_Q217675_Q5432619.json`, `path_Q847073_Q912925.json`, `path_Q3115587_Q3498041.json`).  En el registro hay 65 cadenas de este tipo.  La ficha de Pitágoras sí muestra el par como «En disputa».  Esto se arreglaría compilando. |
| Nombres en español | **FAIL** | En las teselas z0–z2 publicadas (3,229 nodos) hay **1,157 derivados**, todos con «(Mathlib, derivado)».  Solo 1,338 nodos (41.4 %) tienen una etiqueta española de verdad.  Si se cuentan como españolas las etiquetas de los derivados, salen 2,495 (77.3 %).  Del lado del registro, el propio trabajador midió 64.5 % (punto de control 2), también por debajo de la meta de ≥ 80 % (`b_names.py` → `b-names.json`). |
| Fuera de dominio | **FAIL** | En la capa siguen los 28 nodos de la lista del diagnóstico (ajedrez, países, física, informática), y las 49 exclusiones `out-of-domain` de la curaduría también siguen en la capa publicada (`b-names.json`).  La exclusión sí quedó registrada y es reversible (49 exclusiones y 40 `retire_claims`), pero el mapa no la aplica. |
| Muestra nueva de 20 conexiones | **FAIL** | Semilla 20261020 y 20261021.  Los pools tienen 1,467 directas y 25,806 agregadas, sobre el layout `e4ccedf1b8b46087` (`r5-protocol-sample.json`).  Juez B: **9 de 20** correctas y útiles, IC de Wilson al 95 % [0.258, 0.658].  Directas 6/10 [0.313, 0.832]; agregadas 3/10 [0.108, 0.603] (`juicio-muestra-B.tsv`).  El veredicto conservador exige que pasen los dos jueces, así que da ≤ 9 diga lo que diga el juez A.  Por eso la meta de ≥ 14 no se alcanza.  La κ no la puedo calcular sin ver al juez A, y no me toca. |
| Fichas | **FAIL** | 0 afirmaciones en `editorially_reviewed` en todo el registro.  Las 94 `explains` están en `model_proposed`, así que el piloto de 20 fichas revisadas por Adrián está en 0.  Extracto completo y citado: 1,340 conceptos de la capa publicada, menos que la meta de 1,500.  Contando nodos de cualquier tipo salen 1,855 (`b_extracts.py` → `b-extracts.json`), así que la regla depende de si «conceptos» quiere decir tipo `concept`. |
| Prueba de usuario común | **FAIL** | **0 de 7** conceptos con las cinco preguntas respondidas, en dos pasadas en ES a 1440 con 0 errores de consola (`b_concepts.mjs` → `out-concepts-{1,2}-es.json`, `shots/`).  «Qué es» se responde en 7/7 con el extracto de Wikipedia.  «Por qué importa», «Qué saber antes» y «Cómo compruebo» están vacíos en 7/7: solo hay borradores plegados, que no cuentan, y al abrirlos solo dicen «(Fuente: Wikipedia (es))», sin la frase.  Bayes muestra «se usa en estadística bayesiana», que no explica por qué importa.  Detalle abajo. |
| Obras | **FAIL** | Solo existe el piloto, en `data/pilot/atlas-pilot-v2.db`: 114,736 obras, contra unos 9.8 M de la regla.  No está en lo publicado: el servidor arranca sin `--works`.  Del piloto se cumplen 0 fechas rechazadas y la tabla de cobertura (`piloto-obras.md` §3.5 y §5).  Según el harness, la ingesta completa queda para una decisión de Adrián. |
| Escala | **NO EVALUABLE** | No existe la escala real (unos 15× el grafo), así que no se puede medir `compile --all` ni la memoria.  Lo que sí medí, a la escala de hoy y sobre lo publicado (`bench-v3.1-test/report.md`): búsqueda c1 p95 15.9 ms, búsqueda **c8 p95 117.7 ms [101.7, 132.6]**, ya por encima de los 100 ms con otro servidor corriendo en la máquina, y teselas p95 3.06 ms.  Las cifras del piloto son extrapolaciones (`piloto-obras.md` §4). |
| Licencias | **FAIL** | La tabla `sources` tiene licencia para las fuentes nuevas: `src:wiktionary-en` CC-BY-SA-4.0, `src:mathlib4-topics` Apache-2.0 y `src:planetmath` CC-BY-SA-3.0, con la diferencia frente a 2.5 anotada.  Pero `docs/STATUS.md` no tiene §16 y no menciona ninguna: `grep` de planetmath, wiktionary, zbmath y openalex no da nada.  Le toca al cierre (paso 6). |
| Sin regresión | **FAIL** | En verde: `pytest` 278, `cargo test` 166/0, `clippy` 0 advertencias, `npm test` 88/88 y `tsc --noEmit` 0.  AtlasBench v3.1 test sobre lo publicado: procedencia 3,608/3,608 PASS y latencias PASS, pero rutas y retrieval dan **NOT_EVALUABLE** por el snapshot desfasado.  Diseño: 0 errores de consola y 0 jerga de la lista en 14 fichas.  Hay una **regresión que enseña algo falso** (B5-01): la ficha de integral dice «integración instance of integral abeliana», con el verbo en inglés y al revés. |

**Resumen**: pasa 1, fallan 9 y 1 no es evaluable.

## Prueba de usuario común (lo que ve el lector, sin abrir borradores)

| Concepto | Qué es | Por qué importa | Qué saber antes | Con qué se conecta | Cómo comprobarlo | ¿Las cinco? |
|---|---|---|---|---|---|---|
| integral | Sí: extracto de Wikipedia (ES) completo | No | No | Engaña: «integración instance of integral abeliana», «… of Parseval's integral», «… of Integral de Borwein», «… of Fórmula de cuadratura de Cavalieri».  Va al revés y en inglés (B5-01) | No | No |
| derivada | Sí: extracto y fórmula | No | No | A medias: «Incluye 16», «forma parte de cálculo diferencial» | No | No |
| teorema de Pitágoras | Sí | No | No | Ya no es contradictorio: el quinto postulado sale «En disputa / En conflicto» (C4-03 corregido).  «PlanetMath … se trata en (sin nombre)» | No | No |
| teorema de Bayes | Sí: es el primer resultado de la búsqueda (punto 6 del plan cumplido) | A medias: «se usa en estadística bayesiana», sin decir para qué | No | A medias | No | No |
| número primo | Sí, gracias al extracto de Wikipedia.  La descripción de Wikidata que se muestra encima sigue siendo incorrecta: «solo tiene dos divisores enteros» (C4-04, a medias) | No | No | No: «Incluye 194» números sueltos | No | No |
| pi | Sí | No | No | Engaña: «lleva el nombre de Ludolph van Ceulen» (C4-06 sin corregir) | No | No |
| matriz | Sí | No | No | A medias: «álgebra lineal», «teoría de Matrices» | No | No |

Con las cinco preguntas respondidas: 0 de 7.  La regla pide ≥ 5.

## Hallazgos que pueden enseñar algo falso (prioridad A)

| id | Dónde | Qué se ve | Por qué es falso o confunde | Dueño probable |
|---|---|---|---|---|
| B5-01 | Ficha de integral, «Con qué se conecta» | «Según Wikidata, integración instance of integral abeliana» | Wikidata dice lo contrario (`c:17e2c9f963964ce2`, `Q318761#P31`: la integral abeliana es una instancia de la integración).  Al verla desde el objeto, la ficha no invierte el verbo.  Además, `instance_of` no está en `web/src/i18n.ts`: no hay ni la forma directa ni la inversa (`~`), así que el verbo sale en inglés en la interfaz en español.  En pi, que es el sujeto, el sentido sí sale bien. | `web/src/i18n.ts` / `panel.ts` |
| B5-02 | Recorridos | «cero negativo ≡ falso», «distancia euclídea ≡ ortodrómica», con «equivalencia… mathematical» | El snapshot publicado es anterior a M2 y compone P460 de forma transitiva. | integrador (compilar y publicar) |
| B5-03 | Muestra D8: Wikidata P279 → «es un caso particular de» | «serie matemática es un caso particular de sumatoria» | La lectura de la relación («lo que vale en general vale en el caso particular») es falsa: las series condicionalmente convergentes no se pueden reordenar.  No es culpa de M2: P279 entra como `special_case_of` sin revisión.  Pasa igual en D6 (monoide y categoría pequeña). | conexiones (2a) |
| B5-04 | Aristas agregadas (A1, A3, A6, A7, A9, A10) | «conjunto → disyunción exclusiva», «cuadrado → espacio vectorial», «triángulo → indexed category» | Son pertenencias homónimas que **siguen vigentes en el registro**, así que un `compile` nuevo no las quita: `Disjoint`→XOR, `sq`→cuadrado, triángulo distinguido→polígono, `Finite`→correspondencia, `IsIso`→isomorfismo de categorías.  El retiro de homónimos de la ronda solo cubrió los nodos fuera de dominio.  Llevan la marca «Candidata», pero se muestran en la capa general. | conexiones (2a), proyección de conceptos |
| B5-05 | Ficha de pi | «Lleva el nombre de: Ludolph van Ceulen» | Es C4-06, sin cambio desde la ronda 4. | `web/src/panel.ts` |
| B5-06 | Mapa z0–z2 | «Sudáfrica», «jaque mate», «NaN», «homology › … (Mathlib, derivado)» | La curaduría está en el registro, pero el snapshot publicado no la aplica. | integrador |

Otros hallazgos (B y C):
- B5-07: los borradores plegados, al abrirlos, muestran solo «(Fuente: Wikipedia (es))», sin el texto de la frase.  Coincide con lo que pidió 3 Fichas: «seguir mandando la etiqueta de la nota en `labels`».
- B5-08: «Según PlanetMath (entry metadata, github.com/planetmath), … se trata en (sin nombre)», en Pitágoras y en pi.
- B5-09: derivada, número primo y matriz se rotulan «ÁREA», y Pitágoras y Bayes «ENUNCIADO».  Para un lector común no está claro qué quiere decir eso.

## Archivos de este juez

- **Scripts**:
  - Copias de `docs/plans/r5/consultas/` sin cambios de lógica: `r5_fetch_tiles.py`, `r5_load.py`, `r5_protocol_sample.py` y `r5_edges.py`.  A `r5_edges.py` solo se le cambió la URL base, que ahora se lee del argumento.
  - Míos: `b_items.py`, `b_names.py`, `b_weakest.py`, `b_extracts.py`, `b_concepts.mjs` y `b_lib.mjs`.  `b_lib.mjs` es una copia del de r4 con la base y la salida cambiadas.
- **Datos en crudo**:
  - Muestra y juicio: `r5-general.json`, `r5-edges.txt`, `r5-protocol-sample.json`, `b-items.json` (lo que ve el lector por ítem, con testigo y pertenencias) y `juicio-muestra-B.tsv`.
  - Compuertas y recorridos: `m2_check.json`, `b-weakest.json`, `b-names.json`, `b-extracts.json` y `path_*.json`.
  - Prueba de usuario: `out-concepts-1-es.json`, `out-concepts-2-es.json` y `shots/`.
  - AtlasBench: `bench-v3.1-test/`.
- **Descartado**: borré una pasada en EN porque el script no cambió el idioma de la interfaz.  No se usa como evidencia.
