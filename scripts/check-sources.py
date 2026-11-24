#!/usr/bin/env python3
"""正本比对（issue #60，规则见 specs/architecture-rules.md 第 2 条）：契约里的表和代码，谁是正本、另一边逐项比。

前四条是「文档里手写的一张表」对「代码里的正本」；workspace 一条查目录与工作区成员规则是否相容；testkit 一条查测试夹具不会进正式构建。对不上 → 退出码 1，并说出哪一项差了什么。

  python3 scripts/check-sources.py            # 全部
  python3 scripts/check-sources.py --only error-codes,exit-codes

| 检查 | 正本 | 第二处 |
|---|---|---|
| error-codes | src/atg-core/src/error.rs `CoreError::code` + src/atg-cli/src/out.rs `Code::as_str` | specs/contracts/errors.md 第一节机读码表 + 「CLI 自有码」 |
| exit-codes  | src/atg-cli/src/out.rs `core_exit` + `Code::exit` | specs/contracts/cli.md 第一节第 4 小节退出码表 |
| subcommands | src/atg-cli/src/args.rs `enum Command` | specs/contracts/cli.md 第一节第 2 小节子命令表 |
| versions    | src/atg-core/src/tree/mod.rs `SCHEMA_V` · src/atg-core/src/ipc.rs `PROTOCOL_V` | cli.md / content-tree.md 里的外壳例子 `"v":N` · config-privacy.md 的请求例子 |
| workspace   | 根 Cargo.toml `members = ["src/atg-*"]` | src/ 下每个 `atg-*` 目录都得有 Cargo.toml（否则整个工作区的 cargo 命令失败；守则第 10 条） |
| testkit     | `atg-core` 的 `testkit` 特性只给测试（`src/atg-core/src/testkit.rs`） | 根与每个 `src/atg-*/Cargo.toml`：只许在 dev-dependencies 里开它；正式依赖、`[features]`、`default`、`workspace.dependencies` 都不许带（#108） |

只用标准库（testkit 一条用 `tomllib`，要 Python ≥ 3.11）。其余几条解析是正则，靠的是现有写法；写法变了解析不出 → 报「解析不出」并红，不会悄悄变绿。
"""
from __future__ import annotations

import argparse
import os
import re
import sys
import tomllib

ERROR_RS = "src/atg-core/src/error.rs"
OUT_RS = "src/atg-cli/src/out.rs"
ARGS_RS = "src/atg-cli/src/args.rs"
TREE_RS = "src/atg-core/src/tree/mod.rs"
IPC_RS = "src/atg-core/src/ipc.rs"
ERRORS_MD = "specs/contracts/errors.md"
CLI_MD = "specs/contracts/cli.md"
TREE_MD = "specs/content-tree.md"
PRIVACY_MD = "specs/contracts/config-privacy.md"


class Unparsed(Exception):
    """写法变了、解析不出：当失败处理"""


def read(root: str, rel: str) -> str:
    with open(os.path.join(root, rel), encoding="utf-8") as f:
        return f.read()


def fn_body(src: str, sig: str, where: str) -> str:
    """从 `sig` 起到下一个顶格或同缩进的 `}` 为止（足够应付本仓库的写法）"""
    i = src.find(sig)
    if i < 0:
        raise Unparsed(f"{where} 里找不到 `{sig}`")
    indent = src.rfind("\n", 0, i)
    pad = src[indent + 1:i]
    end = src.find("\n" + pad + "}", i)
    if end < 0:
        raise Unparsed(f"{where} 里 `{sig}` 没有结尾")
    return src[i:end]


def section(md: str, heading_re: str, where: str) -> str:
    m = re.search(heading_re, md, re.M)
    if not m:
        raise Unparsed(f"{where} 里找不到标题 /{heading_re}/")
    rest = md[m.end():]
    nxt = re.search(r"^#{1,4} ", rest, re.M)
    return rest[: nxt.start()] if nxt else rest


def table_rows(text: str) -> list[list[str]]:
    rows = []
    for line in text.splitlines():
        if line.startswith("|") and not re.match(r"^\|\s*-", line):
            rows.append([c.strip() for c in line.strip().strip("|").split("|")])
    return rows[1:]  # 去表头


