"""行数上限 + 棘轮 + 门槛守卫（scripts/check-file-size.py）的自测"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
import unittest

from _common import REPO, REVIEW, SIZE, SOURCES, git, run  # noqa: F401


class FileSize(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp(prefix="atg-size-")
        git(self.d, "init", "-q")
        os.makedirs(os.path.join(self.d, "src/a/src"))
        os.makedirs(os.path.join(self.d, "src/tdlib-rs"))
        os.makedirs(os.path.join(self.d, "scripts"))
        self.write("src/a/src/big.rs", 700)
        self.write("src/a/src/small.rs", 10)
        self.write("src/tdlib-rs/upstream.rs", 5000)  # 上游 subtree 不管
        shutil.copy(SIZE, os.path.join(self.d, "scripts/check-file-size.py"))  # 合并目标上也有门槛脚本
        git(self.d, "add", "-A")
        git(self.d, "commit", "-qm", "init")
        r = run(SIZE, "--root", self.d, "--write-baseline", "--rev", "HEAD")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)

    def tearDown(self):
        shutil.rmtree(self.d)

    def write(self, rel: str, n: int) -> None:
        with open(os.path.join(self.d, rel), "w") as f:
            f.write("x\n" * n)

    def base(self) -> dict:
        with open(os.path.join(self.d, "scripts/file-size-baseline.json")) as f:
            return json.load(f)["files"]

    def check(self) -> subprocess.CompletedProcess:
        return run(SIZE, "--root", self.d)

    def test_baseline_from_rev_skips_upstream(self):
        self.assertEqual(self.base(), {"src/a/src/big.rs": 700})

    def test_clean_is_green(self):
        self.assertEqual(self.check().returncode, 0)

    def test_new_file_over_limit_is_red(self):
        self.write("src/a/src/new.rs", 601)
        git(self.d, "add", "-A")
        r = self.check()
        self.assertEqual(r.returncode, 1)
        self.assertIn("new.rs：601 行，新超标", r.stdout)

    def test_exactly_at_limit_is_green(self):
        self.write("src/a/src/new.rs", 600)
        git(self.d, "add", "-A")
        self.assertEqual(self.check().returncode, 0)

    def test_untracked_file_is_ignored(self):
        self.write("src/a/src/scratch.rs", 900)
        self.assertEqual(self.check().returncode, 0)

    def test_baseline_file_growing_is_red(self):
        self.write("src/a/src/big.rs", 701)
        r = self.check()
        self.assertEqual(r.returncode, 1)
        self.assertIn("比基线 700 行更大", r.stdout)

    def test_shrink_without_tighten_is_red(self):
        # 变小了不收紧，以后就能悄悄长回去：必须当场红
        self.write("src/a/src/big.rs", 650)
        r = self.check()
        self.assertEqual(r.returncode, 1)
        self.assertIn("700 → 650 行，基线没收紧", r.stdout)

    def test_shrink_then_tighten_ratchets_down(self):
        self.write("src/a/src/big.rs", 650)
        self.assertEqual(run(SIZE, "--root", self.d, "--tighten").returncode, 0)
        self.assertEqual(self.base(), {"src/a/src/big.rs": 650})
        self.assertEqual(self.check().returncode, 0)
        self.write("src/a/src/big.rs", 660)  # 收紧后再长回去就红
        self.assertEqual(self.check().returncode, 1)

    def test_fixed_file_left_in_baseline_is_red(self):
        self.write("src/a/src/big.rs", 300)
        r = self.check()
        self.assertEqual(r.returncode, 1)
        self.assertIn("已守线或已删除", r.stdout)

    def test_code_in_fixtures_dir_is_still_checked(self):
        os.makedirs(os.path.join(self.d, "src/a/fixtures"))
        self.write("src/a/fixtures/gen.rs", 700)
        git(self.d, "add", "-A")
        self.assertEqual(self.check().returncode, 1)

    def test_max_in_baseline_is_not_a_second_source(self):
        p = os.path.join(self.d, "scripts/file-size-baseline.json")
        with open(p) as f:
            data = json.load(f)
        data["max"] = 900
        with open(p, "w") as f:
            json.dump(data, f)
        r = self.check()
        self.assertEqual(r.returncode, 1)
        self.assertIn("上限只在脚本里改", r.stdout + r.stderr)

    # ── --against：基线本身不许被顺手放宽 ──

    def commit_baseline(self):
        git(self.d, "add", "-A")
        git(self.d, "commit", "-qm", "baseline")

    def against(self, *extra: str) -> subprocess.CompletedProcess:
        return run(SIZE, "--root", self.d, "--against", "HEAD", *extra)

    def test_against_first_entry_passes(self):
        # 门槛第一次进库：合并目标上连门槛脚本都还没有
        git(self.d, "rm", "-q", "--cached", "scripts/check-file-size.py")
        git(self.d, "commit", "-qm", "base without gate")
        git(self.d, "add", "-A")  # 本次 PR 带进脚本和基线
        r = self.against()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("第一次进库", r.stdout)

    def test_renaming_the_baseline_is_red(self):
        # 上一轮实测的绕法：改 BASELINE 路径 + 改名基线 → 以前被当成「第一次进库」整个放行
        self.commit_baseline()
        self.copy_script(('BASELINE = "scripts/file-size-baseline.json"', 'BASELINE = "scripts/fsb.json"'))
        os.rename(os.path.join(self.d, "scripts/file-size-baseline.json"), os.path.join(self.d, "scripts/fsb.json"))
        git(self.d, "add", "-A")
        r = run(SIZE, "--root", self.d, "--against", "HEAD", "--baseline", "scripts/fsb.json")
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("门槛文件 scripts/check-file-size.py 改了", r.stdout)
        self.assertIn("没有基线 scripts/fsb.json", r.stdout)

    def test_base_script_catches_a_pr_that_disables_its_own_check(self):
        # 上一轮实测的绕法：PR 把「脚本改了就红」那句改成恒假，再放宽范围。CI 跑的是合并目标那份脚本，所以照样红
        self.commit_baseline()
        self.copy_script(('for f in gate_touched(root, against, head)]', 'for f in []]'))
        self.write("src/a/src/huge.rs", 3000)
        git(self.d, "add", "-A")
        gate = os.path.join(self.d, "base-gate.py")
        with open(gate, "wb") as f:
            f.write(subprocess.run(["git", "-C", self.d, "show", "HEAD:scripts/check-file-size.py"],
                                   capture_output=True, check=True).stdout)
        r = run(gate, "--root", self.d, "--against", "HEAD")
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("门槛文件 scripts/check-file-size.py 改了", r.stdout)

    def test_other_gate_files_need_the_label_too(self):
        # 改 CI 工作流、比对脚本、自测同样能让门槛变松
        self.commit_baseline()
        for rel in (".github/workflows/arch.yml", "scripts/check-sources.py", "scripts/tests/test_x.py"):
            os.makedirs(os.path.dirname(os.path.join(self.d, rel)), exist_ok=True)
            with open(os.path.join(self.d, rel), "w") as f:
                f.write("# 改了\n")
        git(self.d, "add", "-A")
        r = self.against()
        self.assertEqual(r.returncode, 1)
        for rel in (".github/workflows/arch.yml", "scripts/check-sources.py", "scripts/tests/test_x.py"):
            self.assertIn(f"门槛文件 {rel} 改了", r.stdout)

    def test_head_mode_reads_the_pr_from_git_only(self):
        # pull_request_target：工作区是合并目标，PR 只当 git 数据读
        self.commit_baseline()
        git(self.d, "tag", "base")
        self.write("src/a/src/big.rs", 750)
        run(SIZE, "--root", self.d, "--write-baseline")  # PR 顺手放宽基线
        git(self.d, "add", "-A")
        git(self.d, "commit", "-qm", "pr")
        git(self.d, "tag", "pr")
        git(self.d, "checkout", "-q", "base")  # 工作区回到合并目标
        r = run(SIZE, "--root", self.d, "--against", "base", "--head", "pr")
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("基线放宽 src/a/src/big.rs：700 → 750", r.stdout)
        ok = run(SIZE, "--root", self.d, "--against", "base", "--head", "base")
        self.assertEqual(ok.returncode, 0, ok.stdout)

    def test_label_lets_a_baseline_rename_through(self):
        # codex 019 #2：以前带了标签也过不去——旧脚本按旧路径找不到基线就提前退出
        self.commit_baseline()
        os.rename(os.path.join(self.d, "scripts/file-size-baseline.json"), os.path.join(self.d, "scripts/fsb.json"))
        git(self.d, "add", "-A")
        red = self.against()
        self.assertEqual(red.returncode, 1, red.stdout)
        self.assertIn("被改名或删了", red.stdout)
        ok = self.against("--allow-loosen")
        self.assertEqual(ok.returncode, 0, ok.stdout + ok.stderr)

    def test_label_lets_a_new_limit_through(self):
        self.commit_baseline()
        p = os.path.join(self.d, "scripts/file-size-baseline.json")
        with open(p) as f:
            data = json.load(f)
        data["max"] = 800
        with open(p, "w") as f:
            json.dump(data, f)
        self.copy_script(("MAX = 600", "MAX = 800"))
        git(self.d, "add", "-A")
        red = self.against()
        self.assertEqual(red.returncode, 1)
        self.assertIn("max 改成了 800", red.stdout)
        self.assertEqual(self.against("--allow-loosen").returncode, 0)

    def test_unreadable_new_baseline_counts_as_loosening(self):
        self.commit_baseline()
        with open(os.path.join(self.d, "scripts/file-size-baseline.json"), "w") as f:
            f.write("{坏")
        r = self.against()
        self.assertEqual(r.returncode, 1)
        self.assertIn("读不懂", r.stdout)

    def test_logic_change_in_script_is_red(self):
        self.commit_baseline()
        self.copy_script(("def in_scope(rel: str) -> bool:\n", "def in_scope(rel: str) -> bool:\n    return False\n"))
        r = self.against()
        self.assertEqual(r.returncode, 1)
        self.assertIn("门槛文件 scripts/check-file-size.py 改了", r.stdout)

    def test_rewriting_baseline_to_hide_growth_is_red(self):
        self.commit_baseline()
        self.write("src/a/src/big.rs", 750)
        self.write("src/a/src/new.rs", 700)
        git(self.d, "add", "-A")
        self.assertEqual(run(SIZE, "--root", self.d, "--write-baseline").returncode, 0)
        self.assertEqual(self.check().returncode, 0)  # 不比合并目标时确实被骗过去了
        r = self.against()
        self.assertEqual(r.returncode, 1)
        self.assertIn("基线放宽 src/a/src/big.rs：700 → 750", r.stdout)
        self.assertIn("基线新增 src/a/src/new.rs", r.stdout)

    def test_loosen_label_lets_it_through(self):
        self.commit_baseline()
        self.write("src/a/src/big.rs", 750)
        run(SIZE, "--root", self.d, "--write-baseline")
        r = self.against("--allow-loosen")
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertIn("baseline:loosen", r.stdout)

    def copy_script(self, edit=None):
        dst = os.path.join(self.d, "scripts/check-file-size.py")
        with open(SIZE, encoding="utf-8") as f:
            text = f.read()
        if edit:
            self.assertIn(edit[0], text)
            text = text.replace(edit[0], edit[1], 1)
        with open(dst, "w", encoding="utf-8") as f:
            f.write(text)

    def test_raising_the_limit_in_the_script_is_red(self):
        self.copy_script()
        self.commit_baseline()
        self.copy_script(("MAX = 600", "MAX = 2000"))
        r = self.against()
        self.assertEqual(r.returncode, 1)
        self.assertIn("门槛文件 scripts/check-file-size.py 改了", r.stdout)

    def test_widening_exclusions_in_the_script_is_red(self):
        self.copy_script()
        self.commit_baseline()
        self.copy_script(('EXCLUDE_DIRS = {"target", ', 'EXCLUDE_DIRS = {"src", "target", '))
        r = self.against()
        self.assertEqual(r.returncode, 1)
        self.assertIn("门槛文件 scripts/check-file-size.py 改了", r.stdout)

    def test_unchanged_script_is_fine(self):
        self.copy_script()
        self.commit_baseline()
        self.assertEqual(self.against().returncode, 0)

    def test_tightening_against_base_is_fine(self):
        self.commit_baseline()
        self.write("src/a/src/big.rs", 650)
        run(SIZE, "--root", self.d, "--tighten")
        self.assertEqual(self.against().returncode, 0)

    def test_tighten_never_raises(self):
        self.write("src/a/src/big.rs", 800)
        run(SIZE, "--root", self.d, "--tighten")
        self.assertEqual(self.base(), {"src/a/src/big.rs": 700})

    def test_tighten_drops_files_back_under_limit(self):
        self.write("src/a/src/big.rs", 300)
        run(SIZE, "--root", self.d, "--tighten")
        self.assertEqual(self.base(), {})


if __name__ == "__main__":
    unittest.main()
