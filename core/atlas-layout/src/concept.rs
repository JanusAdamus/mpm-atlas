//! The concept layer of the map (docs/plans/2026-10-17-capa-conceptual.md).
//!
//! - `primary_concepts`: each active declaration's primary concept (snapshot
//!   sections 100–110), or `NONE`.
//! - `general_set`: the nodes the general zooms show.  Concepts with primary
//!   members first, then `area` entities, by layout score, at most `cap`.  Sources,
//!   people, infrastructure, declarations, Mathlib module directories (raw paths;
//!   the derived concepts stand for Mathlib zones) and concepts with only secondary
//!   members (nothing hidden would be represented by them) are never in it.
//! - `place_members`: each declaration is drawn in a disc around its primary
//!   concept, so the declarations of a concept appear next to it when the map
//!   zooms in.  The disc radius grows with the square root of the member count.
//!   Declarations stay out of the force layout (`pipeline`): their position is a
//!   function of their concept's position and their ids, so rebuilds do not drift.
//!
//! Without concepts in the snapshot the general set is empty and nothing moves:
//! the map behaves as before the concept layer.

use atlas_core::{EntityType, NONE, Snapshot};
use rayon::prelude::*;

/// Disc radius (unit square) per square root of members: 10,000 members -> 0.05.
pub(crate) const MEMBER_DISC: f64 = 5e-4;

pub(crate) fn primary_concepts(snap: &Snapshot, active: &[bool]) -> Vec<u32> {
    (0..active.len() as u32)
        .into_par_iter()
        .map(|v| {
            if !active[v as usize] {
                return NONE;
            }
            match snap.primary_concept(v).map(|c| snap.canonical(c)) {
                Some(c) if c != v && active[c as usize] => c,
                _ => NONE,
            }
        })
        .collect()
}

/// Tier of a node in the general layer (lower first), or `None` if it is not in it.
fn general_tier(id: &str, primary_members: Option<usize>, is_area: bool) -> Option<u8> {
    match primary_members {
        Some(m) if m > 0 => Some(0),
        // ponytail: ingest bookkeeping recognized by id prefix (atlas/ingest/mathlib.py
        // `dir_id`, `LAYER_IDS`); an explicit flag in the snapshot if more kinds appear.
        _ if is_area && !id.starts_with("mathlib-") => Some(1),
        _ => None,
    }
}

/// Entity types that never enter the general layer, whatever memberships they have:
/// documents and their fragments, people, infrastructure (round 3 plan, «capa general»).
const NOT_GENERAL: [EntityType; 4] =
    [EntityType::SourceFragment, EntityType::Publication, EntityType::Person, EntityType::Resource];

/// A label that is only a number («10», «1105», «1,728»): a Wikidata number item, not a
/// concept to study.  ponytail: dates, years and other non-mathematical items need the
/// item's class (P31/P279), which the snapshot does not carry; the concept projection
/// keeps them out of the assignment (plan point 1), and with no primary members they
/// are already outside the general set.
fn is_bare_number(label: &str) -> bool {
    label.bytes().any(|b| b.is_ascii_digit())
        && label.bytes().all(|b| b.is_ascii_digit() || b" .,+-".contains(&b))
}

/// Whether a node of this type and these labels may be in the general layer.
fn general_kind(t: EntityType, labels: [&str; 2]) -> bool {
    !NOT_GENERAL.contains(&t) && !labels.iter().any(|l| is_bare_number(l))
}

/// `true` for the nodes of the general layer (see module doc).
pub(crate) fn general_set(
    snap: &Snapshot,
    active: &[bool],
    score: &[f64],
    cap: usize,
) -> Vec<bool> {
    let n = active.len();
    let mut out = vec![false; n];
    if snap.concepts().is_empty() {
        return out;
    }
    let tier = |v: u32| {
        let t = snap.node_type(v);
        if !general_kind(t, [snap.label(v, "es"), snap.label(v, "en")]) {
            return None;
        }
        let members = snap.concept_index(v).map(|_| snap.concept_members(v).len());
        general_tier(snap.node_id(v), members, t == EntityType::Area)
    };
    let mut cand: Vec<(u8, u32)> = (0..n as u32)
        .filter(|&v| active[v as usize])
        .filter_map(|v| tier(v).map(|t| (t, v)))
        .collect();
    cand.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(score[b.1 as usize].total_cmp(&score[a.1 as usize]))
            .then(a.1.cmp(&b.1))
    });
    for &(_, v) in cand.iter().take(cap) {
        out[v as usize] = true;
    }
    out
}

