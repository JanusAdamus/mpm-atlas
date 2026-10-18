# MPM · Atlas de las conexiones matemáticas

Proyecto personal de Adrián González (AJ), abierto y extensible. Vive en `C:\MPM`, fuera de OneDrive y de los sistemas de Delos Advisors; no se mueve de vuelta ahí. No es trabajo de cliente de Delos Advisors: aquí no entra información de Natech, PPC, Monex, Infonavit ni de ningún otro cliente.

Objetivo: un sistema de conocimiento matemático que se pueda recorrer, interrogar, verificar y usar para experimentar. Es un grafo heterogéneo, contextual y versionado, con relaciones de varias premisas, búsqueda híbrida, motor de recorridos tipados, mapa multiescala y evaluación pública (AtlasBench).

## Registro del proyecto (leer antes de actuar)

| Archivo | Qué es |
|---|---|
| `research/2026-10-16/atlas-investigacion-y-plan.md` | Informe fuente: decisiones de diseño, arquitectura, evaluación, plan de 18 meses |
| `research/2026-10-16/plan-ejecucion.json` / `.md` | 16 paquetes de trabajo (WP01–WP16) con dependencias y criterios de aceptación |
| `research/2026-10-16/bibliografia-y-evidencia.md` | 60 referencias (R01–R60): técnica, uso propuesto, límite, nivel de lectura |
| `research/2026-10-16/references.json` / `.bib` | Bibliografía en formato reutilizable |
| `docs/STATUS.md` | Estado vivo: qué está hecho, qué se midió, qué sigue. Se actualiza al cerrar cada ronda |
| `docs/SPEC.md` | Especificación ejecutable: objetos, afirmaciones, estados, contratos |

`research/` es registro histórico: no se edita. Si un hallazgo nuevo contradice el informe, se anota en `docs/STATUS.md` con fecha.

## Principios no negociables (vienen del informe)

1. **Procedencia desde el primer registro.** Toda afirmación publicada lleva fuente, localizador, extractor y versión, estado de evidencia y fecha. Sin procedencia no entra.
2. **La fuente de verdad es el registro de afirmaciones y sus eventos.** Índices, layouts, resúmenes y embeddings son derivados reconstruibles. Nunca la única copia de nada.
3. **Relaciones n-arias.** Si A y B juntos sostienen C, se guarda la conjunción (nodo de afirmación con roles), no dos aristas sueltas. Las demostraciones alternativas se conservan como alternativas.
4. **Los tipos de relación no se mezclan.** Cita ≠ prerrequisito ≠ dependencia lógica ≠ analogía ≠ equivalencia ≠ aplicación ≠ influencia histórica. El motor de recorridos declara qué autoriza cada salto; una cadena de analogías no concluye una aplicación demostrada.
5. **Estados de evidencia explícitos:** `extracted`, `model_proposed`, `editorially_reviewed`, `formally_linked`, `disputed`, `superseded`, `retracted`. "Formal" nombra el artefacto (biblioteca, versión, axiomas) y no se extiende a la explicación.
6. **Fusiones de entidades reversibles.** Los embeddings proponen candidatos; fusionar exige una decisión registrada que se puede deshacer.
7. **Cobertura honesta.** Una región sin relaciones extraídas es "desconocida", no "aislada".
8. **Nada de cifras inventadas.** Toda métrica en un documento cita archivo, fecha, datos y versión. Las metas del informe (20,000 ítems, >99% de precisión de fusiones, p95 <100 ms) son objetivos, no resultados.
9. **Confianza con significado.** Un score dice qué evento estima y cómo se calibró. No se multiplican probabilidades de aristas como si los errores fueran independientes.
10. **Bilingüe ES/EN** como idiomas de primera clase. Accesible: todo lo que hace el mapa se puede hacer con teclado y en vista textual.

## Arquitectura del código (políglota: cada parte en el lenguaje que la hace más rápida)

El registro (SQLite, `data/atlas.db`) es el contrato entre lenguajes. Python escribe el registro; Rust lo compila a un snapshot binario mapeado en memoria y sirve todo lo que es caliente.

