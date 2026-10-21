"""R5 step 2b: Math Coherence Map pilot, ~50 proposed connections for Adrián's review.  NOT ingested.

Input: Learning Commons Knowledge Graph v1.15.0 exports (relationships.jsonl, nodes.jsonl), filtered to the
Coherence Map relations (`buildsTowards`, author Student Achievement Partners) into cm.json:
    python -I r5b_coherence_pilot.py <cm.json> > ../coherence-map-piloto.md
Mapping standard -> concept: hand-made by the agent from each standard's description (STD below); only
standards whose main idea is one registry concept are mapped.  Pairs that map to the same concept are dropped.
"""
import json
import re
import sqlite3
import sys

STD = {  # statementCode -> (entity id, note)
    "3.NF.A.1": ("wd:Q66055", "fracción a/b como partes de un todo"),
    "3.NF.A.2": ("wd:Q35689", "la fracción como número en la recta"),
    "3.OA.A.1": ("wd:Q40276", "interpretar productos"),
    "3.OA.A.2": ("wd:Q1226939", "interpretar cocientes"),
    "3.OA.B.5": ("wd:Q187959", "propiedades de las operaciones (incluye la distributiva)"),
    "3.OA.C.7": ("wd:Q206049", "multiplicar y dividir con fluidez hasta 100"),
    "4.NF.B.3.a": ("wd:Q32043", "suma y resta de fracciones"),
    "4.NF.B.4.a": ("wd:Q255388", "a/b como múltiplo de 1/b"),
    "4.NF.C.6": ("wd:Q840057", "notación decimal de fracciones con denominador 10 o 100"),
    "4.OA.B.4": ("wd:Q50708", "pares de factores, múltiplos, primos y compuestos"),
    "5.NF.B.3": ("wd:Q41118", "la fracción como división"),
    "5.NF.B.5": ("wd:Q40276", "multiplicación como escalamiento"),
    "5.NF.B.7": ("wd:Q255388", "dividir fracciones unitarias"),
    "5.OA.B.3": ("wd:Q62912", "pares ordenados en el plano"),
    "6.EE.A.1": ("wd:Q33456", "exponentes enteros"),
    "6.EE.A.2": ("wd:Q4724001", "expresiones con letras"),
    "6.EE.A.3": ("wd:Q187959", "expresiones equivalentes con la propiedad distributiva"),
    "6.EE.B.5": ("wd:Q11345", "resolver una ecuación o desigualdad"),
    "6.EE.B.6": ("wd:Q50701", "variables"),
    "6.EE.B.7": ("wd:Q11345", "ecuaciones x+p=q y px=q"),
    "6.EE.B.8": ("wd:Q165309", "desigualdades x>c"),
    "6.EE.C.9": ("wd:Q188597", "variable dependiente e independiente"),
    "6.NS.A.1": ("wd:Q216906", "división de fracciones (recíproco)"),
    "6.NS.B.4": ("wd:Q131752", "máximo común divisor y mínimo común múltiplo"),
    "6.NS.C.5": ("wd:Q200227", "números positivos y negativos"),
    "6.NS.C.6.a": ("wd:Q715358", "opuesto de un número"),
    "6.NS.C.7.c": ("wd:Q120812", "valor absoluto como distancia a 0"),
    "6.RP.A.1": ("wd:Q3481047", "razón"),
    "6.RP.A.2": ("wd:Q3481047", "tasa unitaria de una razón"),
    "6.RP.A.3.c": ("wd:Q11229", "porcentaje"),
    "7.EE.A.1": ("wd:Q4724001", "expresiones lineales"),
    "7.EE.B.4.a": ("wd:Q11345", "ecuaciones px+q=r"),
    "7.EE.B.4.b": ("wd:Q165309", "desigualdades px+q>r"),
    "7.NS.A.1.d": ("wd:Q1244890", "sumar y restar racionales"),
    "7.NS.A.2.c": ("wd:Q1244890", "multiplicar y dividir racionales"),
    "7.NS.A.3": ("wd:Q1244890", "problemas con racionales"),
    "7.RP.A.2": ("wd:Q51379", "relaciones proporcionales"),
    "8.EE.A.1": ("wd:Q33456", "exponentes enteros (negativos incluidos)"),
    "8.EE.A.2": ("wd:Q134237", "raíz cuadrada y cúbica"),
    "8.EE.A.3": ("wd:Q219142", "potencias de 10 para estimar"),
    "8.EE.B.5": ("wd:Q275447", "tasa unitaria como pendiente"),
    "8.EE.C.8": ("wd:Q11203", "sistemas de dos ecuaciones lineales"),
    "8.NS.A.1": ("wd:Q607728", "números irracionales"),
}
DOUBT = {("wd:Q66055", "wd:Q35689"), ("wd:Q187959", "wd:Q206049"), ("wd:Q41118", "wd:Q3481047"),
         ("wd:Q50708", "wd:Q4724001"), ("wd:Q62912", "wd:Q4724001")}

