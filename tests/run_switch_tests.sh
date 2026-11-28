#!/usr/bin/env bash
# 批量运行所有 switch 相关测试
# 用法: ./run_switch_tests.sh [axc_path]
# axc_path 默认为 dist/axc.exe

set -uo pipefail

PROJECT_DIR="D:/Project/axlang"
AXC="${1:-$PROJECT_DIR/dist/axc.exe}"

PASS=0
FAIL=0
TOTAL=0

run_case() {
    local ax_file="$1"
    local out_file="${ax_file%.ax}.out"
    local test_name
    test_name=$(basename "$ax_file")
    TOTAL=$((TOTAL + 1))

    if [ ! -f "$ax_file" ]; then
        echo "  [SKIP] $test_name (source not found)"
        return
    fi
    if [ ! -f "$out_file" ]; then
        echo "  [SKIP] $test_name (.out not found)"
        return
    fi

    local actual
    actual=$("$AXC" run "$ax_file" 2>/dev/null)
    local rc=$?

    if [ $rc -ne 0 ]; then
        echo "  [FAIL] $test_name (axc run exited with code $rc)"
        FAIL=$((FAIL + 1))
        return
    fi

    local expected
    expected=$(cat "$out_file")

    # 规范化：去除尾部空行
    actual=$(printf '%s' "$actual" | sed '/^[[:space:]]*$/d')
    expected=$(printf '%s' "$expected" | sed '/^[[:space:]]*$/d')

    if [ "$actual" = "$expected" ]; then
        echo "  [PASS] $test_name"
        PASS=$((PASS + 1))
    else
        echo "  [FAIL] $test_name (output mismatch)"
        echo "    expected:"
        echo "$expected" | sed 's/^/      /'
        echo "    actual:"
        echo "$actual" | sed 's/^/      /'
        FAIL=$((FAIL + 1))
    fi
}

run_sema_case() {
    local ax_file="$1"
    local expect_code="$2"
    local test_name
    test_name=$(basename "$ax_file")
    TOTAL=$((TOTAL + 1))

    if [ ! -f "$ax_file" ]; then
        echo "  [SKIP] $test_name (source not found)"
        return
    fi

    local output
    output=$("$AXC" check "$ax_file" 2>&1)
    local rc=$?

    if [ $rc -eq 0 ]; then
        echo "  [FAIL] $test_name (expected error $expect_code, but axc check succeeded)"
        FAIL=$((FAIL + 1))
        return
    fi

    if echo "$output" | grep -q "$expect_code"; then
        echo "  [PASS] $test_name (got $expect_code)"
        PASS=$((PASS + 1))
    else
        echo "  [FAIL] $test_name (expected $expect_code, got:)"
        echo "$output" | sed 's/^/      /'
        FAIL=$((FAIL + 1))
    fi
}

echo "========================================"
echo "  switch 全覆盖测试"
echo "========================================"
echo ""

echo "--- Run 测试（功能验证）---"
for f in "$PROJECT_DIR/tests/run/switch_test.ax" \
         "$PROJECT_DIR/tests/run/switch_go.ax" \
         "$PROJECT_DIR/tests/run/switch_expr.ax" \
         "$PROJECT_DIR/tests/run/switch_type.ax" \
         "$PROJECT_DIR/tests/run/switch_enum.ax" \
         "$PROJECT_DIR/tests/run/switch_nested.ax" \
         "$PROJECT_DIR/tests/run/switch_only_default.ax"; do
    run_case "$f"
done

echo ""
echo "--- Sema 测试（错误验证）---"
run_sema_case "$PROJECT_DIR/tests/sema/S-20.ax" "E4028"
run_sema_case "$PROJECT_DIR/tests/sema/S-21.ax" "E4031"
run_sema_case "$PROJECT_DIR/tests/sema/S-22.ax" "E4029"
run_sema_case "$PROJECT_DIR/tests/sema/S-23.ax" "E4030"
run_sema_case "$PROJECT_DIR/tests/sema/S-24.ax" "E4030"
run_sema_case "$PROJECT_DIR/tests/sema/S-25.ax" "E4040"
run_sema_case "$PROJECT_DIR/tests/sema/S-26.ax" "E4040"
run_sema_case "$PROJECT_DIR/tests/sema/S-27.ax" "E4028"
run_sema_case "$PROJECT_DIR/tests/sema/S-28.ax" "E4030"
run_sema_case "$PROJECT_DIR/tests/sema/S-29.ax" "E4040"
run_sema_case "$PROJECT_DIR/tests/sema/S-30.ax" "E4028"
run_sema_case "$PROJECT_DIR/tests/sema/S-31.ax" "E4030"

echo ""
echo "========================================"
echo "  结果: $PASS / $TOTAL 通过"
if [ $FAIL -eq 0 ]; then
    echo "  全部通过 ✓"
else
    echo "  $FAIL 个失败 ✗"
fi
echo "========================================"

exit $FAIL
