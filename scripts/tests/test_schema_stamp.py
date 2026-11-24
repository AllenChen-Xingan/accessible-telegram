"""内容哈希章（scripts/schema-stamp.py）的自测：原样绿，改内容不改哈希、缺版本、缺哈希都红"""
from __future__ import annotations

import json
import os
import shutil
import tempfile
import unittest

from _common import REPO, run

STAMP = os.path.join(REPO, "scripts", "schema-stamp.py")


class SchemaStamp(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.mkdtemp(prefix="atg-stamp-")
        self.f = os.path.join(self.d, "s.json")
        self.put({"v": 1, "elements": [{"key": "chat_item", "role": "ListItem"}], "texts": ["会话"]})
        self.assertEqual(run(STAMP, "stamp", self.f).returncode, 0)

    def tearDown(self):
        shutil.rmtree(self.d)

    def put(self, obj):
        with open(self.f, "w", encoding="utf-8") as f:
            json.dump(obj, f, ensure_ascii=False, indent=2)

    def get(self):
        with open(self.f, encoding="utf-8") as f:
            return json.load(f)

    def test_stamped_is_green(self):
        self.assertEqual(run(STAMP, "check", self.f).returncode, 0)

    def test_reformat_and_reorder_stay_green(self):
        obj = self.get()
        with open(self.f, "w", encoding="utf-8") as f:
            json.dump(dict(reversed(list(obj.items()))), f, ensure_ascii=True)
        self.assertEqual(run(STAMP, "check", self.f).returncode, 0)

    def test_content_change_without_restamp_is_red(self):
        obj = self.get()
        obj["texts"] = ["消息"]
        self.put(obj)
        r = run(STAMP, "check", self.f)
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("哈希没更新", r.stdout)

    def test_missing_version_is_red_even_when_hash_matches(self):
        # 删掉 v 之后重新盖章，哈希是对的：只剩「缺版本」这一条能让它红（codex 064 第 3 轮 #1：不重新盖章的话，
        # 哈希不配会先把它弄红，删掉检查器里的缺版本判断也看不出来）
        obj = self.get()
        del obj["v"]
        self.put(obj)
        self.assertEqual(run(STAMP, "stamp", self.f).returncode, 0)
        r = run(STAMP, "check", self.f)
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("缺 v 字段", r.stdout)
        self.assertNotIn("哈希没更新", r.stdout)

    def test_missing_hash_is_red(self):
        obj = self.get()
        del obj["x-content-hash"]
        self.put(obj)
        r = run(STAMP, "check", self.f)
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn("缺 x-content-hash", r.stdout)

    def test_unreadable_is_red(self):
        with open(self.f, "w") as f:
            f.write("[1, 2")
        self.assertEqual(run(STAMP, "check", self.f).returncode, 1)



if __name__ == "__main__":
    unittest.main()