/// Move each declaration into the disc of its concept (positions in the unit square).
///
/// The arrangement depends only on the concept's position and its members' ids, never
/// on earlier positions, so a rebuild cannot drift: members sorted by id (Mathlib ids
/// share namespace prefixes, so a namespace lands in one wedge) take evenly spaced
/// angles, and radii `R sqrt(frac((i + 1/2) phi))` fill the disc with even density.
/// A new member shifts the others by one slot at most.
pub(crate) fn place_members<'a>(
    concept_of: &[u32],
    pos: &mut [[f32; 2]],
    id: &(dyn Fn(u32) -> &'a str + Sync),
) {
    let mut members: Vec<(u32, u32)> = (0..concept_of.len() as u32)
        .filter(|&v| concept_of[v as usize] != NONE)
        .map(|v| (concept_of[v as usize], v))
        .collect();
    members.par_sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| id(a.1).cmp(id(b.1))));
    let at: &[[f32; 2]] = pos;
    let moved: Vec<(u32, [f32; 2])> = members
        .par_chunk_by(|a, b| a.0 == b.0)
        .flat_map_iter(|run| {
            let c = at[run[0].0 as usize];
            let k = run.len() as f64;
            let radius = MEMBER_DISC * k.sqrt();
            run.iter().enumerate().map(move |(i, &(_, v))| {
                let u = i as f64 + 0.5;
                let angle = std::f64::consts::TAU * u / k;
                let rho = radius * (u * GOLDEN).fract().sqrt();
                let q = |o: f32, d: f64| (o as f64 + d).clamp(0.0, 1.0) as f32;
                (v, [q(c[0], rho * angle.cos()), q(c[1], rho * angle.sin())])
            })
        })
        .collect();
    for (v, p) in moved {
        pos[v as usize] = p;
    }
}

/// Put each hub (a concept with members but no relation in the layout graph) at
/// its members' centroid, in the layout's raw coordinates.
pub(crate) fn center_hubs(raw: &mut [[f64; 2]], hub: &[bool], concept_of: &[u32]) {
    let mut sum: ahash::AHashMap<u32, [f64; 3]> = ahash::AHashMap::new();
    for (v, &c) in concept_of.iter().enumerate() {
        if c != NONE && hub[c as usize] {
            let s = sum.entry(c).or_insert([0.0; 3]);
            *s = [s[0] + raw[v][0], s[1] + raw[v][1], s[2] + 1.0];
        }
    }
    for (c, s) in sum {
        raw[c as usize] = [s[0] / s[2], s[1] / s[2]];
    }
}

/// Fractional part of the golden ratio: a low-discrepancy step for the radii.
const GOLDEN: f64 = 0.618_033_988_749_894_9;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_layer_keeps_concepts_with_members_and_real_areas_only() {
        assert_eq!(general_tier("wd:Q11348", Some(12), false), Some(0));
        assert_eq!(general_tier("concept:Mathlib/Order", Some(3), false), Some(0));
        assert_eq!(general_tier("wd:Q395", None, true), Some(1));
        assert_eq!(general_tier("wd:Q395", Some(0), true), Some(1), "an area stays an area");
        // A concept with only secondary members ("54") represents nothing hidden.
        assert_eq!(general_tier("wd:Q290669", Some(0), false), None);
        // Module paths and the infrastructure layer are bookkeeping, not concepts.
        assert_eq!(general_tier("mathlib-dir:Mathlib/Tactic", None, true), None);
        assert_eq!(general_tier("mathlib-dir:Mathlib/Order", None, true), None);
        assert_eq!(general_tier("mathlib-layer:infrastructure", None, true), None);
        assert_eq!(general_tier("wd:Q5", None, false), None);
    }

    #[test]
    fn sources_people_infrastructure_and_bare_numbers_stay_out_of_the_general_layer() {
        for t in NOT_GENERAL {
            assert!(!general_kind(t, ["x", "x"]), "{t:?}");
        }
        assert!(general_kind(EntityType::Concept, ["número primo", "prime number"]));
        assert!(general_kind(EntityType::Statement, ["teorema 90 de Hilbert", "Hilbert's Theorem 90"]));
        assert!(general_kind(EntityType::Concept, ["", ""]), "a missing label is not a number");
        for n in ["10", "1105", "1,728", "-1", "1.5"] {
            assert!(!general_kind(EntityType::Concept, [n, "ten"]), "{n}");
            assert!(!general_kind(EntityType::Concept, ["diez", n]), "{n}");
        }
    }

    #[test]
    fn members_fill_their_concept_disc_whatever_their_previous_place() {
        // node 0 = concept at (0.8, 0.8); 1..=3 its members; 4 has no concept.
        let ids = ["c", "m:b", "m:a", "m:c", "x"];
        let id = |v: u32| ids[v as usize];
        let concept_of = vec![NONE, 0, 0, 0, NONE];
        let mut pos = vec![[0.8, 0.8], [0.1, 0.1], [0.2, 0.1], [0.1, 0.3], [0.5, 0.5]];
        place_members(&concept_of, &mut pos, &id);
        let r = (MEMBER_DISC * 3f64.sqrt()) as f32 + 1e-6;
        for (v, p) in pos.iter().enumerate().take(4).skip(1) {
            let d = ((p[0] - 0.8).powi(2) + (p[1] - 0.8).powi(2)).sqrt();
            assert!(d <= r, "member {v} at distance {d} > {r}");
        }
        assert_eq!(pos[0], [0.8, 0.8]);
        assert_eq!(pos[4], [0.5, 0.5]);
        // History-free: other starting points (a rebuild) give the same places.
        let mut again = vec![[0.8, 0.8], [0.9, 0.2], [0.0, 0.0], [0.3, 0.7], [0.5, 0.5]];
        place_members(&concept_of, &mut again, &id);
        assert_eq!(pos, again);
    }
}
