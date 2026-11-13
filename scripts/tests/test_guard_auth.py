"""arch-guard 授权判断（scripts/arch-guard-auth.sh）的自测：用假 gh 模拟 GitHub 的应答。

codex 019 门槛第 2 轮 #1：标签摘掉以后重跑带标签的旧运行，不许再放行。
"""
from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import unittest

from _common import REPO

AUTH = os.path.join(REPO, "scripts", "arch-guard-auth.sh")

# 假 gh：按 STUB_* 环境变量回 PR 的头提交与标签；STUB_FAIL=head|labels 让对应查询失败
FAKE_GH = """#!/usr/bin/env bash
case "$2" in
  */pulls/*)  [ "${STUB_FAIL:-}" = head ] && exit 1; printf '%s\\n' "$STUB_HEAD" ;;
  */labels)   [ "${STUB_FAIL:-}" = labels ] && exit 1; printf '%s' "$STUB_LABELS" ;;
  *) exit 2 ;;
esac
"""


class GuardAuth(unittest.TestCase):
    def setUp(self):
        self.bin = tempfile.mkdtemp(prefix="atg-gh-")
        with open(os.path.join(self.bin, "gh"), "w") as f:
            f.write(FAKE_GH)
        os.chmod(os.path.join(self.bin, "gh"), 0o755)

    def tearDown(self):
        shutil.rmtree(self.bin)

    def auth(self, head_now="abc", labels="by:ai\nbaseline:loosen\n", fail="", event_head="abc"):
        env = {"PATH": self.bin + os.pathsep + os.environ["PATH"], "REPO": "o/r", "PR": "7",
               "EVENT_HEAD": event_head, "STUB_HEAD": head_now, "STUB_LABELS": labels, "STUB_FAIL": fail}
        r = subprocess.run(["bash", AUTH], capture_output=True, text=True, env=env, timeout=30)
        self.assertEqual(r.returncode, 0, r.stderr)
        return r.stdout.strip(), r.stderr

    def test_current_label_on_current_commit_allows(self):
        self.assertEqual(self.auth()[0], "--allow-loosen")

    def test_label_removed_does_not_allow(self):
        self.assertEqual(self.auth(labels="by:ai\n")[0], "")

    def test_rerun_of_an_old_run_does_not_allow(self):
        # 事件里的提交是旧的，PR 现在的头提交已经变了
        out, err = self.auth(head_now="new", event_head="old")
        self.assertEqual(out, "")
        self.assertIn("旧运行", err)

    def test_label_query_failure_does_not_allow(self):
        self.assertEqual(self.auth(fail="labels")[0], "")

    def test_head_query_failure_does_not_allow(self):
        self.assertEqual(self.auth(fail="head")[0], "")

    def test_similar_label_name_does_not_count(self):
        self.assertEqual(self.auth(labels="baseline:loosen-later\nnot-baseline:loosen\n")[0], "")


if __name__ == "__main__":
    unittest.main()
