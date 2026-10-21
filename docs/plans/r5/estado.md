# Ronda 5 · estado (entrega entre operadores)

Se actualiza en cada punto de control.  Formato: plan → cola → ejecución → compuertas → entrega.

## 2026-10-19, punto de control 1

- **Plan:** aprobado (`docs/plans/2026-10-19-ronda-5-calidad-y-fuentes.md`).
  - ADR-0005 aceptado.
  - Registro migrado a v4 (`migration` 2026-10-19T13:45:15-06:00).
  - Respaldo v3 verificado (`quick_check` ok, 2,828,345 afirmaciones).
  - Snapshot `8d4259348f8d6a6d` publicado (build10-v4, 83.3 s).
- **Cola:** 2a, 2b y 2c en curso; 3–6 pendientes (`harness.md`).
- **Ejecución:** los tres trabajadores se cortaron por el límite de uso y se reanudaron con su contexto.  La herramienta Workflow está apagada en esta sesión.
- **Compuertas** (`python docs/plans/r5/consultas/r5_gate.py`, salida 0):

  | Comprobación | Resultado |
  |---|---|
  | schema v4 | PASA |
  | `part_of_area` en `formally_linked` | 0 (PASA) |
  | `disputed` | 8 |
  | Lotes del piloto | 0 (la copia todavía no tiene lotes) |

  Curadurías activas:

  | Tipo | Regla | Filas |
  |---|---|---|
  | exclude_presentation | derived-out-of-general | 1,481 |
  | exclude_presentation | out-of-domain | 49 |
  | presentation_label | capitalization | 1,417 |
  | presentation_label | qualifier | 55 |
  | propose_merge | duplicate-label | 36 |
  | retire_claims | out-of-domain | 40 |

- **Entrega:**
  - Nada falla todavía.
  - Decisión pendiente de Adrián: ninguna hasta el piloto de obras y las fichas en borrador.

## 2026-10-19, punto de control 2

- **2b Nombres y fuentes: terminado.**
  - Curaduría: 3,075 filas activas, reversibles.  Retiradas 2,512 pertenencias homónimas.  0 derivados y 0 fuera de dominio en la capa general (snapshot de scratch).
  - Etiquetas ES en z0–z2: 41.5 % → 64.5 %.  **FAIL** contra la meta de ≥ 80 %.  Las fuentes libres ya no dan más; quedan 719 nodos en `cola-traduccion-es.tsv`.
  - Extractos de Wikipedia: 5,581 cortados en límite de oración.  1,810 de 2,023 nodos con extracto citado (meta ≥ 1,500: PASA).
  - Fuentes nuevas: listas de temas de Mathlib (762 alias), PlanetMath (946 entradas, 290 `cites` que no son prerrequisitos) y el piloto del Coherence Map (44 conexiones, sin ingerir).
  - Pruebas: 249 pasan.  1 falla por el `composition.json` en curso de 2a; se le avisó a 2a.
- **3 Fichas: en curso.**
- **Para el integrador:**
  - Layout, teselas y servidor todavía no leen `NodeFlags`: el mapa no refleja la curaduría hasta que lo hagan.
  - Los adaptadores nuevos se corren con `python -m atlas.ingest.<nombre>` y no están en `run_ingest`.
- **Decisiones de Adrián:**
  - 36 propuestas de fusión (el agente cree que solo 3 son duplicados reales).
  - `cola-traduccion-es.tsv`.
  - `coherence-map-piloto.md`.

## 2026-10-19, punto de control 3: pausa (Adrián, por uso)

- 2a Conexiones: detenido mientras esperaba un AtlasBench.  Su trabajo está en el árbol y en el registro (re-ejecutable).
- 2c Obras: detenido revisando una descarga fallida.  Copia del piloto en `data/pilot/`.
- 3 Fichas: detenido mientras escribía la API del store para las fichas.
- Para retomar: reanudar cada rol con «revisa git diff y el registro, y sigue»; luego integrador → jueces A/B → cierre §16.  No hay commit desde `b4c6616`.

## 2026-10-20, punto de control 4: reanudación

- Revisión antes de reanudar:
  - `r5_gate.py`: las 4 comprobaciones pasan.  25 lotes del piloto; nueva regla de curaduría `wiktionary-es` (24 filas).
  - `python -m pytest -q`: 251 passed.
  - `data/pilot/logs/fetch_zb30.log` está vacío: es la descarga fallida de zbMATH.
