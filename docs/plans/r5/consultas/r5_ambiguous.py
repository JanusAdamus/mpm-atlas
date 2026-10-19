"""R5 diagnosis: the 72 ambiguous or non-math single-word labels (agent judgment, list fixed 2026-10-19)."""
import json
from pathlib import Path

HOMO = {"red", "haz", "fin", "campo", "cara", "más", "Doble", "Diferencial", "head", "tail", "cut", "bola", "carta", "clave",
        "palabra", "toro", "Estrella", "factor", "Localmente", "Semisimple", "Abelian", "ambiguous", "complete", "core", "type",
        "outcome", "ruta", "vector", "producto", "unidad", "dato", "información", "programa", "escala", "moneda", "interés",
        "corriente", "trabajo", "momento", "Dual", "Cero", "captura", "final", "torre", "jaque", "tablas", "clavado",
        "promoción", "intercambio", "combinación", "táctica", "era", "luz", "NaN", "ALL", "Fish", "Dinamarca", "Jordania",
        "Sudáfrica", "Galileo", "energía", "masa", "gravedad", "Lean", "Trie", "computation", "vuelta", "sextillón"}
o = json.load(open(Path(__file__).parent / "r5-names.json", encoding="utf-8"))
hit = [x for x in o["single_word"]["examples"] if x[1] in HOMO]
print(len(hit))
for x in hit:
    print(x[0], x[1], "=", x[2])
