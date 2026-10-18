# Plan: capa conceptual sobre Mathlib (proyección conceptual, WP06/WP08/WP10)

Fecha: 2026-10-17.  Dueño: Adrián.  Estado: aprobado para ejecutar por workflow.

## Problema (medido, ver docs/STATUS.md)

- El snapshot `03c61399` tiene 318,733 nodos; 245,656 son declaraciones de Mathlib (statement 190,407 y definition 49,987, más infraestructura).  Los conceptos de Wikidata son 20,594.
- La vista general queda dominada por lemas técnicos: la zona de Mathlib se ve como una maraña y los caminos pasan por detalles que no explican nada.
- El puente formal–informal es débil.  Contra 355 enlaces curados: recall@1 0.43, recall@3 0.57 y precisión 0.75 sobre el umbral 0.90 (117 ítems).  Además hay 2,950 fusiones activas y una cola de 9,527 candidatos sin revisar.

## Objetivo

Un nivel de **conceptos** por encima de las declaraciones:

1. Cada declaración de Mathlib queda asignada a uno o varios conceptos, con procedencia y confianza declaradas.  También puede quedar sin asignar, de forma explícita.
2. Las relaciones entre conceptos se derivan agregando las dependencias entre sus declaraciones.  Conservan los conteos por tipo de relación y estado de evidencia, y nunca inventan una relación que no esté detrás.
3. El mapa muestra conceptos en las capas generales y despliega las declaraciones solo al acercarse.
4. Los recorridos buscan primero entre conceptos y bajan al detalle formal cuando se pide.

Es una de las contribuciones propias del informe: «proyección conceptual de dependencias formales» (R06).

## Diseño

**Nodos concepto.**  Los conceptos de Wikidata existentes y las teorías/áreas ya presentes son los primeros candidatos.  Cuando una zona coherente de Mathlib no tiene concepto en Wikidata, se crea un concepto derivado a partir de la jerarquía de módulos y namespaces de Mathlib (p. ej. `Mathlib/NumberTheory/Primes` → «números primos»), rotulado como derivado.

**Asignación declaración → concepto.**  Se combinan señales:

- enlaces `formalized_as` y fusiones existentes;
- módulo y namespace de Mathlib;
- nombres y docstrings frente a etiquetas y alias ES/EN de Wikidata;
- vecindad en el grafo de dependencias.

Cada asignación es una afirmación nueva con relación de pertenencia a concepto (si SPEC no la tiene, se agrega con migración), estado `model_proposed` o `extracted` según la señal, extractor `concept-projection` con versión, y la evidencia que la sostiene.  Es reversible y nunca sobrescribe datos fuente.

**Aristas agregadas.**  Concepto A → concepto B existe solo si hay declaraciones de A que dependen de declaraciones de B.  Lleva el conteo por relación y por estado de evidencia, y un ejemplo trazable.  La infraestructura de Mathlib (tácticas, notación) se separa, como ya hace la ingesta (R06).

**Rendimiento.**  La proyección se calcula en Rust dentro de `atlas compile`, en paralelo, y queda en el snapshot como derivado reconstruible.  El registro SQLite guarda las afirmaciones de asignación.

## Reglas de decisión (fijadas antes de medir)

| Componente | Pasa si |
|---|---|
| Asignación vs 355 enlaces curados | El concepto curado está entre los conceptos asignados (recall@3) en ≥ 0.75, frente a 0.57 hoy, y la precisión top-1 es ≥ 0.75.  Se mide sin ajustar umbrales sobre esos 355: el ajuste se hace en una mitad (dev) y se reporta en la otra (test). |
| Cobertura | ≥ 85 % de las declaraciones de contenido (no infraestructura) asignadas a algún concepto; el resto queda marcado como sin asignar, nunca forzado. |
| Muestra auditada | 200 asignaciones estratificadas por área, revisadas por un agente distinto del que construyó la proyección.  Se reportan como juicio de agente (provisional), con su intervalo. |
| Mapa | La capa general (z0–z2) muestra ≤ 6,000 nodos, todos conceptos o áreas; las aristas agregadas conservan conteos por relación; las declaraciones aparecen solo en zoom profundo. |
| Recorridos | Sin regresión en AtlasBench v3 test: abstención en respondibles ≤ 1/261, sobreafirmación 0, adversariales 188/188.  Más 20 consultas nuevas concepto→concepto escritas a mano, con respuesta a nivel concepto y opción de bajar a declaraciones. |
| Rendimiento | `atlas compile` completo ≤ 2× el tiempo actual; p95 de teselas y de recorridos ≤ 2× lo actual. |
| Reproducibilidad | Dos compilaciones desde el mismo registro dan el mismo hash de snapshot. |

Si la asignación no llega a su vara, el mapa por conceptos se publica igual, con las asignaciones de baja confianza visibles como tales, y el resultado se reporta como no aprobado.  No se mueve la vara.

