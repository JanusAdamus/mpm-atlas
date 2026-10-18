//! Presentation MathML (as Wikipedia/MediaWiki renders it) -> LaTeX-like text that
//! `parse` reads.  A TeX annotation, when present, wins.  Total: malformed markup
//! yields whatever text could be recovered, never a panic.

/// True if `s` looks like MathML rather than LaTeX or Lean.
pub fn is_mathml(s: &str) -> bool {
    let t = s.trim_start();
    t.starts_with("<math") || t.starts_with("<m:math")
}

enum Xml {
    El(String, Vec<Xml>, Option<String>),
    Text(String),
}

/// `<math>...</math>` to LaTeX-like text.
pub fn to_latex(src: &str) -> String {
    let mut pos = 0;
    let nodes = parse_children(src.as_bytes(), src, &mut pos, None, 0);
    let mut out = String::new();
    for n in &nodes {
        emit(n, &mut out);
    }
    out
}

const MAX_DEPTH: usize = 256;

fn parse_children(b: &[u8], src: &str, pos: &mut usize, close: Option<&str>, depth: usize) -> Vec<Xml> {
    let mut out = Vec::new();
    while *pos < b.len() {
        if b[*pos] != b'<' {
            let end = src[*pos..].find('<').map_or(b.len(), |i| *pos + i);
            let text = decode(&src[*pos..end]);
            if !text.trim().is_empty() {
                out.push(Xml::Text(text));
            }
            *pos = end;
            continue;
        }
        let Some(gt) = src[*pos..].find('>').map(|i| *pos + i) else {
            *pos = b.len();
            break;
        };
        let tag = &src[*pos + 1..gt];
        *pos = gt + 1;
        if let Some(name) = tag.strip_prefix('/') {
            if close.is_some_and(|c| c == local(name.trim())) {
                return out;
            }
            continue; // stray closer
        }
        if tag.starts_with('!') || tag.starts_with('?') {
            continue;
        }
        let self_closing = tag.ends_with('/');
        let tag = tag.trim_end_matches('/');
        let name = local(tag.split_whitespace().next().unwrap_or("")).to_string();
        let encoding = attr(tag, "encoding");
        let kids = if self_closing || depth >= MAX_DEPTH {
            Vec::new()
        } else {
            parse_children(b, src, pos, Some(&name), depth + 1)
        };
        out.push(Xml::El(name, kids, encoding));
    }
    out
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn attr(tag: &str, key: &str) -> Option<String> {
    let i = tag.find(&format!("{key}=\""))? + key.len() + 2;
    let j = tag[i..].find('"')? + i;
    Some(tag[i..j].to_string())
}

fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(j) = tail.find(';').filter(|&j| j <= 10) else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let ent = &tail[1..j];
        let c = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => out.push(c),
            None => out.push_str(&tail[..=j]),
        }
        rest = &tail[j + 1..];
    }
    out.push_str(rest);
    out
}

const BIG_OPS: &str = "∑∏∐∫∬∭∮⋃⋂⨁⨂";

fn text_of(n: &Xml) -> String {
    match n {
        Xml::Text(t) => t.clone(),
        Xml::El(_, kids, _) => kids.iter().map(text_of).collect(),
    }
}

fn braced(n: Option<&Xml>, out: &mut String) {
    out.push('{');
    if let Some(n) = n {
        emit(n, out);
    }
    out.push('}');
}

