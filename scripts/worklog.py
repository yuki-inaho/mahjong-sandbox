#!/usr/bin/env python3
"""検収済みの手順を一項目ずつ記録する補助。合否の判断自体は行いません。"""

import argparse
import os
from pathlib import Path
import re
import subprocess


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("step", help="例: 1、5-a。記録前に成果物を実際に検収すること")
    parser.add_argument("evidence", help="実際に確認した証跡と結果。公開可能な相対パスを使用")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    path = root / "docs/workdocs/workdoc_Nov23-2026_suzume.md"
    pattern = rf"(### 手順{re.escape(args.step)}：.*?)(?=\n### |\n## )"
    for kind in ("🖐", "🔎", "🧪", "🛠"):
        text = path.read_text()
        found = re.search(pattern, text, re.S)
        if not found:
            raise SystemExit(f"手順が見つかりません: {args.step}")
        section = found.group(1)
        unchecked = f"- [ ] {kind}"
        if unchecked not in section:
            continue
        timestamp = subprocess.check_output(
            ["date", "+%Y-%m-%d %H:%M:%S %Z%z"],
            env={**os.environ, "TZ": "Asia/Tokyo"}, text=True,
        ).strip()
        date, time, zone = timestamp.split()
        section = section.replace(unchecked, f"- [x] {kind}", 1)
        text = text[:found.start()] + section + text[found.end():]
        detail = args.evidence if kind != "🛠" else "対処手順の妥当性を確認。未発生の復旧を実施済みとは扱わない。"
        text += f"| {date} | {time} {zone} | 主担当 | 手順{args.step} {kind} 検収 | {detail.replace('|', '/')} |\n"
        path.write_text(text)
    print(f"手順{args.step}を証跡とともに記録しました")


if __name__ == "__main__":
    main()
