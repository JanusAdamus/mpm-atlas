# Plan: ronda 3 · corregir la capa conceptual

Fecha: 2026-10-18.  Dueño: Adrián.  Estado: aprobado para ejecutar por workflow.  Parte del cierre de la ronda 2 (`docs/STATUS.md` §13, commit `d290e32`).

## Problema (medido en la ronda 2)

| Hueco | Cifra | Fuente |
|---|---|---|
| Asignación por debajo de la vara | top-1 0.7143 (test) contra 0.75; recall@3 0.7638 pasa | STATUS §13 |
| Auditoría de 200 asignaciones | estricta 0.420 [0.354, 0.489]; primarias 0.542; rango 1 0.309; rango 2 0.237 | `bench/concepts/audit-sample-200.judged.jsonl` |
| Homónimos de `name_doc` | `ball` → «snooker ball», `M` → «Mandelbrot set», `IsLocalization.AtPrime` → «número primo» | auditoría y trabajador de recorridos |
| Conceptos que no son matemáticos en la capa general | wd:Q52 «Wikipedia», números («10», «1105», «1728»), «año bisiesto que comienza en miércoles» | trabajador del mapa |
| Teselas lentas | p95 8.78 ms contra 2.57 ms (3.4×); z0 pesa 523 KB | AtlasBench 2026-10-18 |
| Búsqueda | `integral` sin concepto en el top 5; en/text nDCG@10 0.771 (antes 0.787) | STATUS §13 |
| La web no dibuja la ruta por conceptos | `PathResponse.concepts` sin uso; CLI sin `--concepts` ni `--drill-down` | STATUS §13 |
| Respuestas grandes cortadas | ECONNRESET con `Connection: close` sobre ~200 KB | integrador, ronda 2 |

## Qué se hace

1. **Homónimos y calidad de asignación** (Python `atlas/concepts.py`, `atlas/resolve.py`; Rust `core/atlas-core/src/concepts.rs` solo si hace falta).
   - Un concepto solo es candidato si es matemático: su tipo o su cadena P31/P279 llega al dominio matemático, o tiene enlace de Mathlib curado.  Fuentes, números sueltos, fechas, años, objetos físicos y personas no son conceptos de asignación.
   - `name_doc` exige concordancia de dominio: el módulo o namespace de Mathlib debe ser compatible con el área del concepto (`Topology/MetricSpace/Ball` no va a un objeto de billar).  Los nombres de uno o dos caracteres (`M`, `X`) no generan señal por nombre.
   - Las asignaciones secundarias (rango 1 y 2) solo se conservan con una señal independiente de la primaria; si no, se retiran como afirmaciones `superseded`, con historial.
   - El respaldo por zona se rotula como tal en la ficha y en el mapa, y no cuenta como señal propia.
   - Todo se ajusta en la mitad dev.  La mitad test se usó una vez en la ronda 2, así que esta segunda lectura se reporta como tal; por eso también hay una muestra de auditoría nueva.
2. **Búsqueda** (`core/atlas-search`).  Conceptos antes que declaraciones en consultas cotidianas: `integral`, `derivada` y `Bayes` devuelven su concepto educativo en el top 3, sin bajar en/text.
3. **Mapa, servidor y web** (`core/atlas-layout`, `core/atlas-server`, `web/`).
   - z0 pesa ≤ 250 KB: presupuesto de nodos y aristas por tesela, y coordenadas compactas.
   - Corregir el corte con `Connection: close` y dejar una prueba que lo reproduzca.
   - La web dibuja la ruta por conceptos, con un botón para bajar a las declaraciones.
   - La CLI `atlas path` acepta `--concepts` y `--drill-down`.
4. **Integración y verificación**.
   - Recompilar el binario compartido, correr el build de punta a punta y las pruebas, AtlasBench y las 20 consultas de concepto.
   - Muestra de auditoría nueva de 200 asignaciones, con otra semilla y juzgada por un agente distinto de quien construyó.
   - Capturas del navegador.
   - Escribir `docs/STATUS.md` §14 con el veredicto PASS/FAIL/NOT_EVALUABLE por regla.

## Reglas de decisión (fijadas antes de medir)

| Componente | Pasa si |
|---|---|
| Asignación vs 355 curados (mitad test, segunda lectura) | top-1 ≥ 0.75 y recall@3 ≥ 0.75 |
| Auditoría nueva de 200 (juicio de agente, provisional) | precisión estricta ≥ 0.60 en total y ≥ 0.70 en primarias |
| Homónimos | Pruebas de regresión que fallan con el código anterior: `ball` no va a «snooker ball»; `M` no va a «Mandelbrot set»; `IsLocalization.AtPrime` no va a «número primo»; integración no va a `IsIntegral` |
| Capa general | 0 nodos de tipo fuente, número suelto, fecha o año, persona o infraestructura en z0–z2; ≤ 6,000 nodos |
| Cobertura | ≥ 85 % de las declaraciones de contenido con señal propia.  Si baja de ahí, se reporta y no se rellena con el respaldo por zona |
| Búsqueda | `integral`, `derivada` y `Bayes` con su concepto en el top 3; en/text nDCG@10 ≥ 0.787 |
| Teselas | p95 de teselas en AtlasBench ≤ 5.14 ms (2× la base de 2.57 ms); z0 ≤ 250 KB |
| Servidor | Respuesta de 500 KB o más completa con `Connection: close` (prueba automática) |
| Recorridos | Sin regresión en AtlasBench v3 test: abstención ≤ 1/261, sobreafirmación 0, adversariales 188/188; 20 consultas de concepto con 14/14 respondibles |
| Web | Ruta por conceptos dibujada y bajada a declaraciones, con captura; 0 errores de consola; sin `p=`; sin desbordamiento en móvil |
| Reproducibilidad y rendimiento | Mismo hash en dos compilaciones; `compile --all` ≤ 2× el de la ronda 1 (64.35 s) |

Si una regla no pasa, se reporta como no aprobada.  No se mueve la vara.

## Ejecución (workflow dinámico, 4 agentes)

1. En paralelo, cada uno con su propio `CARGO_TARGET_DIR`:
   - **Asignación**: punto 1.
   - **Búsqueda**: punto 2.
   - **Mapa, servidor y web**: punto 3.
2. **Integración y verificación** (Sonnet): punto 4.

Ningún trabajador publica en `data/snapshot`, `data/index` ni `data/layout`; eso lo hace solo el integrador.  Solo el trabajador de asignación escribe en el registro.  El commit local lo hace el orquestador si las pruebas pasan.