fn emit(n: &Xml, out: &mut String) {
    let Xml::El(name, kids, _) = n else {
        if let Xml::Text(t) = n {
            out.push_str(t.trim());
        }
        return;
    };
    let k = |i: usize| kids.get(i);
    match name.as_str() {
        "semantics" => {
            // prefer a TeX annotation, else the presentation child
            let tex = kids.iter().find_map(|c| match c {
                Xml::El(n, _, Some(e)) if n == "annotation" && e.contains("tex") => Some(text_of(c)),
                _ => None,
            });
            match tex {
                Some(t) => out.push_str(&t),
                None => kids.iter().take(1).for_each(|c| emit(c, out)),
            }
        }
        "annotation" | "annotation-xml" | "mphantom" | "mspace" | "none" | "mprescripts" => {}
        "mi" | "mn" => {
            let t = text_of(n);
            let t = t.trim();
            if t.chars().count() > 1 && name == "mi" {
                out.push_str(&format!(" \\operatorname{{{t}}} "));
            } else {
                out.push_str(t);
            }
        }
        "mo" => {
            let t = text_of(n);
            out.push(' ');
            out.push_str(match t.trim() {
                "\u{2061}" | "\u{2062}" | "\u{2063}" => "",
                "{" => "\\{",
                "}" => "\\}",
                other => other,
            });
            out.push(' ');
        }
        "mtext" | "ms" => {}
        "mfrac" => {
            out.push_str("\\frac");
            braced(k(0), out);
            braced(k(1), out);
        }
        "msqrt" => {
            out.push_str("\\sqrt{");
            kids.iter().for_each(|c| emit(c, out));
            out.push('}');
        }
        "mroot" => {
            out.push_str("\\sqrt[");
            if let Some(c) = k(1) {
                emit(c, out);
            }
            out.push(']');
            braced(k(0), out);
        }
        "msup" | "mover" => script(k(0), None, k(1), name == "mover", out),
        "msub" | "munder" => script(k(0), k(1), None, name == "munder", out),
        "msubsup" | "munderover" => script(k(0), k(1), k(2), false, out),
        "mtable" | "mtr" | "mlabeledtr" => {
            for (i, c) in kids.iter().enumerate() {
                if i > 0 {
                    out.push_str(" , ");
                }
                emit(c, out);
            }
        }
        _ => {
            // math, mrow, mstyle, mpadded, menclose, mtd, mfenced, merror, ...
            let group = name == "mrow" && kids.len() > 1;
            if group {
                out.push('{');
            }
            kids.iter().for_each(|c| emit(c, out));
            if group {
                out.push('}');
            }
        }
    }
}

/// Base with optional sub/superscript.  A big operator stays bare so `parse` sees
/// `\sum_{lo}^{hi} body`; `mover` with an accent (`x̄`, `x̂`) keeps only the base.
fn script(base: Option<&Xml>, sub: Option<&Xml>, sup: Option<&Xml>, under_over: bool, out: &mut String) {
    let base_text = base.map(text_of).unwrap_or_default();
    let big = base_text.trim().chars().count() == 1 && base_text.trim().chars().all(|c| BIG_OPS.contains(c));
    let accent = under_over && sup.is_some_and(|s| {
        let t = text_of(s);
        let t = t.trim();
        t.chars().count() == 1 && t.chars().all(|c| "¯^~˙¨→ˆ˜‾̅".contains(c))
    });
    if big {
        out.push(' ');
        out.push_str(base_text.trim());
    } else {
        braced(base, out);
    }
    if accent {
        return;
    }
    if let Some(s) = sub {
        out.push('_');
        braced(Some(s), out);
    }
    if let Some(s) = sup {
        out.push('^');
        braced(Some(s), out);
    }
    if big {
        out.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use super::super::parse::parse;
    use super::*;

    fn tree(mathml: &str) -> String {
        parse(&to_latex(mathml)).map(|n| n.sexpr()).unwrap_or_default()
    }

    #[test]
    fn wikipedia_mathml_becomes_the_same_tree_as_latex() {
        let pyth = r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><mrow><msup><mi>a</mi><mn>2</mn></msup><mo>+</mo><msup><mi>b</mi><mn>2</mn></msup><mo>=</mo><msup><mi>c</mi><mn>2</mn></msup></mrow></math>"#;
        assert!(is_mathml(pyth));
        assert_eq!(tree(pyth), parse("a^2+b^2=c^2").unwrap().sexpr());
        let frac = r#"<math><mfrac><mrow><mi>n</mi><mo stretchy="false">(</mo><mi>n</mi><mo>+</mo><mn>1</mn><mo stretchy="false">)</mo></mrow><mn>2</mn></mfrac></math>"#;
        assert_eq!(tree(frac), parse(r"\frac{n(n+1)}{2}").unwrap().sexpr());
    }

    #[test]
    fn big_operators_entities_and_tex_annotations() {
        let sum = r#"<math><munderover><mo>&#x2211;</mo><mrow><mi>i</mi><mo>=</mo><mn>1</mn></mrow><mi>n</mi></munderover><msub><mi>x</mi><mi>i</mi></msub><mo>&#x2265;</mo><mn>0</mn></math>"#;
        assert_eq!(tree(sum), parse(r"\sum_{i=1}^{n} x_i \ge 0").unwrap().sexpr());
        let ann = r#"<math><semantics><mrow><mi>q</mi></mrow><annotation encoding="application/x-tex">e^{i\pi}+1=0</annotation></semantics></math>"#;
        assert_eq!(to_latex(ann), r"e^{i\pi}+1=0");
        for bad in ["<math><mfrac><mi>a</mi>", "<<<>>>", "&#xZZZZ; &amp", "<math><msup></msup></math>"] {
            let _ = parse(&to_latex(bad));
        }
    }
}
