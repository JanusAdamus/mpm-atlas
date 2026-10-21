//! Which registry rows a compile reads (ADR-0005 §8.4): the core snapshot, or the works layer.
//!
//! Membership is by id prefix (`shapes/works_layer.json`, shared with `atlas/ingest/works`): an
//! entity is a works entity iff its id starts with one of `entity_id_prefixes`, a claim is a works
//! claim iff its id starts with `claim_id_prefix`.  The core reads everything else.  The layer reads
//! its own rows plus the core entities its claims point at (stubs: concepts, areas, registry persons)
//! and the targets of merges out of the layer, so every participant and merge resolves in the file.
//! GLOB on the primary-key columns keeps both filters index range scans.

use rusqlite::Connection;

use crate::error::{Error, Result};

const LAYER_SPEC: &str = include_str!("../../../shapes/works_layer.json");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    Core,
    Works,
}

#[derive(serde::Deserialize)]
struct Spec {
    entity_id_prefixes: Vec<String>,
    claim_id_prefix: String,
}

fn spec() -> Result<Spec> {
    serde_json::from_str(LAYER_SPEC).map_err(|e| Error::Registry(format!("shapes/works_layer.json: {e}")))
}

fn glob(col: &str, prefix: &str) -> String {
    // prefixes are fixed ASCII in the shapes file; reject anything that could break the SQL
    assert!(prefix.chars().all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '_' || c == '-'));
    format!("{col} GLOB '{prefix}*'")
}

/// `(… OR …)`: the column holds a works-entity id.
pub(crate) fn works_entity(col: &str) -> Result<String> {
    let s = spec()?;
    Ok(format!("({})", s.entity_id_prefixes.iter().map(|p| glob(col, p)).collect::<Vec<_>>().join(" OR ")))
}

/// The id names a works-layer entity (same prefixes as [`works_entity`]).
pub(crate) fn is_works_id(id: &str) -> Result<bool> {
    Ok(spec()?.entity_id_prefixes.iter().any(|p| id.starts_with(p.as_str())))
}

/// `AND …` restricting entity rows (column `id`) to the scope.  The works layer adds its stubs separately.
pub(crate) fn entity_filter(scope: Scope) -> Result<String> {
    let w = works_entity("id")?;
    Ok(match scope {
        Scope::Core => format!(" AND NOT {w}"),
        Scope::Works => format!(" AND {w}"),
    })
}

/// `AND …` restricting rows whose column `col` holds a claim id to the scope.
pub(crate) fn claim_filter(scope: Scope, col: &str) -> Result<String> {
    let g = glob(col, &spec()?.claim_id_prefix);
    Ok(match scope {
        Scope::Core => format!(" AND NOT {g}"),
        Scope::Works => format!(" AND {g}"),
    })
}

/// Core entities the works layer must carry: participants of works claims and merge targets of works
/// entities, closed over `merged_into` (rowid order, so the output is deterministic).
pub(crate) fn stub_ids(conn: &Connection) -> Result<Vec<String>> {
    let we = works_entity("e.id")?;
    let cw = glob("p.claim_id", &spec()?.claim_id_prefix);
    let sql = format!(
        "WITH RECURSIVE s(id) AS ( \
           SELECT p.entity_id FROM claim_participants p WHERE {cw} \
           UNION SELECT e.merged_into FROM entities e WHERE {we} AND e.merged_into IS NOT NULL \
           UNION SELECT e.merged_into FROM entities e JOIN s ON e.id = s.id WHERE e.merged_into IS NOT NULL) \
         SELECT e.id FROM entities e WHERE e.id IN (SELECT id FROM s) AND NOT {we} ORDER BY e.rowid"
    );
    let mut st = conn.prepare(&sql)?;
    let ids = st.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ids)
}
