//! Query parsing (inline filters) and the router that weights the lanes.
//!
//! Inline filters, removed from the text before retrieval (`-` negates):
//! `type:theorem`, `-type:person`, `assume:<entity id or text>`, `-assume:choice`,
//! `lane:lexical|formula|semantic` (only that lane, for diagnosis).  Lean's `x : T`
//! has a space after the colon, so it is never mistaken for a filter.
//!
//! Routes and lane weights (lexical, formula, semantic) for the weighted RRF:
//! - `named_object`  short query, identifier or name: lexical leads;
//! - `formula`       LaTeX, Unicode math or operators between symbols: formula leads
//!   (alone when the query has no words);
//! - `natural_language` a sentence or a question: semantic leads;
//! - `cross_domain`  "relation between X and Y", "applications of X in Y": semantic
//!   leads and results are spread over areas.
//!
//! The weights are a starting point, not tuned on AtlasBench yet.

use atlas_core::EntityType;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteKind {
    NamedObject,
    Formula,
    NaturalLanguage,
    CrossDomain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    Lexical,
    Formula,
    Semantic,
}

impl Lane {
    pub const ALL: [Lane; 3] = [Lane::Lexical, Lane::Formula, Lane::Semantic];
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Route {
    pub kind: RouteKind,
    /// RRF weight per lane, indexed like `Lane::ALL`; 0 = lane not run.
    pub weights: [f64; 3],
    /// Spread hits over areas (cross-domain questions).
    pub diversify: bool,
}

impl Route {
    pub fn weight(&self, lane: Lane) -> f64 {
        self.weights[lane as usize]
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Filters {
    pub types: Vec<EntityType>,
    pub not_types: Vec<EntityType>,
    pub assume: Vec<String>,
    pub not_assume: Vec<String>,
    pub only_lane: Option<&'static str>,
}

impl Filters {
    pub fn is_empty(&self) -> bool {
        *self == Filters::default()
    }
}

#[derive(Clone, Debug)]
pub struct Parsed {
    /// Query text without the filters.
    pub text: String,
    pub filters: Filters,
    /// Filter tokens that could not be understood (reported, not fatal).
    pub ignored: Vec<String>,
}

pub fn parse_query(q: &str) -> Parsed {
    let mut f = Filters::default();
    let mut words = Vec::new();
    let mut ignored = Vec::new();
    for w in q.split_whitespace() {
        let (neg, body) = match w.strip_prefix('-') {
            Some(b) => (true, b),
            None => (false, w),
        };
        let Some((key, val)) = body.split_once(':').filter(|(k, v)| !v.is_empty() && !k.is_empty()) else {
            words.push(w);
            continue;
        };
        match key {
            "type" | "tipo" => match val.parse::<EntityType>() {
                Ok(t) if neg => f.not_types.push(t),
                Ok(t) => f.types.push(t),
                Err(_) => ignored.push(w.to_string()),
            },
            "assume" | "supone" => {
                if neg { f.not_assume.push(val.to_string()) } else { f.assume.push(val.to_string()) }
            }
            "lane" | "via" => match val {
                "lexical" | "lexica" => f.only_lane = Some("lexical"),
                "formula" => f.only_lane = Some("formula"),
                "semantic" | "semantica" => f.only_lane = Some("semantic"),
                _ => ignored.push(w.to_string()),
            },
            _ => words.push(w),
        }
    }
    Parsed { text: words.join(" "), filters: f, ignored }
}

const MATH_CHARS: &str = "=+^_<>≤≥≠≈≡∑∏∫∮√∀∃∈∉⊂⊆⊃⊇∪∩→↦⇒⇔↔×·∘⊗⊕∂∇∞‖⟨⟩²³⁻¹ℕℤℚℝℂ";
/// Relation symbols that, standing alone between two sides, make a Lean/Unicode
/// statement (`(μ.map φ).variation = μ.variation.map φ`, `encode cf < encode c`).
const RELATIONS: [&str; 14] = ["=", "<", ">", "≤", "≥", "≠", "≈", "≡", "↔", "→", "∣", "⊆", "∈", "≃"];
/// Up to this many prose words, a statement with a standalone relation routes as a formula.
const MAX_STATEMENT_WORDS: usize = 5;
const CONNECTORS: [&str; 22] = [
    "entre", "between", "conexion", "conexiones", "connection", "connections", "relacion", "relaciones",
    "relationship", "relationships", "relate", "relates", "puente", "bridge", "vinculo", "link", "links",
    "interplay", "aplicaciones", "applications", "versus", "vs",
];
const QUESTION: [&str; 16] = [
    "que", "qué", "como", "cómo", "por", "porque", "cual", "cuál", "what", "how", "why", "which", "when", "is",
    "does", "explain",
];
const STOP: [&str; 24] = [
    "de", "la", "el", "los", "las", "del", "en", "y", "a", "un", "una", "con", "para", "the", "of", "in", "and",
    "a", "an", "to", "for", "on", "with", "o",
];

fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            c => c,
        })
        .collect()
}

/// Lean / Mathlib identifier: `Nat.Prime`, `add_comm`, `InnerProductSpace`.
fn is_identifier(w: &str) -> bool {
    let inner_dot = w.trim_matches('.').contains('.');
    let snake = w.contains('_') && w.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.');
    let camel = w.chars().next().is_some_and(char::is_uppercase)
        && w.chars().skip(1).any(char::is_uppercase)
        && w.chars().any(char::is_lowercase);
    (inner_dot && w.chars().next().is_some_and(char::is_alphabetic)) || snake || camel
}

