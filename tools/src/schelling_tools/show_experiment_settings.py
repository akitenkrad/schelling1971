"""schelling-tools show-experiment-settings — Display experiment settings.

This command has two display modes:

1. Display paper reproduction experiment definitions (default)
   Displays the settings for the Fig.7-17 experiments defined by
   `paper_experiments()` in `reproduce_paper.py`, along with the reference
   ranges reported in the paper. This provides a preview before running `reproduce`.

2. Display settings for existing results (`--results-dir <path>`)
   Reads config.json from a runvault run directory (an envelope whose conditions
   are under `parameters`) and displays all parameters used for the run. Whether
   it is a run or sweep is determined from subcommand in run.json. Legacy flat
   config.json and sweep_config.json files are also supported.

   The run directory path can be obtained as follows:
       runvault path --experiment schelling --latest --subcommand run
       runvault path --experiment schelling --latest --subcommand sweep

Usage:
    schelling-tools show-experiment-settings
    schelling-tools show-experiment-settings --only fig11_tau_one_third
    schelling-tools show-experiment-settings --json
    schelling-tools show-experiment-settings --results-dir "$(runvault path --experiment schelling --latest --subcommand run)"
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from dataclasses import asdict
from pathlib import Path

from runvault.read import config_parameters, load_run_meta
from schelling_tools.reproduce_paper import (
    PROJECT_ROOT,
    Experiment,
    paper_experiments,
    tau_sweep_taus,
)


# ---------------------------------------------------------------------------
# 1. Display paper experiment definitions
# ---------------------------------------------------------------------------


def _format_range(rng: tuple[float, float] | None, unit: str = "") -> str:
    if rng is None:
        return "-"
    if unit == "%":
        return f"{rng[0]:.1f}–{rng[1]:.1f}%"
    return f"{rng[0]:.3f}–{rng[1]:.3f}"


def _agents_label(exp: Experiment) -> str:
    if exp.n_a > 0 and exp.n_b > 0:
        return f"A:{exp.n_a} / B:{exp.n_b}"
    total = exp.rows * exp.cols
    n_vacant = round(total * exp.vacant_rate)
    n_agents = total - n_vacant
    a = n_agents // 2
    b = n_agents - a
    return f"A:{a} / B:{b} (auto)"


def render_paper_experiments(exps: list[Experiment]) -> str:
    lines: list[str] = []
    lines.append("=" * 90)
    lines.append("Schelling (1971) paper reproduction experiments — settings")
    lines.append("=" * 90)
    for i, exp in enumerate(exps):
        if i > 0:
            lines.append("-" * 90)
        lines.append(f"[{exp.key}]  {exp.figure}")
        lines.append(f"    description    : {exp.description}")
        lines.append(f"    rule           : {exp.rule_label()}")
        lines.append(f"    grid           : {exp.rows}×{exp.cols} (vacant rate {exp.vacant_rate:.2f})")
        lines.append(f"    agents         : {_agents_label(exp)}")
        lines.append(f"    reported values:")
        lines.append(f"      avg_same_ratio   : {_format_range(exp.paper_avg_same_ratio)}")
        lines.append(f"      pct_no_opposite  : {_format_range(exp.paper_pct_no_opposite, unit='%')}")
        if exp.paper_minority_avg_same is not None:
            lines.append(f"      minority_avg_same: {_format_range(exp.paper_minority_avg_same)}")
    lines.append("-" * 90)
    taus = tau_sweep_taus()
    lines.append(
        f"[fig14_tau_sweep]  Fig. 14 — τ sensitivity analysis ({taus[0]:.2f}–{taus[-1]:.2f}, increments of 0.05, {len(taus)} points)"
    )
    lines.append("=" * 90)
    return "\n".join(lines)


def experiments_as_dicts(exps: list[Experiment]) -> list[dict]:
    """Convert Experiment instances to dictionaries for JSON output."""
    out: list[dict] = []
    for exp in exps:
        d = asdict(exp)
        d["rule_label"] = exp.rule_label()
        d["agents"] = _agents_label(exp)
        out.append(d)
    return out


# ---------------------------------------------------------------------------
# 2. Display settings from a results directory
# ---------------------------------------------------------------------------


def _resolve_results_dir(arg: str) -> Path:
    """Resolve a user-specified results_dir to an absolute path.

    - Leave an absolute path unchanged.
    - Resolve a relative path from PROJECT_ROOT; if it does not exist, also try CWD.
    - Resolve symbolic links (results/latest) to their targets with os.path.realpath.
    """
    p = Path(arg)
    if not p.is_absolute():
        candidates = [PROJECT_ROOT / arg, Path.cwd() / arg]
        for c in candidates:
            if c.exists():
                p = c
                break
        else:
            p = candidates[0]
    return Path(os.path.realpath(p))


def _load_config(results_dir: Path) -> tuple[dict, Path, str]:
    """Return the experiment conditions and whether the directory is a run or sweep.

    In a runvault run, config.json is an envelope with conditions under `parameters`.
    The `subcommand` in run.json identifies a run or sweep (`sweep_config.json` is
    no longer written). Legacy flat config.json and sweep_config.json files are
    also supported.
    """
    # Missing settings may mean that sweep_config.json is still in use, so do not
    # treat their absence as an error here (sweep_config.json is checked below).
    params = config_parameters(results_dir, required=False)
    if params is not None:
        meta = load_run_meta(results_dir, required=False)
        if meta is not None:
            kind = "sweep" if meta.get("subcommand") == "sweep" else "run"
        else:
            # Legacy: the formerly hand-written config.json has a "command" field.
            kind = "sweep" if params.get("command") == "sweep" else "run"
        return params, results_dir / "config.json", kind

    sweep_cfg = results_dir / "sweep_config.json"
    if sweep_cfg.exists():
        with sweep_cfg.open() as f:
            return json.load(f), sweep_cfg, "sweep"

    raise FileNotFoundError(
        f"Settings file not found: {results_dir}\n"
        f"  Expected file: config.json (runvault envelope / legacy flat format) "
        f"or sweep_config.json (legacy sweep)\n"
        f"  Note: results generated by older versions may not include config.json."
    )


def render_run_config(cfg: dict, source: Path) -> str:
    lines: list[str] = []
    lines.append("=" * 90)
    lines.append("Run settings")
    lines.append("=" * 90)
    lines.append(f"settings file: {source}")
    lines.append("-" * 90)
    lines.append(f"rule           : {cfg.get('rule', '-')}  (kind={cfg.get('rule_kind', '-')})")
    if cfg.get("threshold") is not None:
        lines.append(f"  threshold  : {cfg['threshold']}")
    if cfg.get("min_same") is not None:
        lines.append(f"  min_same   : {cfg['min_same']}")
    if cfg.get("max_same") is not None:
        lines.append(f"  max_same   : {cfg['max_same']}")
    rows = cfg.get("rows", "-")
    cols = cfg.get("cols", "-")
    n_vacant = cfg.get("n_vacant", "-")
    lines.append(f"grid           : {rows}×{cols} ({n_vacant} vacant cells / vacant rate {cfg.get('vacant_rate', '-')})")
    lines.append(f"agents         : A={cfg.get('n_a', '-')}  B={cfg.get('n_b', '-')}")
    lines.append(f"seed           : {cfg.get('seed', '-')}")
    lines.append(f"max iterations : {cfg.get('max_iterations', '-')}")
    lines.append(f"snapshot interval: {cfg.get('snapshot_interval', '-')}")
    # The output destination is the run directory itself, so it is not a condition
    # (only legacy configurations include it).
    if cfg.get("output_dir") is not None:
        lines.append(f"output         : {cfg['output_dir']}")
    lines.append("=" * 90)
    return "\n".join(lines)


def render_sweep_config(cfg: dict, source: Path) -> str:
    lines: list[str] = []
    lines.append("=" * 90)
    lines.append("Sweep settings")
    lines.append("=" * 90)
    lines.append(f"settings file: {source}")
    lines.append("-" * 90)

    def fmt_range(v) -> str:
        if isinstance(v, dict) and {"start", "stop", "step"} <= v.keys():
            return f"{v['start']}:{v['stop']}:{v['step']}  (range)"
        return f"{v}  (single)"

    lines.append(f"threshold    : {fmt_range(cfg.get('threshold'))}")
    lines.append(f"vacant_rate  : {fmt_range(cfg.get('vacant_rate'))}")
    lines.append(f"grid           : {cfg.get('rows', '-')}×{cfg.get('cols', '-')}")
    lines.append(f"seeds          : {cfg.get('seeds', '-')}")
    lines.append(f"max iterations : {cfg.get('max_iterations', '-')}")
    lines.append(f"snapshot interval: {cfg.get('snapshot_interval', '-')}")
    lines.append("=" * 90)
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Main entry point
# ---------------------------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="schelling-tools show-experiment-settings",
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--results-dir", "--results_dir",
        default=None,
        help=(
            "Specify a run directory to display the experiment conditions in its config.json."
            "If omitted, display the paper reproduction experiment definitions."
        ),
    )
    parser.add_argument(
        "--only",
        default=None,
        help="Display only the specified paper reproduction experiment keys (comma-separated); ignored with --results-dir.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Output JSON instead of a table.",
    )
    args = parser.parse_args(argv)

    if args.results_dir is not None:
        results_dir = _resolve_results_dir(args.results_dir)
        if not results_dir.exists():
            print(f"Error: directory does not exist: {results_dir}", file=sys.stderr)
            return 1
        cfg, cfg_path, kind = _load_config(results_dir)
        if args.json:
            payload = {"source": str(cfg_path), "kind": kind, "config": cfg}
            print(json.dumps(payload, indent=2, ensure_ascii=False))
        else:
            if kind == "run":
                print(render_run_config(cfg, cfg_path))
            else:
                print(render_sweep_config(cfg, cfg_path))
        return 0

    exps = paper_experiments()
    if args.only:
        wanted = {s.strip() for s in args.only.split(",")}
        exps = [e for e in exps if e.key in wanted]
        if not exps:
            print(f"Error: no keys specified by --only were found: {args.only}", file=sys.stderr)
            return 1

    if args.json:
        payload = {
            "experiments": experiments_as_dicts(exps),
            "tau_sweep": {
                "key": "fig14_tau_sweep",
                "figure": "Fig. 14",
                "taus": tau_sweep_taus(),
            },
        }
        print(json.dumps(payload, indent=2, ensure_ascii=False))
    else:
        print(render_paper_experiments(exps))
    return 0


if __name__ == "__main__":
    sys.exit(main())
