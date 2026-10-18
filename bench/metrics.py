"""Metrics and intervals for AtlasBench (bench/PROTOCOL.md §5).

Wilson for proportions, percentile bootstrap (optionally clustered, optionally paired)
for per-query metrics, graded nDCG / recall / MRR, latency percentiles, Cohen's kappa
and Krippendorff's alpha (nominal and interval) with missing data.
"""

from __future__ import annotations

import math
from collections import Counter, defaultdict
from typing import Callable, Hashable, Iterable, Mapping, Sequence

import numpy as np

Z95 = 1.959963984540054
BOOT_B = 2000
SEED = 20261016


def wilson(k: int, n: int, z: float = Z95) -> tuple[float, float]:
    """Wilson score interval for k successes out of n.  n=0 gives the vacuous (0, 1)."""
    if n <= 0:
        return 0.0, 1.0
    if not 0 <= k <= n:
        raise ValueError(f"k={k} outside [0, {n}]")
    p = k / n
    den = 1 + z * z / n
    center = (p + z * z / (2 * n)) / den
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / den
    # the closed form gives exactly 0 / 1 at k = 0 / k = n; pin them against rounding (0.9999999999999999)
    return (0.0 if k == 0 else max(0.0, center - half)), (1.0 if k == n else min(1.0, center + half))


def proportion(k: int, n: int) -> dict:
    lo, hi = wilson(k, n)
    return {"k": k, "n": n, "value": (k / n) if n else None, "ci95": [lo, hi], "interval": "wilson"}


def min_n_for_lower_bound(target: float, z: float = Z95) -> int:
    """Smallest n whose Wilson lower bound with zero failures reaches `target` (= n/(n+z^2))."""
    return math.ceil(target * z * z / (1 - target) - 1e-9)


def bootstrap(values: Sequence[float], *, clusters: Sequence[Hashable] | None = None,
              stat: Callable[[np.ndarray], float] = np.mean, b: int = BOOT_B, seed: int = SEED) -> dict:
    """Percentile bootstrap CI of `stat`.  With `clusters`, whole clusters are resampled."""
    x = np.asarray(values, dtype=float)
    if x.size == 0:
        return {"value": None, "n": 0, "ci95": [None, None], "interval": "bootstrap"}
    rng = np.random.default_rng(seed)
    point = float(stat(x))
    if clusters is None:
        reps = np.empty(b)
        for lo in range(0, b, 100):  # chunks keep the index matrix at 100 x n
            idx = rng.integers(0, x.size, size=(min(100, b - lo), x.size))
            reps[lo:lo + idx.shape[0]] = [stat(x[row]) for row in idx]
    else:
        groups = _group_indices(clusters)
        k = len(groups)
        reps = np.empty(b)
        for i in range(b):
            pick = rng.integers(0, k, size=k)
            reps[i] = stat(x[np.concatenate([groups[j] for j in pick])])
    lo, hi = np.percentile(reps, [2.5, 97.5])
    return {"value": point, "n": int(x.size), "clusters": None if clusters is None else len(set(clusters)),
            "ci95": [float(lo), float(hi)], "interval": "bootstrap-percentile", "B": b, "seed": seed}


def paired_delta(system: Sequence[float], baseline: Sequence[float], *,
                 clusters: Sequence[Hashable] | None = None, b: int = BOOT_B, seed: int = SEED) -> dict:
    """Bootstrap CI of mean(system - baseline), resampling queries (or clusters) jointly."""
    if len(system) != len(baseline):
        raise ValueError("paired_delta needs aligned arrays")
    d = np.asarray(system, dtype=float) - np.asarray(baseline, dtype=float)
    return bootstrap(d, clusters=clusters, b=b, seed=seed)


def _group_indices(clusters: Sequence[Hashable]) -> list[np.ndarray]:
    groups: dict[Hashable, list[int]] = defaultdict(list)
    for i, c in enumerate(clusters):
        groups[c].append(i)
    return [np.asarray(v) for v in groups.values()]


# ------------------------------------------------------------------ retrieval

def dcg(gains: Sequence[float]) -> float:
    return sum((2.0 ** g - 1.0) / math.log2(i + 2) for i, g in enumerate(gains))