```
atlas/            Python: dominio, registro, ingesta (trabajo de E/S y glue)
  model.py        tipos del dominio: Entity, Claim, Role, Evidence, Event
  store.py        registro SQLite: DDL, eventos, migraciones, exportación JSON-LD
  ingest/         adaptadores por fuente (wikidata, mathlib, wikipedia…); licencia y procedencia
  resolve.py      resolución de entidades y fusiones reversibles
  cli.py          `python -m atlas <comando>`: orquesta ingesta y llama a los binarios Rust
core/             Rust (workspace Cargo): el motor
  atlas-core      formato de snapshot (CSR mmap, ids, enums de relación/evidencia) compilado desde SQLite
  atlas-search    tantivy (BM25 ES/EN), índice de fórmulas tipo Tangent, vectores densos + RRF y router
  atlas-paths     álgebra de composición + búsqueda multiobjetivo con nodos AND/OR
  atlas-layout    Leiden multirresolución, layout multinivel Barnes-Hut (rayon), teselas por zoom
  atlas-server    axum: API JSON + web estática; binario `atlas` con subcomandos y salida --json
web/              TypeScript + Vite + WebGL: mapa multiescala, panel, vista textual; se compila a web/dist
shapes/           SHACL, contexto JSON-LD, tabla de composición (compartida por Python y Rust)
bench/            Python: AtlasBench; llama al binario `atlas ... --json` o a la API
tests/            pytest; las pruebas de Rust viven en cada crate (`cargo test`)
data/             generado, no versionado: atlas.db, snapshot/, índices, cache/
docs/             SPEC.md, STATUS.md, ADRs
```

## Comandos

```bash
python -m atlas ingest                     # fuentes → registro SQLite
core/target/release/atlas compile          # SQLite → snapshot mmap + índices + layout (reconstruible)
core/target/release/atlas serve            # API + web en http://localhost:8765
core/target/release/atlas search "..." --json
core/target/release/atlas path A B --json
python -m atlas build                      # todo lo anterior de punta a punta
python -m bench run                        # AtlasBench
python -m pytest -q ; cargo test --workspace --release   # pruebas
```

Rust en Windows: el directorio de compilación debe quedar fuera de OneDrive y con ruta corta, si no el linker falla (LNK1104). Usar `CARGO_TARGET_DIR=C:/Users/AdrianGonzalez/.cargo-target/atlas` (un directorio por agente si compilan en paralelo). Cargo está en `~/.cargo/bin`; MSVC viene de Visual Studio Build Tools 2026.

## Convenciones

- Rendimiento es requisito: lo caliente (búsqueda, recorridos, layout, servidor) va en Rust con `--release`, LTO y paralelismo (rayon); estructuras compactas (CSR, ids `u32`, mmap) en lugar de objetos. Cada componente reporta benchmarks medidos (p50/p95, memoria, throughput) a la escala real.
- Python 3.14 para ingesta, glue y evaluación; numpy disponible. Web en TypeScript sin frameworks pesados. Toda dependencia nueva se justifica en `docs/STATUS.md`.
- Tipos en todas las firmas; dataclasses `frozen=True` para el dominio; nada se muta en sitio.
- Todo adaptador de ingesta: caché local en `data/cache/`, `User-Agent` identificable, límites de tasa, y modo offline con fixtures para las pruebas. Las pruebas no tocan la red.
- Cada componente nuevo deja una prueba que falla si la lógica se rompe; las métricas de `bench/` se reportan por clase y con intervalo, nunca solo un promedio global.
- Prosa de documentos y UI en español (con inglés como segundo idioma). Código, identificadores y commits en inglés.
- No se hace commit ni push sin que Adrián lo pida.

## Orquestación (ECC)

El trabajo grande se corre como workflow con un orquestador y trabajadores con dueño claro:

| Rol | Dueño de | Agente |
|---|---|---|
| Arquitecto | `docs/SPEC.md`, `atlas/model.py`, `atlas/store.py` (el contrato) | `ecc:architect` / general |
| Arquitecto Rust | `core/Cargo.toml`, `core/atlas-core` (snapshot y tipos compartidos) | general |
| Ingesta | `atlas/ingest/`, `atlas/resolve.py` | general |
| Búsqueda | `core/atlas-search` | general |
| Recorridos | `core/atlas-paths`, `shapes/composition.json` | general |
| Mapa | `core/atlas-layout` | general |
| Servidor y web | `core/atlas-server`, `web/` | general |
| Evaluación | `bench/` | `data-scientist` |
| Integrador | `atlas/cli.py`, build de punta a punta | general |
| Revisión | lectura crítica, sin escribir código de producto | `ecc:code-reviewer`, `ecc:python-reviewer`, `data-scientist`, `computational-mathematician` |

Reglas: nadie cambia el contrato (`model.py`, `store.py`) salvo el arquitecto o el integrador; cada trabajador escribe solo en sus carpetas; cada ronda cierra con pruebas verdes y `docs/STATUS.md` actualizado (hecho, medido, pendiente, bloqueado).