- Se reanudan en paralelo 2a (Conexiones), 2c (Obras, solo el piloto) y 3 (Fichas), cada uno sobre sus archivos.
- Sigue después: integrador (4) → jueces A/B (5) → cierre §16 (6).  No hay commit desde `b4c6616`.

## 2026-10-20, punto de control 5: 3 Fichas terminado

- Código:
  - `atlas/fichas.py`, en formato markdown `ficha/1`: `import`, `render` y `review`.  `review` exige un revisor que sea persona.
  - Cinco líneas en `atlas/cli.py`: `python -m atlas fichas …`.
  - `web/src/panel.ts`: el extracto completo de Wikipedia, con la licencia que viene de su procedencia, y los borradores plegados.
- 20 borradores, 94 frases en `draft-llm/1`, en `docs/plans/r5/fichas/`.  `--dry-run` contra el registro: 20/20, con cada cita literal en la revisión guardada.  **No se importaron.**
- Pruebas:
  - pytest 260 passed.
  - npm test 88/88.
  - tsc 0 errores; build ok.
  - gate ok.
- **Para el integrador**, antes de importar las fichas:
  - excluir las entidades `note` del índice y del layout;
  - sacar las `explains` en `/api/entity` aunque se pase el tope de 200;
  - seguir mandando la etiqueta de la nota en `labels`.
- Huecos de contrato, para el arquitecto:
  - No hay `Store.last_event_seq()`: se usa `store._conn`.
  - La sección de la nota viaja dentro del id.
- Decisión de Adrián: revisar los 20 borradores.  Sin esa revisión, la regla de usuario común (≥ 5/7) no puede pasar.  «Qué saber antes» queda casi vacía porque hay pocos `prerequisite_for`.

## 2026-10-20, punto de control 6: 2a Conexiones verificado

- Los puntos 1 y 6 ya estaban en el árbol y en el registro.  Esta vez solo se verificaron y midieron.
  - Eslabón más débil: 74,598 aristas agregadas, 0 más fuertes que su eslabón, todas candidatas (`weakest_link_real`, snapshot `c40e908d…`).
  - M2 en `--dry-run` no tiene nada pendiente, es idempotente.
    - P460: 311 reemplazadas por `said_equivalent_to`.
    - P31: 14,422 `instance_of`.
    - 8 en `disputed`.
    - 244,933 `part_of_area` bajaron de `formally_linked` a `extracted`.
  - «Bayes» devuelve primero `wd:Q182505` (el teorema), en ES y en EN.
- Compuertas:
  - pytest 251.
  - cargo test 162/0.
  - clippy: **1 advertencia** (`manual_clamp`), en `core/atlas-server/src/works.rs:101`, que es de 2c.
  - gate 4/4.
- AtlasBench v3 test (`bench/runs/2026-10-20-r5-conexiones-test`), contra §15:
  - Acierto en respondibles: 260/261 → 217/261.  42 de las 43 bajas pasan por afirmaciones P31/P460 reemplazadas por M2; la que queda pasa por una `disputed`.  En las otras 218: 217/218.
  - Procedencia: FAIL, con 338 `claim_missing`.  Son exactamente las afirmaciones reemplazadas, y todas tienen sucesora con el mismo `source_id` y `locator`.
  - Adversariales 188/188 y sobreafirmación 0, sin cambio.  Retrieval igual o mejor.
  - Latencias: p95 de teselas 9.44 → 28.8 ms y p95 de búsqueda c8 90.4 → 127.6 ms, con la máquina cargada.  No es una comparación limpia; se repite en el paso del integrador.
- **Decisión de Adrián:** el gold de `bench/splits/v3` codifica la semántica de antes de ADR-0005.  Recomendación del orquestador: que Evaluación haga que el chequeo de procedencia siga `superseded_by` y que reetiquete los 42 ítems según ADR-0005, con el cambio registrado.  La alternativa es aceptar las bajas como esperadas.
- Sin verificar: que `web/` muestre «candidata» y respete `explore_hidden`.  Queda para el integrador.

## 2026-10-20, punto de control 7: decisión del bench

