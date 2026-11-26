#!/usr/bin/env python3
import json

JSON_PATH = "STDLIB-STATUS.json"

with open(JSON_PATH, "r", encoding="utf-8") as f:
    data = json.load(f)

def make_func(name, signature, kind, status, c_cp, go_cp, func_desc, desc, example):
    return {
        "name": name,
        "signature": signature,
        "kind": kind,
        "status": status,
        "c_counterpart": c_cp,
        "go_counterpart": go_cp,
        "function": func_desc,
        "description": desc,
        "builtin": True,
        "builtin_plan": "keep",
        "usage_example": example
    }

# ===== array：添加缺失的 15 个内建方法 =====
for mod in data["modules"]:
    if mod["name"] == "array":
        missing = [
            make_func("len", "() -> int", "instance", "implemented", "—", "len", "返回数组元素个数", "对标 C sizeof/arr_len，Go 内置 len()；返回元素数量", "arr.len()"),
            make_func("at", "(index: int) -> T?", "instance", "implemented", "—", "—", "按索引访问数组元素，越界返回 null", "安全访问数组元素，越界返回 T? 可选类型", "arr.at(0)"),
            make_func("contains", "(elem: T) -> bool", "instance", "implemented", "—", "—", "判断数组是否包含指定元素", "线性查找，返回是否存在指定元素", "arr.contains(5)"),
            make_func("index", "(elem: T) -> int", "instance", "implemented", "—", "—", "返回元素首次出现的索引，未找到返回 -1", "线性查找元素索引", "arr.index(5)"),
            make_func("sort", "() -> void", "instance", "implemented", "qsort", "sort.Ints", "对数组元素升序排序", "对标 C qsort，Go sort.Ints；原地排序", "arr.sort()"),
            make_func("sort", "(cmp) -> void", "instance", "implemented", "qsort_r", "sort.Slice", "按自定义比较函数对数组排序", "支持自定义比较函数，但 axlang 不支持闭包，实际受限", "arr.sort(func(a, b int) bool { ... })"),
            make_func("reverse", "() -> void", "instance", "implemented", "—", "sort.Reverse", "将数组元素反转", "原地反转数组元素顺序", "arr.reverse()"),
            make_func("clone", "() -> T[]", "instance", "implemented", "memcpy", "—", "返回数组的深拷贝副本", "返回独立的新数组副本，原数组不受影响", "arr.clone()"),
            make_func("insert", "(index: int, elem: T) -> void", "instance", "implemented", "memmove", "append", "在指定索引处插入元素", "在索引处插入元素，后续元素后移", "arr.insert(0, 1)"),
            make_func("fill", "(elem: T) -> void", "instance", "implemented", "memset", "—", "用指定值填充所有元素", "将数组所有元素设为指定值", "arr.fill(0)"),
            make_func("resize", "(n: int) -> void", "instance", "implemented", "realloc", "—", "调整数组长度", "不足时零初始化，超出时截断", "arr.resize(10)"),
            make_func("merge", "(other: T[]) -> void", "instance", "implemented", "memcpy", "append", "将另一个数组合并到当前数组", "将其他数组的所有元素追加到末尾", "arr.merge(other)"),
            make_func("min", "() -> T", "instance", "implemented", "—", "—", "返回数组中的最小值", "仅适用于可比较类型（int/float 等）", "arr.min()"),
            make_func("max", "() -> T", "instance", "implemented", "—", "—", "返回数组中的最大值", "仅适用于可比较类型（int/float 等）", "arr.max()"),
            make_func("sum", "() -> T", "instance", "implemented", "—", "—", "返回数组所有元素之和", "仅适用于数值类型（int/float 等）", "arr.sum()"),
        ]
        mod["functions"].extend(missing)
        print(f"array: 添加了 {len(missing)} 个内建方法")

# ===== map：添加缺失的 4 个内建方法 =====
for mod in data["modules"]:
    if mod["name"] == "map":
        missing = [
            make_func("len", "() -> int", "instance", "implemented", "—", "len", "返回 map 的键值对数量", "对标 C 无直接对应，Go 内置 len()", "m.len()"),
            make_func("get", "(key: K) -> V?", "instance", "implemented", "—", "—", "按键获取 map 值，未找到返回 null", "安全访问 map 元素，键不存在返回 V? 可选类型", "m.get(\"key\")"),
            make_func("keys", "() -> K[]", "instance", "implemented", "—", "maps.Keys", "返回 map 中所有键的切片", "C 标准库无 map 类型，Go 对标 maps.Keys；返回 K[] 类型，顺序不固定", "m.keys()"),
            make_func("has", "(key: K) -> bool", "instance", "implemented", "—", "—", "判断 map 是否包含指定键", "检查 map 中是否存在指定键", "m.has(\"key\")"),
        ]
        mod["functions"].extend(missing)
        print(f"map: 添加了 {len(missing)} 个内建方法")

# ===== queue：添加缺失的 3 个内建方法 =====
for mod in data["modules"]:
    if mod["name"] == "queue":
        missing = [
            make_func("push", "(v: T) -> void", "instance", "implemented", "—", "—", "将元素入队", "向队列尾部添加元素", "q.push(v)"),
            make_func("pop", "() -> T?", "instance", "implemented", "—", "—", "将元素出队", "从队列头部移除并返回元素，空队列返回 null", "q.pop()"),
            make_func("close", "() -> void", "instance", "implemented", "—", "—", "关闭队列", "关闭队列，后续无法入队", "q.close()"),
        ]
        mod["functions"].extend(missing)
        print(f"queue: 添加了 {len(missing)} 个内建方法")

# ===== decimal：添加缺失的 3 个内建方法 =====
for mod in data["modules"]:
    if mod["name"] == "decimal":
        missing = [
            make_func("round", "(digits: int, rounding: Rounding) -> decimal", "instance", "implemented", "—", "—", "按指定精度和舍入模式四舍五入", "对标 C 无直接对应，需自定义 decimal 实现", "d.round(2, Rounding.HalfUp)"),
            make_func("divide", "(other: decimal, rounding: Rounding) -> decimal", "instance", "implemented", "—", "—", "高精度除法，支持自定义舍入模式", "对标 C 无直接对应，需自定义 decimal 实现", "d.divide(e, Rounding.HalfUp)"),
            make_func("to_string", "() -> string", "instance", "implemented", "—", "—", "将 decimal 转换为字符串表示", "对标 C sprintf，Go fmt.Sprintf", "d.to_string()"),
        ]
        mod["functions"].extend(missing)
        print(f"decimal: 添加了 {len(missing)} 个内建方法")

with open(JSON_PATH, "w", encoding="utf-8") as f:
    json.dump(data, f, ensure_ascii=False, indent=2)
    f.write("\n")

print("\n所有模块修复完成，JSON 已保存")
