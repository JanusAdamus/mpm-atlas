# Continuar el Atlas en otra máquina

Escrito el 2026-10-21, al migrar el proyecto de la laptop (`C:\MPM`) a la computadora buena de Adrián, por espacio en disco.  Lo lee el agente que retome el desarrollo.  El último commit en la laptop es el que trae este archivo.

## 1. Qué es esto y qué leer primero

MPM es el «Atlas de las conexiones matemáticas»: proyecto personal de Adrián González (AJ).  No es de Delos Advisors ni de clientes.  Adrián pide ambición, no conservadurismo: funcionalidad real a escala real, con datos reales.

Antes de proponer nada, leer en este orden:

1. `CLAUDE.md`: reglas no negociables, arquitectura, comandos y orquestación por roles.
2. `docs/STATUS.md` §16: cierre de la ronda 5, con las decisiones pendientes de Adrián (§16.7) y la siguiente ronda (§16.8).  Leer también §13–15 para la historia.
3. `docs/plans/r5/estado.md`: bitácora por puntos de control (1–12) de la ronda 5.
4. `docs/adr/0005-esquema-v4-ronda-5.md`: el contrato vigente (esquema v4).
5. `research/2026-10-16/`: informe fuente y plan de 18 meses.  Es registro histórico y no se edita.

## 2. Dónde quedó

- **Ronda 5 cerrada** (2026-10-21).  Snapshot publicado `atlas-19c6f7dc19dab3e9`, que cubre hasta `event_seq` 4,188,644.
  - Reglas del plan: 3 PASS, 6 FAIL, 1 NO EVALUABLE.
  - Todas las pruebas están en verde: pytest 280, cargo 168/0, clippy 0, npm 93/93.
- **Lo que enseña algo falso todavía** (§16.6):
  - pertenencias homónimas en las aristas agregadas entre conceptos (muestra de 20: 1/10 útiles);
  - P279 publicado como «caso particular».
- **Nada en curso.**  No quedó ningún workflow ni proceso corriendo.

**Primero van las decisiones de Adrián (§16.7).**  No adivinarlas ni tomarlas por él.  Hay que preguntárselas:

1. P279 (`docs/plans/r5/p279-opciones.md`; se recomienda la opción B).
2. Revisar los 20 borradores de fichas (`docs/plans/r5/fichas/`).
3. Ingesta completa de obras (`docs/plans/r5/piloto-obras.md` §8).
4. Bench: el sealed de v3.1 y la versión por omisión.
5. Las 36 fusiones y la cola de traducción al español.
6. El piloto del Coherence Map.

## 3. Qué NO viene en git y hay que copiar a mano

`data/` está en `.gitignore`.  **El registro no se puede reconstruir:** guarda eventos, curaduría, M2 y fichas importadas, y se ingirió de APIs vivas.  Copiar con el proyecto cerrado; no debe haber ningún proceso con la base abierta.

| Ruta (desde la raíz) | Tamaño | ¿Hace falta? |
|---|---|---|
| `data/atlas.db` | 6.0 GB | **Sí, imprescindible.**  Es la fuente de verdad.  Comprobar después con `PRAGMA quick_check` |
| `data/cache/` | 133 MB | **Sí.**  Cachés HTTP para la ingesta offline y para las pruebas de los adaptadores |
| `data/index/emb-multilingual-e5-small.cache` | 125 MB | **Muy recomendable.**  Sin él, el índice neuronal se recodifica en frío: unos 47 min |
| `bench/splits/` y `bench/silver/` (archivos `.jsonl`) | 64 MB | **Sí.**  Ítems del bench; están ignorados y solo los manifiestos van en git |
| `data/pilot/atlas-pilot-v2.db`, `data/pilot/works/`, `data/pilot/logs/` y los scripts `data/pilot/*.py`, `*.ps1` | unos 7.2 GB | Sí, si se va a seguir con las obras.  Es el piloto de 114,736 obras |
| `data/pilot/atlas-pilot.db` | 6.9 GB | **No.**  Copia vieja e inconsistente del piloto.  Su borrado está pendiente de Adrián |
| `data/backups/atlas-v3-2026-10-19.db` | 5.9 GB | Opcional.  Respaldo del registro antes de la migración a v4 |
| `data/snapshot/`, `data/index/<hash>/`, `data/layout/` | unos 5.5 GB | No.  Son derivados: `python -m atlas build` los reconstruye en unos 2 min si está la caché de embeddings |
| `docs/design/**/*.png` | dentro de 768 MB | Opcional.  Capturas de diseño de las rondas 4 y 5 (no versionadas) |

Mínimo para seguir: `data/atlas.db`, `data/cache/`, la caché de embeddings y los `.jsonl` del bench, unos 6.3 GB.

## 4. Preparar la máquina nueva

Versiones con las que se construyó (laptop, Windows 11):