# ───────────── 代码这一侧 ─────────────

# 一个 match 分支的左边：`Self::A`、`Self::A { .. }`、`Self::A(_)`、`Self::A { td }`，可用 `|` 连几个
def _arm(prefix: str) -> str:
    one = rf"{prefix}::\w+\s*(?:\([^)]*\)|\{{[^}}]*\}})?\s*"
    return rf"((?:\|?\s*{one})+)=>\s*"


def arms(body: str, prefix: str, rhs: str, where: str) -> list[tuple[str, str]]:
    """解析 match 的每个分支；分支数和 `=>` 的个数对不上就当解析不出（防止新写法被悄悄跳过）"""
    # 右边必须整个被认出来，靠 rustfmt 的规范写法（CI 有 cargo fmt --check）：简单表达式的分支一律是
    # 「值紧跟逗号」。所以值后面必须**立刻**是逗号；跨行续写、`2 + 1`、`EXIT_AUTH + 1` 都认不全 → 分支数对不上 → 解析不出
    found = re.findall(_arm(prefix) + rhs + r"(?=,)", body)
    total = body.count("=>")
    if len(found) != total:
        raise Unparsed(f"{where}：match 有 {total} 个分支，只认出 {len(found)} 个（有新写法？更新本脚本的解析）")
    return found


def core_codes(root: str) -> dict[str, str]:
    """CoreError 变体 → 机读码"""
    body = fn_body(read(root, ERROR_RS), "pub fn code(&self)", ERROR_RS)
    out = {}
    for arm, code in arms(body, "Self", r"\"([a-z_0-9]+)\"", f"{ERROR_RS} `code()`"):
        for v in re.findall(r"Self::(\w+)", arm):
            out[v] = code
    if not out:
        raise Unparsed(f"{ERROR_RS} `code()` 里没解析出任何 变体 => \"码\"")
    return out


def cli_codes(root: str) -> tuple[dict[str, str], dict[str, int]]:
    """CLI 自有码：变体 → 码，变体 → 退出码"""
    src = read(root, OUT_RS)
    names, exits = {}, {}
    for arm, c in arms(fn_body(src, "pub fn as_str(self)", OUT_RS), "Code", r"\"([a-z_0-9]+)\"", f"{OUT_RS} `Code::as_str`"):
        for v in re.findall(r"Code::(\w+)", arm):
            names[v] = c
    for arm, n in arms(fn_body(src, "pub fn exit(self)", OUT_RS), "Code", r"(\d+)", f"{OUT_RS} `Code::exit`"):
        for v in re.findall(r"Code::(\w+)", arm):
            exits[v] = int(n)
    if not names or set(names) != set(exits):
        raise Unparsed(f"{OUT_RS} 的 `Code::as_str` 与 `Code::exit` 解析不出或变体对不上")
    return names, exits


def core_exits(root: str) -> dict[str, int]:
    """CoreError 变体 → 退出码（`core_exit` 的 match）"""
    src = read(root, OUT_RS)
    consts = {k: int(v) for k, v in re.findall(r"const (\w+): u8 = (\d+);", src)}
    body = fn_body(src, "pub fn core_exit(", OUT_RS)
    out = {}
    for arm, val in arms(body, "E", r"(\w+)", f"{OUT_RS} `core_exit`"):
        n = int(val) if val.isdigit() else consts.get(val)
        if n is None:
            raise Unparsed(f"{OUT_RS} `core_exit` 里的 `{val}` 不是数字也不是本文件的常量")
        for v in re.findall(r"E::(\w+)", arm):
            out[v] = n
    if not out:
        raise Unparsed(f"{OUT_RS} `core_exit` 没解析出任何分支")
    return out


def commands(root: str) -> set[str]:
    src = read(root, ARGS_RS)
    m = re.search(r"pub enum Command \{(.*?)\n\}", src, re.S)
    if not m:
        raise Unparsed(f"{ARGS_RS} 里找不到 `pub enum Command`")
    variants = re.findall(r"^\s{4}([A-Z]\w*)\b", m.group(1), re.M)
    return {re.sub(r"(?<!^)([A-Z])", r"-\1", v).lower() for v in variants}


