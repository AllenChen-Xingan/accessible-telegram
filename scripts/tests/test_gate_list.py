"""门槛文件清单（scripts/check-file-size.py 的 GATE_FILES）要盖住 arch.yml 调用的每个脚本。

arch.yml 里每一步 `python3 scripts/X.py` / `bash scripts/X.sh` 都是一道 CI 门槛；它若不在 GATE_FILES 里，
以后只改这个脚本（比如让检查直接通过）就不用 baseline:loosen 标签，arch-guard 拦不住（atg-review 064 前第 2 轮 P1）。
"""
from __future__ import annotations

import ast
import os
import re
import unittest

from _common import REPO, SIZE

ARCH = os.path.join(REPO, ".github", "workflows", "arch.yml")


def gate_files(size_src: str) -> tuple[str, ...]:
    m = re.search(r"^GATE_FILES = \((.*?)\)", size_src, re.M | re.S)
    if not m:
        raise AssertionError("check-file-size.py 里找不到 GATE_FILES：改了名要同步改这条自测")
    self_m = re.search(r'^SELF = "([^"]+)"', size_src, re.M)
    items = []
    for part in m.group(1).split(","):
        part = re.sub(r"#.*", "", part).strip()
        if not part:
            continue
        items.append(self_m.group(1) if part == "SELF" and self_m else ast.literal_eval(part))
    return tuple(items)


def called_scripts(arch_src: str) -> list[str]:
    return sorted(set(re.findall(r"\b(?:python3|bash|sh)\s+(?:-\S+\s+)*(scripts/[\w./-]+\.(?:py|sh))", arch_src)))


def covered(path: str, gates: tuple[str, ...]) -> bool:
    return any(path == g or path.startswith(g.rstrip("/") + "/") for g in gates)


class GateList(unittest.TestCase):
    def test_every_script_arch_yml_calls_is_a_gate_file(self):
        with open(SIZE, encoding="utf-8") as f:
            gates = gate_files(f.read())
        with open(ARCH, encoding="utf-8") as f:
            calls = called_scripts(f.read())
        self.assertTrue(calls, "arch.yml 里一个脚本调用都没解析到：写法变了要同步改这条自测")
        missing = [c for c in calls if not covered(c, gates)]
        self.assertEqual(missing, [], f"arch.yml 调用了它们，但不在 GATE_FILES 里：{missing}")

    def test_a_missing_gate_is_caught(self):
        # 改坏会红：把一个被调用的脚本从清单里拿掉
        with open(SIZE, encoding="utf-8") as f:
            src = f.read().replace('"scripts/schema-stamp.py", ', "")
        with open(ARCH, encoding="utf-8") as f:
            calls = called_scripts(f.read())
        gates = gate_files(src)
        self.assertIn("scripts/schema-stamp.py", [c for c in calls if not covered(c, gates)])

    def test_directory_entry_covers_files_under_it(self):
        self.assertTrue(covered("scripts/tests/x.py", ("scripts/tests",)))
        self.assertFalse(covered("scripts/testsx.py", ("scripts/tests",)))


if __name__ == "__main__":
    unittest.main()
