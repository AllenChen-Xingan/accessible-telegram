"""正本比对（scripts/check-sources.py）的自测"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
import unittest

from _common import REPO, REVIEW, SIZE, SOURCES, git, run  # noqa: F401


# check-sources 的自测用固定的最小夹具（codex 019 #1）：不复制仓库里的真文件，
# 否则正常的契约升级（如 SCHEMA_V 1 → 2）会先把自测弄红，自测又是门槛文件，改它还要标签。
# 仓库本身一致不一致，由 CI 直接跑 check-sources.py 来查。
FIXTURE = {
    "Cargo.toml": '[workspace]\nmembers = ["src/atg-*"]\n',
    "src/atg-core/Cargo.toml": '[package]\nname = "atg-core"\n',
    "src/atg-cli/Cargo.toml": '[package]\nname = "atg-cli"\n',
    "src/atg-core/src/error.rs": """impl CoreError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::AuthRequired { .. } => "auth_required",
            Self::NotFound { .. } => "not_found",
            Self::Disconnected => "disconnected",
            Self::Cancelled => "cancelled",
            Self::Internal { .. } => "internal",
        }
    }
}
""",
    "src/atg-cli/src/out.rs": """pub enum Code { Usage, NotImplemented, Interrupted, ExecutorGone }

impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Code::Usage => "usage",
            Code::NotImplemented => "not_implemented",
            Code::Interrupted => "interrupted",
            Code::ExecutorGone => "disconnected",
        }
    }

    pub fn exit(self) -> u8 {
        match self {
            Code::Usage => 2,
            Code::NotImplemented => 1,
            Code::Interrupted => 130,
            Code::ExecutorGone => 4,
        }
    }
}

pub fn core_exit(e: &CoreError) -> u8 {
    use CoreError as E;
    match e {
        E::AuthRequired { .. } => EXIT_AUTH,
        E::NotFound { .. } => 6,
        E::Disconnected => 4,
        E::Cancelled => 8,
        E::Internal { .. } => 1,
    }
}

pub const EXIT_AUTH: u8 = 3;
""",
    "src/atg-cli/src/args.rs": "pub enum Command {\n    /// 登录\n    Login,\n    /// 版本\n    Version,\n}\n",
    "src/atg-core/src/tree/mod.rs": "pub const SCHEMA_V: u32 = 1;\n",
    "src/atg-core/src/ipc.rs": "pub const PROTOCOL_V: u32 = 0;\n",
    "specs/contracts/errors.md": """### 1. 错误只有一个类型：`CoreError`

| 机读码 `code` | 什么时候 |
|---|---|
| `auth_required` | 没登录 |
| `not_found` | 不存在 |
| `disconnected` | 断了 |
| `cancelled` | 取消 |
| `internal` | bug |

`send_refused.reason` 的取值：略

**CLI 自有码**（不是 `CoreError`）：`usage`（参数不对）· `interrupted`（Ctrl+C）· `not_implemented`（还没做）。

### 2. 下一节
""",
    "specs/contracts/cli.md": """### 2. 子命令

| 子命令 | 作用 |
|---|---|
| `login` | 登录 |
| `version` | 版本 |

### 4. 退出码

| 码 | 含义 | 对应 `error.code` |
|---|---|---|
| 0 | 成功 | — |
| 1 | 内部 | `internal` `not_implemented` |
| 2 | 用法 | `usage` |
| 3 | 登录 | `auth_*` |
| 4 | 连接 | `disconnected` |
| 6 | 找不到 | `not_found` |
| 8 | 取消 | `cancelled` |
| 130 | 中断 | `interrupted` |

### 5. 例子