def const_u32(root: str, rel: str, name: str) -> int:
    m = re.search(rf"pub const {name}: u32 = (\d+);", read(root, rel))
    if not m:
        raise Unparsed(f"{rel} 里找不到 `pub const {name}: u32`")
    return int(m.group(1))


# ───────────── 检查 ─────────────

def check_error_codes(root: str) -> list[str]:
    md = read(root, ERRORS_MD)
    sec = section(md, r"^### 1\. 错误只有一个类型", ERRORS_MD)
    doc = set()
    for row in table_rows(sec.split("`send_refused.reason`")[0]):
        m = re.fullmatch(r"`([a-z_0-9]+)`", row[0]) if row else None
        if m:
            doc.add(m.group(1))
    if not doc:
        raise Unparsed(f"{ERRORS_MD} 第一节没解析出机读码表")
    own = re.search(r"\*\*CLI 自有码\*\*[^：]*：(.*)", sec)
    if not own:
        raise Unparsed(f"{ERRORS_MD} 里找不到「**CLI 自有码**」一行")
    doc_cli = set(re.findall(r"(?:^|· )`([a-z_0-9]+)`", own.group(1)))
    code = set(core_codes(root).values())
    names, _ = cli_codes(root)
    errs = []
    for c in sorted(code - doc):
        errs.append(f"代码有、{ERRORS_MD} 码表没有：`{c}`（CoreError::code）")
    for c in sorted(doc - code):
        errs.append(f"{ERRORS_MD} 码表有、CoreError::code 没有：`{c}`")
    cli_own = set(names.values()) - code  # 借用 CoreError 码的（如 disconnected）不算自有
    for c in sorted(cli_own ^ doc_cli):
        side = "out.rs Code 有、errors.md「CLI 自有码」没写" if c in cli_own else "errors.md「CLI 自有码」有、out.rs Code 没有"
        errs.append(f"{side}：`{c}`")
    return errs


