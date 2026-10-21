//! Shared vocabulary: the enums of `atlas/model.py`, in declaration order.
//!
//! The u8 code of each variant is its position in the Python declaration and is
//! what the snapshot stores.  `tests/vocab.rs` checks these tables against
//! `atlas/model.py` and `shapes/vocab.json`; `vocab_hash()` goes into every
//! snapshot header so a snapshot built under another vocabulary is rejected.

use std::fmt;
use std::str::FromStr;

/// Error for a string that is not a variant of a vocabulary enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownVariant {
    pub kind: &'static str,
    pub value: String,
}

impl fmt::Display for UnknownVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown {} {:?}", self.kind, self.value)
    }
}

impl std::error::Error for UnknownVariant {}

macro_rules! str_enum {
    ($(#[$m:meta])* $name:ident, $kind:literal { $($var:ident = $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u8)]
        pub enum $name { $($var),+ }

        impl $name {
            /// All variants in code order (= Python declaration order).
            pub const ALL: &'static [$name] = &[$($name::$var),+];
            pub const COUNT: usize = Self::ALL.len();

            pub const fn as_str(self) -> &'static str {
                match self { $($name::$var => $s),+ }
            }
            pub const fn code(self) -> u8 { self as u8 }
            pub fn from_code(code: u8) -> Option<Self> { Self::ALL.get(code as usize).copied() }
            pub const fn bit(self) -> u32 { 1u32 << (self as u8) }
        }

        impl FromStr for $name {
            type Err = UnknownVariant;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($s => Ok($name::$var),)+
                    _ => Err(UnknownVariant { kind: $kind, value: s.to_owned() }),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.as_str()) }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = <std::borrow::Cow<'de, str>>::deserialize(d)?;
                s.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

str_enum!(
    /// `atlas.model.EntityType`.
    EntityType, "entity type" {
        Concept = "concept",
        Definition = "definition",
        Theory = "theory",
        Statement = "statement",
        Proof = "proof",
        Conjecture = "conjecture",
        Counterexample = "counterexample",
        Algorithm = "algorithm",
        Model = "model",
        Problem = "problem",
        Phenomenon = "phenomenon",
        Application = "application",
        Publication = "publication",
        SourceFragment = "source_fragment",
        Person = "person",
        Resource = "resource",
        Experiment = "experiment",
        Area = "area",
        // v4 (ADR-0005): new codes go at the end; no existing code changes
        Witness = "witness",
        Note = "note",
    }
);

str_enum!(
    /// `atlas.model.RelationType`.  SUBJECT --relation--> OBJECT.
    RelationType, "relation type" {
        DependsOnLogically = "depends_on_logically",
        Proves = "proves",
        PrerequisiteFor = "prerequisite_for",
        Generalizes = "generalizes",
        SpecialCaseOf = "special_case_of",
        EquivalentTo = "equivalent_to",
        AnalogousTo = "analogous_to",
        AppliedIn = "applied_in",
        Models = "models",
        Cites = "cites",
        Influenced = "influenced",
        PartOfArea = "part_of_area",
        FormalizedAs = "formalized_as",
        NamedAfter = "named_after",
        CounterexampleTo = "counterexample_to",
        AppearsIn = "appears_in",
        AuthoredBy = "authored_by",
        MemberOfConcept = "member_of_concept",
        // v4 (ADR-0005)
        SaidEquivalentTo = "said_equivalent_to",
        InstanceOf = "instance_of",
        IntroducedIn = "introduced_in",
        CopiedBy = "copied_by",
        EditedBy = "edited_by",
        TranslatedBy = "translated_by",
        Explains = "explains",
    }
);

str_enum!(
    /// `atlas.model.EvidenceState`.
    EvidenceState, "evidence state" {
        Extracted = "extracted",
        ModelProposed = "model_proposed",
        EditoriallyReviewed = "editorially_reviewed",
        FormallyLinked = "formally_linked",
        Disputed = "disputed",
        Superseded = "superseded",
        Retracted = "retracted",
    }
);

str_enum!(
    /// `atlas.model.Role` of a claim participant.
    Role, "role" {
        Subject = "subject",
        Object = "object",
        Premise = "premise",
        Assumption = "assumption",
        Context = "context",
        Via = "via",
    }
);

str_enum!(
    /// `atlas.model.Projection`: the family a relation belongs to.
    Projection, "projection" {
        Structure = "structure",
        Proof = "proof",
        Learning = "learning",
        Application = "application",
        History = "history",
        Biblio = "biblio",
    }
);

impl EvidenceState {
    /// `atlas.model.CURRENT_STATES`: shown by default and used by the path engine.
    pub const fn is_current(self) -> bool {
        !matches!(self, EvidenceState::Superseded | EvidenceState::Retracted)
    }

    /// `atlas.model.EVIDENCE_RANK`: formally_linked 4 > editorially_reviewed 3 > extracted 2 >
    /// model_proposed 1 > disputed 0; `None` for states that are not current.
    pub const fn rank(self) -> Option<u8> {
        match self {
            EvidenceState::FormallyLinked => Some(4),
            EvidenceState::EditoriallyReviewed => Some(3),
            EvidenceState::Extracted => Some(2),
            EvidenceState::ModelProposed => Some(1),
            EvidenceState::Disputed => Some(0),
            EvidenceState::Superseded | EvidenceState::Retracted => None,
        }
    }

    /// The weaker of two current states (ADR-0005 §7a: an aggregate is as strong as its weakest link).
    pub fn weakest(self, other: EvidenceState) -> EvidenceState {
        if other.rank() < self.rank() { other } else { self }
    }
}

/// `atlas.model.PRESENTATION_SCOPES`: scopes of an `exclude_presentation` curation.  The position is
/// the bit of the scope in the snapshot's `NodeFlags` section.
pub const PRESENTATION_SCOPES: [&str; 4] = ["map:general", "map:all", "search:default", "concept_layer"];

/// Bit of a presentation scope in `NodeFlags`, if the scope is known.
pub fn scope_bit(scope: &str) -> Option<u8> {
    PRESENTATION_SCOPES.iter().position(|s| *s == scope).map(|i| 1u8 << i)
}

// Node state masks are u8; relation/type masks are u32.
const _: () = assert!(RelationType::COUNT <= 32 && EvidenceState::COUNT <= 8 && EntityType::COUNT <= 32);

/// Bitmask of `EvidenceState::bit()` for the current states.
pub const CURRENT_STATES_MASK: u32 = (1 << 5) - 1;

/// Static semantics of a relation (`atlas.model.RelationSpec`, without the
/// endpoint type sets, which only the registry enforces).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelationSpec {
    pub relation: RelationType,
    pub projection: Projection,
    pub symmetric: bool,
    pub transitive: bool,
    pub inverse: Option<RelationType>,
    pub allows_premises: bool,
    /// v4: may a claim of this relation be `formally_linked`?
    pub formal_allowed: bool,
}

