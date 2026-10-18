# atlas-layout

Mapa multiescala (WP08): comunidades Leiden en varias resoluciones (área ⊇ región ⊇ vecindario), layout multinivel ForceAtlas2 con Barnes-Hut en paralelo, y teselas z/x/y binarias precalculadas.  El layout es un derivado reconstruible del snapshot; nunca la única copia de nada.

## Qué hace cada módulo

| Módulo | Qué hace |
|---|---|
| `graph.rs` | Grafo no dirigido ponderado en CSR desde las aristas de afirmaciones vigentes.  Una conjunción de k premisas aporta k aristas de peso 1/k.  Agregación por partición. |
| `leiden.rs` | Leiden (R37): movimiento local rápido (por lotes en paralelo con rayon desde 8,192 nodos, determinista para cualquier número de hilos), refinamiento por comunidad en paralelo, agregación.  Modularidad o CPM.  `hierarchy_seeded` anida los niveles y aplica `max_share`. |
| `force.rs` | ForceAtlas2 con quadtree Barnes-Hut sobre puntos en orden Morton; fuerzas por nodo en paralelo, bit a bit idénticas con 1 o 16 hilos.  Gravedad fuerte por defecto. |
| `multilevel.rs` | Layout del grafo de comunidades más grueso, interpolación y refinamiento nivel por nivel hasta los nodos.  Arranque en caliente con anclas.  Nodos sin relaciones en un disco aparte. |
| `tiles.rs` | Teselas binarias (varints LEB128): entidades visibles por prioridad de etiqueta, clústeres para lo oculto, aristas agregadas por (relación, estado de evidencia) con su conteo. |
| `pipeline.rs`, `io.rs` | `compute_with`, `tile()` para `/api/map/tiles`, escritura y lectura (tiles por mmap). |

## Decisiones de esta ronda (2026-10-17)

1. **Tope al tamaño de un área (`LayoutParams::max_share`, 0.25).**  Con modularidad a γ = 0.1, el nivel "área" juntaba toda la componente gigante en una sola comunidad (límite de resolución): 199,094 de 286,757 nodos en los datos reales y 999,969 de 1,000,000 en el sintético.  Ahora un nivel grueso cuya comunidad más grande pasa del 25 % de los nodos con relaciones se recalcula a la resolución más baja, entre la pedida y la del nivel más fino, que cumple el tope (bisección en log γ sobre el grafo agregado, que es pequeño).  El nivel más fino no se toca.  `LevelInfo` reporta `resolution` (pedida, identidad del nivel) y `resolution_used`.  Un rearranque en caliente prueba primero la resolución usada la vez anterior.
2. **Gravedad fuerte (`ForceParams::strong_gravity`, activa).**  Con gravedad constante, la repulsión de la componente gigante empujaba las componentes pequeñas muy lejos y el ajuste al cuadrado unidad dejaba el mapa real en una mancha: el 98 % de los nodos con relaciones cabía en x ∈ [0.481, 0.523].  Con gravedad fuerte ocupa x ∈ [0.044, 0.805], y ∈ [0.121, 0.879] (percentiles 1 y 99), y la razón longitud de arista entre comunidades / dentro de comunidad se mantiene (3.9 → 4.0).
3. **Nodos "desconocidos" fuera de la simulación.**  Un nodo sin relaciones en las capas no tiene nada que lo coloque.  Antes flotaba lejos por la repulsión.  Ahora va en un disco a la derecha del mapa, con la misma área media por nodo (principio 7: se ve como grupo "Relaciones desconocidas", no mezclado con el mapa ni como "aislado").  Las ranuras del disco crecen hacia fuera, así que en un rearranque los nodos ya colocados no se mueven.  Un nodo que cruza la frontera (gana o pierde todas sus relaciones) se recoloca.
4. `TileBudget::full` baja de 2048 a 1024 nodos por tesela en el zoom máximo, para acotar el JSON de las teselas más llenas.

## Mediciones

Máquina: 16 hilos, Windows 11.  **Las corridas de esta ronda se hicieron con otros trabajadores compilando (rustc con LTO) y corriendo benchmarks a la vez, así que los tiempos están inflados y varían mucho entre corridas**; las cifras de calidad, tamaño y estabilidad no dependen de la carga.  Comando:

```bash
export CARGO_TARGET_DIR=C:/Users/AdrianGonzalez/.cargo-target/layout
cargo run --release -p atlas-layout --example bench_layout -- \
    --snapshot <dir del snapshot> --out <dir de salida> --drop 0.01 --det 1
```

`--drop 0.01` construye un layout sin un 1 % de nodos al azar y luego lo reconstruye en caliente con ellos.  El JSON de salida trae todo lo de abajo.  Archivos: `C:/Users/AdrianGonzalez/.cargo-target/layout-bench/{real-v7a,real-v7b,synth1m-v7}/run.json`.

