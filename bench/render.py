"""Markdown rendering of a run report (Spanish prose, PROTOCOL.md §9)."""

from __future__ import annotations

from typing import Any


def _ci(m: dict | None, digits: int = 3) -> str:
    if not m or m.get("value") is None:
        return "—"
    lo, hi = m["ci95"]
    k = f" ({m['k']}/{m['n']})" if "k" in m else f" (n={m['n']})"
    return f"{m['value']:.{digits}f} [{lo:.{digits}f}, {hi:.{digits}f}]{k}"


def markdown(rep: dict[str, Any]) -> str:
    s = rep["summary"]
    lines = [f"# AtlasBench · reporte {rep['date'][:19]}", "",
             f"Protocolo: {rep['protocol']}.  Datos: `{rep['data']}`.  Registro: `{rep['registry']}`.",
             f"Hardware: {rep['hardware']}.  Sistema evaluado: `{rep['system_under_test']}`; baseline: "
             f"`{rep['baseline']}` (vía léxica FTS5).  Aperturas del conjunto sellado: {rep['sealed_openings']}.", "",
             f"Evidencia por tarea: {rep['evidence']}.  Ningún ítem es oro de especialistas salvo que diga "
             "`expert_adjudicated`; los veredictos sobre ítems silver o escritos por agente llevan `PROVISIONAL-`.", "",
             "## Veredictos", "", "| Componente | Veredicto | Regla | Detalle |", "|---|---|---|---|"]
    for k, v in rep["verdicts"].items():
        detail = v.get("reason") or {kk: vv for kk, vv in v.items() if kk not in ("verdict", "rule")}
        lines.append(f"| {k} | {v['verdict']} | {v['rule']} | {str(detail)[:300]} |")
    lines += ["", "## Componentes ausentes", ""]
    for comp in ("retrieval", "routes"):
        for b, why in s[comp].get("missing", {}).items():
            lines.append(f"- {comp} / {b}: {why}")
    for b, why in s["sourced"].get("missing", {}).items():
        lines.append(f"- sourced / {b}: {why}")
    lines += ["", "## Recuperación (nDCG@10, IC 95% bootstrap por familia)", "",
              f"Ítems resueltos: {s['retrieval']['n_items']}; sin resolver en el registro: "
              f"{s['retrieval']['unresolved']}.", "", "| Grupo | n | triviales | " +
              " | ".join(f"{b} nDCG@10 | {b} recall@10" for b in s["retrieval"]["backends"]) + " |",
              "|---|---|---|" + "---|---|" * len(s["retrieval"]["backends"])]
    for section in ("cells", "by_generator"):
        for name, cell in s["retrieval"][section].items():
            cols = " | ".join(f"{_ci(cell[b]['ndcg@10'])} | {_ci(cell[b]['recall@10'])}"
                              for b in s["retrieval"]["backends"])
            lines.append(f"| {section}:{name} | {cell['n']} | {cell['trivial']} | {cols} |")
    lines += ["", "## Recorridos", "", f"Ítems resueltos: {s['routes']['n_items']}; sin resolver: "
              f"{s['routes']['unresolved']}.", ""]
    for b, r in s["routes"]["by_backend"].items():
        lines.append(f"- `{b}`: estructura válida {_ci(r['structurally_valid'])}; adversariales sin conclusión "
                     f"prohibida {_ci(r['adversarial_no_forbidden'])}; acierto en respondibles "
                     f"{_ci(r['answerable_allowed_hit'])}; abstención en respondibles {_ci(r['abstain_on_answerable'])};"
                     f" sobreafirmación vs oráculo {_ci(r['overclaim_vs_oracle'])}.")
    er = s["er"]
    lines += ["", "## Resolución de entidades", "", f"Pares resueltos: {er['n_items']}.  Abstención: "
              f"{_ci(er['abstention'])}.  Fusiones del sistema muestreadas: {er['system_merges_sampled']} "
              f"(etiquetadas: {er['system_merges_labeled']}).", ""]
    lines += [f"- falsa fusión en negativos `{h}`: {_ci(v)}" for h, v in er["false_merge_rate_on_negatives"].items()]
    lines += [f"- cobertura en positivos `{h}`: {_ci(v)}" for h, v in er["merge_recall_on_positives"].items()]
    rel = s["relations"]
    lines += ["", "## Relaciones", "", f"Muestra: {rel['n_items']}; etiquetadas: {rel['labeled']}; aún vigentes: "
              f"{_ci(rel['still_current'])}.", ""]
    lines += [f"- corroboradas por otra fuente, familia `{f}`: {_ci(v)}"
              for f, v in rel["corroborated_by_another_source"].items()]
    lines += ["", "## Procedencia", ""]
    lines += [f"- `{b}`: completa y coincidente {_ci(v['complete_and_matching'])}; problemas {v['problems']}; "
              f"vía sucesora vigente (misma fuente y localizador) {v.get('via_successor')}"
              for b, v in s["sourced"]["by_backend"].items()]
    lines += ["", "## Latencia (ms)", "", "| Serie | n | p50 | p95 | IC p95 | p99 |", "|---|---|---|---|---|---|"]
    for k, v in s["latency_ms"].items():
        if v.get("n"):
            ci = v["p95_ci"]
            lines.append(f"| {k} | {v['n']} | {v['p50']:.2f} | {v['p95']:.2f} | [{ci[0]:.2f}, {ci[1]:.2f}] | "
                         f"{v['p99']:.2f} |")
    lines.append("")
    return "\n".join(lines)