## Criterio estético y pedagógico de Codex (incorporado el 2026-10-17)

Fuente: `docs/design/codex/MPM-mejoras-funcionales-esteticas-pedagogicas.md` (64 mejoras A01–H08) y `docs/design/codex/auditoria-pedagogica-2026-10-17.md` (H01–H09).  Son vinculantes para esta ronda en lo que toca a conceptos, mapa y búsqueda.  El resto queda en el backlog de la siguiente ronda.

**Entra en esta ronda (obligatorio):**

- **P0 semánticos: E01–E04, E06, B03.**  Primero, porque pueden enseñar algo falso.
  - Integración ↔ `IsIntegral` (y todo homónimo análogo) no puede usarse como formalización ni como asignación de concepto.  Una propuesta `formalized_as` en estado `model_proposed` es una candidata, nunca una señal de verdad para la proyección.
  - Ninguna puntuación heurística se muestra como `p=` ni como porcentaje: se rotula «puntuación heurística, no calibrada» o no se muestra al lector.
  - Candidatas y establecidas no comparten encabezado.
  - Se revisan los tipos de entidad mal asignados (p. ej. derivada como ÁREA).
  - «Pertenece al área» deja de agrupar P279/P31/P361/P2579: cada relación conserva su verbo y su dirección se lee como oración.
  - «axioms not computed» y otros límites de extracción van en «Estado de verificación», nunca en «Supuestos» o hipótesis.
  - Los homónimos se desambiguan por dominio y definición breve.
  - Cada caso observado (integración/IsIntegral, derivada como área) queda como prueba de regresión semántica.
- **Mapa: C01, C02, C03, C04, C06, C08.**
  - La vista general no la definen ISBN, Tactic, fechas ni fuentes: la infraestructura y las fuentes documentales salen de las capas generales.
  - El zoom tiene significado (áreas → conceptos → enunciados).
  - Hay presupuesto de etiquetas con control de colisiones y una leyenda que explica tamaño, color, distancia y agregación, y que dice que la proximidad no prueba relación.
  - Las conexiones se leen como oraciones, y «sin aristas en esta vista» no se lee como ausencia de relación.
- **Búsqueda y ficha: B01, B02, B06, B07, D07, H06.**
  - Conceptos antes que declaraciones de biblioteca en consultas cotidianas (`integral`, `Bayes`, `derivada`).
  - Resultados agrupados por tipo.
  - Panel de sugerencias con ancho propio.
  - Nada de rótulos sin traducir como DESCRIPTION.
  - Sin desbordamiento horizontal por URLs o identificadores.
- **Estados sin salida: A07, G05.**  Una ruta sin camino validado dice «MPM todavía no tiene una conexión validada en este corpus» y ofrece una continuación útil, sin inventar.

**Reglas de decisión adicionales (fijadas antes de medir):**

- Integración no aparece asignada ni enlazada como formalización a `IsIntegral`.
- `integral`, `derivada` y `Bayes` devuelven su concepto educativo en el top 3.
- Ninguna vista de lector muestra `p=` sobre una puntuación no calibrada.
- La capa general no muestra fuentes documentales (ISBN, fechas, archivos) ni infraestructura.
- Todo verificado con capturas del navegador.

**Backlog de la siguiente ronda (no se hace ahora):**

- Modelo editorial de lección (F01–F08: objetivo, ejemplo trabajado, símbolos, errores frecuentes, práctica).
- Entrada por objetivos e intenciones (A01–A04).
- Modo de lectura protagonista (D01–D06).
- Recorridos de estudio separados de rutas del grafo (G01–G04).
- Comparación por preguntas (C07), aplicaciones y laboratorios (G06–G08).
- Accesibilidad verificada contra WCAG 2.2 (H02–H05) y pruebas con alumnos (H07).

Siguen vigentes las advertencias de Codex: nada de chat como sustituto de contenido revisado, ni porcentajes de dominio, ni convertir registros importados en lecciones vacías.

## Ejecución (workflow dinámico, 4 agentes)

1. **Proyección** (Rust + Python, esfuerzo alto).  Asignación, conceptos derivados, aristas agregadas, migración si hace falta y medición contra los 355 enlaces.
2. En paralelo, cuando termine la proyección:
   - **Mapa y web.**  Layout y teselas por concepto, despliegue al acercarse, panel de concepto con sus declaraciones y vista de texto.
   - **Recorridos y bench.**  Búsqueda concepto-primero con bajada al detalle, 20 consultas nuevas y AtlasBench sin regresión.
3. **Integración y verificación** (Sonnet, esfuerzo medio).  Compilación completa, pruebas, bench, capturas del navegador, auditoría de la muestra de 200, `docs/STATUS.md` y veredicto contra estas reglas.

Cada agente usa su propio `CARGO_TARGET_DIR`; solo el integrador reconstruye el binario compartido.  Nada se sube a ningún servidor; el commit local lo hace el orquestador al final si las pruebas pasan.
