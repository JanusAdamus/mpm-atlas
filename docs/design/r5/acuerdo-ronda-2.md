# Ronda 5 · acuerdo entre los jueces A y B, ronda 2

Fecha: 2026-10-21.  Entradas: `juez-A/ronda-2/muestra-20.tsv`, `juez-B/ronda-2/muestra-20.tsv` y los dos `ronda-2/veredicto.md`.  Snapshot `atlas-19c6f7dc19dab3e9.snap`, registro `event_seq` 4,188,644, layout `e6ed5eab030b3aec`.  Protocolo: `docs/plans/r5/protocolo.md` §3.  Todo lo de abajo es cálculo sobre los juicios de dos agentes; ningún ítem quedó `editorially_reviewed`.

## 1. ¿Juzgaron los mismos ítems?

Sí.  Los dos TSV tienen los mismos 20 `item_id` en el mismo orden (D1–D10, A1–A10).  A escribe el id con el claim y los extremos (`D1 c:bfb1… wd:Q208237>wd:Q2785361`) y B solo el número (`D1`); se comparó el primer token.  Además, la muestra de A (`juez-A/ronda-2/consultas/r5-protocol-sample-20261021.json`) y la de B (`juez-B/ronda-2/r5-protocol-sample.json`) son idénticas byte a byte (`cmp`).  Semilla 20261021 (directo `Random(20261021)`, agregado `Random(20261022)`), pools 1,475 directas y 20,359 agregadas.  **0 discrepancias de ítems.**

Nota de semilla: el protocolo §2 fija 20261020; la tarea de los jueces fijó 20261021.  A juzgó también la muestra 20261020 (`muestra-20-semilla-20261020.tsv`, 11/20), pero B no, así que κ solo se puede calcular sobre la 20261021.  Esta es la muestra que cuenta aquí.

## 2. Correcta y útil, por juez

Etiqueta: correcta y útil = `correct = 1` y `useful = 1` en el TSV (útil «medio» se registró como 1; «dudosa» como `correct = 0`), como pide el protocolo.  Intervalo de Wilson al 95 %.

| | Total (n = 20) | Directo (n = 10) | Agregado (n = 10) |
|---|---|---|---|
| Juez A | 9/20 [0.258, 0.658] | 8/10 [0.490, 0.943] | 1/10 [0.018, 0.404] |
| Juez B | 11/20 [0.342, 0.742] | 9/10 [0.596, 0.982] | 2/10 [0.057, 0.510] |
| **Los dos (conservador)** | **9/20 [0.258, 0.658]** | 8/10 [0.490, 0.943] | 1/10 [0.018, 0.404] |

Los 9 que A da por buenos también los da B, así que la cuenta conjunta coincide con la de A.  La meta es ≥ 14/20: faltan 5 ítems, y el límite superior de Wilson (0.658) queda por debajo de 0.70.

## 3. Acuerdo

Tabla 2×2 (filas A, columnas B):

| | B sí | B no | Total |
|---|---|---|---|
| A sí | 9 | 0 | 9 |
| A no | 2 | 9 | 11 |
| Total | 11 | 9 | 20 |

- Acuerdo bruto: 18/20 = 0.900.
- Acuerdo esperado por azar: 0.495.
- **κ de Cohen = 0.802.**  Con estos marginales (A 9, B 11) el κ máximo alcanzable es también 0.802: los dos desacuerdos son exactamente los que obliga la diferencia de marginales.  B es más laxo que A, no hay desacuerdo en el otro sentido.
- Lectura (Landis y Koch): acuerdo sustancial, en el borde con casi perfecto (0.61–0.80 / 0.81–1.00).
- **Cautela de n = 20.**  κ con 20 ítems es inestable: un solo ítem que cambie de celda mueve κ unas 0.1–0.2.  Bootstrap por ítem (20,000 réplicas, semilla 20261021): IC percentil 95 % [0.50, 1.00]; P(κ < 0.4) ≈ 0.005.  Jackknife: EE ≈ 0.14.  Los dos son aproximaciones gruesas a este tamaño.  Sirve para decir que κ ≥ 0.4 se cumple con margen; no sirve para afirmar un valor preciso.  Además los dos jueces son agentes del mismo tipo de modelo, así que el acuerdo puede venir en parte de un sesgo común y no solo de que los ítems sean claros.
- El protocolo pide κ ≥ 0.4 **y** ≥ 14/20.  La parte de κ se cumple; la de la cuenta no.