### Datos reales (snapshot `0dd04e66745eaceb`: 286,757 nodos, 728,303 aristas; 4,855 nodos sin relaciones)

| Medida | real-v7a | real-v7b |
|---|---|---|
| Construcción en frío (grafo + Leiden + layout + teselas) | 12.72 s | 13.74 s |
| Leiden, 3 niveles | 2.06 s | 2.30 s |
| Layout | 9.22 s | 9.79 s |
| Teselas | 1.25 s | 1.48 s |
| Memoria pico (privada) | 0.20 GiB | 0.20 GiB |
| Posiciones idénticas en dos corridas en frío | sí | (no se midió) |

Con menos carga, el código anterior a esta ronda tardó 2.66 s en frío (`real-v4`: Leiden 0.26 s, layout 2.01 s, teselas 0.36 s); con gravedad fuerte, una corrida de prueba sin teselas dio 1.38 s de layout.  Esas son las cifras de referencia sin contención.

Niveles (modularidad estándar Q con γ = 1):

| Nivel | γ pedida → usada | Comunidades | Mayor | Q |
|---|---|---|---|---|
| área | 0.1 → 0.2324 | 724 (723 + desconocidos) | 64,679 | 0.780 (antes 0.442 con la mayor de 199,094) |
| región | 1.0 | 801 | 17,924 | 0.853 |
| vecindario | 10.0 | 1,191 | 4,054 | 0.817 |

Muchas comunidades son componentes pequeñas (mediana de tamaño: 2 o 3).

Estabilidad (2,909 nodos agregados, 1 %; desplazamiento en el cuadrado unidad de los 283,848 nodos comunes):

| | media | p50 | p95 | p99 | máx |
|---|---|---|---|---|---|
| reconstrucción en frío | 0.2329 | 0.1973 | 0.5252 | 0.6679 | 0.9445 |
| arranque en caliente | 0.0035 | 0.0019 | 0.0046 | 0.0084 | 0.8152 |
| en caliente, sin los 751 que cruzaron la frontera conocido / desconocido | 0.0021 | 0.0019 | 0.0046 | 0.0076 | 0.1544 |

Comunidades que conservan su mayoría en caliente: 0.960 / 0.941 / 0.985 por nivel.

Teselas: zmax 6, 3,015 teselas, 35.8 MB en total.  Binario por tesela p50 / p95 / máx en el zoom más profundo: 8,138 / 19,202 / 69,532 bytes.  `tile()` con JSON incluido sobre 2,000 teselas al azar: p50 0.28 ms, p95 2.77 ms, máx 14.5 ms (real-v7a).  JSON por tesela p50 42 KB, p95 252 KB, máx 1.34 MB.

### Sintético (1,000,000 nodos, 10,000,000 aristas; `bench-1m/snapshot`, `28423033291dcac7`)

| Medida | synth1m-v7 (con carga) | synth1m-v4 (código anterior, menos carga) |
|---|---|---|
| Frío total | 92.4 s | 34.0 s |
| Leiden | 49.5 s | 18.2 s |
| Layout | 28.9 s | 9.3 s |
| Teselas | 13.3 s | 6.1 s |
| Memoria pico (privada) | 1.62 GiB | 1.53 GiB |
| Posiciones idénticas en dos corridas | sí | sí |

Leiden solo, nivel fino (γ = 10, 2 iteraciones) con traza: 12.5 s, 2,847 comunidades, Q = 0.140 (`probe_leiden`, misma sesión, carga menor).  El grafo sintético tiene estructura de comunidades débil (Q ≈ 0.14 a 0.17), así que sus niveles son menos estables que los reales.

Niveles: área γ 0.1 → 0.8997, 18 comunidades (mayor 177,430; antes una sola de 999,969); región 67 (mayor 91,353); vecindario 2,817 (mayor 4,880).

Estabilidad (10,293 nodos agregados): desplazamiento medio en caliente 0.0056 (p95 0.0135, máx 0.103) contra 0.376 en frío.  Comunidades que conservan su mayoría: 0.252 / 0.278 / 0.634; con estructura tan débil, Leiden reacomoda las comunidades aunque las posiciones casi no se muevan.

Teselas: zmax 6, 3,653 teselas, 212 MB.  En el zoom más profundo, 2,113 de 2,680 teselas pasan del tope de 4,096 aristas; el resto se cuenta por relación en `hidden_edges`, no se tira.  `tile()` con JSON: p50 2.9 ms, p95 13.7 ms, máx 309 ms.

## Pendiente

- Repetir las mediciones con la máquina sin otros trabajadores.
- En el layout de nodos, construir el quadtree pesa tanto como la repulsión bajo carga (real-v7a: 4.9 s de árbol contra 2.3 s de repulsión); la parte superior del árbol es secuencial.
- Las ~700 componentes pequeñas de los datos reales salen como comunidades propias en todos los niveles; el zoom 0 dibuja 724 clústeres, más que el presupuesto de 64.
