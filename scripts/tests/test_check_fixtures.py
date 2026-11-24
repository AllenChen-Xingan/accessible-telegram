"""夹具隐私检查（scripts/check-fixtures.py）的自测：干净的副本绿，每条规则注入一处违规就红"""
from __future__ import annotations

import json
import os
import shutil
import tempfile
import unittest

from _common import REPO, run

CHECK = os.path.join(REPO, "scripts", "check-fixtures.py")

NAMES = "# 注释行\n林小雨\n周大山\n周末爬山群\n领队 老王\n"


def user(uid=101, first="林小雨", last="", phone=""):
    return {"@type": "user", "id": uid, "first_name": first, "last_name": last, "phone_number": phone}


def chat(cid=101, title="林小雨"):
    return {"@type": "chat", "id": cid, "title": title}


def msg(mid=1001, cid=101, text="明天见", uid=101):
    return {
        "@type": "message", "id": mid, "chat_id": cid,
        "sender_id": {"@type": "messageSenderUser", "user_id": uid},
        "content": {"@type": "messageText", "text": {"@type": "formattedText", "text": text, "entities": []}},
    }


class CheckFixtures(unittest.TestCase):
    def setUp(self):
        self.root = tempfile.mkdtemp(prefix="atg-fixtures-")
        self.fx = os.path.join(self.root, "src", "atg-core", "fixtures")
        os.makedirs(os.path.join(self.fx, "scenarios", "s", "history"))
        with open(os.path.join(self.fx, "names.txt"), "w", encoding="utf-8") as f:
            f.write(NAMES)
        self.put_json("speech/a.json", [
            {"@type": "updateUser", "user": user()},
            {"@type": "updateUser", "user": user(102, "领队", "老王")},
            {"@type": "updateNewChat", "chat": chat(-1001001, "周末爬山群")},
            {"@type": "updateNewMessage", "message": msg(text="看 https://example.invalid/route，或写信 a@b.invalid")},
        ])
        self.put_lines("scenarios/s/chats.jsonl", [chat(), chat(-1001, "周末爬山群")])
        self.put_lines("scenarios/s/history/101.jsonl", [msg(1002), msg(1001)])

    def tearDown(self):
        shutil.rmtree(self.root)

    def put_json(self, rel, obj):
        p = os.path.join(self.fx, rel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "w", encoding="utf-8") as f:
            json.dump(obj, f, ensure_ascii=False)

    def put_lines(self, rel, objs):
        p = os.path.join(self.fx, rel)
        with open(p, "w", encoding="utf-8") as f:
            for o in objs:
                f.write(json.dumps(o, ensure_ascii=False) + "\n")

    def check(self):
        return run(CHECK, "--root", self.root)

    def assert_red(self, rule):
        r = self.check()
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn(f"[{rule}]", r.stdout)

    def test_clean_is_green(self):
        r = self.check()
        self.assertEqual(r.returncode, 0, r.stdout)

    def test_unknown_name_is_red(self):
        self.put_lines("scenarios/s/users.jsonl", [user(103, "王小明")])
        self.assert_red("names")

    def test_unknown_chat_title_is_red(self):
        self.put_lines("scenarios/s/chats.jsonl", [chat(103, "真实的群")])
        self.assert_red("names")

    def test_unknown_username_is_red(self):
        u = user()
        u["usernames"] = {"@type": "usernames", "active_usernames": ["realperson"], "editable_username": ""}
        self.put_lines("scenarios/s/users.jsonl", [u])
        self.assert_red("names")

    def test_real_phone_number_field_is_red(self):
        self.put_lines("scenarios/s/users.jsonl", [user(phone="13800001111")])
        self.assert_red("phone")

    def test_test_phone_number_is_green(self):
        self.put_lines("scenarios/s/users.jsonl", [user(phone="9996612345")])
        self.assertEqual(self.check().returncode, 0)

    def test_phone_in_text_is_red(self):
        self.put_lines("scenarios/s/history/101.jsonl", [msg(text="打我电话 13912345678")])
        self.assert_red("phone")

    def test_real_email_is_red(self):
        self.put_lines("scenarios/s/history/101.jsonl", [msg(text="写信到 someone@gmail.com")])
        self.assert_red("email")

    def test_real_link_is_red(self):
        self.put_lines("scenarios/s/history/101.jsonl", [msg(text="看 https://t.me/somechannel")])
        self.assert_red("url")

    def test_id_out_of_range_is_red(self):
        self.put_lines("scenarios/s/history/101.jsonl", [msg(mid=123456789)])
        self.assert_red("ids")

    def test_real_looking_supergroup_id_is_red(self):
        self.put_lines("scenarios/s/chats.jsonl", [chat(-1001234567890, "周末爬山群")])
        self.assert_red("ids")

    def test_minithumbnail_and_remote_ids_are_red(self):
        m = msg()
        m["content"] = {"@type": "messagePhoto", "photo": {"minithumbnail": {"data": "AAAA"}}}
        self.put_lines("scenarios/s/history/101.jsonl", [m])
        self.assert_red("files")
        m["content"] = {"@type": "messageDocument", "document": {"document": {"remote": {"id": "BQACAgIAAx", "unique_id": ""}}}}
        self.put_lines("scenarios/s/history/101.jsonl", [m])
        self.assert_red("files")

    def test_placeholder_remote_id_is_green(self):
        m = msg()
        m["content"] = {"@type": "messageDocument", "document": {"document": {"remote": {"id": "fixture-doc-1", "unique_id": ""}}}}
        self.put_lines("scenarios/s/history/101.jsonl", [m])
        self.assertEqual(self.check().returncode, 0, self.check().stdout)

    def test_negative_user_or_message_id_is_red(self):
        # 只有会话 id 许负；user、message 自己的 id 不许（codex 068 第 1 轮 #1）
        self.put_lines("scenarios/s/users.jsonl", [user(uid=-5)])
        self.assert_red("ids")
        self.put_lines("scenarios/s/users.jsonl", [])
        self.put_lines("scenarios/s/history/101.jsonl", [msg(mid=-1001)])
        self.assert_red("ids")

    def test_placeholder_minithumbnail_is_green(self):
        m = msg()
        m["content"] = {"@type": "messagePhoto", "photo": {"minithumbnail": {"data": "fixture-thumb-1"}}}
        self.put_lines("scenarios/s/history/101.jsonl", [m])
        r = self.check()
        self.assertEqual(r.returncode, 0, r.stdout)

    @unittest.skipIf(hasattr(os, "geteuid") and os.geteuid() == 0, "root 不受目录权限限制，造不出读不了")
    def test_unreadable_subdir_is_red_even_if_others_scan(self):
        # 一个子目录读不了、别的照常：不许报 0 处命中（codex 068 第 1 轮 #3）
        locked = os.path.join(self.fx, "scenarios", "locked")
        os.makedirs(locked)
        self.put_lines("scenarios/locked/chats.jsonl", [chat()])
        os.chmod(locked, 0)
        try:
            self.assert_red("read")
        finally:
            os.chmod(locked, 0o700)

    def test_reserved_link_with_query_or_fragment_is_green(self):
        # 保留域名带查询、片段、端口都是正常写法，不许误报（codex 068 第 2 轮 #2）
        self.put_lines("scenarios/s/history/101.jsonl", [
            msg(1003, text="看 https://example.invalid?q=1 和 https://example.invalid#section"),
            msg(1004, text="还有 https://example.invalid:8443/p?x=1#y"),
        ])
        r = self.check()
        self.assertEqual(r.returncode, 0, r.stdout)

    def test_real_link_with_query_is_still_red(self):
        self.put_lines("scenarios/s/history/101.jsonl", [msg(text="看 https://example.com.evil.net?q=example.invalid")])
        self.assert_red("url")

    def test_bad_json_line_is_red(self):
        with open(os.path.join(self.fx, "scenarios/s/users.jsonl"), "w", encoding="utf-8") as f:
            f.write('{"@type": "user", \n')
        self.assert_red("json")

    def test_no_fixtures_at_all_is_red(self):
        # 扫不到任何夹具（路径改了）不许安静变绿
        shutil.rmtree(os.path.join(self.fx, "speech"))
        shutil.rmtree(os.path.join(self.fx, "scenarios"))
        r = self.check()
        self.assertEqual(r.returncode, 1, r.stdout)


if __name__ == "__main__":
    unittest.main()