d = json.load(open(sys.argv[1], encoding="utf-8"))
nodes = d["nodes"]
c = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
label = {i: json.loads(c.execute("select data from entities where id=?", (i,)).fetchone()[0])["labels"]["es"]
         for i, _ in STD.values()}
rows, seen = [], set()
for r in sorted(d["rels"], key=lambda r: r["identifier"]):
    if r["label"] != "buildsTowards":
        continue
    a = nodes[r["source_identifier"]]["properties"]["statementCode"]
    b = nodes[r["target_identifier"]]["properties"]["statementCode"]
    if a not in STD or b not in STD or STD[a][0] == STD[b][0] or (STD[a][0], STD[b][0]) in seen:
        continue
    seen.add((STD[a][0], STD[b][0]))
    rows.append((a, b, r["identifier"]))
rows.sort(key=lambda x: [int(t) if t.isdigit() else t for t in re.split(r"[.]", x[0] + "." + x[1])])
print("# Piloto Math Coherence Map: conexiones propuestas (sin ingerir)\n")
print("Fecha: 2026-10-19.  Estado: **para revisión de Adrián**; nada de esto está en el registro.  Fuente: Learning "
      "Commons Knowledge Graph v1.15.0 (`relationships.jsonl`, relación `buildsTowards`, autor Student Achievement "
      "Partners).  Licencia por fila: CC BY 4.0 de Learning Commons, que recibió las progresiones bajo CC0 "
      "(ficha de licencias de docs.learningcommons.org, leída 2026-10-19).  El texto de los estándares no se copia: "
      "solo su código.\n")
print("Cómo se hizo: cada estándar se asignó a mano a **un** concepto del registro a partir de su descripción "
      "(juicio de agente, columna «idea»).  Se descartan los pares que caen en el mismo concepto y los repetidos.  "
      "Tipo propuesto para todas: «progresión escolar» (`buildsTowards`), que **no** es prerrequisito lógico; la "
      "propia documentación advierte que las secuencias no nombran prerrequisitos definitivos.  Si se ingiere, "
      "entraría como `prerequisite_for` en `model_proposed` con esa nota, o como relación nueva si el arquitecto la crea.\n")
print(f"Total: {len(rows)} conexiones.  «Dudosa» = el agente duda de que la conexión entre conceptos sea útil aunque la "
      "progresión entre estándares sea correcta.\n")
print("| # | Desde (estándar) | Hacia (estándar) | Concepto desde | Concepto hacia | Idea desde → idea hacia | Juicio | Id de la relación (Learning Commons) | Revisión AJ |")
print("|---|---|---|---|---|---|---|---|---|")
for k, (a, b, rid) in enumerate(rows, 1):
    (ia, na), (ib, nb) = STD[a], STD[b]
    j = "dudosa" if (ia, ib) in DOUBT else "plausible"
    print(f"| {k} | {a} | {b} | {label[ia]} (`{ia}`) | {label[ib]} (`{ib}`) | {na} → {nb} | {j} | `{rid}` |  |")
print("\nGenerado por `docs/plans/r5/consultas/r5b_coherence_pilot.py` (sin cambios al registro).")
