# P279 publicado como «caso particular» (B5-03): medición y opciones

Fecha: 2026-10-20.  Rol 2a/2b, para decisión de Adrián.  No cambié la política: las afirmaciones siguen como están.

## Qué se ve

El juez B (muestra D8) leyó en pantalla «serie matemática es un caso particular de sumatoria».  La UI acompaña `special_case_of` con esta lectura: «Lo que vale en general vale en el caso particular, con los mismos supuestos» (`web/src/i18n.ts`).  En este caso la lectura es falsa: una serie es un límite de sumas parciales, y reordenar una serie condicionalmente convergente cambia su suma (teorema de Riemann), cosa que una suma finita no permite.  `composition.json` trata además `special_case_of` como transitiva (`chain: transitive`).

## Medición

Registro `data/atlas.db`, `event_seq` 4,188,644, script `docs/plans/r5/consultas/r5_p279_sample.py` (solo lectura).

| Qué | Cifra |
|---|---|
| `special_case_of` vigentes | 14,408 (14,407 `extracted` y 1 `disputed`) |
| De ellas desde Wikidata P279 (localizador `…#P279`) | **14,408 (100 %)**.  No hay otra fuente de `special_case_of` |
| Reemplazadas por M2 (desde P31, ahora `instance_of`) | 14,422 `superseded` |
| Con objeto «magnitud física» (Q107715) | 563 |
| Revisadas por una persona | 0 |

## Muestra de 20 (semilla 20261022, sobre las 14,408)

Criterio: es correcta si es verdad **y** la lectura de la UI vale sin advertencia.  Juicio de un agente, provisional.

| n | Afirmación | Juicio | Nota |
|---|---|---|---|
| P1 | viscosidad dinámica ⊂ magnitud física | fuera de dominio | Taxonomía de física, no una especialización matemática |
| P2 | bound vector ⊂ vector euclídeo | dudosa | Un vector ligado no es un vector libre |
| P3 | ecuación sinh-Gordon ⊂ EDP hiperbólica | sí | |
| P4 | vector contravariante ⊂ vector | sí | |
| P5 | espacio completamente regular ⊂ espacio regular | sí | |
| P6 | grafo k-conexo ⊂ grafo conexo | sí | Para k ≥ 1 |
| P7 | tenacidad ⊂ magnitud física | fuera de dominio | Ciencia de materiales |
| P8 | número semiprimo ⊂ número compuesto | sí | |
| P9 | temperatura ⊂ propiedad física intensiva | fuera de dominio | |
| P10 | número autodescriptivo ⊂ número de Harshad | sí | Es verdad, aunque es una curiosidad |
| P11 | media ponderada ⊂ media potencial ponderada | sí | El caso p = 1 |
| P12 | calor residual ⊂ calor | fuera de dominio | |
| P13 | wrapped distribution ⊂ distribución de probabilidad | sí | |
| P14 | parábola de Kiepert ⊂ envolvente | dudosa | Es la envolvente de una familia; «envolvente» no es un tipo de curva |
| P15 | agrupamiento jerárquico ⊂ análisis de grupos | dudosa | Un método frente a una tarea: tipo de, no caso particular |
| P16 | transformada continua en ondículas ⊂ transformada integral | sí | |
| P17 | anomalía verdadera ⊂ magnitud física | fuera de dominio | Mecánica celeste |
| P18 | superficie de Bordiga ⊂ superficie algebraica | sí | |
| P19 | dominio de ideales principales ⊂ dominio de Dedekind | sí | Con la convención usual, en la que los cuerpos cuentan |
| P20 | poliedro semirregular ⊂ poliedro | sí | |

Resultado: **12/20 correctas [Wilson 95 %: 38.7 %, 78.1 %]**, 5/20 fuera de dominio [11.2 %, 46.9 %], 3/20 dudosas [5.2 %, 36.0 %] y 0/20 falsas [0 %, 16.1 %].  Con n = 20, un 0 no descarta casos como D8: falsas raras pero graves.  Las muestras de los jueces dan algo parecido: de las 10 P279 de B, 3 fallan por la relación (D3 fuera de dominio, D6 dudosa y D8 falsa); de las 6 de A, 1 (D3, dudosa).

Resumen: P279 es una taxonomía, «es un tipo de», y no siempre una especialización con los mismos supuestos.  La cuarta parte de lo que entra es física u otra ciencia.

## Opciones

**A. Cambiar solo cómo se lee.**  Se muestra «según Wikidata, es un tipo de», con una lectura que no prometa transferir resultados.  Se quita `transitive` para P279 o se limita a un salto.
- A favor: no toca el registro ni el bench.  Lo hace web más una línea en `composition.json`.
- En contra: la relación sigue llamándose `special_case_of`.  El motor de recorridos y las fichas siguen usándola como si fuera una especialización verificada.

**B. Relación propia `subclass_of`, igual que se hizo con P31 y P460 en ADR-0005.**  M2 reemplaza las 14,408 con `supersede` por `subclass_of`, que no es transitiva y se rotula «Wikidata la clasifica como un tipo de».  Solo compone en `instance_of ∘ subclass_of*` (R12).  `special_case_of` queda para lo que la respalde otra fuente: Mathlib (una instancia o un `extends` entre las dos formalizaciones) o una revisión editorial.
- A favor: dice exactamente lo que dice la fuente, como pide el principio 4.  Es reversible y conserva el historial.  Usa el mismo camino que ya se probó con M2.
- En contra: hace falta una enmienda de ADR-0005, del arquitecto.  Hay que reetiquetar el bench, como en v3.1, y rehacer «Qué saber antes» y «Con qué se conecta» en las fichas.

**C. Mantener `special_case_of` pero bajar su estado.**  P279 pasa a `model_proposed` (candidata) y no compone, salvo que lo confirme Mathlib o una revisión.  Además se excluye la taxonomía de física: objeto en magnitud física y similares, 563 directas como mínimo.
- A favor: no hace falta relación nueva.  La marca «candidata» ya existe en la UI.
- En contra: «candidata» dice que el dato es dudoso, cuando el problema es otro: Wikidata lo afirma, pero significa otra cosa.  Se mezclan incertidumbre y semántica.

## Recomendación

**B**, y mientras no esté, **A** como parche de presentación.  B es lo mismo que ADR-0005 ya decidió para P31 (`instance_of`) y P460 (`said_equivalent_to`): nombrar la relación por lo que la fuente dice y no darle una autoridad que no tiene.  La parte de física se resuelve aparte, con la regla `out-of-domain` de la curaduría, no cambiando el tipo de relación.  Decide Adrián.  Si elige B, la enmienda la escribe el arquitecto y la ejecuta 2a con `wikidata_m2.py`.
