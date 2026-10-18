//! Binary tile v1 is a cross-language contract: the encoder must reproduce the
//! golden file that web/src/tile.test.ts decodes.  Regenerate both sides together
//! with `ATLAS_WRITE_GOLDEN=1 cargo test -p atlas-server --test tile_format`.

use atlas_core::{EntityType, EvidenceState, Projection, RelationType};
use atlas_layout::{Aggregate, TileEdge, TileNode, TileNodeKind, TileResponse};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn sample() -> TileResponse {
    let node = |id: &str, kind, label: &str, x, y, size, community: &str, t, priority, central: Option<Vec<String>>| TileNode {
        id: id.into(),
        kind,
        label: label.into(),
        x,
        y,
        size,
        community: community.into(),
        entity_type: t,
        priority,
        central,
    };
    TileResponse {
        layer: Projection::Structure,
        layout_version: "lv-test".into(),
        z: 1,
        x: 1,
        y: 0,
        max_zoom: 7,
        bbox: [0.5, 0.0, 1.0, 0.5],
        nodes: vec![
            node("L0:3", TileNodeKind::Cluster, "Geometría euclidiana", 0.75, 0.25, 17, "L0:3", None, None, Some(vec!["s:pythagoras".into(), "c:triangle".into()])),
            node("s:pythagoras", TileNodeKind::Entity, "Teorema de Pitágoras", 0.625, 0.125, 1, "L0:3", Some(EntityType::Statement), Some(0), None),
        ],
        external: vec![node("L0:7", TileNodeKind::Cluster, "Álgebra", 0.25, 0.75, 0, "L0:7", None, None, None)],
        edges: vec![
            TileEdge { source: "L0:3".into(), target: "L0:7".into(), relation: RelationType::Generalizes, evidence_state: EvidenceState::Extracted, count: 12 },
            TileEdge { source: "s:pythagoras".into(), target: "L0:3".into(), relation: RelationType::Cites, evidence_state: EvidenceState::Disputed, count: 1 },
        ],
        aggregates: vec![Aggregate {
            cluster_id: "L0:3".into(),
            members: 18,
            hidden_members: 17,
            hidden_relations: BTreeMap::from([(RelationType::Cites, 4), (RelationType::DependsOnLogically, 2)]),
        }],
        hidden_edges: BTreeMap::from([(RelationType::AppearsIn, 9)]),
    }
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}

#[test]
fn binary_tile_matches_golden_and_header() {
    let bytes = atlas_server::encode_tile(&sample());
    assert_eq!(&bytes[..4], atlas_server::TILE_MAGIC);
    assert_eq!((u32_at(&bytes, 4), u32_at(&bytes, 8), u32_at(&bytes, 12)), (1, 1, 0));
    assert_eq!(u32_at(&bytes, 16), 7, "max_zoom");
    assert_eq!(f32::from_le_bytes(bytes[20..24].try_into().unwrap()), 0.5);
    // strings: lv-test, L0:3, Geometría…, s:pythagoras, Teorema…, L0:7, Álgebra, c:triangle
    assert_eq!(u32_at(&bytes, 44), 8);
    let counts: Vec<u32> = (0..7).map(|i| u32_at(&bytes, 48 + 4 * i)).collect();
    assert_eq!(counts, [2, 1, 2, 1, 2, 1, 2], "nodes, external, edges, aggs, agg_rel, hidden, central");
    assert_eq!(bytes.len() % 4, 0);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).canonicalize().unwrap();
    let golden = dir.join("../../web/tests/fixtures/tile-v1.bin");
    if std::env::var_os("ATLAS_WRITE_GOLDEN").is_some() {
        std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
        std::fs::write(&golden, &bytes).unwrap();
    }
    let want = std::fs::read(&golden).expect("golden tile missing: run with ATLAS_WRITE_GOLDEN=1");
    assert_eq!(bytes, want, "binary tile v1 changed: update web/src/tile.ts and the golden together");
}
