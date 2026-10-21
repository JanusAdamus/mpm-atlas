# core/ · motor Rust del atlas

Workspace Cargo con cinco crates.  El registro SQLite (`data/atlas.db`, esquema v1 que escribe `atlas/store.py`) es el contrato entre lenguajes; `atlas-core` lo compila a un snapshot binario mapeado en memoria y los demás crates leen ese snapshot sin copiarlo.

| Crate | Estado (2026-10-16) | Dueño |
|---|---|---|
| `atlas-core` | completo: vocabulario, formato, compilador, cargador, verificación, estadísticas, generador sintético | arquitecto Rust |
| `atlas-search` | lleno por búsqueda: tantivy, fórmulas, vectores densos | búsqueda |
| `atlas-paths` | lleno por recorridos: álgebra de composición, búsqueda con nodos AND/OR | recorridos |
| `atlas-layout` | lleno por mapa: Leiden, layout multinivel, teselas | mapa |
| `atlas-server` | lleno por servidor: rutas axum y binario `atlas` con todos los subcomandos | servidor y web |

`cargo build --release --workspace` y `cargo test --workspace` pasan (80 pruebas de Rust, 2026-10-16).  Las cifras de los crates que no son `atlas-core` las reporta cada dueño.

## Comandos

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd core
cargo build --release --workspace            # target-dir por defecto: C:/Users/AdrianGonzalez/.cargo-target/atlas
cargo test --workspace
<target>/release/atlas compile --db ../data/atlas.db --out ../data/snapshot --json
<target>/release/atlas stats --snapshot ../data/snapshot --verify --json
cargo run --release -p atlas-core --example bench_snapshot -- --dir C:/Users/AdrianGonzalez/.cargo-target/bench-1m --entities 1000000 --edges 10000000 --reps 5
python core/atlas-core/vocab_gen.py [--check]                     # regenera / revisa shapes/vocab.json
python core/atlas-core/tests/fixtures/make_fixture.py             # regenera el registro de prueba
```

`core/.cargo/config.toml` fija el directorio de compilación fuera de OneDrive (si no, LNK1104) y las banderas del SQLite empaquetado.  `CARGO_TARGET_DIR` en el entorno lo sustituye; cada trabajador en paralelo usa el suyo.  Con `--json`, `atlas` imprime un solo valor JSON; los errores salen como `{"error": {"code", "message"}}` (códigos de SPEC §15 más `not_implemented`) con código de salida 1.  `serve`, `search`, `path` y `layout` llaman a las funciones de su crate.

## Vocabulario compartido

`atlas_core::vocab` repite los enums de `atlas/model.py` en el mismo orden: `EntityType` (18), `RelationType` (17), `EvidenceState` (7), `Role` (6), `Projection` (6), más `RELATIONS` (proyección, simetría, transitividad, inversa, premisas).  El código `u8` de cada variante es su posición en Python.  Tres comprobaciones lo mantienen alineado:

- `tests/vocab.rs` lee `atlas/model.py` como texto (clases `StrEnum` y llamadas `_spec(...)`) y compara;
- el mismo archivo compara con `shapes/vocab.json`, generado desde Python por `vocab_gen.py`;
- `tests/test_vocab_json.py` (pytest) falla si `shapes/vocab.json` no coincide con `atlas.model`.

Cada snapshot guarda `vocab_hash`; un snapshot compilado con otro vocabulario no abre.  `shapes/composition.json` (versión inicial; la refina recorridos) se prueba en `tests/vocab.rs`: solo nombra relaciones, estados y conclusiones conocidos, y `chain = transitive` coincide con `RELATIONS`.

## Formato del snapshot (v1)

Un archivo `data/snapshot/atlas-<hash16>.snap` y un `CURRENT` con su nombre.  Little-endian, mapeado en sitio (memmap2 + bytemuck, sin copiar).

```
[Header 256 B][tabla de secciones 24 B × n][sección alineada a 64 B]...
```

El header lleva magic `ATLSNAP\0`, versión de formato, versión de esquema del registro, `vocab_hash`, el hash BLAKE3 del archivo completo (con ese campo en ceros), `source_event_seq` y los conteos.  Ids `u32`; `NONE = u32::MAX`.

| Grupo | Secciones | Contenido |
|---|---|---|
| Cadenas | `StrOffsets`, `StrBytes` | tabla de cadenas UTF-8 (ids, etiquetas ES/EN, supuestos, localizadores, licencias); desde el formato v3 los desplazamientos son `u64` |
| Presentación (v3) | `NodeFlags`, `NodeShowEs`, `NodeShowEn` | curaduría activa (ADR-0005 §5): bits de exclusión por alcance y rótulos de presentación |
| Nodos | `NodeId`, `NodeType`, `NodeCanon`, `NodeLabelEs`, `NodeLabelEn`, `NodeArea`, `NodeClaimCount`, `NodeStateMask`, `NodeHash`, `MergedNodes` | una fila por entidad; fusiones como redirección (`NodeCanon`); `NodeLabelEn` cae a la primera etiqueta, igual que `Entity.label`; `NodeClaimCount = 0` es cobertura "desconocida" |
| Afirmaciones | `ClaimId`, `ClaimRel`, `ClaimState`, `ClaimSubject`, `ClaimGroup`, `ClaimTgtOff/ClaimTgt`, `ClaimCtxOff/ClaimCtxNode/ClaimCtxRole`, `ClaimAsmOff/ClaimAsm`, `ClaimValidity`, `ClaimHash` | todas las afirmaciones, en cualquier estado; `ClaimTgt` es el objeto o el conjunto de premisas (nodo AND) |
| Grupos | `GroupId`, `GroupOff`, `GroupClaims` | alternativas (nodo OR), ordenadas por id |
| Procedencia | `ProvOff`, `ProvSource`, `ProvLocator`, `ProvLicense`; `SourceId`, `SourceTitle`, `SourceLicense` | afirmación → fuente, localizador, licencia |
| Aristas | `OutOff/OutNbr/OutClaim/OutRel/OutState`, `InOff/InNbr/InClaim/InRel/InState` | CSR tipado por dirección; solo afirmaciones vigentes (`CURRENT_STATES`); una arista por objeto o premisa; extremos ya resueltos por fusiones; ordenado por (nodo, relación, vecino, afirmación) |

`NodeHash` y `ClaimHash` son tablas hash en disco (`hashidx.rs`: 64 regiones, sondeo lineal, carga ≤ 0.5, construcción paralela y determinista).  Las aristas del CSR coinciden con `Store.iter_edges()` (prueba `csr_equals_store_edge_semantics`).

Consistencia: el compilador lee con varias conexiones en paralelo.  Como toda escritura del registro deja un evento, compara `MAX(events.seq)` antes y después; si cambió, reintenta.  El snapshot es exactamente el registro después del evento `source_event_seq`.  Mismo registro, mismo hash (prueba `compile_is_deterministic`).

Integridad: `Snapshot::open` revisa header, tabla, longitudes y extremos de cada arreglo de offsets en O(número de secciones).  Un archivo corrupto puede hacer que un accesor entre en pánico por límites, nunca comportamiento indefinido.  `verify()` recalcula el hash y revisa cada referencia, código de enum y cadena UTF-8.

## API pública de `atlas-core`

```rust
compile(&CompileOptions { db, out_dir, threads, chunks_per_thread, core_content_hash }) -> Result<CompileReport>
CompileOptions::new(db, out_dir); current_snapshot(dir) -> Result<PathBuf>
Snapshot::open(file_or_dir) -> Result<Snapshot>;  .verify(); .verify_hash(); .stats() -> SnapshotStats
  .header() .path() .file_len() .content_hash_hex() .num_nodes() .num_claims() .num_edges() .num_groups() .num_sources()
  .str(StrId) -> &str
  .node_id(n) .node_type(n) .label(n, lang) .canonical(n) .is_merged(n) .merged_from(n) .area(n) .evidence(n) -> EvidenceSummary
  .find_node(id) -> Option<NodeIx> (tal cual) ; .resolve(id) -> Option<NodeIx> (sigue fusiones) ; .node_types() -> &[u8]
  .csr(Dir::Out | Dir::In) -> Csr { offsets, nbr, claim, rel, state }   // slices sobre el mmap
  .out_edges(n) .in_edges(n) -> EdgeSlice ; Csr::edges(n) / edges_rel(n, RelationType) / degree(n) / range(n)
  EdgeSlice { nbr, claim, rel, state }: .len() .get(i) -> EdgeRef { nbr, claim, relation, state } .iter()
  .claim_id(c) .find_claim(id) .claim_relation(c) .claim_state(c) .claim_subject(c)
  .claim_targets(c) -> &[NodeIx] (nodo AND) .conjunct_count(c) .claim_context(c) -> (Role, NodeIx)*
  .claim_assumptions(c) -> &str* .claim_validity_domain(c) .claim_group(c) .claim_provenance(c) -> ProvRef*
  .claim_states() .claim_relations() -> &[u8]
  .group_id(g) .find_group(id) .group_claims(g) -> &[ClaimIx] (nodo OR)
  .source_id(s) .source_title(s) .source_license(s)