def ndcg_at_k(ranked: Sequence[str], grades: Mapping[str, int], k: int = 10) -> float:
    """Graded nDCG@k (gain 2^g - 1).  `grades` maps relevant ids to 0-3; unjudged ids count as 0."""
    ideal = dcg(sorted((g for g in grades.values() if g > 0), reverse=True)[:k])
    if ideal == 0:
        raise ValueError("ndcg undefined: no relevant item")
    return dcg([grades.get(d, 0) for d in _dedup(ranked)[:k]]) / ideal


def recall_at_k(ranked: Sequence[str], grades: Mapping[str, int], k: int = 10, min_grade: int = 2) -> float:
    rel = {d for d, g in grades.items() if g >= min_grade}
    if not rel:
        raise ValueError("recall undefined: no relevant item")
    return len(rel & set(_dedup(ranked)[:k])) / len(rel)


def mrr_at_k(ranked: Sequence[str], grades: Mapping[str, int], k: int = 10, min_grade: int = 2) -> float:
    for i, d in enumerate(_dedup(ranked)[:k]):
        if grades.get(d, 0) >= min_grade:
            return 1.0 / (i + 1)
    return 0.0


def _dedup(ranked: Sequence[str]) -> list[str]:
    seen: set[str] = set()
    return [d for d in ranked if not (d in seen or seen.add(d))]


# ------------------------------------------------------------------ latency

def percentiles(ms: Sequence[float], qs: Iterable[float] = (50, 95, 99)) -> dict:
    x = np.asarray(ms, dtype=float)
    if x.size == 0:
        return {"n": 0}
    out = {f"p{int(q)}": float(np.percentile(x, q)) for q in qs}
    out.update(n=int(x.size), mean=float(x.mean()), max=float(x.max()))
    return out


def p95_ci(ms: Sequence[float]) -> dict:
    return bootstrap(ms, stat=lambda a: float(np.percentile(a, 95)))


# ------------------------------------------------------------------ agreement

def cohen_kappa(a: Sequence[Hashable], b: Sequence[Hashable]) -> float:
    """Cohen's kappa for two raters over the same units (no missing values)."""
    if len(a) != len(b) or not a:
        raise ValueError("cohen_kappa needs two equal-length, non-empty label lists")
    n = len(a)
    po = sum(x == y for x, y in zip(a, b)) / n
    ca, cb = Counter(a), Counter(b)
    pe = sum(ca[c] * cb[c] for c in set(ca) | set(cb)) / (n * n)
    if pe == 1.0:
        return 1.0 if po == 1.0 else 0.0
    return (po - pe) / (1 - pe)


def krippendorff_alpha(units: Mapping[Hashable, Mapping[Hashable, Hashable | None]], metric: str = "nominal") -> float:
    """Krippendorff's alpha.  `units[u][rater] = value` (None or absent = missing).

    Units with fewer than two values are not pairable and are dropped (Krippendorff 2011).
    metric: 'nominal' (0/1 disagreement) or 'interval' (squared difference, numeric values).
    """
    if metric not in ("nominal", "interval"):
        raise ValueError(f"metric {metric!r}")
    delta = (lambda x, y: 0.0 if x == y else 1.0) if metric == "nominal" else (lambda x, y: (x - y) ** 2)
    coinc: Counter = Counter()
    for vals in units.values():
        v = [x for x in vals.values() if x is not None]
        m = len(v)
        if m < 2:
            continue
        for i, x in enumerate(v):
            for j, y in enumerate(v):
                if i != j:
                    coinc[(x, y)] += 1.0 / (m - 1)
    n_c: Counter = Counter()
    for (x, _), w in coinc.items():
        n_c[x] += w
    n = sum(n_c.values())
    if n <= 1:
        raise ValueError("krippendorff_alpha needs at least one pairable unit")
    d_o = sum(w * delta(x, y) for (x, y), w in coinc.items()) / n
    d_e = sum(n_c[x] * n_c[y] * delta(x, y) for x in n_c for y in n_c) / (n * (n - 1))
    return 1.0 if d_e == 0 else 1.0 - d_o / d_e
