#!/usr/bin/env python3
# generate_checklist.py —— 从 STDLIB-STATUS.json 生成 STDLIB-CHECKLIST.md
#
# 用法：python generate_checklist.py
#
# 功能：
#   1. 读取 STDLIB-STATUS.json（人工维护的主数据源）
#   2. 将所有模块函数名转换为小写 C 风格（snake_case，如 ReadLine → read_line）
#   3. 将转换后的名称回写到 STDLIB-STATUS.json，保持主数据源与清单一致
#   4. 生成人类可读的 STDLIB-CHECKLIST.md（按模块逐函数清点）

import json
import re
import os
from pathlib import Path

BASE = Path(__file__).resolve().parent
JSON_PATH = BASE / "STDLIB-STATUS.json"
MD_PATH = BASE / "STDLIB-CHECKLIST.md"


def pascal_to_snake(name: str) -> str:
    """将 PascalCase / camelCase 转换为小写下划线分隔（C 风格）。
    
    例如：
        ReadLine        → read_line
        MarshalIndent   → marshal_indent
        CompareAndSwapInt → compare_and_swap_int
        Getpagesize     → getpagesize
        NowMs           → now_ms
    """
    s1 = re.sub(r'([a-z])([A-Z])', r'\1_\2', name)
    return s1.lower()


def convert_names_in_obj(obj):
    """递归遍历 JSON 对象，将所有 name 字段转换为小写 C 风格。"""
    if isinstance(obj, dict):
        for key, value in obj.items():
            if key == "name" and isinstance(value, str):
                obj[key] = pascal_to_snake(value)
            else:
                convert_names_in_obj(value)
    elif isinstance(obj, list):
        for item in obj:
            convert_names_in_obj(item)


def load_json():
    with open(JSON_PATH, "r", encoding="utf-8") as f:
        return json.load(f)


def save_json(data):
    with open(JSON_PATH, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)
        f.write("\n")


def _key_to_kind(key):
    """将 JSON 中的函数字段名映射为人类可读的类型标签。"""
    mapping = {
        "functions": "模块级函数",
        "static_functions": "静态方法",
        "instance_functions": "实例方法",
        "module_functions": "模块函数",
    }
    return mapping.get(key, key)


def collect_functions(module):
    """从模块对象中收集所有函数条目（functions / static_functions / instance_functions / module_functions / types[].methods）。

    每个返回条目保证包含以下字段（缺失时补默认值）：
      name, signature, status, c_counterpart(对标 C), go_counterpart(对标 Go),
      function(函数功能), description(说明), kind(函数类型标签)
    """
    funcs = []
    for key in ("functions", "static_functions", "instance_functions", "module_functions"):
        if key in module and isinstance(module[key], list):
            kind_label = _key_to_kind(key)
            for f in module[key]:
                entry = _normalize_func(f)
                entry["kind"] = kind_label
                funcs.append(entry)
    if "types" in module and isinstance(module["types"], list):
        for t in module["types"]:
            tname = t.get("name", "?")
            tkind = t.get("kind", "struct")
            methods = t.get("methods", [])
            if isinstance(methods, list):
                kind_label = f"{tname}（{tkind}）方法"
                for m in methods:
                    if isinstance(m, dict):
                        entry = _normalize_func(m)
                        entry["name"] = f"{tname}.{m.get('name', '')}"
                        entry["kind"] = kind_label
                        entry["type_name"] = tname
                        funcs.append(entry)
                    elif isinstance(m, str):
                        # 向后兼容：字符串形式的方法名
                        entry = _normalize_func({"name": m, "signature": "", "status": "implemented", "note": ""})
                        entry["name"] = f"{tname}.{m}"
                        entry["kind"] = kind_label
                        entry["type_name"] = tname
                        funcs.append(entry)
    return funcs


def _normalize_func(f):
    """补全函数条目缺失的对标/功能/说明字段（返回新 dict，不修改原对象）。"""
    return {
        "name": f.get("name", ""),
        "signature": f.get("signature", ""),
        "status": f.get("status", "implemented"),
        "c_counterpart": f.get("c_counterpart", "—"),
        "go_counterpart": f.get("go_counterpart", "—"),
        "function": f.get("function", "—"),
        "description": f.get("description", "—"),
        "kind": f.get("kind", "模块级函数"),
        "builtin": f.get("builtin", False),
        "builtin_plan": f.get("builtin_plan", "keep"),
        "usage_example": f.get("usage_example", "—"),
    }


def ensure_func_fields(f):
    """原地补充函数条目缺失的四个新字段，使主数据源 JSON 持久化列名。

    注意：`function`（函数功能）与 `note` 不做自动映射，避免两列重复。
    缺失时均以 "—" 占位，待人工填充。
    """
    f.setdefault("c_counterpart", "—")
    f.setdefault("go_counterpart", "—")
    f.setdefault("function", "—")
    f.setdefault("description", "—")


def status_mark(status):
    """将状态字符串转换为清单中的标记。"""
    m = {
        "implemented": "✅ 已落地",
        "partial": "🔄 部分落地",
        "not_implemented": "⬜ 未落地",
        "planned": "⬜ 计划中",
    }
    return m.get(status, f"⬜ {status}")