EvidenceSummary { claims, states }: .is_unknown() .has(state) .strongest()
Error { Io, Sqlite, SqlValue, Registry, Format, Integrity, NotFound, BadRequest, NotImplemented }: .code() .to_json()
vocab::{EntityType, RelationType, EvidenceState, Role, Projection, RELATIONS, RelationSpec, CURRENT_STATES_MASK, vocab_hash}
  cada enum: ALL, COUNT, as_str(), code(), from_code(u8), bit(), FromStr, Display, serde como cadena
synth::generate(db, &SynthConfig { entities, edges, seed }) -> Result<SynthReport>   // solo para benchmarks
```

## Rendimiento medido

Hardware: Intel Core Ultra 7 255H (16 hilos), 31.5 GB de RAM, Windows 11 Pro, rustc 1.99.1, perfil release (LTO fat, 1 unidad de código).  Fecha: 2026-10-16.  Otros procesos del usuario corrían a la vez; hay ruido entre corridas, por eso se dan rangos.  Caché de páginas tibia (no se puede vaciar en Windows sin privilegios): son tiempos en caliente.

Datos: registro sintético de `synth::generate` con esquema creado por `atlas.store.Store`: 1,000,000 entidades (10 tipos), 8,601,826 afirmaciones (13 relaciones, con tipos de extremos válidos), 19,340,236 participantes, 1,068,258 conjunciones, 127,342 grupos de alternativas, ids con forma de `stable_id` (orden aleatorio en los b-trees), destinos con grado sesgado, 98% vigentes.  Archivo: 8.48 GB.  Generarlo tomó 258 s.  No tiene filas FTS ni un evento por fila: sirve para medir el snapshot, no para probar el significado.

Comando: `bench_snapshot --entities 1000000 --edges 10000000 --reps 5` (salida JSON completa en cada corrida).

| Medida | Resultado |
|---|---|
| Compilar (SQLite → snapshot), 5 corridas | 7.27 / 7.56 / 7.62 / 7.81 / 8.18 s (mediana 7.62 s) |
| Fases típicas | entidades 0.11 s, afirmaciones 1.2–1.5 s, índice de ids 0.44–0.59 s, participantes 2.5–2.9 s, procedencia 0.9–1.1 s, ensamblado 0.84–0.95 s, CSR 0.37–0.49 s, escritura 0.31–0.48 s |
| Memoria pico al compilar | 1.51–1.55 GiB privada; 2.5–2.6 GiB de working set (incluye páginas mapeadas del registro) |
| Snapshot | 1,071,693,696 bytes; 10,000,000 aristas por dirección; 11,851,065 cadenas |
| Abrir (`Snapshot::open`) | p50 0.10–0.15 ms, p95 0.14–0.44 ms (20 aperturas) |
| `verify()` (hash + referencias) | 61–72 ms; primer cálculo de hash solo: 85–96 ms |
| Recorrer todo el CSR de salida, 1 hilo | 0.60–0.76 G aristas/s |
| Recorrer todo el CSR, 16 hilos (rayon) | salida 5.2–6.3 G aristas/s; entrada 4.1–5.6 G aristas/s |
| Vecinos tipados de un nodo al azar (`edges_rel`, salida + entrada) | 97–210 ns por consulta |
| Expansión a 2 saltos desde 2,000 semillas | p50 17–19 µs, p95 164–206 µs (alcanza p50 2,512 y p95 71,171 nodos) |
| `find_node(id)` al azar | 560–830 ns |

Registro real (`data/atlas.db` de la ingesta, 2026-10-16, esquema v1, evento 801,796): 286,757 entidades, 515,036 afirmaciones (270,103 `extracted`, 244,933 `formally_linked`), 1,512,223 participantes, 2 fuentes, 0 grupos de alternativas.  `atlas compile --json`, 3 corridas: 1.13 / 1.08 / 1.08 s, mismo `content_hash` `0dd04e66…ecc6` las tres veces.  Snapshot de 128,635,055 bytes, 728,303 aristas por dirección, 1,609,747 cadenas.  `atlas stats --verify`: abrir 0.29 ms, verificar 22 ms, sin errores.  Las 286,757 filas de `provenance` con `entity_id` no entran al snapshot (solo procedencia de afirmaciones); por eso `provenance_rows` = afirmaciones.

Repetición del benchmark sintético con el build actual (mismo registro de 1M/10M, `--reps 3`): compilar 6.46 y 6.11 s en las dos corridas visibles, memoria privada pico 1.53 GiB; abrir p50 0.10 ms, p95 0.18 ms; `verify()` 61 ms; recorrer CSR 0.73 G aristas/s en un hilo, 6.3 G (salida) y 5.3 G (entrada) con 16; vecinos tipados 104 ns; 2 saltos p50 16.9 µs, p95 166 µs; `find_node` 600 ns.  Dentro de los rangos de la tabla.

Dos decisiones medidas en el camino (mismo registro):

- Lectura paralela del registro con `mmap` de SQLite en lugar de `ReadFile`: con 16 hilos, la lectura de afirmaciones del registro de 20k entidades bajó de 1.27 s a 0.19 s; sin mmap, más hilos eran más lentos que uno.  Por eso `.cargo/config.toml` sube `SQLITE_MAX_MMAP_SIZE` (por defecto 2 GiB) y apaga `MEMSTATUS`.
- Escanear `claim_participants` y `provenance` en orden de llave y resolver ids con tablas hash, en vez de un JOIN que busca por afirmación: la compilación completa pasó de 18.1–19.8 s a 7.3–8.2 s; los ids ordenados para búsqueda binaria se cambiaron por `NodeHash`/`ClaimHash` (de 3.4–3.9 s de ordenamiento a 0.44–0.59 s, y `find_node` de 5.5 µs a 0.56–0.83 µs).

## Límites conocidos

- Formato v1: hasta 2^32 − 1 nodos, afirmaciones, aristas y cadenas, y 4 GiB de texto.  El compilador falla con un error claro si se pasa.
- El snapshot no guarda alias, descripciones, enunciados, fórmulas ni explicaciones: búsqueda los lee del registro al construir sus índices, y el servidor lee el registro para las vistas de detalle.
- Resolver ids de participantes cuesta ~1.4 µs por fila en un hilo (fallos de caché y TLB en tablas de GB); es la fase más larga.  El siguiente paso medible es la migración v2 del contrato (llaves enteras en `claim_participants` y `provenance`, ya anotada en SPEC §16).
- El proyecto vive en `C:\MPM`, fuera de OneDrive (movido el 2026-10-17), así que `data/` y los snapshots no se sincronizan.
