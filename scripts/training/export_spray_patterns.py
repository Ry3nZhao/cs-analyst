"""导出压枪练习用的弹道序列（W2-F）。

数据来源：W1-D 枪法分析写入的 `spray_shots` 表（迁移 005）。对每把武器、每个弹序（第 N 发），
取所有连发里 `recoil_v` / `recoil_h` 的中位数。

- recoil = -2 × Δpunch（以第一发为原点，单位为度，屏幕方向：v > 0 向上，h > 0 向右），
  即「只有后坐力时子弹会落在哪」，和 docs/aim.md 的定义一致。
- 只保留至少 MIN_SAMPLES 次连发打到的弹序，样本更少的尾部截掉。样例库里的连发多数不到 15 发，
  所以序列偏短；解析的 demo 越多，重新导出后序列越长。

用法（只读打开数据库，不会改动应用数据）：
    python -I scripts/training/export_spray_patterns.py [analyst.db 路径]
默认读取 %LOCALAPPDATA%/com.cs-analyst.app/db/analyst.db，
输出 src/desktop/app/training/spray-patterns.json。
"""

import datetime
import json
import os
import sqlite3
import statistics
import sys

MIN_SAMPLES = 4

# 和 crates/aim/src/weapons.rs 保持一致（cycle_s），弹匣容量取 CS2 数值。
WEAPONS = [
    ("ak47", "AK-47", 0.100, 30),
    ("m4a1", "M4A4", 0.090, 30),
    ("m4a1_silencer", "M4A1-S", 0.100, 20),
    ("galilar", "Galil AR", 0.090, 35),
    ("famas", "FAMAS", 0.090, 25),
    ("mp9", "MP9", 0.070, 30),
    ("mac10", "MAC-10", 0.075, 30),
]


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    db_path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        os.environ["LOCALAPPDATA"], "com.cs-analyst.app", "db", "analyst.db"
    )
    out_path = os.path.join(repo, "src", "desktop", "app", "training", "spray-patterns.json")
    conn = sqlite3.connect("file:" + db_path.replace("\\", "/") + "?mode=ro", uri=True)
    n_matches = conn.execute("SELECT COUNT(DISTINCT match_id) FROM sprays").fetchone()[0]

    weapons = {}
    for key, label, cycle_s, magazine in WEAPONS:
        rows = conn.execute(
            "SELECT s.idx, s.recoil_v, s.recoil_h FROM spray_shots s JOIN sprays p ON p.id = s.spray_id "
            "WHERE p.weapon = ? ORDER BY s.idx",
            (key,),
        ).fetchall()
        by_idx = {}
        for idx, v, h in rows:
            by_idx.setdefault(idx, []).append((v, h))
        shots, samples = [], []
        for idx in sorted(by_idx):
            xs = by_idx[idx]
            if len(xs) < MIN_SAMPLES or idx != len(shots) + 1:
                break
            shots.append([
                round(statistics.median(a for a, _ in xs), 3),
                round(statistics.median(b for _, b in xs), 3),
            ])
            samples.append(len(xs))
        sprays = conn.execute("SELECT COUNT(*) FROM sprays WHERE weapon = ?", (key,)).fetchone()[0]
        weapons[key] = {
            "label": label,
            "cycleMs": round(cycle_s * 1000),
            "magazine": magazine,
            "sprays": sprays,
            "samples": samples,
            "shots": shots,
        }

    doc = {
        "source": "W1-D spray_shots（样例 demo 里实际连发的 recoil = -2 × Δpunch，按弹序取中位数）",
        "script": "scripts/training/export_spray_patterns.py",
        "generatedAt": datetime.date.today().isoformat(),
        "matches": n_matches,
        "minSamples": MIN_SAMPLES,
        "units": "度；[v, h]，v > 0 向上，h > 0 向右；以第一发为原点",
        "weapons": weapons,
    }
    with open(out_path, "w", encoding="utf-8", newline="\n") as f:
        json.dump(doc, f, ensure_ascii=False, indent=1)
        f.write("\n")
    for k, w in weapons.items():
        print(f"{k:14s} {len(w['shots']):2d} shots from {w['sprays']} sprays")
    print("->", out_path)


if __name__ == "__main__":
    main()
