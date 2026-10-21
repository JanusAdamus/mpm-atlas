//! The concept layer of the map, on the committed fixture plus four `member_of_concept`
//! claims (same as core/atlas-core/tests/concepts.rs): general zooms show only
//! concepts and areas, declarations sit next to their concept and appear only from
//! `detail_zoom`, and aggregated edges are exactly the real edges under the new
//! representatives (nothing invented, nothing lost).

use atlas_core::{CompileOptions, Dir, EntityType, Projection, RelationType, Snapshot, compile};
use atlas_layout::{
    ComputeOptions, Layout, LayoutParams, Prior, TileBudget, TileNodeKind, compute, compute_with,
    tile,
};
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

/// (claim, declaration, concept, confidence)
const MEMBERS: [(&str, &str, &str, f64); 4] = [
    ("c:m1", "s:pythagoras", "c:right-triangle", 0.9),
    ("c:m2", "s:pythagoras", "c:lonely", 0.5),
    ("c:m3", "s:sas-congruence", "c:lonely", 0.8),
    ("c:m4", "s:parallelogram-area", "c:lonely", 0.7),
];

fn snapshot(name: &str) -> Snapshot {
    snapshot_with(name, &[])
}

/// The fixture with active curation rows (kind, target, payload) of ADR-0005 §5 (schema v4).
fn snapshot_with(name: &str, curation: &[(&str, &str, &str)]) -> Snapshot {
    let dir = std::env::temp_dir().join(format!(
        "atlas-layout-concepts-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("atlas.db");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../atlas-core/tests/fixtures/mini_atlas.db"),
        &db,
    )
    .unwrap();
    let c = Connection::open(&db).unwrap();
    for (id, s, o, conf) in MEMBERS {
        let data = format!(
            r#"{{"id":"{id}","relation":"member_of_concept","participants":[{{"role":"subject","entity":"{s}"}},{{"role":"object","entity":"{o}"}}],"evidence_state":"model_proposed","confidence":{{"value":{conf},"estimates":"e","calibration":"not calibrated"}}}}"#
        );
        c.execute(
            "INSERT INTO claims VALUES (?1,'member_of_concept','model_proposed',NULL,NULL,NULL,NULL,?2,'t')",
            params![id, data],
        )
        .unwrap();
        c.execute(
            "INSERT INTO claim_participants VALUES (?1,'subject',?2,0)",
            params![id, s],
        )
        .unwrap();
        c.execute(
            "INSERT INTO claim_participants VALUES (?1,'object',?2,1)",
            params![id, o],
        )
        .unwrap();
        c.execute(
            "INSERT INTO provenance VALUES (?1,NULL,'src:fixture-hand','test','concept-projection','2','CC0-1.0','t',NULL)",
            params![id],
        )
        .unwrap();
    }
    if curation.is_empty() {
        c.execute_batch("PRAGMA user_version = 2").unwrap();
    } else {
        c.execute_batch(
            "CREATE TABLE curation(id INTEGER PRIMARY KEY, kind TEXT NOT NULL, target_id TEXT NOT NULL,
               rule TEXT NOT NULL, rule_version TEXT NOT NULL, payload TEXT NOT NULL, actor TEXT NOT NULL,
               reason TEXT NOT NULL, created_at TEXT NOT NULL, undone_at TEXT, undone_by TEXT, undo_reason TEXT);
             PRAGMA user_version = 4;",
        )
        .unwrap();
        for (kind, target, payload) in curation {
            c.execute(
                "INSERT INTO curation(kind, target_id, rule, rule_version, payload, actor, reason, created_at)
                 VALUES (?1, ?2, 'test', '1', ?3, 'curation:test/1', 'test', 't')",
                params![kind, target, payload],
            )
            .unwrap();
        }
    }
    drop(c);
    compile(&CompileOptions::new(db, dir.join("snapshot"))).expect("compile");
    Snapshot::open(dir.join("snapshot")).unwrap()
}

fn params() -> LayoutParams {
    LayoutParams {
        resolutions: vec![0.5, 2.0],
        tiles: TileBudget {
            entities: 3,
            clusters: 1,
            edges: 1000,
            full: 1,
            max_zoom: 8,
            general_zoom: 1,
            general_nodes: 2,
            detail_zoom: 3,
            ..Default::default()
        },
        ..Default::default()
    }
}

type EdgeKey = (String, String, RelationType, String);