const fn spec(
    relation: RelationType,
    projection: Projection,
    symmetric: bool,
    transitive: bool,
    inverse: Option<RelationType>,
    allows_premises: bool,
    formal_allowed: bool,
) -> RelationSpec {
    RelationSpec { relation, projection, symmetric, transitive, inverse, allows_premises, formal_allowed }
}

use Projection as P;
use RelationType as R;

/// Indexed by `RelationType::code()`.
pub const RELATIONS: [RelationSpec; RelationType::COUNT] = [
    spec(R::DependsOnLogically, P::Proof, false, true, None, true, true),
    spec(R::Proves, P::Proof, false, false, None, false, true),
    spec(R::PrerequisiteFor, P::Learning, false, true, None, false, true),
    spec(R::Generalizes, P::Structure, false, true, Some(R::SpecialCaseOf), false, true),
    spec(R::SpecialCaseOf, P::Structure, false, true, Some(R::Generalizes), false, true),
    spec(R::EquivalentTo, P::Structure, true, true, None, false, true),
    spec(R::AnalogousTo, P::Structure, true, false, None, false, true),
    spec(R::AppliedIn, P::Application, false, false, None, false, true),
    spec(R::Models, P::Application, false, false, None, false, true),
    spec(R::Cites, P::Biblio, false, false, None, false, false),
    spec(R::Influenced, P::History, false, false, None, false, false),
    spec(R::PartOfArea, P::Structure, false, true, None, false, false),
    spec(R::FormalizedAs, P::Proof, false, false, None, false, true),
    spec(R::NamedAfter, P::History, false, false, None, false, false),
    spec(R::CounterexampleTo, P::Proof, false, false, None, false, true),
    spec(R::AppearsIn, P::Biblio, false, false, None, false, false),
    spec(R::AuthoredBy, P::History, false, false, None, false, false),
    spec(R::MemberOfConcept, P::Structure, false, false, None, false, false),
    spec(R::SaidEquivalentTo, P::Structure, true, false, None, false, true),
    spec(R::InstanceOf, P::Structure, false, false, None, false, true),
    spec(R::IntroducedIn, P::History, false, false, None, false, false),
    spec(R::CopiedBy, P::History, false, false, None, false, false),
    spec(R::EditedBy, P::History, false, false, None, false, false),
    spec(R::TranslatedBy, P::History, false, false, None, false, false),
    spec(R::Explains, P::Learning, false, false, None, false, false),
];

