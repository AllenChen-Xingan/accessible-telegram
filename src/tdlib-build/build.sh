#!/usr/bin/env bash
# 按锁定的提交拉 TDLib 源码，只编 tdjson（Release），装到 _build/<时间戳>/tdlib。
# 依据：issue #25、docs/plan/004_tdlib-patch.md 第三节。TDLib 源码不进库。
#
# 用法（在仓库任意位置跑都行）：
#   bash src/tdlib-build/build.sh          # 编一次（约 3–4 分钟，M 系列 Mac）；最后一行打印 LOCAL_TDLIB_PATH
#   bash src/tdlib-build/build.sh path     # 只打印最近一次编好的前缀（没有就报错、退出码 1）
#   bash src/tdlib-build/build.sh link     # 只把常驻软链接 ~/.atg-dev/tdlib 指到最近一次编好的前缀，不重编
#   export LOCAL_TDLIB_PATH="$(bash src/tdlib-build/build.sh path)"
#
# 需要：git、cmake、gperf、Homebrew 的 openssl@3（含静态库），以及开发签名身份（bash scripts/setup-dev-codesign.sh 检查并给步骤；
#   #42 起编完要给 libtdjson 签名）。缺了就报错退出，不自动安装。
# 源码来源：先从本地 resources/td（或环境变量 ATG_TDLIB_SRC 指的本地 TDLib 仓库，工作副本里没有 resources/ 时用）
#   只取锁定的那个提交（复制对象，不走网络），取不到再从 GitHub 拉这一个提交。
#
# 强化运行时（#42）：开发版签名开了 `--options runtime`，库验证只许载入苹果签的、或与程序同团队号签的库，
#   `DYLD_LIBRARY_PATH` 也被忽略。所以这里：① OpenSSL 静态链进 libtdjson（Homebrew 的 libssl / libcrypto 是临时签名、
#   没有团队号，动态链会被拒载）；② 编完用开发身份（scripts/dev-codesign-id.sh）给 libtdjson 签名，签在算 sha256 之前；
#   ③ 维护常驻、不带空格的软链接 ~/.atg-dev/tdlib → 最新前缀，程序里写的 rpath 指它（两个包共用的构建脚本
#   src/tdlib-build/rpath.rs；路径的正本是 src/tdlib-build/dev-link，#170）。不加 disable-library-validation：加了，同用户可写目录里被换掉的库也能进来。
#   依据：docs/experiments/exp005-keychain-acl/README.md「开强化运行时要做的」。
#
# 项目路径末尾有空格（state/memory/20261025-path-trailing-space.md）：TDLib 链接参数
# `-exported_symbols_list <源码目录>/…` 不加引号，会在空格处被拆开。所以路径带空格时，
# 源码、构建、前缀全走一个临时的、不带空格的软链接。CMakeCache.txt 记的是软链接路径，
# 软链接用完即删，所以别在旧构建目录里原地重编——每次跑都新建一个时间戳目录。
set -euo pipefail

# ── 锁定版本（改版本时三处一起改：这里、src/tdlib-rs 的 TDLIB_VERSION、docs/plan/004）──
TDLIB_VERSION="1.8.69"
TDLIB_COMMIT="ea3f69ce11aef458bfbb09869b809ede4447486f"
TDLIB_REMOTE="https://github.com/tdlib/td.git"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
dylib="libtdjson.${TDLIB_VERSION}.dylib"

die() { echo "✗ $*" >&2; exit 1; }

