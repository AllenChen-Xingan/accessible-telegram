#!/usr/bin/env bash
# 按锁定的提交拉 TDLib 源码，只编 tdjson（Release），装到 _build/<时间戳>/tdlib。
# 依据：issue #25、docs/plan/004_tdlib-patch.md 第三节。TDLib 源码不进库。
#
# 用法（在仓库任意位置跑都行）：
#   bash src/tdlib-build/build.sh          # 编一次（约 3–4 分钟，M 系列 Mac）；最后一行打印 LOCAL_TDLIB_PATH
#   bash src/tdlib-build/build.sh path     # 只打印最近一次编好的前缀（没有就报错、退出码 1）
#   export LOCAL_TDLIB_PATH="$(bash src/tdlib-build/build.sh path)"
#
# 需要：git、cmake、gperf、Homebrew 的 openssl@3。缺了就报错退出，不自动安装。
# 源码来源：先从本地 resources/td 只取锁定的那个提交（复制对象，不走网络），取不到再从 GitHub 拉这一个提交。
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

# 最近一次编好的前缀：_build/YYYYMMDD-HHMMSS/tdlib，里面有对应版本的 dylib 和 build-info.txt
latest_prefix() {
  local d i
  local dirs=("${root}"/_build/[0-9]*-[0-9]*/)   # 通配按名字升序 = 按时间升序；倒着找
  for (( i = ${#dirs[@]} - 1; i >= 0; i-- )); do
    d="${dirs[i]%/}"
    if [[ -f "${d}/tdlib/lib/${dylib}" && -f "${d}/build-info.txt" ]] \
       && grep -q "^commit=${TDLIB_COMMIT}$" "${d}/build-info.txt"; then
      echo "${d}/tdlib"
      return 0
    fi
  done
  return 1
}

if [[ "${1:-}" == "path" && $# -eq 1 ]]; then
  latest_prefix || die "还没有编好的 TDLib ${TDLIB_VERSION}（先跑 bash src/tdlib-build/build.sh）"
  exit 0
fi
[[ $# -eq 0 ]] || die "用法：bash src/tdlib-build/build.sh [path]"

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
local_td="${root}/resources/td"
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

secs=$(( $(date +%s) - start ))
sha="$(shasum -a 256 "${prefix}/lib/${dylib}" | cut -d' ' -f1)"
cat >"${out}/build-info.txt" <<EOF
version=${TDLIB_VERSION}
commit=${TDLIB_COMMIT}
arch=${arch}
openssl=${openssl_root}
seconds=${secs}
dylib_sha256=${sha}
EOF

echo "✓ TDLib ${TDLIB_VERSION} 编好了（${secs} 秒）· ${dylib} sha256 ${sha}"
echo "  库按绝对路径依赖 ${openssl_root}；卸了 openssl@3 它就打不开"
old_dirs=("${root}"/_build/[0-9]*-[0-9]*/)
if (( ${#old_dirs[@]} > 1 )); then
  echo "  _build/ 里共 ${#old_dirs[@]} 个构建目录（每个约 300 MB）；旧的可以删，删之前确认没人在用"
fi
echo "  设环境变量："
echo "export LOCAL_TDLIB_PATH=\"${prefix}\""
