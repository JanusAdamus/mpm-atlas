"""R5 diagnosis, stage 2: odd-name classes on the general layer (3,229 nodes of z0-z2, layout 649b68d3fc5a6cca)."""
import json, pickle, re, sys, unicodedata
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).parent
d = pickle.load(open(HERE / "r5-load.pkl", "rb"))
ents = d["ents"]
g = json.load(open(HERE / "r5-general.json", encoding="utf-8"))
shown = {n["id"]: n["label"] for n in g["nodes"]}  # label the ES map tile shows

def norm(s: str) -> str:
    s = unicodedata.normalize("NFKD", s.casefold())
    s = "".join(ch for ch in s if not unicodedata.combining(ch))
    s = re.sub(r"[^\w]+", " ", s).strip()
    return s

def ver(e):
    p = e.get("provenance") or [{}]
    return f"{p[0].get('extractor')} {p[0].get('extractor_version')}"

MATH_WORDS = re.compile(r"matem|mathemat|n[uú]mero|number|funci|function|[aá]lgebra|geometr|conjunto|\bset\b|teor|theor|"
                        r"grupo|group|espacio|space|matri|vector|ecuaci|equation|operaci|operation|estad|statist|probab|"
                        r"topolog|anillo|ring|cuerpo|field|integral|deriv|l[oó]gic|logic|axiom|lema|lemma|polin|polynom|"
                        r"curva|curve|superficie|surface|medida|measure|orden|order|relaci|relation|categor|morfism|"
                        r"secuencia|sucesi|sequence|serie|series|grafo|graph|algoritm|algorithm|c[aá]lculo|calculus|"
                        r"aritm|arithm|geom|trigon|combinat|analys|an[aá]lisis|transform|estructura|structure|"
                        r"propiedad|property|elemento de|element of|map|aplicaci|tensor|m[oó]dulo|module|ideal|"
                        r"divisor|primo|prime|entero|integer|real|complej|complex|racional|rational", re.I)
UNIT_PHYS = re.compile(r"unidad|unit of|\bunit\b|elemento qu[ií]mico|chemical element|magnitud f[ií]sica|physical quantity|"
                       r"part[ií]cula|particle|qu[ií]mic|chemi|f[ií]sica|physics|is[oó]topo|isotope|mol[eé]cula|molecule|"
                       r"\bcolor\b|colour|\bSI\b|sistema internacional|astronom|planeta|planet|estrella|\bstar\b|"
                       r"moneda|currency|biolog|enfermedad|disease|empresa|company|software|programa inform|video ?game", re.I)
AMBIG = {"red", "luz", "era", "nan", "head", "cut", "haz", "fin", "más", "all", "set", "map", "ring", "field", "group",
         "net", "sheaf", "cover", "bundle", "base", "term", "lift", "tower", "game", "chain", "flag", "frame", "kernel",
         "root", "cell", "fiber", "germ", "knot", "braid", "word", "tree", "path", "loop", "cone", "join", "meet"}
LEAN = re.compile(r"^[A-Z][a-z]+[A-Z]\w*$|^\w+\.\w+|_|^Is[A-Z]|^mathlib|^Mathlib")

cls = defaultdict(list)
for eid, (e, merged) in ents.items():
    lab = e.get("labels", {})
    es, en = lab.get("es"), lab.get("en")
    shown_lab = shown[eid]
    row = (eid, shown_lab, en, (e.get("descriptions") or {}).get("es") or (e.get("descriptions") or {}).get("en"), ver(e), e["type"])
    if merged:
        cls["merged_but_shown"].append(row)
    if es is None:
        cls["no_es_label"].append(row)
    if en is None:
        cls["no_en_label"].append(row)
    if es and en and es == en and len(es.split()) >= 2:
        cls["es_equals_en_multiword"].append(row)
    if "›" in shown_lab or "derivado" in shown_lab or "derived" in shown_lab:
        cls["derived_zone_label"].append(row)
    if LEAN.search(shown_lab) and "›" not in shown_lab:
        cls["lean_or_code_like"].append(row)
    words = norm(shown_lab).split()
    descr = " ".join((e.get("descriptions") or {}).values())
    if len(words) == 1 and not re.fullmatch(r"\d+", words[0]):
        cls["single_word"].append(row)
        if words[0] in AMBIG or not MATH_WORDS.search(descr):
            cls["single_word_ambiguous_or_nonmath_descr"].append(row)
    if re.fullmatch(r"[\d\s.,]+", shown_lab) or re.search(r"\d{3,}", shown_lab):
        cls["numeric"].append(row)
    if UNIT_PHYS.search(descr) and not MATH_WORDS.search(descr):
        cls["units_phys_chem_nonmath"].append(row)
    elif UNIT_PHYS.search(descr):
        cls["units_phys_chem_mixed"].append(row)
    if "…" in shown_lab or shown_lab.endswith("..."):
        cls["truncated"].append(row)
    if shown_lab[:1].isupper() and not shown_lab[:2].isupper() and en and en[:1].islower():
        cls["caps_es_upper_en_lower"].append(row)
    if shown_lab.isupper() and len(shown_lab) > 1 and re.search("[A-Z]", shown_lab):
        cls["all_caps"].append(row)
    if not e.get("descriptions"):
        cls["no_description"].append(row)

# near-duplicates: same normalized ES-shown label for >1 general-layer entity
byn = defaultdict(list)
for eid, lab in shown.items():
    byn[norm(lab)].append(eid)
dups = {k: v for k, v in byn.items() if len(v) > 1}
for k, v in dups.items():
    for eid in v:
        e = ents[eid][0]
        cls["dup_normalized_label"].append((eid, shown[eid], e["labels"].get("en"), k, ver(e), e["type"]))

out = {k: {"n": len(v), "examples": v} for k, v in cls.items()}
out["_dup_groups"] = {"groups": len(dups), "entities": sum(len(v) for v in dups.values()),
                      "groups_list": sorted(dups.items(), key=lambda kv: -len(kv[1]))}
json.dump(out, open(HERE / "r5-names.json", "w", encoding="utf-8"), ensure_ascii=False, indent=1)
for k, v in sorted(cls.items()):
    print(k, len(v))
print("dup groups", len(dups), "entities", sum(len(v) for v in dups.values()))
