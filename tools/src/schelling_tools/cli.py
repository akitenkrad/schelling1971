"""schelling-tools — Schelling (1971) 分離モデル ツール統合 CLI．

Usage:
    schelling-tools visualize [...]
    schelling-tools visualize-sweep [...]
    schelling-tools visualize-bnm [...]
    schelling-tools reproduce [...]
    schelling-tools show-experiment-settings [...]

各サブコマンドに続く引数は，対応するモジュールの argparse がそのまま受け取る．
サブコマンドレベルで `--help` を付けると，そのサブコマンド自身のヘルプが表示される．
"""
from __future__ import annotations

import argparse
import sys


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(
        prog="schelling-tools",
        description="Schelling (1971) 分離モデル 可視化・分析ツール",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("visualize", help="単一実行結果の可視化", add_help=False)
    subparsers.add_parser("visualize-sweep", help="スイープ結果の可視化", add_help=False)
    subparsers.add_parser("visualize-bnm", help="境界近隣モデル (BNM) の可視化", add_help=False)
    subparsers.add_parser("visualize-tipping", help="ティッピングモデルの可視化", add_help=False)
    subparsers.add_parser("reproduce", help="論文 Fig.7-17 の一括再現", add_help=False)
    subparsers.add_parser(
        "show-experiment-settings",
        help="実験設定値の表示 (論文再現定義 / 実行結果ディレクトリの設定)",
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
        # 未知のコマンドは argparse のエラーメッセージに委ねる
        parser.parse_args(argv)


if __name__ == "__main__":
    main()
