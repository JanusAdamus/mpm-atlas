//! LaTeX and Lean-style expressions -> operator trees (OPT, as in Tangent-CFT, R24).
//!
//! Best effort and total: any input yields a tree (unknown tokens are skipped), never
//! a panic.  Normalisations: `>`/`≥` become `lt`/`le` with swapped operands, commutative
//! operators are flattened and their operands sorted, `\frac` is `div`, `|x|` is `abs`,
//! `‖x‖` is `norm`, `⟨x, y⟩` is `inner`, superscript digits and `⁻¹` become `pow`/`inv`.
//! LaTeX juxtaposition is multiplication; in Lean mode (binders, `∀`, dotted names)
//! juxtaposition after a name is application.  Lean binder names count as variables.

use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Node {
    Op(String, Vec<Node>),
    /// Variable: unified to `V` in the unified features.
    Var(String),
    Num(String),
    /// Constant symbol or named object (`π`, `ℝ`, `Nat.Prime`): never unified.
    Sym(String),
}

impl Node {
    fn op(name: &str, kids: Vec<Node>) -> Node {
        Node::Op(name.to_string(), kids)
    }

    /// S-expression, for diagnostics and tests: `(eq (add (pow a 2) (pow b 2)) (pow c 2))`.
    pub fn sexpr(&self) -> String {
        match self {
            Node::Op(name, kids) => {
                let mut s = format!("({name}");
                for k in kids {
                    s.push(' ');
                    s.push_str(&k.sexpr());
                }
                s.push(')');
                s
            }
            Node::Var(v) | Node::Num(v) | Node::Sym(v) => v.clone(),
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Node::Op(_, kids) => 1 + kids.iter().map(Node::size).sum::<usize>(),
            _ => 1,
        }
    }
}