/// `text` with LaTeX control words (`\langle`, `\sum`) blanked: they are notation, not prose.
fn without_latex_commands(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\' && it.peek().is_some_and(|n| n.is_ascii_alphabetic()) {
            while it.next_if(char::is_ascii_alphabetic).is_some() {}
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

pub fn route(text: &str) -> Route {
    let words: Vec<&str> = text.split_whitespace().collect();
    let folded: Vec<String> = words.iter().map(|w| fold(w.trim_matches(|c: char| !c.is_alphanumeric()))).collect();
    let prose = without_latex_commands(text);
    let content = prose
        .split_whitespace()
        .map(|w| fold(w.trim_matches(|c: char| !c.is_alphanumeric())))
        .filter(|w| w.chars().count() > 2 && w.chars().all(char::is_alphabetic) && !STOP.contains(&w.as_str()))
        .count();
    let math = text.chars().filter(|c| MATH_CHARS.contains(*c)).count();
    let latex = text.contains('\\') || text.contains('$');
    let ident = words.iter().any(|w| is_identifier(w));
    // math between short symbols (`a^2+b^2`, `x ≤ y`), not `type-theory` prose
    let symbolic = math > 0 && (latex || content <= 2 || math * 3 >= words.len());
    let connector = folded.iter().any(|w| CONNECTORS.contains(&w.as_str()));
    let question = text.contains('?') || folded.first().is_some_and(|w| QUESTION.contains(&w.as_str()));
    let relation = words.iter().position(|w| RELATIONS.contains(w)).is_some_and(|i| i > 0 && i + 1 < words.len());
    let statement = relation && !question && content <= MAX_STATEMENT_WORDS;
    let (kind, weights, diversify) = if latex || (symbolic && !ident) || statement {
        // a bare formula has no words: lexical and semantic hits on `x`, `2` are noise;
        // Lean identifiers in it (`μ.variation`) are worth a lexical match
        let w = match (content, ident) {
            (0, false) => [0.0, 1.0, 0.0],
            (_, true) => [0.8, 1.0, 0.3],
            _ => [0.5, 1.0, 0.3],
        };
        (RouteKind::Formula, w, false)
    } else if connector && content >= 2 {
        (RouteKind::CrossDomain, [0.6, 0.0, 1.0], true)
    } else if question || content >= 4 {
        (RouteKind::NaturalLanguage, [0.8, if math > 0 { 0.3 } else { 0.0 }, 1.0], false)
    } else {
        (RouteKind::NamedObject, [1.0, if math > 0 { 0.5 } else { 0.0 }, 0.5], false)
    };
    Route { kind, weights, diversify }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_follow_the_shape_of_the_query() {
        assert_eq!(route("teorema de Pitágoras").kind, RouteKind::NamedObject);
        assert_eq!(route("Nat.Prime.two_le").kind, RouteKind::NamedObject);
        assert_eq!(route("InnerProductSpace").kind, RouteKind::NamedObject);
        assert_eq!(route("a^2 + b^2 = c^2").kind, RouteKind::Formula);
        assert_eq!(route("a^2 + b^2 = c^2").weights, [0.0, 1.0, 0.0]);
        assert!(route("teorema a^2 + b^2 = c^2").weight(Lane::Lexical) > 0.0);
        assert_eq!(route(r"\int_0^1 f(x)\,dx").kind, RouteKind::Formula);
        // LaTeX control words are not prose words
        assert_eq!(route(r"\|v\|^2 = \sum_j |\langle v, u_j\rangle|^2").weights, [0.0, 1.0, 0.0]);
        assert!(route(r"desigualdad de Cauchy \langle x, y\rangle").weight(Lane::Lexical) > 0.0);
        assert_eq!(route("x ≤ y").kind, RouteKind::Formula);
        // Lean statements with identifiers and a standalone relation
        let r = route("(μ.map φ).variation = μ.variation.map φ");
        assert_eq!(r.kind, RouteKind::Formula);
        assert!(r.weight(Lane::Lexical) > 0.0);
        assert_eq!(route("encode cf < encode (rfind' cf)").kind, RouteKind::Formula);
        assert_eq!(route("Nat.le_of_lt_succ").kind, RouteKind::NamedObject);
        assert_eq!(route("why is every x = y in an abelian group commutative?").kind, RouteKind::NaturalLanguage);
        assert_eq!(route("¿qué teorema dice que todo número se factoriza en primos?").kind, RouteKind::NaturalLanguage);
        assert_eq!(route("every bounded monotone sequence of reals converges").kind, RouteKind::NaturalLanguage);
        let r = route("relación entre teoría de grupos y química");
        assert_eq!(r.kind, RouteKind::CrossDomain);
        assert!(r.diversify && r.weight(Lane::Formula) == 0.0);
    }

    #[test]
    fn inline_filters_are_extracted_and_lean_colons_are_not() {
        let p = parse_query("grupo type:statement -type:person assume:choice -assume:wd:Q191 lane:semantic");
        assert_eq!(p.text, "grupo");
        assert_eq!(p.filters.types, [EntityType::Statement]);
        assert_eq!(p.filters.not_types, [EntityType::Person]);
        assert_eq!(p.filters.assume, ["choice"]);
        assert_eq!(p.filters.not_assume, ["wd:Q191"]);
        assert_eq!(p.filters.only_lane, Some("semantic"));
        let p = parse_query("(n m : ℕ) : n + m = m + n type:nonsense");
        assert_eq!(p.text, "(n m : ℕ) : n + m = m + n");
        assert_eq!(p.ignored, ["type:nonsense"]);
    }
}
