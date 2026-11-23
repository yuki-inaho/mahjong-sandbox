#!/usr/bin/env python3
"""必要な道具の実バージョンとWASM対象を検査します。導入や更新はしません。"""

import argparse
import json

from platform_support import environment_report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("native", "frontend", "formal", "full"), default="full")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    report = environment_report(args.profile)
    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2))
    else:
        for name, item in report["checks"].items():
            print(f"{'OK' if item['success'] else 'NG'} {name}: {item.get('observed', item.get('detail'))}")
    return 0 if report["success"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