pub fn is_commutative(op: &str) -> bool {
    matches!(op, "add" | "mul" | "eq" | "ne" | "and" | "or" | "iff" | "cup" | "cap" | "oplus" | "otimes" | "approx" | "equiv")
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(String),
    /// Single-letter or greek variable name.
    Var(String),
    /// Multi-letter identifier (Lean name, `\operatorname`), or a known function.
    Ident(String),
    Sym(String),
    /// Binary operator, canonical name.
    Bin(&'static str),
    Func(String),
    Big(&'static str),
    Frac,
    Sqrt,
    Binom,
    /// Wrapper whose single argument is kept (`\mathbf{x}` -> `x`).
    Wrap,
    /// `\mathbb{R}` and friends: the argument becomes a symbol.
    Blackboard,
    Open(char),
    Close(char),
    Comma,
    Caret,
    Under,
    Bar,
    Norm,
    LAngle,
    RAngle,
    Quant(&'static str),
    Colon,
    FatArrow,
    Not,
    Minus,
    Postfix(&'static str),
    SupNum(String),
}

const LATEX_FUNCS: &[&str] = &[
    "sin", "cos", "tan", "cot", "sec", "csc", "sinh", "cosh", "tanh", "arcsin", "arccos", "arctan", "log", "ln", "lg",
    "exp", "det", "dim", "ker", "deg", "gcd", "lcm", "max", "min", "sup", "inf", "arg", "Pr", "tr", "rank", "sgn",
    "limsup", "liminf", "hom", "Re", "Im",
];

const GREEK_VARS: &[&str] = &[
    "alpha", "beta", "gamma", "delta", "epsilon", "varepsilon", "zeta", "eta", "theta", "vartheta", "iota", "kappa",
    "lambda", "mu", "nu", "xi", "rho", "varrho", "sigma", "varsigma", "tau", "upsilon", "phi", "varphi", "chi", "psi",
    "omega", "Gamma", "Delta", "Theta", "Lambda", "Xi", "Sigma", "Upsilon", "Phi", "Psi", "Omega",
];

fn command_tok(name: &str) -> Option<Tok> {
    let t = match name {
        "frac" | "dfrac" | "tfrac" => Tok::Frac,
        "sqrt" => Tok::Sqrt,
        "binom" | "choose" => Tok::Binom,
        "sum" => Tok::Big("sum"),
        "prod" => Tok::Big("prod"),
        "int" | "iint" | "iiint" => Tok::Big("int"),
        "oint" => Tok::Big("oint"),
        "lim" => Tok::Big("lim"),
        "bigcup" => Tok::Big("bigcup"),
        "bigcap" => Tok::Big("bigcap"),
        "coprod" => Tok::Big("coprod"),
        "cdot" | "times" | "ast" => Tok::Bin("mul"),
        "div" => Tok::Bin("div"),
        "pm" => Tok::Bin("pm"),
        "mp" => Tok::Bin("mp"),
        "le" | "leq" | "leqslant" => Tok::Bin("le"),
        "ge" | "geq" | "geqslant" => Tok::Bin("ge"),
        "lt" => Tok::Bin("lt"),
        "gt" => Tok::Bin("gt"),
        "ne" | "neq" => Tok::Bin("ne"),
        "approx" => Tok::Bin("approx"),
        "equiv" | "cong" => Tok::Bin("equiv"),
        "sim" | "simeq" => Tok::Bin("sim"),
        "in" => Tok::Bin("in"),
        "notin" => Tok::Bin("notin"),
        "subset" => Tok::Bin("subset"),
        "subseteq" => Tok::Bin("subseteq"),
        "cup" => Tok::Bin("cup"),
        "cap" => Tok::Bin("cap"),
        "setminus" => Tok::Bin("setminus"),
        "to" | "rightarrow" | "longrightarrow" | "implies" | "Rightarrow" | "Longrightarrow" => Tok::Bin("imp"),
        "mapsto" => Tok::Bin("mapsto"),
        "iff" | "Leftrightarrow" | "leftrightarrow" | "Longleftrightarrow" => Tok::Bin("iff"),
        "land" | "wedge" => Tok::Bin("and"),
        "lor" | "vee" => Tok::Bin("or"),
        "circ" => Tok::Bin("circ"),
        "otimes" => Tok::Bin("otimes"),
        "oplus" => Tok::Bin("oplus"),
        "mid" => Tok::Bin("dvd"),
        "bmod" | "mod" => Tok::Bin("mod"),
        "neg" | "lnot" => Tok::Not,
        "forall" => Tok::Quant("forall"),
        "exists" => Tok::Quant("exists"),
        "langle" => Tok::LAngle,
        "rangle" => Tok::RAngle,
        "infty" => Tok::Sym("∞".into()),
        "pi" => Tok::Sym("π".into()),
        "emptyset" | "varnothing" => Tok::Sym("∅".into()),
        "partial" => Tok::Func("partial".into()),
        "nabla" => Tok::Func("nabla".into()),
        "operatorname" | "mathrm" | "text" | "mathbf" | "mathit" | "boldsymbol" | "vec" | "hat" | "bar" | "tilde"
        | "overline" | "dot" | "ddot" | "mathcal" | "mathfrak" | "mathsf" => Tok::Wrap,
        "mathbb" => Tok::Blackboard,
        "lbrace" | "{" => Tok::Open('{'),
        "rbrace" | "}" => Tok::Close('}'),
        "|" | "Vert" | "lVert" | "rVert" => Tok::Norm,
        "vert" | "lvert" | "rvert" => Tok::Bar,
        n if LATEX_FUNCS.contains(&n) => Tok::Func(n.into()),
        n if GREEK_VARS.contains(&n) => Tok::Var(n.into()),
        _ => return None, // spacing and layout: \left \right \, \quad \displaystyle ...
    };
    Some(t)
}

fn unicode_tok(c: char) -> Option<Tok> {
    let t = match c {
        '+' => Tok::Bin("add"),
        '-' | '−' => Tok::Minus,
        '*' | '·' | '×' | '⋅' => Tok::Bin("mul"),
        '/' | '÷' => Tok::Bin("div"),
        '%' => Tok::Bin("mod"),
        '=' => Tok::Bin("eq"),
        '<' => Tok::Bin("lt"),
        '>' => Tok::Bin("gt"),
        '≤' => Tok::Bin("le"),
        '≥' => Tok::Bin("ge"),
        '≠' => Tok::Bin("ne"),
        '≈' => Tok::Bin("approx"),
        '≡' | '≃' | '≅' => Tok::Bin("equiv"),
        '∼' => Tok::Bin("sim"),
        '∈' => Tok::Bin("in"),
        '∉' => Tok::Bin("notin"),
        '⊂' => Tok::Bin("subset"),
        '⊆' => Tok::Bin("subseteq"),
        '∪' => Tok::Bin("cup"),
        '∩' => Tok::Bin("cap"),
        '\\' => Tok::Bin("setminus"),
        '→' | '⟶' | '⇒' => Tok::Bin("imp"),
        '↔' | '⇔' => Tok::Bin("iff"),
        '↦' => Tok::Bin("mapsto"),
        '∧' => Tok::Bin("and"),
        '∨' => Tok::Bin("or"),
        '∘' => Tok::Bin("circ"),
        '•' => Tok::Bin("smul"),
        '⊗' => Tok::Bin("otimes"),
        '⊕' => Tok::Bin("oplus"),
        '∣' => Tok::Bin("dvd"),
        '±' => Tok::Bin("pm"),
        '¬' => Tok::Not,
        '∀' => Tok::Quant("forall"),
        '∃' => Tok::Quant("exists"),
        'λ' => Tok::Quant("fun"),
        '∑' => Tok::Big("sum"),
        '∏' => Tok::Big("prod"),
        '∫' => Tok::Big("int"),
        '∮' => Tok::Big("oint"),
        '⋃' => Tok::Big("bigcup"),
        '⋂' => Tok::Big("bigcap"),
        '√' => Tok::Sqrt,
        '∞' => Tok::Sym("∞".into()),
        'π' => Tok::Sym("π".into()),
        '∅' => Tok::Sym("∅".into()),
        'ℝ' => Tok::Sym("R".into()),
        'ℕ' => Tok::Sym("N".into()),
        'ℤ' => Tok::Sym("Z".into()),
        'ℚ' => Tok::Sym("Q".into()),
        'ℂ' => Tok::Sym("C".into()),
        '∂' => Tok::Func("partial".into()),
        '∇' => Tok::Func("nabla".into()),
        '(' | '[' | '{' => Tok::Open(c),
        ')' | ']' | '}' => Tok::Close(c),
        '⟨' | '⟪' => Tok::LAngle,
        '⟩' | '⟫' => Tok::RAngle,
        ',' => Tok::Comma,
        '^' => Tok::Caret,
        '_' => Tok::Under,
        '|' => Tok::Bar,
        '‖' => Tok::Norm,
        ':' => Tok::Colon,
        '!' => Tok::Postfix("fact"),
        '′' => Tok::Postfix("prime"),
        '⁻' => Tok::Postfix("inv"), // ⁻¹: the ¹ is dropped below
        '²' => Tok::SupNum("2".into()),
        '³' => Tok::SupNum("3".into()),
        _ => return None,
    };
    Some(t)
}

/// Lean mode: dotted names, binders, quantifiers or `fun ... =>`.
fn looks_lean(src: &str) -> bool {
    !src.contains('\\')
        && (src.contains(" : ") || src.contains('∀') || src.contains('∃') || src.contains("=>") || src.contains("fun ")
            || src.split(|c: char| !(c.is_alphanumeric() || c == '.' || c == '_'))
                .any(|w| w.len() > 2 && w.contains('.') && w.chars().next().is_some_and(char::is_uppercase)))
}

fn lex(src: &str, lean: bool) -> Vec<Tok> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() || c == '$' || c == '&' || c == '~' {
            i += 1;
        } else if c == '\\' {
            let j = i + 1;
            let mut k = j;
            while k < chars.len() && chars[k].is_ascii_alphabetic() {
                k += 1;
            }
            if k == j && k < chars.len() {
                k += 1; // one-char command: \, \{ \|
            }
            let name: String = chars[j..k].iter().collect();
            if let Some(t) = command_tok(&name) {
                if t == Tok::Wrap && matches!(name.as_str(), "text") {
                    // \text{...}: skip the braced argument entirely
                    let mut depth = 0i32;
                    let mut m = k;
                    while m < chars.len() {
                        match chars[m] {
                            '{' => depth += 1,
                            '}' => {
                                depth -= 1;
                                if depth <= 0 {
                                    m += 1;
                                    break;
                                }
                            }
                            _ if depth == 0 => break,
                            _ => {}
                        }
                        m += 1;
                    }
                    k = m;
                } else {
                    out.push(t);
                }
            }
            i = k;
        } else if c.is_ascii_digit() {
            let mut k = i;
            while k < chars.len() && (chars[k].is_ascii_digit() || (chars[k] == '.' && chars.get(k + 1).is_some_and(char::is_ascii_digit))) {
                k += 1;
            }
            out.push(Tok::Num(chars[i..k].iter().collect()));
            i = k;
        } else if c.is_alphabetic() && unicode_tok(c).is_none() {
            let mut k = i + 1;
            let ident_char = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '\'' || ch == '.';
            while k < chars.len() && ident_char(chars[k]) && unicode_tok(chars[k]).is_none_or(|_| chars[k] == '_') {
                if !lean && !chars[k].is_alphabetic() {
                    break;
                }
                k += 1;
            }
            let word: String = chars[i..k].iter().collect::<String>().trim_end_matches('.').to_string();
            i += word.chars().count().max(1);
            if lean {
                match word.as_str() {
                    "fun" => out.push(Tok::Quant("fun")),
                    "theorem" | "lemma" | "def" | "instance" | "abbrev" | "structure" | "class" => {}
                    w if w.chars().count() == 1 => out.push(Tok::Var(w.into())),
                    w => out.push(Tok::Ident(w.into())),
                }
            } else if LATEX_FUNCS.contains(&word.as_str()) {
                out.push(Tok::Func(word));
            } else {
                out.extend(word.chars().map(|ch| Tok::Var(ch.to_string()))); // LaTeX: `ab` is a*b
            }
        } else if c == '=' && chars.get(i + 1) == Some(&'>') {
            out.push(Tok::FatArrow);
            i += 2;
        } else if c == ':' && chars.get(i + 1) == Some(&'=') {
            out.push(Tok::Bin("eq"));
            i += 2;
        } else if c == '<' && chars.get(i + 1) == Some(&'=') {
            out.push(Tok::Bin("le"));
            i += 2;
        } else if c == '>' && chars.get(i + 1) == Some(&'=') {
            out.push(Tok::Bin("ge"));
            i += 2;
        } else if c == '!' && chars.get(i + 1) == Some(&'=') {
            out.push(Tok::Bin("ne"));
            i += 2;
        } else if c == '-' && chars.get(i + 1) == Some(&'>') {
            out.push(Tok::Bin("imp"));
            i += 2;
        } else if c == '⁻' {
            out.push(Tok::Postfix("inv"));
            i += if chars.get(i + 1) == Some(&'¹') { 2 } else { 1 };
        } else {
            if let Some(t) = unicode_tok(c) {
                out.push(t);
            }
            i += 1;
        }
    }
    out
}

/// Binary operators: (precedence, right-associative).
fn bin_info(op: &str) -> (u8, bool) {
    match op {
        "imp" | "iff" | "mapsto" => (1, true),
        "or" => (2, false),
        "and" => (3, false),
        "eq" | "ne" | "lt" | "le" | "gt" | "ge" | "approx" | "equiv" | "sim" | "in" | "notin" | "subset"
        | "subseteq" | "dvd" => (4, false),
        "add" | "sub" | "pm" | "mp" | "cup" | "oplus" | "setminus" => (5, false),
        _ => (6, false), // mul div mod cap otimes circ smul
    }
}

const JUXT_PREC: u8 = 7;
const MAX_DEPTH: usize = 128;
const MAX_TOKENS: usize = 4096;

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    lean: bool,
    bound: HashSet<String>,
    depth: usize,
    /// Pending closing delimiters that look like openers (`|`, `‖`).
    closers: Vec<Tok>,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn starts_atom(t: &Tok) -> bool {
        matches!(
            t,
            Tok::Num(_)
                | Tok::Var(_)
                | Tok::Ident(_)
                | Tok::Sym(_)
                | Tok::Func(_)
                | Tok::Big(_)
                | Tok::Frac
                | Tok::Sqrt
                | Tok::Binom
                | Tok::Wrap
                | Tok::Blackboard
                | Tok::Open(_)
                | Tok::Norm
                | Tok::Bar
                | Tok::LAngle
                | Tok::Not
                | Tok::Quant(_)
        )
    }

    fn expr(&mut self, min_prec: u8) -> Node {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.depth -= 1;
            self.pos = self.toks.len();
            return Node::op("err", vec![]);
        }
        let mut lhs = self.unary();
        while let Some(t) = self.peek().cloned() {
            if self.closers.last() == Some(&t) {
                break;
            }
            let (op, prec, right) = match &t {
                Tok::Bin(op) => {
                    let (p, r) = bin_info(op);
                    (*op, p, r)
                }
                Tok::Minus => ("sub", 5, false),
                Tok::Bar => ("st", 1, false), // set builder `{x | p x}` in operator position
                t if Self::starts_atom(t) => ("juxt", JUXT_PREC, false),
                _ => break,
            };
            if prec < min_prec {
                break;
            }
            if op == "juxt" {
                let apply = self.lean || matches!(lhs, Node::Sym(_)) && is_name(&lhs);
                let rhs = if apply { self.postfix() } else { self.unary_at(JUXT_PREC + 1) };
                lhs = if apply { apply_to(lhs, rhs) } else { Node::op("mul", vec![lhs, rhs]) };
                continue;
            }
            self.pos += 1;
            let rhs = self.expr(if right { prec } else { prec + 1 });
            lhs = match op {
                "gt" => Node::op("lt", vec![rhs, lhs]),
                "ge" => Node::op("le", vec![rhs, lhs]),
                "sub" => Node::op("add", vec![lhs, Node::op("neg", vec![rhs])]),
                _ => Node::op(op, vec![lhs, rhs]),
            };
        }
        self.depth -= 1;
        lhs
    }

    fn unary_at(&mut self, prec: u8) -> Node {
        match self.peek() {
            Some(t) if Self::starts_atom(t) || matches!(t, Tok::Minus) => {}
            _ => return Node::op("err", vec![]),
        }
        let _ = prec;
        self.unary()
    }

    fn unary(&mut self) -> Node {
        match self.peek() {
            Some(Tok::Minus) => {
                self.pos += 1;
                Node::op("neg", vec![self.expr(JUXT_PREC)])
            }
            Some(Tok::Not) => {
                self.pos += 1;
                Node::op("not", vec![self.expr(JUXT_PREC)])
            }
            Some(Tok::Big(_)) => self.big(),
            Some(Tok::Quant(_)) => self.quantifier(),
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> Node {
        let mut base = self.atom();
        loop {
            match self.peek() {
                Some(Tok::Caret) => {
                    self.pos += 1;
                    let e = self.script();
                    base = Node::op("pow", vec![base, e]);
                }
                Some(Tok::Under) => {
                    self.pos += 1;
                    let e = self.script();
                    base = Node::op("idx", vec![base, e]);
                }
                Some(Tok::SupNum(n)) => {
                    let n = n.clone();
                    self.pos += 1;
                    base = Node::op("pow", vec![base, Node::Num(n)]);
                }
                Some(Tok::Postfix(op)) => {
                    let op = *op;
                    self.pos += 1;
                    base = Node::op(op, vec![base]);
                }
                _ => return base,
            }
        }
    }

    /// Argument of `^` or `_`: a braced group or a single atom (`a^2`, `x_{n+1}`, `x^-1`).
    fn script(&mut self) -> Node {
        if self.eat(&Tok::Minus) {
            return Node::op("neg", vec![self.atom()]);
        }
        self.atom()
    }

    fn group(&mut self, close: char) -> Node {
        let mut items = vec![self.expr(0)];
        while self.eat(&Tok::Comma) {
            items.push(self.expr(0));
        }
        if self.peek() == Some(&Tok::Colon) {
            // type ascription `(x : T)`: keep the term, skip the type
            let mut depth = 0i32;
            while let Some(t) = self.peek() {
                match t {
                    Tok::Open(_) => depth += 1,
                    Tok::Close(_) if depth == 0 => break,
                    Tok::Close(_) => depth -= 1,
                    _ => {}
                }
                self.pos += 1;
            }
        }
        self.eat(&Tok::Close(close));
        if items.len() == 1 { items.pop().unwrap() } else { Node::op("tuple", items) }
    }

    /// `{...}` argument of a LaTeX command, or a single atom when unbraced (`\frac12`).
    fn arg(&mut self) -> Node {
        if self.eat(&Tok::Open('{')) {
            return self.group('}');
        }
        match self.next() {
            Some(Tok::Num(n)) if n.len() > 1 && !n.contains('.') => {
                // \frac12: digits are separate arguments
                let rest: String = n.chars().skip(1).collect();
                self.toks.insert(self.pos, Tok::Num(rest));
                Node::Num(n.chars().next().unwrap().to_string())
            }
            Some(t) => {
                self.pos -= 1;
                if Self::starts_atom(&t) { self.atom() } else { self.pos += 1; Node::op("err", vec![]) }
            }
            None => Node::op("err", vec![]),
        }
    }

    fn atom(&mut self) -> Node {
        let Some(t) = self.next() else { return Node::op("err", vec![]) };
        match t {
            Tok::Num(n) => Node::Num(n),
            Tok::Var(v) => Node::Var(v),
            Tok::Ident(name) => {
                if self.bound.contains(&name) { Node::Var(name) } else { Node::Sym(name) }
            }
            Tok::Sym(s) => Node::Sym(s),
            Tok::Open(c) => {
                let close = match c {
                    '(' => ')',
                    '[' => ']',
                    _ => '}',
                };
                self.group(close)
            }
            Tok::Frac => {
                let a = self.arg();
                let b = self.arg();
                Node::op("div", vec![a, b])
            }
            Tok::Binom => {
                let a = self.arg();
                let b = self.arg();
                Node::op("binom", vec![a, b])
            }
            Tok::Sqrt => {
                if self.eat(&Tok::Open('[')) {
                    let n = self.group(']');
                    let x = self.arg();
                    return Node::op("root", vec![x, n]);
                }
                Node::op("sqrt", vec![self.arg()])
            }
            Tok::Wrap => self.arg(),
            Tok::Blackboard => match self.arg() {
                Node::Var(v) | Node::Sym(v) => Node::Sym(v),
                Node::Op(_, kids) => {
                    let s: String = kids.iter().map(Node::sexpr).collect();
                    Node::Sym(s)
                }
                n => n,
            },
            Tok::Func(f) => {
                let mut exp = None;
                if self.eat(&Tok::Caret) {
                    exp = Some(self.script());
                }
                if self.eat(&Tok::Under) {
                    let _ = self.script();
                }
                let x = match self.peek() {
                    Some(Tok::Open('(')) => self.atom(),
                    Some(t) if Self::starts_atom(t) || matches!(t, Tok::Minus) => self.expr(JUXT_PREC),
                    _ => Node::op("err", vec![]),
                };
                let applied = Node::op(&f, vec![x]);
                match exp {
                    Some(e) => Node::op("pow", vec![applied, e]),
                    None => applied,
                }
            }
            Tok::Bar | Tok::Norm => {
                self.closers.push(t.clone());
                let x = self.expr(2);
                self.closers.pop();
                self.eat(&t);
                Node::op(if t == Tok::Bar { "abs" } else { "norm" }, vec![x])
            }
            Tok::LAngle => {
                let mut items = vec![self.expr(2)];
                while self.eat(&Tok::Comma) || self.eat(&Tok::Bar) {
                    items.push(self.expr(2));
                }
                self.eat(&Tok::RAngle);
                Node::op("inner", items)
            }
            Tok::Big(_) | Tok::Quant(_) | Tok::Not | Tok::Minus => {
                self.pos -= 1;
                self.unary()
            }
            _ => Node::op("err", vec![]), // stray token: consumed, so parsing always advances
        }
    }

    /// `\sum_{i=1}^{n} body`, `\int_a^b f`, `\lim_{x \to 0} f`.
    fn big(&mut self) -> Node {
        let Some(Tok::Big(op)) = self.next() else { return Node::op("err", vec![]) };
        let mut kids = vec![Node::op("err", vec![])];
        loop {
            if self.eat(&Tok::Under) {
                kids.push(Node::op("lo", vec![self.script()]));
            } else if self.eat(&Tok::Caret) {
                kids.push(Node::op("hi", vec![self.script()]));
            } else {
                break;
            }
        }
        kids[0] = match self.peek() {
            Some(t) if Self::starts_atom(t) || matches!(t, Tok::Minus) => self.expr(6),
            _ => Node::op("err", vec![]),
        };
        Node::op(op, kids)
    }

    /// `∀ x y : T, body`, `∃ x ∈ s, body`, `fun x => body`.
    fn quantifier(&mut self) -> Node {
        let Some(Tok::Quant(q)) = self.next() else { return Node::op("err", vec![]) };
        let mut depth = 0i32;
        let mut binder_type: Option<Node> = None;
        while let Some(t) = self.peek().cloned() {
            match t {
                Tok::Open(_) => depth += 1,
                Tok::Close(_) => depth -= 1,
                Tok::Comma | Tok::FatArrow if depth <= 0 => {
                    self.pos += 1;
                    break;
                }
                Tok::Var(v) | Tok::Ident(v) => {
                    self.bound.insert(v);
                }
                Tok::Colon | Tok::Bin("in") if depth <= 0 => {
                    let rel = if t == Tok::Colon { "type" } else { "in" };
                    self.pos += 1;
                    let ty = self.expr(5);
                    binder_type = Some(Node::op(rel, vec![ty]));
                    continue;
                }
                _ => {}
            }
            self.pos += 1;
        }
        let body = self.expr(0);
        let mut kids = vec![body];
        kids.extend(binder_type);
        Node::op(q, kids)
    }
}

fn is_name(n: &Node) -> bool {
    matches!(n, Node::Sym(s) if s.chars().count() > 1 && s.chars().next().is_some_and(char::is_alphabetic))
}

fn apply_to(f: Node, arg: Node) -> Node {
    match f {
        Node::Op(name, mut kids) if name == "app" => {
            kids.push(arg);
            Node::Op(name, kids)
        }
        f => Node::op("app", vec![f, arg]),
    }
}

/// Flatten nested commutative operators and sort their operands; drop `err` leaves.
fn normalize(n: Node) -> Node {
    match n {
        Node::Op(name, kids) => {
            let mut out = Vec::with_capacity(kids.len());
            for k in kids.into_iter().map(normalize) {
                match k {
                    Node::Op(ref kn, _) if kn == "err" => {}
                    Node::Op(kn, kk) if kn == name && is_commutative(&name) => out.extend(kk),
                    k => out.push(k),
                }
            }
            if is_commutative(&name) {
                out.sort();
            }
            Node::Op(name, out)
        }
        leaf => leaf,
    }
}

/// Collect Lean binder names: `(a b : T)`, `{x : T}`, `[inst : C]`, `∀ x y,`.
fn binder_names(toks: &[Tok]) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut i = 0;
    while i < toks.len() {
        if let Tok::Open(_) = toks[i] {
            let mut j = i + 1;
            let mut found = Vec::new();
            while let Some(Tok::Var(v) | Tok::Ident(v)) = toks.get(j) {
                found.push(v.clone());
                j += 1;
            }
            if toks.get(j) == Some(&Tok::Colon) {
                names.extend(found);
            }
        }
        i += 1;
    }
    names
}

/// Parse one expression (LaTeX, Unicode or Lean term).  `None` if nothing parsed.
pub fn parse(src: &str) -> Option<Node> {
    parse_all(src).into_iter().next()
}

/// Parse a formula or a Lean declaration.  A declaration `theorem N (h : P) : Q`
/// yields the conclusion `Q` first and then each hypothesis type that is not a plain
/// type name.  Plain formulas yield one tree.
pub fn parse_all(src: &str) -> Vec<Node> {
    let lean = looks_lean(src);
    let mut toks = lex(src, lean);
    toks.truncate(MAX_TOKENS);
    if toks.is_empty() {
        return Vec::new();
    }
    let bound = if lean { binder_names(&toks) } else { HashSet::new() };
    let mut trees = Vec::new();
    if lean {
        // split at depth-0 colon: binders before it, conclusion after
        let mut depth = 0i32;
        let mut split = None;
        for (i, t) in toks.iter().enumerate() {
            match t {
                Tok::Open(_) => depth += 1,
                Tok::Close(_) => depth -= 1,
                Tok::Colon if depth == 0 => {
                    split = Some(i);
                    break;
                }
                Tok::Quant(_) if depth == 0 => break,
                _ => {}
            }
        }
        if let Some(s) = split {
            let concl: Vec<Tok> = toks[s + 1..].to_vec();
            trees.extend(parse_tokens(concl, true, &bound));
            for hyp in hypotheses(&toks[..s]) {
                trees.extend(parse_tokens(hyp, true, &bound).filter(|t| t.size() > 2));
            }
            return trees;
        }
    }
    trees.extend(parse_tokens(toks, lean, &bound));
    trees
}

/// Types of the binders `(h : P)` in a declaration header.
fn hypotheses(toks: &[Tok]) -> Vec<Vec<Tok>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        if let Tok::Open(_) = toks[i] {
            let mut depth = 0i32;
            let mut colon = None;
            let mut j = i;
            while j < toks.len() {
                match toks[j] {
                    Tok::Open(_) => depth += 1,
                    Tok::Close(_) => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    Tok::Colon if depth == 1 && colon.is_none() => colon = Some(j),
                    _ => {}
                }
                j += 1;
            }
            if let Some(c) = colon {
                out.push(toks[c + 1..j.min(toks.len())].to_vec());
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

fn parse_tokens(toks: Vec<Tok>, lean: bool, bound: &HashSet<String>) -> Option<Node> {
    if toks.is_empty() {
        return None;
    }
    let mut p = Parser { toks, pos: 0, lean, bound: bound.clone(), depth: 0, closers: Vec::new() };
    let mut parts = Vec::new();
    while p.pos < p.toks.len() {
        let before = p.pos;
        let n = p.expr(0);
        if p.pos == before {
            p.pos += 1; // operator or closer in atom position: skip it
        }
        parts.push(n);
    }
    let tree = normalize(if parts.len() == 1 { parts.pop().unwrap() } else { Node::op("seq", parts) });
    match &tree {
        Node::Op(name, kids) if kids.is_empty() && (name == "err" || name == "seq") => None,
        Node::Op(name, kids) if name == "seq" && kids.len() == 1 => Some(kids[0].clone()),
        _ => Some(tree),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(src: &str) -> String {
        parse(src).map(|n| n.sexpr()).unwrap_or_default()
    }

    #[test]
    fn latex_and_plain_pythagoras_give_the_same_tree() {
        let t = "(eq (add (pow a 2) (pow b 2)) (pow c 2))";
        assert_eq!(s("a^2 + b^2 = c^2"), t);
        assert_eq!(s("$a^{2}+b^{2}=c^{2}$"), t);
        assert_eq!(s("c^2 = b^2 + a^2"), t, "commutative operands are sorted");
        assert_eq!(s("a² + b² = c²"), t);
    }

    #[test]
    fn latex_commands_fractions_functions_and_big_operators() {
        assert_eq!(s(r"\frac{a}{b}"), "(div a b)");
        assert_eq!(s(r"\frac12"), "(div 1 2)");
        assert_eq!(s(r"c^2 = a^2 + b^2 - 2ab\cos\gamma"), "(eq (add (neg (mul (cos gamma) a b 2)) (pow a 2) (pow b 2)) (pow c 2))");
        assert_eq!(s(r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}"), "(eq (div (mul (add n 1) n) 2) (sum i (lo (eq i 1)) (hi n)))");
        assert_eq!(s(r"\sqrt{x^2}"), "(sqrt (pow x 2))");
        assert_eq!(s(r"\|x\|^2 = \sum_k |\langle x, e_k\rangle|^2"), "(eq (pow (norm x) 2) (sum (pow (abs (inner x (idx e k))) 2) (lo k)))");
        assert_eq!(s(r"x \ge 0"), "(le 0 x)");
        assert_eq!(s(r"\mathbb{R}"), "R");
    }

    #[test]
    fn lean_declarations_use_binders_as_variables_and_application() {
        let trees = parse_all("theorem Nat.add_comm (n m : ℕ) : n + m = m + n");
        assert_eq!(trees[0].sexpr(), "(eq (add m n) (add m n))");
        let trees = parse_all("theorem Nat.Prime.two_le {p : ℕ} (hp : Nat.Prime p) : 2 ≤ p");
        assert_eq!(trees[0].sexpr(), "(le 2 p)");
        assert_eq!(trees[1].sexpr(), "(app Nat.Prime p)");
        assert_eq!(s("∀ x y : ℝ, x * y = y * x"), "(forall (eq (mul x y) (mul x y)) (type R))");
        assert_eq!(s("Real.sqrt (x ^ 2) = |x|"), "(eq (abs x) (app Real.sqrt (pow x 2)))");
    }

    #[test]
    fn garbage_never_panics() {
        for src in [")))", "\\frac{", "^^^", "∀", "⟨,,⟩", "a_", "\\left( x \\right", "||", "∑∑∑", "x⁻¹", "{x | x ∈ s}", ""] {
            let _ = parse_all(src);
        }
        let deep = "(".repeat(5000) + "x";
        let _ = parse_all(&deep);
    }
}
