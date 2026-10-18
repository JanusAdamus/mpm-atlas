//! Text analysis shared by indexing and querying.
//!
//! - `fold`: words, lower case, accents folded (`Pitágoras` -> `pitagoras`).
//! - `es` / `en`: `fold` plus the Snowball stemmer of the language.
//! - `lean`: identifiers as Lean and Mathlib write them.  `Nat.Prime.two_le` emits the
//!   whole name plus `nat`, `prime`, `two`, `le`; `InnerProductSpace` emits the whole
//!   name plus `inner`, `product`, `space`.  So `Nat.Prime`, `prime` and
//!   `InnerProductSpace` all reach the declaration.

use tantivy::tokenizer::{
    AsciiFoldingFilter, Language, LowerCaser, RemoveLongFilter, SimpleTokenizer, Stemmer, TextAnalyzer, Token,
    TokenStream, Tokenizer, TokenizerManager,
};

pub const FOLD: &str = "atlas_fold";
pub const ES: &str = "atlas_es";
pub const EN: &str = "atlas_en";
pub const LEAN: &str = "atlas_lean";

const MAX_TOKEN_BYTES: usize = 64;

pub fn register(manager: &TokenizerManager) {
    let fold = || {
        TextAnalyzer::builder(SimpleTokenizer::default())
            .filter(RemoveLongFilter::limit(MAX_TOKEN_BYTES))
            .filter(LowerCaser)
            .filter(AsciiFoldingFilter)
    };
    manager.register(FOLD, fold().build());
    manager.register(ES, fold().filter(Stemmer::new(Language::Spanish)).build());
    manager.register(EN, fold().filter(Stemmer::new(Language::English)).build());
    manager.register(
        LEAN,
        TextAnalyzer::builder(LeanTokenizer)
            .filter(RemoveLongFilter::limit(MAX_TOKEN_BYTES * 2))
            .filter(LowerCaser)
            .filter(AsciiFoldingFilter)
            .build(),
    );
}

/// All tokens `analyzer` produces for `text`.
pub fn analyze(analyzer: &mut TextAnalyzer, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut ts = analyzer.token_stream(text);
    while ts.advance() {
        out.push(ts.token().text.clone());
    }
    out
}

#[derive(Clone, Default)]
pub struct LeanTokenizer;

pub struct LeanTokenStream {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Tokenizer for LeanTokenizer {
    type TokenStream<'a> = LeanTokenStream;
    fn token_stream<'a>(&'a mut self, text: &'a str) -> LeanTokenStream {
        LeanTokenStream { tokens: lean_tokens(text), cursor: 0 }
    }
}

impl TokenStream for LeanTokenStream {
    fn advance(&mut self) -> bool {
        self.cursor += 1;
        self.cursor <= self.tokens.len()
    }
    fn token(&self) -> &Token {
        &self.tokens[self.cursor - 1]
    }
    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.cursor - 1]
    }
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.' || c == '\''
}

fn lean_tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let push = |from: usize, to: usize, out: &mut Vec<Token>| {
        let s = &text[from..to];
        if s.is_empty() {
            return;
        }
        out.push(Token { offset_from: from, offset_to: to, position: out.len(), text: s.to_string(), position_length: 1 });
    };
    let mut iter = text.char_indices().peekable();
    while let Some((start, c)) = iter.next() {
        if !is_ident_char(c) || c == '.' || c == '\'' {
            continue;
        }
        let mut end = start + c.len_utf8();
        while let Some(&(i, c)) = iter.peek() {
            if !is_ident_char(c) {
                break;
            }
            end = i + c.len_utf8();
            iter.next();
        }
        let ident = text[start..end].trim_end_matches(['.', '\'']);
        let end = start + ident.len();
        let parts = split_ident(ident);
        if parts.len() > 1 {
            push(start, end, &mut out);
        }
        for (a, b) in parts {
            push(start + a, start + b, &mut out);
        }
    }
    out
}

/// Byte ranges of the parts of an identifier: split on `.`, `_`, `'` and CamelCase /
/// letter-digit boundaries (`HTTPServer` -> `HTTP`, `Server`; `L2Space` -> `L`, `2`, `Space`).
pub fn split_ident(ident: &str) -> Vec<(usize, usize)> {
    let chars: Vec<(usize, char)> = ident.char_indices().collect();
    let mut parts = Vec::new();
    let mut start: Option<usize> = None;
    for (k, &(i, c)) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if let Some(s) = start.take() {
                parts.push((s, i));
            }
            continue;
        }
        let Some(s) = start else {
            start = Some(i);
            continue;
        };
        let prev = chars[k - 1].1;
        let next = chars.get(k + 1).map(|x| x.1);
        let boundary = (prev.is_lowercase() && c.is_uppercase())
            || (prev.is_uppercase() && c.is_uppercase() && next.is_some_and(char::is_lowercase))
            || (prev.is_alphabetic() && c.is_numeric())
            || (prev.is_numeric() && c.is_alphabetic());
        if boundary {
            parts.push((s, i));
            start = Some(i);
        }
    }
    if let Some(s) = start {
        parts.push((s, ident.len()));
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(name: &str, text: &str) -> Vec<String> {
        let m = TokenizerManager::default();
        register(&m);
        analyze(&mut m.get(name).unwrap(), text)
    }

    #[test]
    fn lean_identifiers_split_on_dots_underscores_and_camel_case() {
        assert_eq!(run(LEAN, "Nat.Prime.two_le"), ["nat.prime.two_le", "nat", "prime", "two", "le"]);
        assert_eq!(run(LEAN, "InnerProductSpace"), ["innerproductspace", "inner", "product", "space"]);
        assert_eq!(run(LEAN, "HTTPServer x"), ["httpserver", "http", "server", "x"]);
        assert_eq!(run(LEAN, "L2Space."), ["l2space", "l", "2", "space"]);
        // ℕ is a letter (Unicode Lu): kept, so `ℕ` itself is searchable
        assert_eq!(run(LEAN, "(n m : ℕ) : n + m = m + n"), ["n", "m", "ℕ", "n", "m", "m", "n"]);
    }

    #[test]
    fn spanish_and_english_fold_accents_and_stem() {
        assert_eq!(run(ES, "Teorema de Pitágoras"), ["teorem", "de", "pitagor"]);
        assert_eq!(run(EN, "Pythagorean theorems"), ["pythagorean", "theorem"]);
        assert_eq!(run(FOLD, "Ecuación DIFERENCIAL"), ["ecuacion", "diferencial"]);
    }
}