def check_exit_codes(root: str) -> list[str]:
    sec = section(read(root, CLI_MD), r"^### 4\. 退出码", CLI_MD)
    # 表里每一项（精确码或 `前缀*`）原样收下、不合并：一个码可能被几行同时说到（codex 019 门槛第 4、5 轮）
    entries: list[tuple[str, bool, int]] = []  # (码或前缀, 是不是前缀, 退出码)
    success_rows: list[int] = []  # 不带错误码的行（「成功」）的退出码
    for row in table_rows(sec):
        if len(row) < 3 or not row[0].isdigit():
            continue
        found = re.findall(r"`([a-z_0-9*]+)`", row[2])
        if not found:
            success_rows.append(int(row[0]))
        for c in found:
            entries.append((c.rstrip("*"), c.endswith("*"), int(row[0])))
    if not any(not is_p for _, is_p, _ in entries):
        raise Unparsed(f"{CLI_MD} 退出码表解析不出")

    def matches(c: str) -> set[int]:
        """表里所有说到这个码的项（精确相等，或前缀匹配）给出的退出码"""
        return {n for k, is_p, n in entries if (c.startswith(k) if is_p else c == k)}

    errs: list[str] = []
    # 成功：进程退出码 0 是操作系统的约定（Rust 的 ExitCode::SUCCESS），表里必须恰好一行不带错误码、写 0；错误码不许占 0（codex 019 门槛第 7 轮 #1）
    if success_rows != [0]:
        errs.append(f"{CLI_MD} 退出码表要恰好一行「成功」写 0（不带错误码），现在是 {success_rows or '没有'}")
    for k, is_p, n in entries:
        if n == 0:
            errs.append(f"{CLI_MD} 退出码表里 `{k}{'*' if is_p else ''}` 占了 0，0 只给成功")
    codes = core_codes(root)
    names, exits = cli_codes(root)
    known = set(codes.values()) | set(names.values())
    # ① 表自己不许自相矛盾：对每个已知的码，所有说到它的项必须给同一个退出码（同名重复、精确对前缀、前缀对前缀都在这里）
    for c in sorted(known):
        got = matches(c)
        if len(got) > 1:
            errs.append(f"{CLI_MD} 退出码表自相矛盾：`{c}` 同时被写成 {sorted(got)}")
    # 前缀之间互相包含却给不同退出码，哪怕眼下没有已知的码落进去，也算矛盾（以后加码就会踩到）
    prefixes = [(k, n) for k, is_p, n in entries if is_p]
    for k1, n1 in prefixes:
        for k2, n2 in prefixes:
            if k1 != k2 and k1.startswith(k2) and n1 != n2:
                errs.append(f"{CLI_MD} 退出码表自相矛盾：`{k1}*` 写 {n1}，包住它的 `{k2}*` 写 {n2}")
    for k, is_p, n in entries:
        if not is_p and sum(1 for k2, p2, n2 in entries if not p2 and k2 == k and n2 != n):
            errs.append(f"{CLI_MD} 退出码表自相矛盾：`{k}` 写了不同的退出码")

    def want(c: str) -> int | None:
        got = matches(c)
        return next(iter(got)) if len(got) == 1 else None

    # ② 代码 → 表
    for variant, n in sorted(core_exits(root).items()):
        c = codes.get(variant)
        if c is None:
            errs.append(f"`core_exit` 里的 E::{variant} 在 CoreError::code 里没有")
            continue
        w = want(c)
        if w != n:
            errs.append(f"`{c}`：代码退出码 {n}，{CLI_MD} 表里是 {sorted(matches(c)) or '没有'}")
    for v in sorted(set(codes) - set(core_exits(root))):
        errs.append(f"CoreError::{v} 在 `core_exit` 里没有分支")
    for v, c in sorted(names.items()):
        w = want(c)
        if w != exits[v]:
            errs.append(f"CLI 码 `{c}`（Code::{v}）：代码退出码 {exits[v]}，{CLI_MD} 表里是 {sorted(matches(c)) or '没有'}")
    # ③ 表 → 代码：表里写了、代码里已经没有的码或前缀（死行或拼错）
    for k, is_p, _ in entries:
        if is_p and not any(c.startswith(k) for c in known):
            errs.append(f"{CLI_MD} 退出码表里的 `{k}*` 在代码里一个都对不上")
        elif not is_p and k not in known:
            errs.append(f"{CLI_MD} 退出码表里的 `{k}` 在代码里没有")
    return list(dict.fromkeys(errs))  # 去重、保序


def check_subcommands(root: str) -> list[str]:
    sec = section(read(root, CLI_MD), r"^### 2\. 子命令", CLI_MD)
    doc = set()
    for row in table_rows(sec):
        m = re.match(r"`([a-z][a-z-]*)", row[0]) if row else None
        if m:
            doc.add(m.group(1))
    if not doc:
        raise Unparsed(f"{CLI_MD} 子命令表解析不出")
    code = commands(root)
    errs = [f"args.rs 有、{CLI_MD} 子命令表没有：`{c}`" for c in sorted(code - doc)]
    errs += [f"{CLI_MD} 子命令表有、args.rs `enum Command` 没有：`{c}`" for c in sorted(doc - code)]
    return errs


def check_versions(root: str) -> list[str]:
    errs = []
    schema = const_u32(root, TREE_RS, "SCHEMA_V")
    for rel in (CLI_MD, TREE_MD):
        seen = {int(v) for v in re.findall(r"\{\s*\"v\"\s*:\s*(\d+)\s*,\s*\"kind\"", read(root, rel))}
        if not seen:
            errs.append(f"{rel} 里没有外壳例子 {{\"v\":N,\"kind\"…}}（解析不出）")
        for v in sorted(seen - {schema}):
            errs.append(f"{rel} 外壳例子写 \"v\":{v}，代码 tree::SCHEMA_V = {schema}")
    proto = const_u32(root, IPC_RS, "PROTOCOL_V")
    seen = {int(v) for v in re.findall(r"\{\s*\"v\"\s*:\s*(\d+)\s*,\s*\"origin\"", read(root, PRIVACY_MD))}
    if not seen:
        errs.append(f"{PRIVACY_MD} 里没有请求例子 {{\"v\":N,\"origin\"…}}（解析不出）")
    for v in sorted(seen - {proto}):
        errs.append(f"{PRIVACY_MD} 请求例子写 \"v\":{v}，代码 ipc::PROTOCOL_V = {proto}")
    return errs