### Desacuerdos

| Ítem | A | B | Qué se discute |
|---|---|---|---|
| D7 | correcta 0 (dudosa), útil 1 | 1, 1 | «extensión de cuerpos es caso particular de espacio vectorial».  A: una extensión es un par de cuerpos, no un espacio vectorial.  B: laxa pero correcta (L es un K-espacio).  Es la lectura de P279 como «caso particular», pendiente en `p279-opciones.md`. |
| A3 | 0, 0 | 1, 1 | `Matrix.SpecialLinearGroup.diag2n` usa `Matrix.diagonal`.  A: asignación dudosa a «matriz diagonal».  B: verdadera, útil a medias. |

Fuera de la etiqueta combinada, también difieren en la pregunta «¿correcta?» en A8 (A 0, B 1; los dos dicen útil 0, así que no cambia la etiqueta).

## 4. Reglas, juez por juez

Una regla pasa solo si los dos jueces dicen PASS.

| Regla | A | B | Combinado | Nota |
|---|---|---|---|---|
| Aristas agregadas | PASS | PASS | **PASS** | A: 71,047 aristas, 0 más fuertes que su eslabón (`weakest_link_real`).  B: lo mismo y 29 `extracted` revisadas. |
| Equivalencias | PASS | PASS | **PASS** | 0 `equivalent_to` desde P460; 8 pares en los dos sentidos, los 8 `disputed`. |
| Nombres en español | FAIL | FAIL | **FAIL** | 1,329/2,041 = 65.1 % [0.630, 0.672] contra ≥ 80 %.  0 derivados (esa mitad pasa). |
| Fuera de dominio | PASS | FAIL | **FAIL** | Desacuerdo.  A contó las 49 exclusiones curadas (0 en z0–z2).  B usó la lista del diagnóstico y encontró `wd:Q387015` «trie» en z0–z2.  La regla dice «según la lista del diagnóstico», así que manda B. |
| Muestra de 20 conexiones | FAIL | FAIL | **FAIL** | Conservador 9/20 [0.258, 0.658]; κ = 0.80. |
| Fichas | FAIL | FAIL | **FAIL** | 1,324 `concept` con extracto (1,831 nodos); 0 fichas revisadas por Adrián. |
| Prueba de usuario común | FAIL | FAIL | **FAIL** | 0/7 [0, 0.354]. |
| Obras | FAIL | FAIL | **FAIL** | Solo el piloto. |
| Escala | NO EVALUABLE | NO EVALUABLE | **NO EVALUABLE** | No hay escala real.  p95 búsqueda c8: A 100.9 ms, B 111.1 ms (corridas distintas; los dos ≥ 100). |
| Licencias | FAIL | FAIL | **FAIL** | No están en `docs/STATUS.md`. |
| Sin regresión | PASS (con reservas) | PASS | **PASS (con reservas)** | Reservas de A: v3.1 tiene otros ítems que v3, y dos decisiones de v3.1 siguen pendientes de Adrián. |

**Combinado: 3 PASS, 7 FAIL y 1 NO EVALUABLE.**  A tenía 4 PASS; la diferencia es «Fuera de dominio».

## 5. Cómo se reproduce

`python -I acuerdo_r2.py C:/MPM/docs/design/r5` (Python 3.14, solo la biblioteca estándar).  El script comprueba que los `item_id` coinciden antes de calcular y trae dos pruebas de κ (acuerdo perfecto = 1, independencia = 0).  Salida de la corrida del 2026-10-21:

```
A     total 9/20  Wilson95 [0.258, 0.658]
B     total 11/20  Wilson95 [0.342, 0.742]
ambos total 9/20  Wilson95 [0.258, 0.658]
2x2 (A,B): {(1, 1): 9, (1, 0): 0, (0, 1): 2, (0, 0): 9}
acuerdo bruto 0.900  kappa 0.8020  kappa_max 0.8020
discrepantes: ['D7', 'A3']
jackknife SE 0.136  ~95% [0.536, 1.068]
bootstrap percentil 95% [0.500, 1.000]  P(kappa<0.4)=0.0051  B=20000
```