#[test]
fn general_layer_shows_only_concepts_and_counts_every_edge() {
    let s = snapshot("general");
    let p = params();
    let l = compute(&s, &p).unwrap();
    let meta = &l.tiles.meta;
    assert_eq!(meta.general_zoom, Some(1));
    assert_eq!(
        meta.general_nodes, 2,
        "the cap keeps the two concepts, not the area"
    );
    assert!(
        meta.zmax >= p.tiles.detail_zoom,
        "fixture spans the detail zoom"
    );
    let n = |id: &str| s.resolve(id).unwrap();
    let decls = ["s:pythagoras", "s:sas-congruence", "s:parallelogram-area"];

    // Declarations sit in the disc of their primary concept.
    for d in decls {
        let c = s.primary_concept(n(d)).unwrap();
        let k = s.concept_members(c).len() as f64;
        let (a, b) = (l.positions[n(d) as usize], l.positions[c as usize]);
        let dist = (((a[0] - b[0]) as f64).powi(2) + ((a[1] - b[1]) as f64).powi(2)).sqrt();
        assert!(
            dist <= 5e-4 * k.sqrt() + 1e-5,
            "{d} is {dist} from its concept"
        );
    }

    let out = s.csr(Dir::Out);
    let membership = RelationType::MemberOfConcept;
    let active = (0..s.num_nodes()).filter(|&v| !s.is_merged(v)).count();
    for z in 0..=meta.zmax {
        let general = z <= 1;
        let mut visible: HashSet<String> = HashSet::new();
        let mut got: BTreeMap<EdgeKey, u32> = BTreeMap::new();
        let mut internal: BTreeMap<(String, RelationType), u32> = BTreeMap::new();
        let mut hidden: BTreeMap<RelationType, u32> = BTreeMap::new();
        let mut represented = 0u32;
        let mut clustered = 0u32;
        for (tz, x, y) in l.tiles.keys().filter(|k| k.0 == z) {
            for &layer in Projection::ALL.iter() {
                let t = tile(&s, &l, layer, tz, x, y, "es").unwrap();
                for nd in t.nodes.iter().chain(&t.external) {
                    if general {
                        assert_ne!(
                            nd.kind,
                            TileNodeKind::Cluster,
                            "no clusters in the general layer"
                        );
                        let v = n(&nd.id);
                        assert!(
                            s.concept_index(v).is_some() || s.node_type(v) == EntityType::Area,
                            "{} is neither a concept nor an area at z{z}",
                            nd.id
                        );
                    }
                    if nd.kind == TileNodeKind::Concept {
                        assert_eq!(nd.size as usize, s.concept_members(n(&nd.id)).len());
                    }
                }
                for nd in t.nodes.iter().filter(|nd| nd.kind != TileNodeKind::Cluster) {
                    visible.insert(nd.id.clone());
                }
                for e in &t.edges {
                    let k = (
                        e.source.clone(),
                        e.target.clone(),
                        e.relation,
                        e.evidence_state.to_string(),
                    );
                    let prev = got.insert(k, e.count);
                    assert!(prev.is_none() || prev == Some(e.count));
                }
                for a in &t.aggregates {
                    for (r, c) in &a.hidden_relations {
                        internal.insert((a.cluster_id.clone(), *r), *c);
                    }
                    if layer == Projection::Structure {
                        let kind = t
                            .nodes
                            .iter()
                            .find(|nd| nd.id == a.cluster_id)
                            .map(|nd| nd.kind);
                        match kind {
                            Some(TileNodeKind::Concept) => represented += a.hidden_members,
                            Some(TileNodeKind::Cluster) => clustered += a.hidden_members,
                            _ => panic!("aggregate without its node"),
                        }
                    }
                }
                for (r, c) in &t.hidden_edges {
                    *hidden.entry(*r).or_default() += c;
                }
            }
        }
        if general {
            assert!(
                visible.len() <= 2,
                "z{z}: {} nodes in the general layer",
                visible.len()
            );
        }
        if z < p.tiles.detail_zoom {
            for d in decls {
                assert!(
                    !visible.contains(d),
                    "{d} visible at z{z}, before the detail zoom"
                );
            }
        }
        // Independent recount with the documented representatives.
        let dl = meta.display_level[z as usize].map(|d| d as usize);
        let rep = |v: u32| -> Option<String> {
            let id = s.node_id(v).to_string();
            if visible.contains(&id) {
                return Some(id);
            }
            if let Some(c) = s
                .primary_concept(v)
                .filter(|&c| visible.contains(s.node_id(c)))
            {
                return Some(s.node_id(c).to_string());
            }
            if general {
                return None;
            }
            let d = dl.expect("hidden nodes only below zmax");
            Some(l.community_id(d, l.levels[d][v as usize]))
        };
        let mut want: BTreeMap<EdgeKey, u32> = BTreeMap::new();
        let mut want_internal: BTreeMap<(String, RelationType), u32> = BTreeMap::new();
        let mut want_hidden: BTreeMap<RelationType, u32> = BTreeMap::new();
        let mut want_unplaced = 0u64;
        for u in 0..s.num_nodes() {
            if s.is_merged(u) {
                continue;
            }
            let ru = rep(u);
            for e in out.edges(u).iter() {
                if e.relation == membership {
                    continue; // the grouping itself: placement and member counts
                }
                match (ru.clone(), rep(e.nbr)) {
                    (None, None) => want_unplaced += 1,
                    (Some(_), None) | (None, Some(_)) => {
                        *want_hidden.entry(e.relation).or_default() += 1
                    }
                    (Some(a), Some(b)) if a == b => {
                        *want_internal.entry((a, e.relation)).or_default() += 1
                    }
                    (Some(a), Some(b)) => {
                        *want
                            .entry((a, b, e.relation, e.state.to_string()))
                            .or_default() += 1
                    }
                }
            }
        }
        assert_eq!(got, want, "zoom {z}");
        assert_eq!(internal, want_internal, "zoom {z} internal");
        assert_eq!(
            hidden, want_hidden,
            "zoom {z} edges with an unrepresented end"
        );
        let zs = &meta.zooms[z as usize];
        assert_eq!(
            zs.edges_unplaced, want_unplaced,
            "zoom {z} edges between unrepresented nodes"
        );
        assert_eq!(zs.represented_by_concept as u32, represented);
        assert_eq!(
            visible.len() + (represented + clustered) as usize + zs.unplaced,
            active,
            "zoom {z}: every node is visible, represented, clustered or counted as unplaced"
        );
        assert!(!general || clustered == 0);
        if z == 0 {
            // The fixture exercises every case at the top zoom.
            assert_eq!(
                represented, 3,
                "the three declarations stand behind their concepts"
            );
            assert!(zs.unplaced > 0 && !hidden.is_empty() && want_unplaced > 0);
        }
    }
}