{"v":1,"kind":"message","data":{}}
""",
    "specs/content-tree.md": '外壳：{"v":1,"kind":"message"}\n',
    "specs/contracts/config-privacy.md": '请求：{"v":0,"origin":"cli"}\n',
}


class Sources(unittest.TestCase):
    """在固定夹具上改坏一处，对应那条检查必须红；夹具原样必须全绿"""

    def setUp(self):
        self.d = tempfile.mkdtemp(prefix="atg-src-")
        for rel, text in FIXTURE.items():
            os.makedirs(os.path.dirname(os.path.join(self.d, rel)) or self.d, exist_ok=True)
            with open(os.path.join(self.d, rel), "w", encoding="utf-8") as f:
                f.write(text)

    def tearDown(self):
        shutil.rmtree(self.d)

    def edit(self, rel: str, old: str, new: str) -> None:
        p = os.path.join(self.d, rel)
        with open(p, encoding="utf-8") as f:
            s = f.read()
        self.assertIn(old, s, f"夹具 {rel} 里找不到要改坏的那段")
        with open(p, "w", encoding="utf-8") as f:
            f.write(s.replace(old, new, 1))

    def check(self, only: str) -> subprocess.CompletedProcess:
        return run(SOURCES, "--root", self.d, "--only", only)

    def test_pristine_copy_is_green(self):
        r = run(SOURCES, "--root", self.d)
        self.assertEqual(r.returncode, 0, r.stdout)

    def test_renamed_error_code_is_red(self):
        self.edit("src/atg-core/src/error.rs", '=> "not_found"', '=> "not_founded"')
        r = self.check("error-codes")
        self.assertEqual(r.returncode, 1)
        self.assertIn("not_founded", r.stdout)
        self.assertIn("`not_found`", r.stdout)

    def test_doc_row_removed_is_red(self):
        self.edit("specs/contracts/errors.md", "| `cancelled` |", "| cancelled |")
        r = self.check("error-codes")
        self.assertEqual(r.returncode, 1)
        self.assertIn("cancelled", r.stdout)

    def test_cli_own_code_drift_is_red(self):
        self.edit("src/atg-cli/src/out.rs", '=> "interrupted"', '=> "aborted"')
        r = self.check("error-codes")
        self.assertEqual(r.returncode, 1)
        self.assertIn("aborted", r.stdout)

    def test_exit_code_drift_is_red(self):
        self.edit("src/atg-cli/src/out.rs", "E::NotFound { .. } => 6", "E::NotFound { .. } => 7")
        r = self.check("exit-codes")
        self.assertEqual(r.returncode, 1)
        self.assertIn("`not_found`：代码退出码 7", r.stdout)

    def test_exit_const_drift_is_red(self):
        self.edit("src/atg-cli/src/out.rs", "pub const EXIT_AUTH: u8 = 3;", "pub const EXIT_AUTH: u8 = 30;")
        r = self.check("exit-codes")
        self.assertEqual(r.returncode, 1)
        self.assertIn("auth_required", r.stdout)

    def test_cli_exit_drift_is_red(self):
        self.edit("src/atg-cli/src/out.rs", "Code::Usage => 2", "Code::Usage => 64")
        self.assertEqual(self.check("exit-codes").returncode, 1)

    def test_tuple_variant_code_is_not_skipped(self):
        # 上一轮实测：元组变体、带字段绑定的分支以前被正则悄悄跳过，五条全绿
        self.edit("src/atg-core/src/error.rs", 'Self::Cancelled => "cancelled",',
                  'Self::Cancelled => "cancelled",\n            Self::Quota(_) => "quota_exceeded",')
        r = self.check("error-codes")
        self.assertEqual(r.returncode, 1)
        self.assertIn("quota_exceeded", r.stdout)

    def test_field_binding_arm_is_not_skipped(self):
        self.edit("src/atg-core/src/error.rs", 'Self::Cancelled => "cancelled",',
                  'Self::Cancelled => "cancelled",\n            Self::Flood { secs } => "flood_wait",')
        r = self.check("error-codes")
        self.assertEqual(r.returncode, 1)
        self.assertIn("flood_wait", r.stdout)

    def test_unrecognised_arm_shape_is_unparsed_not_green(self):
        self.edit("src/atg-core/src/error.rs", 'Self::Cancelled => "cancelled",',
                  'Self::Cancelled => "cancelled",\n            other if false => "x",')
        r = self.check("error-codes")
        self.assertEqual(r.returncode, 1)
        self.assertIn("解析不出", r.stdout)

    def test_arithmetic_on_the_right_is_unparsed_not_green(self):
        # codex 019 门槛第 2 轮 #2：右边只认出前缀时以前照绿
        for old, new in (("Code::Usage => 2,", "Code::Usage => 2 + 1,"),
                         ("E::AuthRequired { .. } => EXIT_AUTH,", "E::AuthRequired { .. } => EXIT_AUTH + 1,"),
                         ("Code::Usage => 2,", "Code::Usage => 2\n                + 1,"),  # 门槛第 3 轮 #1：跨行续写
                         ("E::NotFound { .. } => 6,", "E::NotFound { .. } => 6 // 注释\n            + 1,")):
            self.tearDown()
            self.setUp()
            self.edit("src/atg-cli/src/out.rs", old, new)
            r = self.check("exit-codes")
            self.assertEqual(r.returncode, 1, old)
            self.assertIn("解析不出", r.stdout)

    def test_conflicting_rows_in_exit_table_are_red(self):
        # codex 019 门槛第 4 轮 #1：重复的码以前后行盖前行、照绿
        cases = (("| 2 | 用法 | `usage` |", "| 64 | 旧 | `usage` |\n| 2 | 用法 | `usage` |", "`usage` 同时被写成 [2, 64]"),
                 ("| 3 | 登录 | `auth_*` |", "| 3 | 登录 | `auth_*` |\n| 5 | 旧 | `auth_*` |", "`auth_required` 同时被写成 [3, 5]"),
                 ("| 3 | 登录 | `auth_*` |", "| 3 | 登录 | `auth_*` |\n| 5 | 旧 | `auth_required` |", "`auth_required` 同时被写成 [3, 5]"),
                 # 门槛第 5 轮 #1：不同前缀互相包含，两种行序都要红
                 ("| 3 | 登录 | `auth_*` |", "| 3 | 登录 | `auth_*` |\n| 5 | 拆分 | `auth_req*` |", "`auth_req*` 写 5，包住它的 `auth_*` 写 3"),
                 ("| 3 | 登录 | `auth_*` |", "| 5 | 拆分 | `auth_req*` |\n| 3 | 登录 | `auth_*` |", "`auth_req*` 写 5，包住它的 `auth_*` 写 3"))
        for old, new, want in cases:
            self.tearDown()
            self.setUp()
            self.edit("specs/contracts/cli.md", old, new)
            r = self.check("exit-codes")
            self.assertEqual(r.returncode, 1, want)
            self.assertIn(want, r.stdout)

    def test_success_row_is_checked(self):
        # codex 019 门槛第 7 轮 #1：成功行以前完全不查
        for old, new in (("| 0 | 成功 | — |", "| 99 | 成功 | — |"), ("| 0 | 成功 | — |\n", ""),
                         ("| 0 | 成功 | — |", "| 0 | 成功 | — |\n| 0 | 也成功 | — |"),
                         ("| 2 | 用法 | `usage` |", "| 0 | 用法 | `usage` |")):
            self.tearDown()
            self.setUp()
            self.edit("specs/contracts/cli.md", old, new)
            self.assertEqual(self.check("exit-codes").returncode, 1, repr(new))

    def test_dead_row_in_exit_table_is_red(self):
        self.edit("specs/contracts/cli.md", "| `not_found` |", "| `not_found` `gone_away` |")
        r = self.check("exit-codes")
        self.assertEqual(r.returncode, 1)
        self.assertIn("`gone_away` 在代码里没有", r.stdout)

    def test_spaced_version_example_is_still_checked(self):
        self.edit("specs/contracts/cli.md", '{"v":1,"kind":"message"', '{"v": 7, "kind":"message"')
        r = self.check("versions")
        self.assertEqual(r.returncode, 1)
        self.assertIn('"v":7', r.stdout)

    def test_new_subcommand_without_doc_is_red(self):
        self.edit("src/atg-cli/src/args.rs", "    Version,\n", "    Version,\n    /// 新\n    MarkRead(Rest),\n")
        r = self.check("subcommands")
        self.assertEqual(r.returncode, 1)
        self.assertIn("`mark-read`", r.stdout)

    def test_schema_version_bump_without_doc_is_red(self):
        self.edit("src/atg-core/src/tree/mod.rs", "pub const SCHEMA_V: u32 = 1;", "pub const SCHEMA_V: u32 = 2;")
        r = self.check("versions")
        self.assertEqual(r.returncode, 1)
        self.assertIn("SCHEMA_V = 2", r.stdout)

    def test_protocol_version_drift_is_red(self):
        self.edit("src/atg-core/src/ipc.rs", "pub const PROTOCOL_V: u32 = 0;", "pub const PROTOCOL_V: u32 = 1;")
        self.assertEqual(self.check("versions").returncode, 1)

    def test_atg_dir_without_manifest_is_red(self):
        os.makedirs(os.path.join(self.d, "src/atg-stt"))
        r = self.check("workspace")
        self.assertEqual(r.returncode, 1)
        self.assertIn("src/atg-stt/", r.stdout)

    def test_changed_members_rule_is_unparsed_not_green(self):
        self.edit("Cargo.toml", 'members = ["src/atg-*"]', 'members = ["src/*"]')
        r = self.check("workspace")
        self.assertEqual(r.returncode, 1)
        self.assertIn("解析不出", r.stdout)

    def test_unparseable_is_red_not_green(self):
        self.edit("src/atg-cli/src/out.rs", "pub fn core_exit(", "pub fn exit_for_core(")
        r = self.check("exit-codes")
        self.assertEqual(r.returncode, 1)
        self.assertIn("解析不出", r.stdout)


if __name__ == "__main__":
    unittest.main()
