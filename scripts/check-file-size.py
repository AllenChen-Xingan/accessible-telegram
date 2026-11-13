#!/usr/bin/env python3
"""单文件行数上限 + 棘轮（issue #60，规则见 specs/architecture-rules.md 第 1 条）。

上限 600 行。存量超标的记在基线里，只许变小；新文件、基线外的文件一律守线。

  python3 scripts/check-file-size.py                      # 对照基线查：新超标、变大、变小了没收紧 → 退出码 1
  python3 scripts/check-file-size.py --tighten            # 文件拆小后：把基线里的数改成现状（只降不升），删掉已守线的
  python3 scripts/check-file-size.py --write-baseline --rev origin/main   # 按某个提交的现状重写基线（先 git fetch；会放宽，要 baseline:loosen 标签）
  python3 scripts/check-file-size.py --against origin/main   # CI（PR）：另查基线有没有比合并目标放宽

只看被 git 跟踪的文件；--rev 时从那个提交读，不看工作区。只用标准库。
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys

MAX = 600
# 本项目自己写的代码：src/ 下各 crate 与 scripts/。tdlib-rs 是上游 subtree，不归这条管
ROOTS = ("src/", "scripts/")
EXCLUDE_PREFIX = ("src/tdlib-rs/",)
EXCLUDE_DIRS = {"target", "node_modules", "_build"}  # 构建产物；夹具、快照目录不排除（里面放代码也照管）
EXT = {".rs", ".swift", ".py", ".sh", ".m", ".c", ".h"}
BASELINE = "scripts/file-size-baseline.json"


def git(root: str, *args: str) -> bytes:
    r = subprocess.run(["git", "-C", root, *args], capture_output=True, timeout=60)
    if r.returncode != 0:
        sys.exit(f"git {' '.join(args)} 失败：{r.stderr.decode('utf-8', 'replace').strip()}")
    return r.stdout


def in_scope(rel: str) -> bool:
    if not rel.startswith(ROOTS) or rel.startswith(EXCLUDE_PREFIX):
        return False
    if set(rel.split("/")[:-1]) & EXCLUDE_DIRS:
        return False
    return os.path.splitext(rel)[1] in EXT


def count(data: bytes) -> int:
    # 与 `wc -l` 一致再加上没换行结尾的最后一行
    return data.count(b"\n") + (1 if data and not data.endswith(b"\n") else 0)


def measure(root: str, rev: str | None) -> dict[str, int]:
    """所有在范围内的文件 → 行数"""
    if rev:
        names = git(root, "ls-tree", "-r", "-z", "--name-only", rev).decode("utf-8", "replace")
    else:
        names = git(root, "ls-files", "-z").decode("utf-8", "replace")
    out = {}
    for rel in filter(None, names.split("\0")):
        if not in_scope(rel):
            continue
        if rev:
            data = git(root, "show", f"{rev}:{rel}")
        else:
            path = os.path.join(root, rel)
            if not os.path.isfile(path):
                continue  # 已删未提交
            with open(path, "rb") as f:
                data = f.read()
        out[rel] = count(data)
    return out


def parse_baseline(text: str, where: str) -> dict[str, int]:
    try:
        data = json.loads(text)
    except json.JSONDecodeError as e:
        sys.exit(f"基线 {where} 不是 JSON：{e}")
    files = data.get("files") if isinstance(data, dict) else None
    if not isinstance(files, dict) or not all(isinstance(v, int) for v in files.values()):
        sys.exit(f"基线 {where} 形状不对：要 {{\"max\": {MAX}, \"files\": {{路径: 行数}}}}")
    # 上限的正本是本脚本的 MAX；基线里的 max 只是记录，对不上就说明有人以为改它能改上限
    if data.get("max") != MAX:
        sys.exit(f"基线 {where} 写的 max={data.get('max')}，脚本的上限是 {MAX}：上限只在脚本里改")
    return files


def load(path: str) -> dict[str, int]:
    try:
        with open(path, encoding="utf-8") as f:
            return parse_baseline(f.read(), path)
    except FileNotFoundError:
        sys.exit(f"没有基线文件 {path}：先跑 --write-baseline")


def save(path: str, files: dict[str, int], note: str) -> None:
    with open(path, "w", encoding="utf-8") as f:
        json.dump({"max": MAX, "note": note, "files": dict(sorted(files.items()))},
                  f, ensure_ascii=False, indent=2)
        f.write("\n")


SELF = "scripts/check-file-size.py"
# 「门槛」本身：这些文件的任何改动都可能让门槛变松（上限、范围、比对逻辑、自测、CI 怎么调用），PR 改了就要标签
GATE_FILES = (SELF, "scripts/check-sources.py", "scripts/tests", ".github/workflows/arch.yml",
              ".github/workflows/arch-guard.yml", "scripts/arch-guard-auth.sh", "clippy.toml")  # clippy.toml：disallowed-methods 等编译期门槛


def gate_touched(root: str, against: str, head: str | None) -> list[str]:
    """合并目标 → 本次（head 提交，或不给 head 时的工作区）之间，门槛文件改了哪些"""
    args = ["git", "-C", root, "diff", "--name-only", against] + ([head] if head else []) + ["--", *GATE_FILES]
    r = subprocess.run(args, capture_output=True, timeout=60)
    if r.returncode != 0:
        sys.exit(f"git diff 失败：{r.stderr.decode('utf-8', 'replace').strip()}")
    return [x for x in r.stdout.decode("utf-8", "replace").splitlines() if x]


def show(root: str, rev: str, rel: str) -> bytes | None:
    """某个提交里的文件内容；没有这个文件 → None"""
    r = subprocess.run(["git", "-C", root, "show", f"{rev}:{rel}"], capture_output=True, timeout=60)
    if r.returncode == 0:
        return r.stdout
    if b"does not exist" in r.stderr or b"exists on disk, but not in" in r.stderr:
        return None
    sys.exit(f"读 {rev}:{rel} 失败：{r.stderr.decode('utf-8', 'replace').strip()}")


def loosened(old: dict[str, int], new: dict[str, int]) -> list[str]:
    """新基线比旧基线宽的地方：多出来的条目、变大的数"""
    out = []
    for rel, n in sorted(new.items()):
        if rel not in old:
            out.append(f"✗ 基线新增 {rel}：{n} 行")
        elif n > old[rel]:
            out.append(f"✗ 基线放宽 {rel}：{old[rel]} → {n} 行")
    return out


def guard(root: str, against: str, head: str | None, baseline: str, allow: bool) -> int:
    """只管「授权」：门槛文件有没有改、基线有没有放宽。行数本身由 arch 工作流查（门槛文件没改时，PR 里那份脚本就是 main 上那份）。
    这里读本次的基线**不许提前退出**：基线改名、max 改了，都只算一条「放宽」，带标签就放行，交给 arch 用新契约去验"""
    if show(root, against, SELF) is None:
        print(f"（{against} 上还没有本脚本：门槛第一次进库，不比放宽）")
        return 0
    wider = [f"✗ 门槛文件 {f} 改了：门槛的任何改动都要人看过" for f in gate_touched(root, against, head)]
    old_text = show(root, against, baseline)
    if head:
        new_text = show(root, head, baseline)
    else:
        try:
            with open(os.path.join(root, baseline), "rb") as f:
                new_text = f.read()
        except FileNotFoundError:
            new_text = None
    if old_text is None:
        wider.append(f"✗ {against} 上有门槛脚本却没有基线 {baseline}")
    elif new_text is None:
        wider.append(f"✗ 本次没有基线 {baseline}：被改名或删了")
    else:
        try:
            old = json.loads(old_text)["files"]
            new_data = json.loads(new_text)
            new = new_data["files"]
            if new_data.get("max") != MAX:
                wider.append(f"✗ 基线的 max 改成了 {new_data.get('max')}（{against} 上是 {MAX}）")
            wider += loosened(old, new)
        except (ValueError, KeyError, TypeError, AttributeError) as e:
            wider.append(f"✗ 基线读不懂（{type(e).__name__}）：当作放宽处理")
    if not wider:
        print("门槛没有被放宽或改动。")
        return 0
    if allow:
        print("门槛有放宽或改动，PR 带了 baseline:loosen 标签，放行（新门槛由 arch 工作流验证）：")
        print("\n".join("  " + w for w in wider))
        return 0
    print("\n".join(wider))
    print("\n放宽门槛要人或大总管给 PR 加 baseline:loosen 标签（specs/architecture-rules.md 第 1 条）。")
    return 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--root", default=".", help="仓库根（默认当前目录）")
    ap.add_argument("--baseline", default=BASELINE)
    ap.add_argument("--rev", help="从这个提交读现状（只配 --write-baseline 用；先 git fetch，用 origin/main）")
    ap.add_argument("--against", help="CI 用：拿本次的基线和这个提交（合并目标）的基线比，放宽了就红")
    ap.add_argument("--head", help="CI（pull_request_target）用：从这个提交读本次的代码与基线，只当数据读、不执行它")
    ap.add_argument("--allow-loosen", action="store_true",
                    help="放宽基线经人同意（CI 里只在 PR 带 baseline:loosen 标签时传）")
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--write-baseline", action="store_true", help="把现在超线的文件整个重写成基线（会放宽，PR 要带 baseline:loosen 标签）")
    g.add_argument("--tighten", action="store_true", help="基线只降不升：变小的改成现状，守线了的删掉")
    a = ap.parse_args()
    if a.rev and not a.write_baseline:
        ap.error("--rev 只配 --write-baseline 用")
    if a.against and (a.write_baseline or a.tighten):
        ap.error("--against 只在检查时用")
    if a.head and (a.rev or a.write_baseline or a.tighten or not a.against):
        ap.error("--head 只配 --against 用")

    if a.against:
        return guard(a.root, a.against, a.head, a.baseline, a.allow_loosen)

    base_path = os.path.join(a.root, a.baseline)
    lines = measure(a.root, a.rev)
    over = {k: v for k, v in lines.items() if v > MAX}

    if a.write_baseline:
        save(base_path, over, f"按 {a.rev or '工作区'} 的现状记；只许变小（scripts/check-file-size.py --tighten）")
        print(f"已写基线：{len(over)} 个文件超过 {MAX} 行 → {a.baseline}")
        print("若合并目标上已经有基线，这次重记就是放宽：PR 要带 baseline:loosen 标签（由人或大总管加），否则 CI 红；"
              "门槛第一次进库的那个 PR 不用。")
        return 0

    base = load(base_path)

    if a.tighten:
        new = {}
        for rel, b in base.items():
            n = lines.get(rel)
            if n is not None and n > MAX:
                new[rel] = min(n, b)
        save(base_path, new, "只许变小（scripts/check-file-size.py --tighten）")
        print(f"基线收紧：{len(base)} → {len(new)} 个文件；行数只降不升")
        return 0

    fails, stale = [], []
    # ① 代码不许超出基线
    for rel, n in sorted(over.items(), key=lambda kv: -kv[1]):
        b = base.get(rel)
        if b is None:
            fails.append(f"✗ {rel}：{n} 行，新超标（上限 {MAX}）")
        elif n > b:
            fails.append(f"✗ {rel}：{n} 行，比基线 {b} 行更大（存量只许变小）")
    # ② 变小了必须当场收紧，否则以后能悄悄长回去
    for rel, b in sorted(base.items()):
        n = lines.get(rel, 0)
        if n <= MAX:
            stale.append(f"✗ {rel}：已守线或已删除（{n} 行），基线里还记着 {b}")
        elif n < b:
            stale.append(f"✗ {rel}：{b} → {n} 行，基线没收紧")

    for s in fails + stale:
        print(s)
    print(f"\n范围内 {len(lines)} 个文件；超线 {len(over)} 个；报错 {len(fails) + len(stale)} 个。")
    if fails:
        print("按职责拆（类型 / 解析 / 渲染 / 测试各一个文件）。行内测试挪出：foo.rs 里留 `#[cfg(test)] mod tests;`，"
              "测试搬到 foo/tests.rs（foo.rs 不用改名成 mod.rs）；测试本身也守 600 行，超了就按主题再分成 "
              "foo/tests/<主题>.rs。不要切成 part1、part2。")
    if stale:
        print("文件变小了：跑 `python3 scripts/check-file-size.py --tighten` 把基线收紧，和代码一起提交。")
    return 1 if fails or stale else 0


if __name__ == "__main__":
    sys.exit(main())
