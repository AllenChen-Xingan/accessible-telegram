#!/usr/bin/env bash
# 给人在「终端」App 里亲手跑：用你自己的账号，在**正式服务器**上登录一次（正式档案 main）。
#
# 它只做三件事：编好带 TDLib 的 atg-cli 并签名 → 运行交互登录 `atg-cli login` → 用 `atg-cli auth state` 报告结果。
# 手机号、验证码、二步验证密码都由你在终端里输入，不回显，不进命令行、不进文件、不进日志；脚本里不写任何号码。
# 不碰会话、不发消息、不读消息；**不登出**（登录留着给你用），末尾告诉你怎么退出。
#
# 用法（在「终端」App 里，不要在 tmux 里——tmux 窗格的内容能被别的程序读走）：
#   bash "src/atg-cli/live/login-main.sh"
# 在仓库任意位置跑都行。项目路径末尾有空格，脚本里全程加了引号。
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$root"

# ── 前置：要真终端、不在 tmux 里 ──
if [[ ! -t 0 || ! -t 1 ]]; then
  echo "要在「终端」App 里直接运行（读不到终端）。" >&2
  exit 2
fi
if [[ -n "${TMUX:-}" ]]; then
  echo "你在 tmux 里：正式号请在 tmux 以外的终端窗口登录（tmux 窗格的内容能被别的程序读走）。" >&2
  exit 2
fi
if [[ ! -f .env.local ]]; then
  echo "缺 .env.local（要有 ATG_API_ID、ATG_API_HASH）。" >&2
  exit 2
fi

# ── 编译、签名 ──
echo "第 1 步：编译带 TDLib 的 atg-cli（第一次要几分钟）……"
LOCAL_TDLIB_PATH="$(bash src/tdlib-build/build.sh path)"
export LOCAL_TDLIB_PATH
cargo build -p atg-cli --features tdlib --quiet
bin="$root/target/debug/atg-cli"
# 钥匙串按签名认程序：用本人的 Apple Development 身份与固定标识签，读本地库钥匙才不弹框（见 src/atg-core/src/keystore.rs 头部）
if ! bash scripts/dev-sign.sh "$bin"; then
  echo "签名失败：跑一次 bash scripts/setup-dev-codesign.sh 看开发签名身份是否就绪。" >&2
  exit 2
fi
export DYLD_LIBRARY_PATH="$LOCAL_TDLIB_PATH/lib"

# ── 登录 ──
echo
echo "第 2 步：登录正式服务器（档案 main）。按提示输入手机号（带国家码）、验证码、二步验证密码；输入时屏幕上不显示。"
echo "随时按 Ctrl+C 退出。"
echo
set +e
"$bin" --format text login
login_status=$?
set -e

# ── 结果 ──
echo
echo "第 3 步：现在的登录状态——"
"$bin" --format text auth state || true

echo
if [[ $login_status -eq 0 ]]; then
  echo "登录完成，登录状态留在本机（档案 main）。"
else
  echo "登录没有完成（退出码 $login_status），上面那行说了原因。"
fi
printf '要退出登录：DYLD_LIBRARY_PATH=%q %q logout （会让你输入 yes 确认）\n' "$DYLD_LIBRARY_PATH" "$bin"
exit "$login_status"
