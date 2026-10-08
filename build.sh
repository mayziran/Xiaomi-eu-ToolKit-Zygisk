#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_ROOT"

TARGET=aarch64-linux-android
ABI=arm64-v8a
LIB_NAME=libxiaomi_eu_toolkit_zygisk.so
MODULE_DIR="$PROJECT_ROOT/module"
OUT_SO="$MODULE_DIR/zygisk/$ABI.so"
ZIP_NAME="xiaomi-eu-toolkit-zygisk-$ABI.zip"

if [[ -d "$HOME/.cargo/bin" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi
if [[ -d "/usr/local/cargo/bin" ]]; then
    export PATH="/usr/local/cargo/bin:$PATH"
fi

find_ndk_home() {
    if [[ -n "${ANDROID_NDK_HOME:-}" && -d "${ANDROID_NDK_HOME}" ]]; then
        printf '%s\n' "${ANDROID_NDK_HOME}"
        return
    fi
    local roots=(
        "${ANDROID_HOME:-}"
        "${ANDROID_SDK_ROOT:-}"
        "$HOME/Android/Sdk"
        "/usr/local/lib/android/sdk"
    )
    local root candidate
    for root in "${roots[@]}"; do
        [[ -n "$root" && -d "$root/ndk" ]] || continue
        candidate="$(find "$root/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -1)"
        if [[ -n "$candidate" ]]; then
            printf '%s\n' "$candidate"
            return
        fi
    done
    echo "Android NDK not found. Set ANDROID_NDK_HOME." >&2
    exit 1
}

NDK_HOME="$(find_ndk_home)"

case "$(uname -s)" in
    Darwin) HOST_TAG=darwin-x86_64 ;;
    *)      HOST_TAG=linux-x86_64 ;;
esac

TOOLCHAIN="$NDK_HOME/toolchains/llvm/prebuilt/$HOST_TAG"

# NDK 里 clang 的目标名带 API level，不同版本带的等级不一样，按优先级找一个存在的。
CC=""
for api in 34 30 26 24 21; do
    candidate="$TOOLCHAIN/bin/aarch64-linux-android${api}-clang"
    if [[ -x "$candidate" ]]; then
        CC="$candidate"
        break
    fi
done
if [[ -z "$CC" ]]; then
    CC="$(find "$TOOLCHAIN/bin" -maxdepth 1 -name 'aarch64-linux-android[0-9]*-clang' 2>/dev/null | sort -V | head -1)"
fi
if [[ -z "$CC" || ! -x "$CC" ]]; then
    echo "NDK clang for $TARGET not found under $TOOLCHAIN/bin" >&2
    exit 1
fi

export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC"
export CC_aarch64_linux_android="$CC"
export AR_aarch64_linux_android="$TOOLCHAIN/bin/llvm-ar"

echo "==> NDK:  $NDK_HOME"
echo "==> CC:   $CC"
echo "==> cargo build --release --target $TARGET"

cargo build --release --target "$TARGET"

mkdir -p "$MODULE_DIR/zygisk"
cp "target/$TARGET/release/$LIB_NAME" "$OUT_SO"

# 打包：优先用 zip 命令，系统没装就退回 python3（保留可执行位）
pack_module() {
    local out="$1"
    rm -f "$out"
    if command -v zip >/dev/null 2>&1; then
        # 显式列顶层条目，不要用 "." —— 避免打包出 "./module.prop" 这种带前缀的条目名，
        # Magisk / KernelSU 解析 module.prop 时更稳。
        (cd "$MODULE_DIR" && zip -r -q "$out" module.prop customize.sh META-INF zygisk -x '*.DS_Store')
        return 0
    fi
    echo "==> 没找到 zip 命令，改用 python3 打包"
    python3 - "$MODULE_DIR" "$out" <<'PY'
import os, sys, zipfile

src, out = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as zf:
    for root, _dirs, files in os.walk(src):
        for name in sorted(files):
            if name == ".DS_Store":
                continue
            path = os.path.join(root, name)
            rel = os.path.relpath(path, src).replace(os.sep, "/")
            info = zipfile.ZipInfo(rel, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = (os.stat(path).st_mode & 0xFFFF) << 16
            with open(path, "rb") as fh:
                zf.writestr(info, fh.read())
PY
}

echo "==> packaging $ZIP_NAME"
pack_module "$PROJECT_ROOT/$ZIP_NAME"

echo "==> done"
ls -la "$OUT_SO"
ls -la "$PROJECT_ROOT/$ZIP_NAME"

# 把 zip 内容打出来：CI 日志里能看到最终结构，出问题一眼可查
echo "==> zip 内容（module.prop 必须在根目录）"
python3 - "$PROJECT_ROOT/$ZIP_NAME" <<'PY'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as z:
    names = z.namelist()
    for info in z.infolist():
        print(f"  {info.file_size:>8}  {oct(info.external_attr >> 16)}  {info.filename}")
print("  module.prop 在根目录:", "module.prop" in names)
PY
