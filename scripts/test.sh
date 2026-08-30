#!/usr/bin/env bash
# 统一测试入口：本地与 CI 共用（CI 的 test job 就是 `bash scripts/test.sh`）。
#
# 用法：
#   scripts/test.sh              # 完整：fmt --check → clippy → cargo test
#   scripts/test.sh --quick      # 快速冒烟：cargo test，跳过慢的用例
#   scripts/test.sh --release    # 完整流程，全部用 release profile
#   scripts/test.sh --coverage   # cargo llvm-cov（阈值 70，与 CI 一致）
set -euo pipefail
cd "$(dirname "$0")/.."

MODE="default"
PROFILE_ARGS=()
for arg in "$@"; do
    case "$arg" in
        --quick) MODE="quick" ;;
        --release) PROFILE_ARGS=(--release) ;;
        --coverage) MODE="coverage" ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

section() { printf '\n\033[1m===== %s =====\033[0m\n' "$1"; }

# 最慢的 4 个真实程序用例（--quick 跳过它们）
SLOW_SKIPS=(--skip programs::nboyer --skip programs::puzzle --skip programs::ack --skip programs::mceval)

case "$MODE" in
    quick)
        section "cargo test (quick: skipping slow program cases)"
        cargo test ${PROFILE_ARGS[@]+"${PROFILE_ARGS[@]}"} -- "${SLOW_SKIPS[@]}"
        ;;
    coverage)
        section "cargo llvm-cov (fail-under-lines 70)"
        if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
            echo "cargo-llvm-cov not installed; install with:"
            echo "  cargo install cargo-llvm-cov   # or: brew install cargo-llvm-cov"
            exit 2
        fi
        cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info \
            --fail-under-lines 70
        ;;
    *)
        section "cargo fmt --check"
        cargo fmt --check
        section "cargo clippy --all-targets -- -D warnings"
        cargo clippy --all-targets -- -D warnings
        section "cargo test"
        cargo test ${PROFILE_ARGS[@]+"${PROFILE_ARGS[@]}"}
        ;;
esac

section "OK"
