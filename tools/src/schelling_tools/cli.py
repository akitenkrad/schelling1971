"""schelling-tools — Unified CLI for Schelling (1971) segregation model tools.

Usage:
    schelling-tools visualize [...]
    schelling-tools visualize-sweep [...]
    schelling-tools visualize-bnm [...]
    schelling-tools reproduce [...]
    schelling-tools show-experiment-settings [...]

Arguments following each subcommand are passed directly to the corresponding
module's argparse parser. Adding `--help` after a subcommand displays help for
that subcommand.
"""
from __future__ import annotations

import argparse
import sys


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(
        prog="schelling-tools",
        description="Visualization and analysis tools for the Schelling (1971) segregation model",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("visualize", help="Visualize results from a single run", add_help=False)
    subparsers.add_parser("visualize-sweep", help="Visualize sweep results", add_help=False)
    subparsers.add_parser("visualize-bnm", help="Visualize the bounded-neighborhood model (BNM)", add_help=False)
    subparsers.add_parser("visualize-tipping", help="Visualize the tipping model", add_help=False)
    subparsers.add_parser("reproduce", help="Reproduce paper Fig.7-17 in a batch", add_help=False)
    subparsers.add_parser(
        "show-experiment-settings",
        help="Display experiment settings (paper reproduction definitions / results directory settings)",
        add_help=False,
    )

    argv = sys.argv[1:] if argv is None else argv
    if not argv or argv[0] in {"-h", "--help"}:
        parser.parse_args(argv)
        return

    command = argv[0]
    rest = argv[1:]
    if command == "visualize":
        from schelling_tools.visualize import main as run_main
        run_main(rest)
    elif command == "visualize-sweep":
        from schelling_tools.visualize_sweep import main as run_main
        run_main(rest)
    elif command == "visualize-bnm":
        from schelling_tools.visualize_bnm import main as run_main
        run_main(rest)
    elif command == "visualize-tipping":
        from schelling_tools.visualize_tipping import main as run_main
        run_main(rest)
    elif command == "reproduce":
        from schelling_tools.reproduce_paper import main as run_main
        run_main(rest)
    elif command == "show-experiment-settings":
        from schelling_tools.show_experiment_settings import main as run_main
        run_main(rest)
    else:
        # Let argparse report the error for unknown commands.
        parser.parse_args(argv)


if __name__ == "__main__":
    main()