def check_workspace(root: str) -> list[str]:
    # 正本是根 Cargo.toml 的 members；本检查只懂 ["src/atg-*"] 这一种写法，改了就报解析不出（不按旧规则继续判）
    m = re.search(r"^members\s*=\s*\[(.*?)\]", read(root, "Cargo.toml"), re.M | re.S)
    members = re.findall(r"\"([^\"]+)\"", m.group(1)) if m else None
    if members != ["src/atg-*"]:
        raise Unparsed(f"根 Cargo.toml 的 members 是 {members}，本检查只认 [\"src/atg-*\"]：改了成员规则要同步改这条检查")
    src = os.path.join(root, "src")
    if not os.path.isdir(src):
        raise Unparsed("没有 src/ 目录")
    return [f"src/{d}/ 以 atg- 开头却没有 Cargo.toml：会被根工作区当成员，所有 cargo 命令都会失败（守则第 10 条）"
            for d in sorted(os.listdir(src))
            if d.startswith("atg-") and os.path.isdir(os.path.join(src, d))
            and not os.path.isfile(os.path.join(src, d, "Cargo.toml"))]


TESTKIT = "testkit"
# 正式构建会用到的依赖表（dev-dependencies 不在其中：测试才用）
_PROD_DEPS = ("dependencies", "build-dependencies")


def _manifests(root: str) -> list[str]:
    src = os.path.join(root, "src")
    if not os.path.isdir(src):
        raise Unparsed("没有 src/ 目录")
    rels = ["Cargo.toml"] + [f"src/{d}/Cargo.toml" for d in sorted(os.listdir(src))
                             if d.startswith("atg-") and os.path.isfile(os.path.join(src, d, "Cargo.toml"))]
    out = []
    for rel in rels:
        if os.path.isfile(os.path.join(root, rel)):
            out.append(rel)
    return out


def _prod_dep_tables(t: dict) -> list[tuple[str, dict]]:
    tabs = [(k, t.get(k) or {}) for k in _PROD_DEPS]
    for tgt, v in (t.get("target") or {}).items():
        tabs += [(f"target.'{tgt}'.{k}", (v or {}).get(k) or {}) for k in _PROD_DEPS]
    ws = (t.get("workspace") or {}).get("dependencies")
    if ws:
        tabs.append(("workspace.dependencies", ws))  # 成员 `workspace = true` 继承它，等于正式依赖也带上
    return tabs


def _pkg(name: str, spec, ws_deps: dict) -> str:
    """一条依赖真正指向哪个包：`package = …` 改名的取它；`workspace = true` 继承的先查根 workspace.dependencies
    里同名那条（根上也可能改了名，codex 046 第 1 轮 #1）；都没有就是键名"""
    if isinstance(spec, dict):
        if spec.get("package"):
            return spec["package"]
        if spec.get("workspace") is True:
            base = ws_deps.get(name)
            if isinstance(base, dict) and base.get("package"):
                return base["package"]
    return name


def _dev_dep_tables(t: dict) -> list[tuple[str, dict]]:
    tabs = [("dev-dependencies", t.get("dev-dependencies") or {})]
    for tgt, v in (t.get("target") or {}).items():
        tabs.append((f"target.'{tgt}'.dev-dependencies", (v or {}).get("dev-dependencies") or {}))
    return tabs