- **Adrián eligió la opción A.**
  - El chequeo de procedencia sigue la cadena de reemplazos y exige la misma fuente y el mismo localizador.
  - Los ítems afectados por M2 se reetiquetan en una versión nueva del split, con un registro de cambios ítem por ítem.  v3 no se edita.
  - Dueño: Evaluación (`bench/`).
- 2c Obras se cortó por el límite de sesión.  Se reanudó con su contexto y con la advertencia de clippy de `works.rs:101`.

## 2026-10-20, punto de control 8: bench v3.1 y piloto de obras

- **Evaluación (opción A): terminado.**
  - La procedencia sigue los reemplazos hasta la afirmación vigente: 338/338 resueltas, 3608/3608 PASS.
  - Split `v3.1`: `python -m bench.relabel --source v3 --target v3.1`, con el registro de cambios en `bench/manifest/v3.1-changelog.jsonl`.
    - 193 ítems cambiados, 50 de ellos a abstención.
    - v3 sigue intacta.
  - Partición test, por http:
    - Respondibles: 244/245 = 0.996 [0.977, 0.999].
    - Adversariales: 204/204.
  - Única pérdida que no viene de M2: `hw:route:038:charpoly-det`, la misma que en §15.
  - pytest 268.
  - **Decisiones de Adrián:**
    - si el reetiquetado mecánico de 29 ítems de sealed se acepta;
    - si `VERSION` pasa a v3.1 por omisión.
- **2c Obras: piloto terminado** (`docs/plans/r5/piloto-obras.md`).
  - Causa de la descarga fallida: el proceso murió con la pausa del 19 y el script no tenía log por partición.  Corregido.  zbMATH clase 30, 2006–2026: 99,401 registros.
  - Copia nueva y consistente: `data/pilot/atlas-pilot-v2.db`.
    - 114,736 obras canónicas, 684,593 afirmaciones.
    - Reanudación: 0 filas parciales.
    - Solapamiento por DOI: 95.6 % de zbMATH clase 30 está en OpenAlex; 36.5 % de OpenAlex está en zbMATH.
  - Compilación de la capa de obras: 7.86 s.  p95 de teselas ≤ 2.4 ms; p95 de búsqueda ≤ 1.6 ms.
  - Extrapolación, no medida:
    - 7.3–8.7 M de obras;
    - registro de 74–144 GB;
    - zbMATH por REST en unas 54 h, contra el tope de 48 h.
  - Recomendación del trabajador: no lanzar todavía la ingesta completa.
  - Compuertas: pytest 267; cargo 162/0; clippy 0, con `works.rs` corregido; gate 4/4, que todavía lee la copia vieja.
  - **Decisiones de Adrián:**
    - cómo bajar OpenAlex: snapshot de S3 o llave gratuita;
    - reducción L1;
    - medir zbMATH por OAI-PMH;
    - meta de unos 8 M de obras;
    - la copia vieja `atlas-pilot.db`;
    - 1,357 registros de OpenAlex sin fusionar.
- Siguiente: integrador (4).  Antes, que excluya `note` del índice y del layout (pedido de 3 Fichas).

## 2026-10-20, punto de control 9: workflow de revisión, integración y jueces (`wf_04553a0f-9fb`)

- **Revisión con agentes de ECC** (rust, python, typescript, silent-failure): 1 HIGH, 11 MEDIUM, 14 LOW.  TypeScript: 0.
- **Integrador, solo tareas 1 y 2.**  Leyó el «haz 1 2» de Adrián como si limitara sus tareas.  Fallo de orquestación.
  - Corrigió el HIGH y los 11 MEDIUM, cada uno con una prueba que falla antes.  Los LOW quedan listados.
  - Excluyó `note` de búsqueda, layout y `/api/neighborhood`.  `/api/entity` lleva las `explains` aunque se pase el tope.
  - Importó los 20 borradores: 94 notas, `model_proposed`, 0 en FTS.  event_seq 4,163,538.
  - Compuertas: pytest 278, cargo 166/0, clippy 0, gate 4/4.
  - **No reconstruyó** el snapshot: `CURRENT` sigue en `8d4259348f8d6a6d`, de antes de M2.  No corrió el bench v3.1 ni los scripts de diseño.