```python
"""Acuerdo entre jueces A y B, ronda 2 (muestra-20.tsv)."""
import csv, math, sys
from pathlib import Path

R5 = Path(sys.argv[1])
Z = 1.959963984540054  # norm.ppf(0.975)

def load(judge: str) -> dict[str, tuple[int, int]]:
    rows = csv.DictReader(open(R5 / f"juez-{judge}/ronda-2/muestra-20.tsv", encoding="utf-8"), delimiter="\t")
    return {r["item_id"].split()[0]: (int(r["correct"]), int(r["useful"])) for r in rows}

def wilson(k: int, n: int) -> tuple[float, float]:
    p = k / n; d = 1 + Z**2 / n
    c = (p + Z**2 / (2 * n)) / d; h = Z * math.sqrt(p * (1 - p) / n + Z**2 / (4 * n * n)) / d
    return c - h, c + h

def kappa(a: list[int], b: list[int]) -> tuple[float, float, float]:
    n = len(a); po = sum(x == y for x, y in zip(a, b)) / n
    pa, pb = sum(a) / n, sum(b) / n
    pe = pa * pb + (1 - pa) * (1 - pb)
    pmax = min(pa, pb) + min(1 - pa, 1 - pb)
    return po, (po - pe) / (1 - pe), (pmax - pe) / (1 - pe)

A, B = load("A"), load("B")
assert list(A) == list(B), (set(A) ^ set(B))
ids = list(A)
ca = [int(A[i] == (1, 1)) for i in ids]
cb = [int(B[i] == (1, 1)) for i in ids]
both = [x & y for x, y in zip(ca, cb)]
for name, v in (("A", ca), ("B", cb), ("ambos", both)):
    for stratum, sel in (("total", ids), ("D", [i for i in ids if i[0] == "D"]), ("Ag", [i for i in ids if i[0] == "A"])):
        k = sum(v[ids.index(i)] for i in sel); lo, hi = wilson(k, len(sel))
        print(f"{name:5} {stratum:5} {k}/{len(sel)}  Wilson95 [{lo:.3f}, {hi:.3f}]")
t = {(x, y): sum(1 for p, q in zip(ca, cb) if (p, q) == (x, y)) for x in (1, 0) for y in (1, 0)}
print("2x2 (A,B):", t)
po, k, kmax = kappa(ca, cb)
print(f"acuerdo bruto {po:.3f}  kappa {k:.4f}  kappa_max {kmax:.4f}")
print("discrepantes:", [i for i, x, y in zip(ids, ca, cb) if x != y])
# check: perfect agreement gives kappa 1; independence-ish gives 0
assert abs(kappa([1, 0, 1, 0], [1, 0, 1, 0])[1] - 1) < 1e-12
assert abs(kappa([1, 1, 0, 0], [1, 0, 1, 0])[1]) < 1e-12
# jackknife SE for kappa (rough, n=20)
js = [kappa(ca[:j] + ca[j+1:], cb[:j] + cb[j+1:])[1] for j in range(len(ca))]
m = sum(js) / len(js); se = math.sqrt((len(js) - 1) / len(js) * sum((x - m) ** 2 for x in js))
print(f"jackknife SE {se:.3f}  ~95% [{k - Z*se:.3f}, {k + Z*se:.3f}]")
try:
    from statsmodels.stats.proportion import proportion_confint
    print("statsmodels wilson 9/20:", proportion_confint(9, 20, method="wilson"))
except ImportError:
    print("statsmodels no disponible")
try:
    from sklearn.metrics import cohen_kappa_score
    print("sklearn kappa:", cohen_kappa_score(ca, cb))
except ImportError:
    print("sklearn no disponible")
import random
rng = random.Random(20261021); bs = []
for _ in range(20000):
    s = [rng.randrange(20) for _ in range(20)]
    a2, b2 = [ca[j] for j in s], [cb[j] for j in s]
    pa, pb = sum(a2) / 20, sum(b2) / 20
    pe = pa * pb + (1 - pa) * (1 - pb)
    if pe < 1: bs.append((sum(x == y for x, y in zip(a2, b2)) / 20 - pe) / (1 - pe))
bs.sort(); print(f"bootstrap percentil 95% [{bs[int(.025*len(bs))]:.3f}, {bs[int(.975*len(bs))-1]:.3f}]  P(kappa<0.4)={sum(x<0.4 for x in bs)/len(bs):.4f}  B={len(bs)}")
```
