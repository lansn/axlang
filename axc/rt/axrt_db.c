/* axrt_db.c -- axlang 运行时模块：数据库查询（axlang 特有，内建模块 db）
 * ax_db_* 和 ax_row_* 操作数据库查询结果。
 * db 模块为编译器内建，不在 std/ 目录下，但 C 运行时单独文件便于定位。
 */
#include "axrt.h"
#include "axrt_internal.h"

/* ---------------- db 模块 ---------------- */

/* db.query(sql, arg)：查询数据库，返回 Row 映射。
 * 夹具来自环境变量 AX_DB_FIXTURE，形如 "id=1001,name=柴犬粮,price=19.99,stock=10"；
 * 命中的行照原样返回，未命中返回空行（n=0）。 */
ax_row ax_db_query(ax_str sql, ax_str arg) {
    (void)sql;
    ax_row r; r.kv = NULL; r.n = 0;
    const char* fx = getenv("AX_DB_FIXTURE");
    if (!fx) return r;
    ax_str s = ax_env_utf8("AX_DB_FIXTURE");
    int64_t n = 0;
    ax_str* parts = ax_str_split(s, ax_str_from_cstr(","), &n);
    if (n == 0) return r;
    /* id 必须匹配 arg；没有 id 字段时视为匹配 */
    int found_id = 0, matched = 0;
    for (int64_t i = 0; i < n; i++) {
        ax_str p = parts[i];
        int64_t eq = -1;
        for (int64_t k = 0; k < p.len; k++) if (p.p[k] == '=') { eq = k; break; }
        if (eq < 0) continue;
        ax_str key = ax_str_from_bytes(p.p, eq);
        ax_str val = ax_str_from_bytes(p.p + eq + 1, p.len - eq - 1);
        if (ax_str_eq(key, ax_str_from_cstr("id"))) {
            found_id = 1;
            if (ax_str_eq(val, arg)) matched = 1;
        }
    }
    if (found_id && !matched) return r;
    r.kv = (ax_str*)ax_alloc((int64_t)sizeof(ax_str) * 2 * n);
    r.n = n;
    for (int64_t i = 0; i < n; i++) {
        ax_str p = parts[i];
        int64_t eq = -1;
        for (int64_t k = 0; k < p.len; k++) if (p.p[k] == '=') { eq = k; break; }
        if (eq < 0) { r.kv[i*2] = p; r.kv[i*2+1] = ax_str_empty(); continue; }
        r.kv[i*2] = ax_str_from_bytes(p.p, eq);
        r.kv[i*2+1] = ax_str_from_bytes(p.p + eq + 1, p.len - eq - 1);
    }
    return r;
}

/* db.query(...) 的 result<Row> 形态（§11.3 / RUN-1）。 */
ax_res_row ax_db_query_result(ax_str sql, int64_t arg) {
    ax_res_row r;
    memset(&r, 0, sizeof r);
    if (!getenv("AX_DB_FIXTURE")) {
        r.tag = 0;
        r.u.err = ax_err_new(1, AX_S("no db fixture (set AX_DB_FIXTURE)"));
        return r;
    }
    char buf[32];
    snprintf(buf, sizeof buf, "%" PRId64, (int64_t)arg);
    ax_str a = ax_str_from_cstr(buf);
    /* 未命中返回空行（tag=1,n=0）：调用方的 row.empty() 才是"查无此记录"的出口，
       与 §16.10 RUN-3 一致。 */
    r.tag = 1;
    r.u.ok = ax_db_query(sql, a);
    return r;
}

/* row.Empty()：判断行是否为空（n == 0） */
int ax_row_empty(ax_row r) { return r.n == 0; }

/* row.Get(key)：根据键获取值，未找到返回空字符串 */
ax_str ax_row_get(ax_row r, ax_str key) {
    for (int64_t i = 0; i < r.n; i++) {
        if (ax_str_eq(r.kv[i*2], key)) return r.kv[i*2+1];
    }
    return ax_str_empty();
}