def generate_markdown(data):
    """生成 STDLIB-CHECKLIST.md 的完整内容。"""
    lines = []

    lines.append("# axlang 标准库清单（对标 Go 标准库 pkg.go.dev/std）")
    lines.append("")
    lines.append("本文档列出 axlang 内建模块与 Go 标准库（基于 <https://pkg.go.dev/std，Go> 1.25.1）及 C 标准库的对标关系，作为标准库落地进度的检视参考。目标：像 Go/C 标准库一样丰富，最大限度满足开发需要。")
    lines.append("")
    lines.append("> **跨项目铁律**：所有涉及及生成的 C 代码 / GCC 源码，均必须使用标准 C（C11），不得使用任何 GNU/Clang 扩展（如 `__int128`、`__builtin_*_overflow`、`__attribute__((unused))` 等），以便跨平台编译。")
    lines.append("")
    lines.append("> **落地规范（每项必须完成）**：")
    lines.append("> 1. **axrt.h**：新增 C 函数声明（标准 C11，无 GNU/Clang 扩展）")
    lines.append("> 1. **axrt.c**：新增 C 函数实现")
    lines.append("> 1. **rust/src/sema/mod.rs**：`BUILTIN_MODULES` 新增模块名（若新模块）")
    lines.append("> 1. **rust/src/sema/expr.rs**：`module_sig` 新增签名（模块级函数）；或 `rust/src/sema/member.rs` `intrinsic()` 表新增签名（类型级/实例方法）")
    lines.append("> 1. **rust/src/backend/codegen_expr.rs**：`module_call` / `method_call` 新增代码生成分支")
    lines.append("> 1. **rust/src/backend/mod.rs**：如需新增 result 类型 C typedef")
    lines.append("> 1. **tests/run/*.ax + .out**：新增测试用例，覆盖正例和负例")
    lines.append("> 1. **回归验证**：`python tests/runtests.py` 零新增回归")
    lines.append("> 1. **提交推送**：Git commit + **GitHub 推 `main` 分支**、**Gitee 推 `master` 分支**（两者不可混淆；分别执行 `git push github main` 和 `git push origin main:master`）")
    lines.append("> 1. **更新文档**：本清单状态标记，`MEMORY.md` 同步更新，追加工作日志")
    lines.append("")
    lines.append("> **生成说明**：本文档由 `generate_checklist.py` 从 `STDLIB-STATUS.json` 自动生成，请勿手动编辑。函数名均为小写 C 风格（snake_case）。")
    lines.append("")
    lines.append("---")
    lines.append("")
    lines.append("## 状态图例")
    lines.append("")
    lines.append("| 标记 | 含义 |                                             |")
    lines.append("| --------- | -------------------------------------------------- |")
    lines.append("| `✅ 已落地`   | axrt.h/axrt.c 实现 + sema/codegen 绑定 + 测试用例齐备 + 回归通过 |")
    lines.append("| `🔄 部分落地` | 核心函数已有，部分扩展函数待补                                    |")
    lines.append("| `⬜ 未落地`   | 尚未开始                                               |")
    lines.append("")
    lines.append("---")
    lines.append("")
    lines.append("## 优先级说明")
    lines.append("")
    lines.append("| 优先级 | 含义 |                                      |")
    lines.append("| ----------- | ---------------------------------------- |")
    lines.append("| **P0 核心基础** | 程序能跑、能做基本 I/O、字符串、数学、文件操作；axlang 最小可用标准库 |")
    lines.append("| **P1 常用扩展** | 日常开发常用功能，与已落地模块形成闭环                      |")
    lines.append("| **P2 进阶功能** | 并发、排序、编解码、更多数学、更多类型工具                    |")
    lines.append("| **P3 持续丰富** | 补充完善、边缘场景、对标更多 Go/C 函数                   |")
    lines.append("")
    lines.append("---")
    lines.append("")

    # 按优先级排序：P0, P1, P2, P3
    priority_order = {"P0": 0, "P1": 1, "P2": 2, "P3": 3}
    modules = sorted(data["modules"], key=lambda m: priority_order.get(m.get("priority", "P3"), 3))

    # ================================================================
    # 收集所有函数，按 builtin_plan 分类（仅用于统计，不再用于分表）
    # ================================================================
    builtin_keep_funcs = []      # 编译器内建，长期保留（builtin_plan="keep"）
    stdlib_funcs = []            # 标准库函数（builtin=false）
    builtin_migrate_funcs = []   # 计划从内建迁移到标准库（builtin_plan="migrate"）

    all_funcs = []  # 统一函数列表，用于生成单一表格
    for mod in modules:
        mod_name = mod["name"]
        go_counterpart = mod.get("go_counterpart", "—")
        funcs = collect_functions(mod)
        for f in funcs:
            f["_mod_name"] = mod_name
            f["_mod_go_counterpart"] = go_counterpart
            builtin = f.get("builtin", False)
            plan = f.get("builtin_plan", "keep")
            if builtin and plan == "keep":
                builtin_keep_funcs.append(f)
                f["_type_label"] = "内建库"
            else:
                # builtin=false（已迁移标准库）或 builtin_plan="migrate"（计划迁移），
                # 均归入“标准库”类别显示
                if builtin and plan == "migrate":
                    builtin_migrate_funcs.append(f)
                else:
                    stdlib_funcs.append(f)
                f["_type_label"] = "标准库"
            all_funcs.append(f)

    # 按模块名 → 函数名 排序，便于查找
    all_funcs.sort(key=lambda f: (f["_mod_name"], f["name"]))

    # ================================================================
    # 第一部分：编译器内建函数说明
    # ================================================================
    lines.append("## 一、编译器内建函数（长期保留在 Rust 编译器端）")
    lines.append("")
    lines.append("以下函数涉及编译器魔法（变参、编译期占位符核对、泛型类型推导、运行时反射等），**无法**用纯 ax 语法独立实现，长期保留在编译器 Rust 端，不迁移到 ax_stdlib。")
    lines.append("")

    # ================================================================
    # 第二部分：标准库函数说明
    # ================================================================
    lines.append("## 二、标准库函数（ax_stdlib）")
    lines.append("")
    lines.append("以下函数已迁移至 `axc/std/` 下的 ax 语法源码，通过 `extern \"C\"` 调用 axrt C 原语实现。")
    lines.append("")

    # ================================================================
    # 统一表格：所有函数按模块名排序，类型列仅显示“内建库”或“标准库”
    # ================================================================
    lines.append("### ax 函数清单（统一表格）")
    lines.append("")
    lines.append("| ax.函数 | 类型 | 签名 | 用法举例 | 对标 C | 对标 Go | 函数功能 | 说明 | 状态 |")
    lines.append("| ------ | ------ | ------ | ------ | ------ | ------ | ------ | ------ | ----- |")
    for f in all_funcs:
        name = f["name"]
        sig = f["signature"]
        mod_name = f["_mod_name"]
        type_label = f["_type_label"]
        mark = status_mark(f["status"])
        disp_name = name if mod_name == "builtin" else f"{mod_name}.{name}"
        lines.append(f"| `{disp_name}` | {type_label} | `{sig}` | {f['usage_example']} | {f['c_counterpart']} | {f['go_counterpart']} | {f['function']} | {f['description']} | {mark} |")
    lines.append("")

    lines.append("---")
    lines.append("")
    lines.append("## 三、当前进度汇总")
    lines.append("")
    lines.append("| 阶段 | 已完成 | 待完成 |")
    lines.append("| --- | --- | --- |")

    total_mods = len(modules)
    implemented_mods = sum(1 for m in modules if m.get("status") == "implemented")
    pending_mods = total_mods - implemented_mods

    total_funcs = len(builtin_keep_funcs) + len(stdlib_funcs) + len(builtin_migrate_funcs)
    implemented_funcs = sum(1 for f in builtin_keep_funcs + stdlib_funcs + builtin_migrate_funcs if f.get("status") == "implemented")

    lines.append(f"| 编译器内建函数（保留） | {len(builtin_keep_funcs)} 个 | 长期保留在编译器 Rust 端 |")
    lines.append(f"| 标准库函数（已迁移） | {len(stdlib_funcs)} 个 | 已在 axc/std/ 下以 ax 源码实现 |")
    lines.append(f"| 标准库函数（待迁移） | {len(builtin_migrate_funcs)} 个 | 计划从编译器内建迁移到 ax_stdlib |")
    lines.append(f"| 已落地函数 | {implemented_funcs} / {total_funcs} | — |")
    lines.append("")
    lines.append("")
    lines.append(f"*更新于 {__import__('datetime').date.today()}，基于 STDLIB-STATUS.json 自动生成*")
    lines.append("")

    return "\n".join(lines)


def main():
    data = load_json()

    # 转换所有函数名为小写 C 风格
    convert_names_in_obj(data)

    # 为每个函数条目补充四个新列名（对标 C / 对标 Go / 函数功能 / 说明），
    # 使主数据源 JSON 持久化这些列；缺失时以 "—" 占位，现有 note 映射为函数功能。
    for mod in data["modules"]:
        for key in ("functions", "static_functions", "instance_functions", "module_functions"):
            if key in mod and isinstance(mod[key], list):
                for f in mod[key]:
                    ensure_func_fields(f)
        if "types" in mod and isinstance(mod["types"], list):
            for t in mod["types"]:
                if "methods" in t and isinstance(t["methods"], list):
                    for m in t["methods"]:
                        if isinstance(m, dict):
                            ensure_func_fields(m)

    # 回写 JSON（主数据源保持一致）
    save_json(data)

    # 生成 Markdown
    md = generate_markdown(data)
    with open(MD_PATH, "w", encoding="utf-8") as f:
        f.write(md)

    print(f"✅ 已更新 {JSON_PATH}")
    print(f"✅ 已生成 {MD_PATH}")


if __name__ == "__main__":
    main()