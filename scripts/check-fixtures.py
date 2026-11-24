#!/usr/bin/env python3
"""夹具隐私检查（规约正本：specs/contracts/testing.md 第一节第 3 小节「夹具隐私」）。

扫 src/atg-core/fixtures/ 下全部 .json / .jsonl（.jsonl 一行一个 JSON），命中任一条即退出码 1：

  names     人名、会话名、用户名只许取自 fixtures/names.txt（user 的 first_name / last_name、chat 的 title、
            usernames、转发来源的 sender_name、author_signature）；空串算没填，放行
  phone     phone_number 只许空串或 99966 开头的测试号；任何字符串里出现国内手机号、长串国际号也算
  email     邮箱只许保留域名（.invalid / .test / .example，RFC 2606），别的一律算真邮箱
  url       链接只许保留域名（同上，另含 example.com / .org / .net）；t.me 等真实链接算
  ids       user / chat / message 的 id，及 chat_id、user_id、message_id、supergroup_id、basic_group_id：
            只许 0–99999；会话 id 可为负（-1…-99999），超级群与频道是 -100 后面接上 1–99999 的数（字符串拼接：-1001 是超级群 1，不是 -100+1）
  files     minithumbnail.data、文件 remote.id / remote.unique_id 只许空串或 fixture- 开头的占位
  read      夹具目录里有读不了的子目录或文件（不悄悄跳过）

用法：python3 scripts/check-fixtures.py [--root <仓库根>]
只用标准库。规则只防无心之失（真号码、真名字、真链接被顺手带进夹具），不防专门构造的绕过。
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys

FIXTURES = os.path.join("src", "atg-core", "fixtures")
NAMES = "names.txt"

ID_KEYS = {"chat_id", "user_id", "message_id", "supergroup_id", "basic_group_id", "sender_chat_id"}
ID_OBJECTS = {"user", "chat", "message"}  # 这几种对象自己的 "id" 也查
NAME_KEYS = {"sender_name", "author_signature"}
RESERVED_TLDS = (".invalid", ".test", ".example")
RESERVED_HOSTS = ("example.com", "example.org", "example.net")
CN_MOBILE = re.compile(r"(?<!\d)1[3-9]\d{9}(?!\d)")
INTL_PHONE = re.compile(r"\+\d[\d -]{9,}\d")
EMAIL = re.compile(r"[\w.+-]+@([\w-]+(?:\.[\w-]+)+)")
URL = re.compile(r"(?:https?://|\bt\.me/|\btelegram\.me/)([^\s/\"'<>)]*)", re.I)


def load_names(root: str) -> set[str]:
    path = os.path.join(root, FIXTURES, NAMES)
    with open(path, encoding="utf-8") as f:
        return {ln.strip() for ln in f if ln.strip() and not ln.lstrip().startswith("#")}


def reserved_host(host: str) -> bool:
    h = host.lower().split(":")[0].rstrip(".")
    return h.endswith(RESERVED_TLDS) or any(h == r or h.endswith("." + r) for r in RESERVED_HOSTS)


def id_ok(key: str, v: int) -> bool:
    if key in ("supergroup_id", "basic_group_id", "user_id", "message_id"):
        return 0 <= v <= 99999
    # 会话 id（chat 对象的 id、chat_id、sender_chat_id）：私聊正、基础群负、超级群 -100 前缀
    if -99999 <= v <= 99999:
        return True
    s = str(v)
    return s.startswith("-100") and s[4:].isdigit() and 1 <= int(s[4:]) <= 99999


class Checker:
    def __init__(self, names: set[str]):
        self.names = names
        self.errors: list[str] = []

    def bad(self, where: str, rule: str, what: str) -> None:
        self.errors.append(f"{where}：[{rule}] {what}")

    def name(self, where: str, value, field: str) -> None:
        if isinstance(value, str) and value.strip() and value.strip() not in self.names:
            self.bad(where, "names", f"{field}「{value}」不在 names.txt 里")

    def text(self, where: str, s: str) -> None:
        if CN_MOBILE.search(s) or INTL_PHONE.search(s):
            self.bad(where, "phone", "字符串里像有手机号")
        for m in EMAIL.finditer(s):
            if not reserved_host(m.group(1)):
                self.bad(where, "email", f"邮箱域名 {m.group(1)} 不是保留域名")
        for m in URL.finditer(s):
            full = m.group(0).lower()
            host = m.group(1) if full.startswith("http") else full.split("/")[0]
            if not reserved_host(host):
                self.bad(where, "url", f"链接 {m.group(0)[:60]} 不是保留域名")

    def walk(self, v, where: str) -> None:
        if isinstance(v, dict):
            t = v.get("@type")
            if t == "user":
                first, last = v.get("first_name") or "", v.get("last_name") or ""
                full = f"{first} {last}".strip()
                if full and full not in self.names:
                    for part, field in ((first, "first_name"), (last, "last_name")):
                        self.name(where, part, field)
            if t == "chat":
                self.name(where, v.get("title"), "title")
            for k, x in v.items():
                w = f"{where}.{k}"
                if k == "phone_number" and isinstance(x, str) and x and not x.startswith("99966"):
                    self.bad(w, "phone", "phone_number 不是 99966 开头的测试号")
                    continue
                if k in NAME_KEYS:
                    self.name(w, x, k)
                if k == "usernames" and isinstance(x, dict):
                    for u in (x.get("active_usernames") or []) + [x.get("editable_username") or ""]:
                        self.name(w, u, "username")
                if isinstance(x, bool):
                    pass
                elif isinstance(x, int) and (k in ID_KEYS or (k == "id" and t in ID_OBJECTS)):
                    # 对象自己的 id 按对象种类走对应的规则：只有会话 id 许负（codex 068 第 1 轮 #1）
                    rule = {"chat": "chat_id", "user": "user_id", "message": "message_id"}.get(t, k) if k == "id" else k
                    if not id_ok(rule, x):
                        self.bad(w, "ids", f"{k}={x} 不在保留区间")
                if k == "minithumbnail" and isinstance(x, dict):
                    data = x.get("data") or ""
                    if data and not str(data).startswith("fixture-"):
                        self.bad(w, "files", "minithumbnail.data 不是空串或 fixture- 占位")
                if k == "remote" and isinstance(x, dict):
                    for f in ("id", "unique_id"):
                        val = x.get(f) or ""
                        if val and not str(val).startswith("fixture-"):
                            self.bad(f"{w}.{f}", "files", "remote 文件标识不是空串或 fixture- 占位")
                self.walk(x, w)
        elif isinstance(v, list):
            for i, x in enumerate(v):
                self.walk(x, f"{where}[{i}]")
        elif isinstance(v, str):
            self.text(where, v)


def iter_docs(path: str):
    """(位置, JSON 值)。.jsonl 一行一个；读不了、解析不了的当成错误交回"""
    with open(path, encoding="utf-8") as f:
        src = f.read()
    if path.endswith(".jsonl"):
        for n, line in enumerate(src.splitlines(), 1):
            if line.strip():
                yield f"{path}:{n}", line
    else:
        yield path, src


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--root", default=os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))
    a = ap.parse_args()
    base = os.path.join(a.root, FIXTURES)
    try:
        c = Checker(load_names(a.root))
    except OSError as e:
        print(f"✗ 读不了 names.txt：{e}")
        return 1
    n = 0
    # 子目录读不了不许被悄悄跳过：记成错误（codex 068 第 1 轮 #3），否则只要别处扫到了 JSON 就会安静变绿
    def walk_error(e: OSError) -> None:
        c.bad(os.path.relpath(getattr(e, "filename", base) or base, a.root), "read", f"目录读不了（{e.strerror or e}）")

    for dirpath, _dirs, files in os.walk(base, onerror=walk_error):
        for fn in sorted(files):
            if not fn.endswith((".json", ".jsonl")):
                continue
            path = os.path.join(dirpath, fn)
            rel = os.path.relpath(path, a.root)
            try:
                for where, text in iter_docs(path):
                    n += 1
                    try:
                        c.walk(json.loads(text), os.path.relpath(where, a.root) if os.sep in where else where)
                    except json.JSONDecodeError as e:
                        c.bad(where, "json", f"不是合法 JSON（{e.msg}）")
            except (OSError, UnicodeDecodeError) as e:
                c.bad(rel, "read", f"读不了（{e}）")
    if n == 0:
        print(f"✗ {FIXTURES} 下一个夹具都没扫到：路径变了要同步改这个脚本")
        return 1
    for e in c.errors:
        print(f"✗ {e}")
    if c.errors:
        print(f"夹具隐私检查：{len(c.errors)} 处命中（规则见 specs/contracts/testing.md 第一节第 3 小节）")
        return 1
    print(f"✓ 夹具隐私检查：扫了 {n} 份 JSON，0 处命中")
    return 0


if __name__ == "__main__":
    sys.exit(main())
