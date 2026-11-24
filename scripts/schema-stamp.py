#!/usr/bin/env python3
"""给对外的机读契约（JSON）盖「内容哈希」章，逼内容变动时有意识地决定升不升版本。

  check <文件...>  重算内容哈希：与文件里记的不一致、缺哈希、缺版本字段 → 退出码 1
  stamp <文件...>  重算并写回哈希，打印出来（改完、定好版本号后运行；生成程序里的哈希常量照抄打印值）

哈希：去掉哈希字段本身后，按键排序、紧凑分隔、不转义非 ASCII 的 JSON 的 sha256，取前 16 位十六进制。
所以只改缩进、键顺序不变；改任何内容都变。只用标准库。

本仓用在：界面自描述金样 `src/atg-gui-spec/tests/golden/gui-schema.json`（版本字段 `v`，
`docs/plan/012_gui-ssot.md` 第一节第 4 条）。算法与 skill `single-source-truth` 的同名脚本一致。
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys


def content_hash(obj: dict, field: str) -> str:
    body = {k: v for k, v in obj.items() if k != field}
    canon = json.dumps(body, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(canon.encode("utf-8")).hexdigest()[:16]


def check_one(path: str, field: str, version_field: str, stamp: bool) -> list[str]:
    try:
        with open(path, encoding="utf-8") as f:
            obj = json.load(f)
    except (OSError, json.JSONDecodeError) as e:
        return [f"读不了（{e}）"]
    if not isinstance(obj, dict):
        return ["顶层不是对象，没法存哈希"]
    h = content_hash(obj, field)
    if stamp:
        obj[field] = h
        with open(path, "w", encoding="utf-8") as f:
            json.dump(obj, f, ensure_ascii=False, indent=2)
            f.write("\n")
        print(f"✓ {path}：{field} = {h}（{version_field} = {obj.get(version_field, '缺')}）")
        return []
    problems = []
    if version_field and version_field not in obj:
        problems.append(f"缺 {version_field} 字段")
    recorded = obj.get(field)
    if recorded is None:
        problems.append(f"缺 {field}（先运行 stamp）")
    elif recorded != h:
        problems.append(f"内容变了但哈希没更新：记的 {recorded}，实际 {h}。想清楚要不要升 {version_field}，再 stamp")
    if not problems:
        print(f"✓ {path}：{h}")
    return problems


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", choices=["check", "stamp"])
    ap.add_argument("files", nargs="+")
    ap.add_argument("--field", default="x-content-hash", help="存哈希的顶层字段（默认 x-content-hash）")
    ap.add_argument("--version-field", default="v", help="版本字段（默认 v；空串则不查）")
    a = ap.parse_args()
    bad = 0
    for path in a.files:
        problems = check_one(path, a.field, a.version_field, a.mode == "stamp")
        if problems:
            bad += 1
            print(f"✗ {path}：" + "；".join(problems))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
