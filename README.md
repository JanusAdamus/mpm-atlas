# Atlas · las conexiones de las matemáticas

> Un mapa de las matemáticas que se puede recorrer, interrogar y verificar.  Cada conexión dice de dónde sale, qué tipo de relación es y cuánta evidencia la respalda.
>
> *A navigable, queryable and verifiable map of mathematics, where every connection carries its source, its relation type and its evidence state.*

El atlas une **Mathlib** (las demostraciones formales de Lean), **Wikidata** y **Wikipedia ES/EN** en un solo registro de afirmaciones con procedencia.  Encima de ese registro hay un motor en Rust que busca, traza rutas tipadas entre ideas y dibuja un mapa multiescala que va de las áreas a cada declaración.

```
   A ──┐
       ├──(A y B juntos)──▶ C      dependencia lógica: autoriza concluir
   B ──┘                    │
                            ├──cita──▶ D       solo dice dónde se menciona
                            └──analogía──▶ E   sugiere, no demuestra
```

Una cita no es un prerrequisito.  Una analogía no es una demostración.  Al trazar una ruta, el motor declara qué permite concluir cada salto y se abstiene si no lo sabe.

## Qué lo hace distinto

- **Procedencia desde el primer registro.** Toda afirmación lleva fuente, localizador, extractor, versión, estado de evidencia y fecha.  Sin procedencia no entra.
- **Relaciones n-arias.** Si A y B juntos sostienen C, se guarda la conjunción, no dos aristas sueltas.  Las demostraciones alternativas se conservan como alternativas.
- **Siete estados de evidencia:** `extracted`, `model_proposed`, `editorially_reviewed`, `formally_linked`, `disputed`, `superseded` y `retracted`.  Nada se borra: se reemplaza con historial.
- **Cobertura honesta.** Una región sin relaciones extraídas aparece como «desconocida», no como «aislada».
- **Medido y no prometido.** Cada cifra cita el comando y el archivo que la produjeron.  Las reglas de aprobación se fijan antes de medir y no se mueven (ver `docs/STATUS.md`).
- **Bilingüe y accesible.** Español e inglés como idiomas de primera clase.  Todo lo que hace el mapa se puede hacer con teclado o en vista de texto.

## Cómo está hecho

Cada parte va en el lenguaje que la hace más rápida.  El contrato entre lenguajes es el registro SQLite.

| Capa | Dónde | Qué hace |
|---|---|---|
| Registro | `atlas/` (Python) | Dominio, eventos de solo agregar, ingesta con caché y modo offline, resolución de entidades con fusiones reversibles, capa de conceptos |
| Snapshot | `core/atlas-core` (Rust) | Compila el registro a un grafo CSR mapeado en memoria, verificado por hash |
| Búsqueda | `core/atlas-search` | BM25 ES/EN con tantivy, índice de fórmulas tipo Tangent, vectores densos multilingües, fusión RRF con router |
| Recorridos | `core/atlas-paths` | Álgebra de composición de relaciones, premisas AND/OR, A* multiobjetivo, k rutas con justificación por paso |
| Mapa | `core/atlas-layout` | Leiden multirresolución, ForceAtlas2 con Barnes-Hut en paralelo, teselas por zoom |
| Servidor | `core/atlas-server` | API axum y binario `atlas` con salida `--json` |
| Web | `web/` (TypeScript + WebGL2) | Mapa multiescala, ficha con insignias de evidencia, ruta por conceptos con bajada a declaraciones, vista de texto |
| Evaluación | `bench/` | AtlasBench v3: particiones dev, test y sellada; reporte por clase con intervalos |

## Escala (snapshot `4058b8ab…`, 2026-10-19)

320,214 nodos, 5,715 conceptos y 237,640 declaraciones de Mathlib con concepto asignado.  El detalle y el comando de cada cifra están en `docs/STATUS.md` §14.

## Arrancar

```bash
python -m atlas build                      # fuentes → registro → snapshot, índices y mapa
core/target/release/atlas serve            # http://localhost:8765
core/target/release/atlas search "teorema de Bayes" --json
core/target/release/atlas path <desde> <hasta> --concepts --drill-down --json
python -m bench run                        # AtlasBench
python -m pytest -q ; cargo test --workspace --release
```

En Windows, `CARGO_TARGET_DIR` debe ser una ruta corta fuera de OneDrive (ver `CLAUDE.md`).

## Estado

Proyecto en marcha, por rondas.  Lo hecho, lo medido y lo que falta están en [`docs/STATUS.md`](docs/STATUS.md).  La especificación está en [`docs/SPEC.md`](docs/SPEC.md) y el diseño de origen en `research/2026-10-16/`.  La capa conceptual se publica con su confianza baja visible: la asignación de declaraciones a conceptos todavía no pasa su vara.

Proyecto personal de Adrián González.