def check_testkit(root: str) -> list[str]:
    # 正本：testkit 只给测试。正式构建一旦开了它，测试夹具（建临时目录、建档案）就进了二进制。
    # 靠两样：① 只在 dev-dependencies 里开（下面逐表查）；② 根 Cargo.toml 的 resolver 是 "2" 或 "3"——只有这样，
    # dev-dependencies 开的特性才不会合并进 `cargo build` 的正式构建（resolver 1 会合并，那时 ① 查了也白查）。
    # 只管清单；`--all-features`、`--features atg-core/testkit` 这类命令行开法不经清单，打正式包的命令不许这么写
    errs = []
    root_t = None
    core_t = None
    ws_deps: dict = {}
    for rel in _manifests(root):  # 根 Cargo.toml 排第一：先拿到 workspace.dependencies，成员的继承才认得出
        try:
            t = tomllib.loads(read(root, rel))
        except tomllib.TOMLDecodeError as e:
            raise Unparsed(f"{rel} 不是合法的 TOML：{e}")
        if rel == "Cargo.toml":
            root_t = t
            ws_deps = (t.get("workspace") or {}).get("dependencies") or {}
        # 这份清单里指向 atg-core 的本地名（键名；改名的 `core = { package = "atg-core" }` 本地名是 core）
        local = set()
        for where, deps in _prod_dep_tables(t) + _dev_dep_tables(t):
            for name, spec in deps.items():
                if _pkg(name, spec, ws_deps) == "atg-core":
                    local.add(name)
        for where, deps in _prod_dep_tables(t):
            for name, spec in deps.items():
                if not isinstance(spec, dict):
                    continue
                if _pkg(name, spec, ws_deps) == "atg-core" and TESTKIT in (spec.get("features") or []):
                    errs.append(f"{rel} [{where}] 的 {name} 开了 testkit：只许在 dev-dependencies 里开")
        is_core = (t.get("package") or {}).get("name") == "atg-core"
        if is_core:
            core_t = t
        for feat, vals in (t.get("features") or {}).items():
            for v in vals or []:
                if is_core and feat != TESTKIT and v == TESTKIT:
                    errs.append(f"{rel} [features] {feat} 连带打开了 testkit（{feat} = [... \"{v}\" ...]）")
                m = re.fullmatch(r"([A-Za-z0-9_-]+)\??/testkit", v)
                if m and m.group(1) in local:
                    errs.append(f"{rel} [features] {feat} 连带打开了 {m.group(1)}/testkit（{m.group(1)} 就是 atg-core）："
                                "用这个特性构建，夹具就进了正式二进制")
    # 正本得在：atg-core 声明了 testkit 特性。没有就是改名或删了，这条检查会空转——报解析不出，不悄悄变绿
    if core_t is None or TESTKIT not in (core_t.get("features") or {}):
        raise Unparsed("src/atg-core/Cargo.toml 的 [features] 里没有 testkit：特性改名或删了要同步改这条检查")
    resolver = str(((root_t or {}).get("workspace") or {}).get("resolver", ""))
    if resolver not in ("2", "3"):
        errs.append(f"根 Cargo.toml 的 workspace.resolver 是 {resolver or '（没写，按 1 算）'}：要 \"2\" 或 \"3\"，"
                    "否则 dev-dependencies 开的 testkit 会合并进正式构建")
    return errs

CHECKS = {
    "error-codes": check_error_codes,
    "exit-codes": check_exit_codes,
    "subcommands": check_subcommands,
    "versions": check_versions,
    "workspace": check_workspace,
    "testkit": check_testkit,
}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--root", default=".")
    ap.add_argument("--only", help="逗号分隔：" + ",".join(CHECKS))
    a = ap.parse_args()
    names = a.only.split(",") if a.only else list(CHECKS)
    bad = 0
    for name in names:
        if name not in CHECKS:
            ap.error(f"没有这条检查：{name}")
        try:
            errs = CHECKS[name](a.root)
        except (Unparsed, OSError, UnicodeDecodeError) as e:
            errs = [f"解析不出：{e}"]
        print(f"{'✓' if not errs else '✗'} {name}" + ("" if not errs else f"（{len(errs)} 处）"))
        for e in errs:
            print(f"    {e}")
        bad += bool(errs)
    if bad:
        print("\n改正本那一侧，再让第二处跟上；正本清单见 docs/plan/010_complexity-budget.md 第二节。")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