- Python 3.14.6, con numpy.  No hay más dependencias de Python.
- Rust: cargo 1.99.1, con MSVC de Visual Studio Build Tools 2026.
- Node 24.19.0.  En `web/`: `npm ci`.
- rtk 0.44.2, opcional: proxy que recorta la salida de comandos para ahorrar tokens.
- Claude Code con:
  - el plugin `ecc@ecc`, que trae los agentes especializados (`ecc:rust-reviewer`, `ecc:python-reviewer`, `ecc:typescript-reviewer`, `ecc:silent-failure-hunter`, `ecc:architect`…);
  - los hooks de ECC en perfil `standard`, sin GateGuard: `ECC_HOOKS_ENABLED=true`, `ECC_HOOK_PROFILE=standard`, `ECC_GATEGUARD=off` y `ECC_DISABLED_HOOKS=pre:observe:continuous-learning,stop:desktop-notify` en el `env` de `~/.claude/settings.json`;
  - «Dynamic workflows» prendido en `/config`, para la herramienta Workflow.

**Ajustar rutas propias de la laptop:**

- `CLAUDE.md` fija `CARGO_TARGET_DIR=C:/Users/AdrianGonzalez/.cargo-target/atlas`.  En la máquina nueva se usa la ruta equivalente: corta, fuera de OneDrive y una sola para todos los agentes.  Si cambia, se actualiza en `CLAUDE.md`.  Ese directorio no se copia: se recompila.
- Si el repo no queda en `C:\MPM`, se cambian las rutas absolutas de `CLAUDE.md`.  Buscar `C:\MPM` y `C:/MPM` con grep: aparecen en algunos docs y scripts de `docs/plans/r5/consultas/`.
- Ningún proceso del proyecto puede dejar ventanas de terminal abiertas.  Python lanza a sus hijos con `atlas.NO_WINDOW` y Rust con `CREATE_NO_WINDOW`.  Cada agente mata lo que lanzó.

## 5. Verificar la migración (antes de cualquier trabajo nuevo)

```bash
git clone https://github.com/JanusAdamus/mpm-atlas.git C:/MPM   # o la ruta elegida
# copiar los archivos de la sección 3 a las mismas rutas relativas
python -c "import sqlite3; c=sqlite3.connect('data/atlas.db'); print(c.execute('pragma quick_check').fetchone(), c.execute('select max(seq) from events').fetchone())"
python -m pytest -q                                   # esperado: 280 passed
cd core && cargo test --workspace --release           # esperado: 168 passed, 0 failed
cd web && npm ci && npm test && npx tsc --noEmit      # esperado: 93/93, 0 errores
python -m atlas build --json                          # reconstruye snapshot, índice y layout
python docs/plans/r5/consultas/r5_gate.py             # esperado: 4/4 PASA
```

- El `max(seq)` esperado de la tabla `events` es 4,188,644.
- El hash del snapshot reconstruido debe coincidir con `19c6f7dc19dab3e9` si el registro llegó intacto.  Si no coincide, hay que investigarlo antes de seguir.
- Para comprobar que no hay regresión, AtlasBench v3.1 por HTTP debe dar 244/245 respondibles y 204/204 adversariales (`docs/STATUS.md` §16.3).

## 6. Cómo se trabaja aquí

- **Rondas orquestadas:**
  - un plan aprobado por Adrián;
  - un harness con dueños por rol (`docs/plans/r5/harness.md` es la plantilla);
  - workflows con trabajadores especializados (agentes de ECC para revisar);
  - dos jueces independientes y el cálculo de su acuerdo;
  - cierre en `docs/STATUS.md`.
- Cada punto de control se anota en el `estado.md` de la ronda.
- En los prompts de los trabajadores se dice que **todas** las tareas listadas están en alcance.  En la ronda 5, un integrador leyó un mensaje de Adrián como si limitara sus tareas y no reconstruyó el snapshot.
- Ninguna cifra sin su fuente (archivo, fecha, comando).  Las metas no son resultados.
- No se hace commit ni push sin que Adrián lo pida.
- Los cortes por límite de uso o por `/login` pueden matar los workflows.  Al retomar, revisar qué sigue vivo y reanudar con `resumeFromRunId`; el agente que se cortó retoma el árbol.
- Las memorias del agente en la laptop (`~/.claude/projects/C--MPM/memory/`) no viajan con el repo.  Su contenido esencial está en este archivo y en `CLAUDE.md`.

## 7. Siguiente ronda propuesta (§16.8), una vez resueltas las decisiones

1. Homónimos en las aristas agregadas: revisar la cola de 1,093 casos (`docs/plans/r5/homonimos-pendientes.tsv`) y el emparejamiento por una sola palabra de Mathlib que los produce.
2. P279, según lo que decida Adrián.
3. Contenido revisado para las fichas y prerrequisitos registrados.  «Qué saber antes» está vacía en 7/7.
4. La búsqueda con 8 clientes está en el límite (p95 100.9 ms).  Resolverlo antes de crecer el grafo con las obras.