# 最近一次编好的、能用的前缀：_build/YYYYMMDD-HHMMSS/tdlib，有对应版本的 dylib，同级 build-info.txt 提交号对得上、
# 且是 #42 以后的构建（OpenSSL 静态链、签过名）。#42 以前的动态链构建在强化运行时下会被拒载，path 和 link 都不认它
latest_prefix() {
  local d i
  local dirs=("${root}"/_build/[0-9]*-[0-9]*/)   # 通配按名字升序 = 按时间升序；倒着找
  for (( i = ${#dirs[@]} - 1; i >= 0; i-- )); do
    d="${dirs[i]%/}"
    if [[ -f "${d}/tdlib/lib/${dylib}" && -f "${d}/build-info.txt" ]] \
       && grep -q "^commit=${TDLIB_COMMIT}$" "${d}/build-info.txt" \
       && grep -q '^openssl_link=static$' "${d}/build-info.txt"; then
      echo "${d}/tdlib"
      return 0
    fi
  done
  return 1
}

# 常驻软链接相对家目录的路径：唯一正本是 src/tdlib-build/dev-link（程序的 rpath 也从它拼，见 rpath.rs，#170）。
# 读出来是空、绝对路径、含 .. 或空白的，报错退出，不悄悄用默认值
dev_link_rel() {
  local f="${root}/src/tdlib-build/dev-link" rel
  [[ -r "${f}" ]] || die "读不了 ${f}"
  # 只去掉行尾换行；中间有空白不去掉、直接拒（不悄悄改成别的路径）
  rel="$(head -n 1 "${f}")"
  [[ -n "${rel}" && "${rel}" != /* && "${rel}" != *..* && "${rel}" != *[[:space:]]* \
     && "$(wc -l <"${f}" | tr -d ' ')" -le 1 ]] || die "${f} 的内容不对（要一行相对家目录的路径，不带空白）"
  printf '%s' "${rel}"
}
# 常驻软链接（#42）：程序的 rpath 指 ~/<dev-link>/lib。那里已有东西但不是软链接 → 不碰，报错退出
dev_link="${HOME}/$(dev_link_rel)"
update_link() {
  local target="$1"
  if [[ -e "${dev_link}" && ! -L "${dev_link}" ]]; then
    die "${dev_link} 已存在且不是软链接，不覆盖；先看看它是什么"
  fi
  mkdir -p "$(dirname "${dev_link}")"
  ln -sfn "${target}" "${dev_link}"
  echo "→ 软链接：${dev_link} → ${target}"
}

no_build="还没有编好的 TDLib ${TDLIB_VERSION}（#42 以前编的不算：那是动态链 OpenSSL、没签名的，要重编；先跑 bash src/tdlib-build/build.sh）"
if [[ "${1:-}" == "path" && $# -eq 1 ]]; then
  latest_prefix || die "${no_build}"
  exit 0
fi
if [[ "${1:-}" == "link" && $# -eq 1 ]]; then
  p="$(latest_prefix)" || die "${no_build}"
  update_link "${p}"
  exit 0
fi
[[ $# -eq 0 ]] || die "用法：bash src/tdlib-build/build.sh [path|link]"

# ── 依赖检查（不自动安装）──
[[ "$(uname -s)" == "Darwin" ]] || die "目前只写了 macOS 的步骤"
missing=()
for tool in git cmake gperf brew; do
  command -v "${tool}" >/dev/null 2>&1 || missing+=("${tool}")
done
[[ ${#missing[@]} -eq 0 ]] || die "缺工具：${missing[*]}（brew install cmake gperf；本脚本不自动装）"
openssl_root="$(brew --prefix openssl@3 2>/dev/null || true)"
[[ -n "${openssl_root}" && -f "${openssl_root}/lib/libssl.3.dylib" ]] \
  || die "缺 Homebrew 的 openssl@3（brew install openssl@3；本脚本不自动装）"
[[ -f "${openssl_root}/lib/libssl.a" && -f "${openssl_root}/lib/libcrypto.a" ]] \
  || die "openssl@3 里没有静态库 libssl.a / libcrypto.a（#42 要把 OpenSSL 静态链进 libtdjson）"
# 签名身份先找好（找不到就别白编 3 分钟）
sign_id="$(bash "${root}/scripts/dev-codesign-id.sh")" || die "找不到开发签名身份（见上一行）"
arch="$(uname -m)"
# Rosetta 终端里 uname -m 是 x86_64，而 /opt/homebrew 的 openssl 是 arm64——编完 3 分钟才在链接时失败，先拦
lipo -archs "${openssl_root}/lib/libssl.3.dylib" 2>/dev/null | tr ' ' '\n' | grep -qx "${arch}" \
  || die "openssl@3 的架构（$(lipo -archs "${openssl_root}/lib/libssl.3.dylib" 2>/dev/null)）和本机终端 ${arch} 不一致（Rosetta 终端？）"
jobs="$(sysctl -n hw.ncpu)"

# ── 目录：_build/<时间戳>/{td-src,build,tdlib}，一次构建一个 ──
stamp="$(date +%Y%m%d-%H%M%S)"
out="${root}/_build/${stamp}"
mkdir -p "${root}/_build"
mkdir "${out}" 2>/dev/null || die "${out} 已存在（同一秒跑了两次？），过一秒再跑"

# 路径带空格 → 走临时软链接
link_dir=""
work="${out}"
if [[ "${out}" == *" "* ]]; then
  link_dir="$(mktemp -d "${TMPDIR:-/tmp}/atg-tdlib.XXXXXX")"
  [[ "${link_dir}" != *" "* ]] || die "临时目录也带空格：${link_dir}"
  ln -s "${out}" "${link_dir}/b"
  work="${link_dir}/b"
fi
cleanup() { [[ -z "${link_dir}" ]] || rm -rf "${link_dir}"; }
trap cleanup EXIT

start=$(date +%s)
echo "→ 构建目录：${out}"

# ── 拉源码 ──
local_td="${ATG_TDLIB_SRC:-${root}/resources/td}"
# 只按提交号取，不看本地仓的分支在哪：resources/td 以后 pull 了别的版本也不受影响；取不到就退回 GitHub
git init --quiet "${work}/td-src"
if [[ -d "${local_td}/.git" ]] \
   && git -C "${work}/td-src" fetch --quiet --depth 1 "file://${local_td}" "${TDLIB_COMMIT}" 2>/dev/null; then
  echo "→ 源码：本地 resources/td"
else
  echo "→ 源码：${TDLIB_REMOTE}"
  git -C "${work}/td-src" fetch --quiet --depth 1 "${TDLIB_REMOTE}" "${TDLIB_COMMIT}" || die "拉不到 TDLib ${TDLIB_COMMIT}"
fi
git -C "${work}/td-src" -c advice.detachedHead=false checkout --quiet FETCH_HEAD
[[ "$(git -C "${work}/td-src" rev-parse HEAD)" == "${TDLIB_COMMIT}" ]] || die "源码提交不对"
grep -q "project(TDLib VERSION ${TDLIB_VERSION} " "${work}/td-src/CMakeLists.txt" \
  || die "源码里的版本号不是 ${TDLIB_VERSION}"

# ── 配置 + 编译（只编 tdjson）──
echo "→ cmake 配置（Release · ${arch}）"
cmake -S "${work}/td-src" -B "${work}/build" -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="${work}/tdlib" \
  -DOPENSSL_ROOT_DIR="${openssl_root}" \
  -DOPENSSL_USE_STATIC_LIBS=TRUE \
  -DCMAKE_OSX_ARCHITECTURES="${arch}" >"${out}/cmake-configure.log" 2>&1 \
  || die "cmake 配置失败，见 ${out}/cmake-configure.log"
echo "→ 编译 tdjson（-j${jobs}，日志 ${out}/cmake-build.log）"
cmake --build "${work}/build" --target tdjson -j"${jobs}" >"${out}/cmake-build.log" 2>&1 \
  || die "编译失败，见 ${out}/cmake-build.log 末尾"

# ── 手工布置前缀（tdjson 单目标不能 cmake --install）──
prefix="${out}/tdlib"
mkdir -p "${prefix}/lib" "${prefix}/include/td/telegram"
cp -P "${out}"/build/libtdjson*.dylib "${prefix}/lib/"
cp "${out}/td-src/td/telegram/td_json_client.h" "${out}/td-src/td/telegram/td_log.h" \
   "${out}/build/td/telegram/tdjson_export.h" "${prefix}/include/td/telegram/"
[[ -f "${prefix}/lib/${dylib}" && -L "${prefix}/lib/libtdjson.dylib" ]] || die "前缀里缺 ${dylib} 或 libtdjson.dylib 软链接"

# ── 核 OpenSSL 已静态链入、再签名（签名会改文件字节，所以在算 sha256 之前）──
# 先拿到 otool 的输出、确认它成功，再匹配：otool 失败时不能当成「没有动态依赖」放行（codex 063 第 1 轮 #2）
deps="$(otool -L "${prefix}/lib/${dylib}")" || die "otool -L 读不了 ${dylib}"
[[ "${deps}" == *"${dylib}"* ]] || die "otool -L 的输出认不出（没列出 ${dylib} 自己）"
if [[ "${deps}" == *libssl* || "${deps}" == *libcrypto* ]]; then
  die "${dylib} 仍动态依赖 OpenSSL（OPENSSL_USE_STATIC_LIBS 没生效），见 ${out}/cmake-configure.log"
fi
# codesign 失败时的报错里可能带身份名，只留一句说明
codesign -s "${sign_id}" --force "${prefix}/lib/${dylib}" 2>/dev/null \
  || die "给 ${dylib} 签名失败（身份 SHA-1 前 8 位 ${sign_id:0:8}）"
# 只核「有团队号」，不把团队号写进文件或打印出来（同 dev-sign.sh：身份只露 SHA-1 前 8 位）
team="$(codesign -dv "${prefix}/lib/${dylib}" 2>&1 | sed -n 's/^TeamIdentifier=//p')"
[[ -n "${team}" && "${team}" != "not set" ]] || die "${dylib} 签完没有团队号"
unset team

secs=$(( $(date +%s) - start ))
sha="$(shasum -a 256 "${prefix}/lib/${dylib}" | cut -d' ' -f1)"
cat >"${out}/build-info.txt" <<EOF
version=${TDLIB_VERSION}
commit=${TDLIB_COMMIT}
arch=${arch}
openssl=${openssl_root}
openssl_link=static
signed=dev_identity_with_team
seconds=${secs}
dylib_sha256=${sha}
EOF

echo "✓ TDLib ${TDLIB_VERSION} 编好了（${secs} 秒）· ${dylib} sha256 ${sha}"
echo "  OpenSSL 已静态链入；用开发身份签过名（带团队号）"
update_link "${prefix}"
old_dirs=("${root}"/_build/[0-9]*-[0-9]*/)
if (( ${#old_dirs[@]} > 1 )); then
  echo "  _build/ 里共 ${#old_dirs[@]} 个构建目录（每个约 300 MB）；旧的可以删，删之前确认没人在用"
fi
echo "  设环境变量："
echo "export LOCAL_TDLIB_PATH=\"${prefix}\""