- **Jueces sobre el snapshot viejo.**  Los dos lo señalan.
  - A: 2 PASS, 8 FAIL, 1 NO EVALUABLE.
  - B: 1 PASS, 9 FAIL, 1 NO EVALUABLE.
  - Coinciden en estos FAIL: nombres ES (≈41 %); fuera de dominio (el layout no aplica `NodeFlags`); muestra de 20 (A 6/20, B 9/20); fichas y usuario común (0 revisadas, 0/7); obras (solo el piloto); licencias (no están en STATUS).
  - **Hallazgos que pueden enseñar algo falso** (B):
    - `instance_of` y `said_equivalent_to` sin traducción en `i18n.ts`; el verbo no se invierte («integración instance of integral abeliana»).
    - P279 se publica como «caso particular» sin revisión («serie ⊂ sumatoria»).
    - Pertenencias homónimas vigentes (`Disjoint`→XOR, `sq`→cuadrado…).
    - pi «lleva el nombre de Ludolph van Ceulen».
- **Siguiente:** integración 2 (layout con `NodeFlags`, i18n, reconstrucción, compuertas, bench v3.1, diseño); luego repetir a los jueces sobre el snapshot nuevo, con la misma semilla para calcular κ.

## 2026-10-20, punto de control 10: arreglos de contenido; integración 2 reanudada

- **Arreglos de contenido** (workflow `wf_3e8b96b4-5d0`, paso Fix): terminados.
  - Homónimos (`homonym-membership/1`): 25,039 pertenencias retiradas en 43 ítems, reversibles; 521 correctas conservadas.
    - El barrido encontró 1,093 casos ambiguos más, sin aplicar: `docs/plans/r5/homonimos-pendientes.tsv`.
    - Candado en `concepts.py`: la proyección no recrea lo retirado.
  - pi: se retiró `c:c77bbb29d006dccd` (`named-after/1`).
  - P279: las 14,408 `special_case_of` vigentes salen todas de P279.  Muestra de 20 con semilla 20261022: 12 correctas, 5 de física, 3 dudosas.  Opciones en `p279-opciones.md`; se recomienda la B (`subclass_of` vía M2, con enmienda al ADR).  **Decisión de Adrián.**
  - Compuertas: gate 4/4; pytest 280.
- **Integrador 2:** se cortó por el límite de uso de la cuenta (15:16).  El workflow se detuvo y se reanudó desde la ejecución `wf_3e8b96b4-5d0`.  El paso Fix sale de caché; el integrador vuelve a correr y retoma el árbol.

## 2026-10-20, punto de control 11: pausa (Adrián se retira)

- El workflow `wf_3e8b96b4-5d0` se detuvo a mitad del paso Integrate.
  - El integrador 2 alcanzó a publicar el snapshot `19c6f7dc19dab3e9` (`data/snapshot/CURRENT`).  Sin reporte final: no se sabe qué compuertas, bench ni scripts de diseño corrió.
  - Se mató el servidor que dejó abierto (`atlas.exe serve` en el puerto 8796).
- Hooks de ECC en perfil standard, activos desde el reinicio.  No chocan con rtk.
- **Para retomar:** `Workflow({scriptPath: ".../workflows/scripts/r5-integrate-2-rejudge-wf_3e8b96b4-5d0.js", resumeFromRunId: "wf_3e8b96b4-5d0"})`.  El paso Fix sale de caché; el integrador vuelve a correr sobre el árbol y el snapshot ya publicado, y verifica todo antes de los jueces.
- Decisiones de Adrián pendientes:
  - P279;
  - los 20 borradores de fichas;
  - las obras completas;
  - el sealed y la versión por omisión del bench (v3.1);
  - las 36 fusiones;
  - la cola de traducción al español.

## 2026-10-21, punto de control 12: cierre

- Workflow `wf_3e8b96b4-5d0` terminado.  Integrador 2: snapshot `19c6f7dc19dab3e9` y todas las compuertas en verde.  Bench v3.1 sin regresión.
- Jueces, ronda 2, sobre el snapshot nuevo, con los mismos 20 ítems; κ 0.80.  Combinado: 3 PASS, 6 FAIL, 1 NO EVALUABLE; la regla de licencias se atiende en el cierre.
- Cierre escrito en `docs/STATUS.md` §16.  Las decisiones de Adrián están en §16.7.
