"""atg-review 的审查范围解析（#41）的自测"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
import unittest

from _common import REPO, REVIEW, SIZE, SOURCES, git, run  # noqa: F401


class ReviewTarget(unittest.TestCase):
    """atg-review 的范围解析（#41）：字符串当范围、对象取 .target、都没有就报错，绝不默认审 src/"""

    @classmethod
    def setUpClass(cls):
        node = shutil.which("node")
        if node is None:
            # 不许静默变绿：CI 上必须有 node；本机没有就明说跳过
            if os.environ.get("CI"):
                raise RuntimeError("CI 上找不到 node，ReviewTarget 自测跑不了")
            raise unittest.SkipTest("本机没有 node")
        cls.node = node
        with open(REVIEW, encoding="utf-8") as f:
            src = f.read()
        begin, end = "// BEGIN resolveTarget", "// END resolveTarget"
        assert begin in src and end in src, "atg-review.js 里的 resolveTarget 标记没了"
        cls.fn = src[src.index(begin):src.index(end)]
        rest = src.split("// END resolveTarget")[1]
        for q in ("'src/'", '"src/"', "`src/`"):
            assert q not in rest, f"又出现了默认审 src/ 的兜底：{q}"

    def resolve(self, arg_js: str) -> tuple[bool, str]:
        code = self.fn + f"\ntry {{ process.stdout.write('OK:' + resolveTarget({arg_js})) }}" \
                         f" catch (e) {{ process.stdout.write('ERR:' + e.message) }}"
        r = subprocess.run([self.node, "-e", code], capture_output=True, text=True, timeout=30)
        self.assertEqual(r.returncode, 0, r.stderr)
        return r.stdout.startswith("OK:"), r.stdout[3:] if r.stdout.startswith("OK:") else r.stdout[4:]

    def test_whole_workflow_file_parses(self):
        # 只测 resolveTarget 抓不到别处的语法错（2026-11-13 改收工判据时多打了个逗号，自测照绿）
        code = ("const fs=require('fs');"
                "const s=fs.readFileSync(process.argv[1],'utf8').replace(/^export const meta/m,'const meta');"
                "const AF=Object.getPrototypeOf(async function(){}).constructor;"
                "new AF('args','phase','parallel','agent','log',s);")
        r = subprocess.run([self.node, "-e", code, REVIEW], capture_output=True, text=True, timeout=30)
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_string_is_the_target(self):
        self.assertEqual(self.resolve('"docs/plan/010.md scripts/"'), (True, "docs/plan/010.md scripts/"))

    def test_object_target(self):
        self.assertEqual(self.resolve('{target: " specs/ "}'), (True, "specs/"))

    def test_missing_is_an_error_not_src(self):
        for bad in ("undefined", "null", '""', '"   "', "{}", '{target: ""}', "{target: 3}", "[]"):
            ok, msg = self.resolve(bad)
            self.assertFalse(ok, bad)
            self.assertIn("#41", msg)


if __name__ == "__main__":
    unittest.main()