impl RelationType {
    pub const fn spec(self) -> &'static RelationSpec {
        &RELATIONS[self as usize]
    }
}

/// Fingerprint of every enum string in code order.  Stored in snapshot headers.
pub fn vocab_hash() -> [u8; 16] {
    let mut h = blake3::Hasher::new();
    let mut feed = |kind: &str, names: &mut dyn Iterator<Item = &'static str>| {
        h.update(kind.as_bytes());
        for n in names {
            h.update(b"\x1f");
            h.update(n.as_bytes());
        }
        h.update(b"\x1e");
    };
    feed("entity_types", &mut EntityType::ALL.iter().map(|v| v.as_str()));
    feed("relation_types", &mut RelationType::ALL.iter().map(|v| v.as_str()));
    feed("evidence_states", &mut EvidenceState::ALL.iter().map(|v| v.as_str()));
    feed("roles", &mut Role::ALL.iter().map(|v| v.as_str()));
    feed("presentation_scopes", &mut PRESENTATION_SCOPES.iter().copied());
    let mut out = [0u8; 16];
    out.copy_from_slice(&h.finalize().as_bytes()[..16]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip() {
        for r in RelationType::ALL {
            assert_eq!(RelationType::from_code(r.code()), Some(*r));
            assert_eq!(r.as_str().parse::<RelationType>().unwrap(), *r);
            assert_eq!(r.spec().relation, *r);
        }
        assert!("nope".parse::<EvidenceState>().is_err());
        let mask: u32 = EvidenceState::ALL.iter().filter(|s| s.is_current()).map(|s| s.bit()).sum();
        assert_eq!(mask, CURRENT_STATES_MASK);
    }

    #[test]
    fn weakest_link_follows_the_rank() {
        use EvidenceState as S;
        assert_eq!(S::Extracted.weakest(S::ModelProposed), S::ModelProposed);
        assert_eq!(S::FormallyLinked.weakest(S::EditoriallyReviewed), S::EditoriallyReviewed);
        assert_eq!(S::Disputed.weakest(S::FormallyLinked), S::Disputed);
        assert!(EvidenceState::ALL.iter().all(|s| s.rank().is_some() == s.is_current()));
        assert_eq!((scope_bit("map:general"), scope_bit("concept_layer"), scope_bit("x")), (Some(1), Some(8), None));
    }

}