/// The force layout ignores memberships and declarations are placed from their
/// concept and ids only, so a warm rebuild of the same registry keeps the drawn map
/// where it was, and the file keeps what the next warm start continues from.
#[test]
fn warm_rebuild_keeps_the_concept_map_in_place() {
    let s = snapshot("warm");
    let p = params();
    let a = compute(&s, &p).unwrap();
    assert_ne!(
        a.positions, a.force_positions,
        "declarations are drawn around their concept"
    );
    let dir = std::env::temp_dir().join(format!(
        "atlas-layout-concepts-{}-warm-out",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    a.write(&dir).unwrap();
    let o = Layout::open(&s, &dir).unwrap();
    assert_eq!(o.force_positions, a.force_positions);
    let b = compute_with(
        &s,
        &p,
        &ComputeOptions {
            prior: Some(Prior::from_layout(&o)),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(b.stats.warm.as_ref().expect("warm start").new, 0);
    let moved: Vec<f32> = (0..s.num_nodes() as usize)
        .filter(|&v| a.positions[v][0].is_finite())
        .map(|v| {
            ((b.positions[v][0] - a.positions[v][0]).powi(2)
                + (b.positions[v][1] - a.positions[v][1]).powi(2))
            .sqrt()
        })
        .collect();
    let mean = moved.iter().sum::<f32>() / moved.len() as f32;
    assert!(mean < 0.05, "nodes moved {mean} on average");
}

#[test]
fn edge_cap_applies_per_layer_so_one_layer_cannot_starve_another() {
    let s = snapshot("cap");
    let mut p = params();
    p.tiles.edges = 1;
    p.tiles.general_edges = 1;
    let l = compute(&s, &p).unwrap();
    let mut multi_layer_tile = false;
    for (z, x, y) in l.tiles.keys() {
        let mut layers_drawn = 0;
        for &layer in Projection::ALL.iter() {
            let t = tile(&s, &l, layer, z, x, y, "es").unwrap();
            assert!(t.edges.len() <= 1, "z{z}/{x}/{y} {layer:?}: {} edges over the cap", t.edges.len());
            layers_drawn += usize::from(!t.edges.is_empty());
        }
        multi_layer_tile |= layers_drawn > 1;
    }
    assert!(
        multi_layer_tile,
        "with a cap of 1, some tile still draws an edge in two layers"
    );
}

/// Round 3: the general layer has a per-tile cap.  With room for one node per tile,
/// z0 shows only the first general node by priority and the other appears deeper.
#[test]
fn general_tile_cap_keeps_the_overview_small() {
    let s = snapshot("general-cap");
    let mut p = params();
    p.tiles.general_tile_nodes = 1;
    let l = compute(&s, &p).unwrap();
    assert_eq!(l.tiles.meta.general_nodes, 2);
    let z0 = tile(&s, &l, Projection::Structure, 0, 0, 0, "es").unwrap();
    assert_eq!(z0.nodes.len(), 1, "one general node fits the z0 tile: {:?}", z0.nodes);
    let mut seen: BTreeMap<String, Option<u32>> = BTreeMap::new();
    for (z, x, y) in l.tiles.keys() {
        let t = tile(&s, &l, Projection::Structure, z, x, y, "es").unwrap();
        let general = t.nodes.iter().filter(|nd| nd.kind == TileNodeKind::Concept).count();
        assert!(z >= l.tiles.meta.zmax || general <= 1, "z{z} {x}/{y}: {general} concepts");
        seen.extend(t.nodes.iter().filter(|nd| nd.kind == TileNodeKind::Concept).map(|nd| (nd.id.clone(), nd.priority)));
    }
    assert_eq!(seen.len(), 2, "every general node is drawn at some zoom: {seen:?}");
    assert_eq!(z0.nodes[0].priority, seen.values().copied().min().unwrap(), "z0 keeps the most important one");
}

/// ADR-0005 §5: the map reads the curation.  `map:general` keeps a concept out of the general
/// zooms (z0..=general_zoom) only; `map:all` keeps a node off the map; a presentation label is
/// what the tiles show, while the registry label stays.
#[test]
fn curation_flags_and_presentation_labels_reach_the_tiles() {
    let s = snapshot_with(
        "curation",
        &[
            ("exclude_presentation", "c:lonely", r#"{"scopes":["map:general"]}"#),
            ("exclude_presentation", "s:law-of-cosines", r#"{"scopes":["map:all"]}"#),
            ("presentation_label", "c:right-triangle", r#"{"lang":"es","text":"Triángulo rectángulo (curado)","provenance":{}}"#),
        ],
    );
    let p = params();
    let l = compute(&s, &p).unwrap();
    let general_zoom = l.tiles.meta.general_zoom.unwrap();
    let gone = s.resolve("s:law-of-cosines").unwrap();
    assert!(l.positions[gone as usize][0].is_nan(), "map:all has no position");
    let mut lonely_deep = false;
    let mut curated_label = false;
    for (z, x, y) in l.tiles.keys() {
        for &layer in Projection::ALL.iter() {
            let t = tile(&s, &l, layer, z, x, y, "es").unwrap();
            let ids: Vec<&str> = t.nodes.iter().chain(&t.external).map(|n| n.id.as_str()).collect();
            let ends = t.edges.iter().flat_map(|e| [e.source.as_str(), e.target.as_str()]);
            assert!(!ids.iter().copied().chain(ends).any(|id| id == "s:law-of-cosines"), "map:all node in {z}/{x}/{y}");
            if z <= general_zoom {
                assert!(!ids.contains(&"c:lonely"), "map:general node in general tile {z}/{x}/{y}");
            } else {
                lonely_deep |= ids.contains(&"c:lonely");
            }
            for n in t.nodes.iter().filter(|n| n.id == "c:right-triangle") {
                assert_eq!(n.label, "Triángulo rectángulo (curado)");
                curated_label = true;
            }
        }
    }
    assert!(curated_label, "the curated concept is drawn somewhere");
    assert!(lonely_deep, "map:general only hides the general zooms");
    let rt = s.resolve("c:right-triangle").unwrap();
    assert_ne!(s.label(rt, "es"), "Triángulo rectángulo (curado)", "the registry label is untouched");
}
