//! 类型表示、规范名、可赋值性（规格书 §7）——全项目唯一的类型判定入口。
//!
//! `Type` 用 enum + `Rc` 共享；`PartialEq/Eq/Hash` 直接派生，可作 HashMap key。
//! sema 判类型、backend 生成 C 时都只调用本模块的 canonical / assignable，
//! 不允许各自另写判定逻辑（改可赋值性规则只改这里）。

use std::rc::Rc;

/// 具名类型 defid：用户类型用 id（u32），内建具名类型（Row/File/Arena/Rounding）用名字。
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum DefId {
    Id(u32),
    Name(&'static str),
}

/// 语言的全部类型。结构相等即可作 HashMap key；构造一律走下方 `t_*`
/// 工厂函数（保证 `Rc` 复用与内建类型的单一实例）。
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum TyS {
    Bool,
    Void,
    Float,
    /// 32 位单精度浮点（与 64 位 `float` 并存，C 层对应 `float`）
    Float32,
    Decimal,
    Str,
    Object,
    Null,
    Never,
    /// 已报错占位类型：与任何类型相容，不再派生新错误
    Unknown,
    Range,
    Error,
    /// (bits, signed)
    Int(i32, bool),
    Array(Ty, Option<i64>),
    Map(Ty, Ty),
    Tuple(Vec<Ty>),
    Func(Vec<Ty>, Ty),
    Nullable(Ty),
    Result(Ty),
    Named(DefId, Vec<Ty>),
    Enum(u32),
    Record(Vec<(String, Ty)>),
    Param(String),
    ArenaRef(Ty),
    /// 语义检查期占位（泛型展开等过程中瞬时存在），不应逃逸到 backend
    Meta(String),
    /// 模块值类型（import 进来的模块名）
    ModuleT(String),
    /// 内建具名类型（Row / File / Arena / Rounding）
    BuiltinT(String),
    /// `super` / 父类引用占位
    ParentT,
}

pub type Ty = Rc<TyS>;

pub fn t_bool() -> Ty {
    Rc::new(TyS::Bool)
}
pub fn t_void() -> Ty {
    Rc::new(TyS::Void)
}
pub fn t_float() -> Ty {
    Rc::new(TyS::Float)
}
pub fn t_float32() -> Ty {
    Rc::new(TyS::Float32)
}
pub fn t_decimal() -> Ty {
    Rc::new(TyS::Decimal)
}
pub fn t_string() -> Ty {
    Rc::new(TyS::Str)
}
pub fn t_object() -> Ty {
    Rc::new(TyS::Object)
}
pub fn t_null() -> Ty {
    Rc::new(TyS::Null)
}
pub fn t_never() -> Ty {
    Rc::new(TyS::Never)
}
pub fn t_unknown() -> Ty {
    Rc::new(TyS::Unknown)
}
pub fn t_range() -> Ty {
    Rc::new(TyS::Range)
}
pub fn t_error() -> Ty {
    Rc::new(TyS::Error)
}

/// bytes 类型：byte[]（uint8_t[]）数组，对标 Go []byte
pub fn t_bytes() -> Ty {
    t_array(t_int_b(8, false), None)
}

/// File 类型：ax_file* 句柄，对标 Go os.File
pub fn t_file() -> Ty {
    t_named(DefId::Name("File"), Vec::new())
}
pub fn t_int() -> Ty {
    Rc::new(TyS::Int(64, true))
}
pub fn t_uint() -> Ty {
    Rc::new(TyS::Int(64, false))
}

pub fn t_int_b(bits: i32, signed: bool) -> Ty {
    Rc::new(TyS::Int(bits, signed))
}

pub fn t_array(elem: Ty, length: Option<i64>) -> Ty {
    Rc::new(TyS::Array(elem, length))
}

pub fn t_map(k: Ty, v: Ty) -> Ty {
    Rc::new(TyS::Map(k, v))
}

pub fn t_nullable(inner: Ty) -> Ty {
    match &*inner {
        TyS::Nullable(_) => inner,
        TyS::Never => t_null(),
        _ => Rc::new(TyS::Nullable(inner)),
    }
}

pub fn t_result(inner: Ty) -> Ty {
    Rc::new(TyS::Result(inner))
}

pub fn t_named(defid: DefId, args: Vec<Ty>) -> Ty {
    Rc::new(TyS::Named(defid, args))
}

pub fn t_fn(params: Vec<Ty>, ret: Ty) -> Ty {
    Rc::new(TyS::Func(params, ret))
}

pub fn is_int(t: &Ty) -> bool {
    matches!(&**t, TyS::Int(..))
}

pub fn is_numeric(t: &Ty) -> bool {
    matches!(&**t, TyS::Int(..) | TyS::Float | TyS::Float32 | TyS::Decimal)
}

/// 剥掉一层可空：`T?` → `T`，其余原样返回。
/// 用于"这个字面量能不能落在这个位置"这类判定——目标写 `decimal?` 时，
/// 字面量的落地规则应看内层 `decimal`，两者是同一件事。
pub fn strip_nullable(t: &Ty) -> &Ty {
    match &**t {
        TyS::Nullable(inner) => inner,
        _ => t,
    }
}

/// 字符串插值 `{x}` 当前能显示的类型：标量、`string`、`enum`，以及由它们构成的可空值
/// （有值显示值、`null` 显示 `null`）。
///
/// 复合类型（数组 / `map` / 元组）已实现默认显示，可直接插值；
/// class / struct / func 的默认显示仍由 `Printable.ToString` 提供，未实现则 E5006。
/// `Unknown`/`Error`/`Never` 是已报错占位类型，返回 `true` 以免派生错误风暴。
pub fn interp_supported(t: &Ty) -> bool {
    match &**t {
        TyS::Int(..) | TyS::Float | TyS::Float32 | TyS::Decimal | TyS::Bool | TyS::Str | TyS::Enum(_) => true,
        TyS::Array(..) | TyS::Map(..) | TyS::Tuple(..) => true,
        TyS::Nullable(i) => interp_supported(i),
        TyS::Unknown | TyS::Error | TyS::Never => true,
        _ => false,
    }
}

pub fn is_signed_int(t: &Ty) -> bool {
    matches!(&**t, TyS::Int(_, true))
}

/// (lo, hi)，用 i128 承载 uint64 上界。
pub fn int_range(t: &Ty) -> (i128, i128) {
    match &**t {
        TyS::Int(bits, signed) => {
            let bits = *bits as u32;
            if *signed {
                (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
            } else {
                (0, (1i128 << bits) - 1)
            }
        }
        _ => (0, 0),
    }
}

pub fn unnullable(t: &Ty) -> Ty {
    match &**t {
        TyS::Nullable(inner) => inner.clone(),
        _ => t.clone(),
    }
}

pub fn is_nullable(t: &Ty) -> bool {
    matches!(&**t, TyS::Nullable(_) | TyS::Null)
}

pub fn builtin_scalar(name: &str) -> Option<Ty> {
    Some(match name {
        "bool" => t_bool(),
        "byte" => t_int_b(8, false),
        "int" => t_int(),
        "int64" => t_int(),
        "uint" => t_uint(),
        "int8" => t_int_b(8, true),
        "int16" => t_int_b(16, true),
        "int32" => t_int_b(32, true),
        "uint8" => t_int_b(8, false),
        "uint16" => t_int_b(16, false),
        "uint32" => t_int_b(32, false),
        "float" => t_float(),
        "float64" => t_float(),
        "float32" => t_float32(),
        "decimal" => t_decimal(),
        "string" => t_string(),
        "object" => t_object(),
        "void" => t_void(),
        _ => return None,
    })
}

pub fn is_builtin_scalar(name: &str) -> bool {
    builtin_scalar(name).is_some()
}

pub fn fnv1a32(s: &str) -> u32 {
    let mut h: u32 = 0x811C9DC5;
    for b in s.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}

fn cn(k: &TyS) -> Option<&'static str> {
    Some(match k {
        TyS::Bool => "bool",
        TyS::Float => "float",
        TyS::Float32 => "float32",
        TyS::Decimal => "decimal",
        TyS::Str => "string",
        TyS::Object => "object",
        TyS::Void => "void",
        TyS::Range => "range",
        TyS::Error => "error",
        TyS::Null => "null",
        TyS::Never => "never",
        _ => return None,
    })
}

/// namer：具名类型 defid -> 名字（sema 提供）。
pub type Namer<'a> = &'a dyn Fn(DefId) -> String;

pub fn canonical(t: &Ty, namer: Option<Namer>) -> String {
    let k: &TyS = t;
    match k {
        TyS::Int(bits, signed) => {
            if (*bits, *signed) == (64, true) {
                "int".to_string()
            } else if (*bits, *signed) == (64, false) {
                "uint".to_string()
            } else {
                format!("{}{}", if *signed { "int" } else { "uint" }, bits)
            }
        }
        _ => {
            if let Some(s) = cn(k) {
                return s.to_string();
            }
            match k {
                TyS::Unknown => "unknown".to_string(),
                TyS::Array(inner, len) => {
                    let i = canonical(inner, namer);
                    match len {
                        None => format!("arr_{}", i),
                        Some(n) => format!("fix_{}_{}", i, n),
                    }
                }
                TyS::Map(a, b) => {
                    format!("map_{}_{}", canonical(a, namer), canonical(b, namer))
                }
                TyS::Tuple(elems) => format!(
                    "tup_{}",
                    elems
                        .iter()
                        .map(|x| canonical(x, namer))
                        .collect::<Vec<_>>()
                        .join("_")
                ),
                TyS::Func(ps, ret) => format!(
                    "fn_{}_{}",
                    ps.iter()
                        .map(|x| canonical(x, namer))
                        .collect::<Vec<_>>()
                        .join("_"),
                    canonical(ret, namer)
                ),
                TyS::Nullable(inner) => format!("opt_{}", canonical(inner, namer)),
                TyS::Result(inner) => format!("res_{}", canonical(inner, namer)),
                TyS::Record(fields) => format!(
                    "rec_{}",
                    fields
                        .iter()
                        .map(|(n, ft)| format!("{}_{}", n, canonical(ft, namer)))
                        .collect::<Vec<_>>()
                        .join("_")
                ),
                TyS::Named(defid, args) => {
                    let nm = match defid {
                        DefId::Name(s) => s.to_string(),
                        DefId::Id(id) => match namer {
                            Some(f) => f(DefId::Id(*id)),
                            None => format!("{}", id),
                        },
                    };
                    if !args.is_empty() {
                        format!(
                            "{}_g_{}",
                            nm,
                            args.iter()
                                .map(|a| canonical(a, namer))
                                .collect::<Vec<_>>()
                                .join("_")
                        )
                    } else {
                        nm
                    }
                }
                TyS::Enum(id) => {
                    let nm = match namer {
                        Some(f) => f(DefId::Id(*id)),
                        None => format!("{}", id),
                    };
                    format!("enum_{}", nm)
                }
                TyS::Param(n) => format!("T_{}", n),
                TyS::ArenaRef(inner) => format!("aref_{}", canonical(inner, namer)),
                _ => format!("{:?}", k),
            }
        }
    }
}

/// 诊断里给人看的名字。
pub fn ty_str(t: &Ty, namer: Option<Namer>) -> String {
    let k: &TyS = t;
    match k {
        TyS::Int(bits, signed) => {
            if (*bits, *signed) == (64, true) {
                "int".to_string()
            } else if (*bits, *signed) == (64, false) {
                "uint".to_string()
            } else {
                format!("{}{}", if *signed { "int" } else { "uint" }, bits)
            }
        }
        _ => {
            if let Some(s) = cn(k) {
                return s.to_string();
            }
            match k {
                TyS::Unknown => "unknown".to_string(),
                TyS::Array(inner, len) => match len {
                    None => format!("{}[]", ty_str(inner, namer)),
                    Some(n) => format!("{}[{}]", ty_str(inner, namer), n),
                },
                TyS::Map(a, b) => {
                    format!("map<{}, {}>", ty_str(a, namer), ty_str(b, namer))
                }
                TyS::Tuple(elems) => format!(
                    "({})",
                    elems
                        .iter()
                        .map(|x| ty_str(x, namer))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                TyS::Func(ps, ret) => format!(
                    "func({}) {}",
                    ps.iter()
                        .map(|x| ty_str(x, namer))
                        .collect::<Vec<_>>()
                        .join(", "),
                    ty_str(ret, namer)
                ),
                TyS::Nullable(inner) => format!("{}?", ty_str(inner, namer)),
                TyS::Result(inner) => format!("result<{}>", ty_str(inner, namer)),
                TyS::Record(fields) => format!(
                    "{{{}}}",
                    fields
                        .iter()
                        .map(|(n, ft)| format!("{}: {}", n, ty_str(ft, namer)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                TyS::Named(defid, args) => {
                    let nm = match defid {
                        DefId::Name(s) => s.to_string(),
                        DefId::Id(id) => match namer {
                            Some(f) => f(DefId::Id(*id)),
                            None => format!("{}", id),
                        },
                    };
                    if !args.is_empty() {
                        format!(
                            "{}<{}>",
                            nm,
                            args.iter()
                                .map(|a| ty_str(a, namer))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    } else {
                        nm
                    }
                }
                TyS::Enum(id) => match namer {
                    Some(f) => f(DefId::Id(*id)),
                    None => format!("{}", id),
                },
                TyS::Param(n) => n.clone(),
                TyS::ArenaRef(inner) => format!("ArenaRef<{}>", ty_str(inner, namer)),
                _ => format!("{:?}", k),
            }
        }
    }
}

// ---------------------------------------------------------------- 可赋值

/// sema 在赋值检查时回调的最小接口。
pub trait AssignCtx {
    fn def_of(&self, id: u32) -> bool; // 是否存在该 typedef
    fn def_kind(&self, id: u32) -> &'static str; // "class" | "interface" | ...
    fn is_subclass(&self, sub_id: u32, super_id: u32) -> bool;
    fn implements(&self, class_id: u32, iface_id: u32) -> bool;
}

#[derive(Clone, Debug, PartialEq)]
pub enum AssignResult {
    Yes,
    Conv(&'static str),
    No(&'static str),
}

pub fn assignable(src: &Ty, dst: &Ty, ctx: &dyn AssignCtx, allow_unbox: bool) -> AssignResult {
    if src == dst {
        return AssignResult::Yes;
    }
    match (&**src, &**dst) {
        (TyS::Unknown, _) | (_, TyS::Unknown) => return AssignResult::Yes,
        (TyS::Never, _) => return AssignResult::Yes,
        _ => {}
    }
    if let TyS::Nullable(inner) = &**dst {
        match &**src {
            TyS::Null => return AssignResult::Yes,
            TyS::Nullable(sinner) => {
                let r = assignable(sinner, inner, ctx, allow_unbox);
                return match r {
                    AssignResult::No(_) => AssignResult::No("可空元素类型不匹配"),
                    _ => AssignResult::Yes,
                };
            }
            _ => {
                let r = assignable(src, inner, ctx, allow_unbox);
                return match r {
                    AssignResult::Yes => AssignResult::Conv("nullwrap"),
                    AssignResult::Conv(k) => AssignResult::Conv(k),
                    AssignResult::No(_) => AssignResult::No("需要先判空"),
                };
            }
        }
    }
    if matches!(&**src, TyS::Null) {
        return AssignResult::No("null 只能赋给可空类型");
    }
    if matches!(&**src, TyS::Nullable(_)) {
        // 可空 -> 非可空：不可隐式
        return AssignResult::No("可空值不能直接赋给非可空类型，请先判空");
    }
    if matches!(&**dst, TyS::Object) {
        if boxable(src) {
            return AssignResult::Conv("box");
        }
        return AssignResult::No("该类型不能装箱到 object");
    }
    if matches!(&**src, TyS::Object) {
        if allow_unbox && unboxable(dst) {
            return AssignResult::Conv("unbox");
        }
        return AssignResult::No("object 需要先落到具体类型（自动解箱只允许 P1..P8 位置）");
    }
    if let (TyS::Named(did, dargs), TyS::Named(sid, sargs)) = (&**dst, &**src) {
        if let (DefId::Id(did), DefId::Id(sid)) = (did, sid) {
            let dinfo_kind = ctx.def_kind(*did);
            let sinfo_kind = ctx.def_kind(*sid);
            if dargs == sargs
                && dinfo_kind == "interface"
                && sinfo_kind == "class"
                && ctx.implements(*sid, *did)
            {
                return AssignResult::Yes;
            }
            if dargs.is_empty() && sargs.is_empty() && dinfo_kind == "class" && sinfo_kind == "class"
            {
                if ctx.is_subclass(*sid, *did) {
                    return AssignResult::Yes;
                }
                if ctx.is_subclass(*did, *sid) {
                    return AssignResult::Conv("downcast");
                }
            }
            if dinfo_kind == "interface"
                && sinfo_kind == "class"
                && ctx.implements(*sid, *did)
            {
                return AssignResult::Yes;
            }
            return AssignResult::No("类型不匹配");
        }
        return AssignResult::No("类型不匹配");
    }
    if let (TyS::Record(_), TyS::Record(_)) = (&**dst, &**src) {
        if src == dst {
            return AssignResult::Yes;
        }
        return AssignResult::No("record 字段不一致");
    }
    if matches!(&**dst, TyS::Record(_)) && matches!(&**src, TyS::Named(..)) {
        return AssignResult::No("struct 不能隐式转 record");
    }
    if matches!(&**dst, TyS::Named(..)) && matches!(&**src, TyS::Record(_)) {
        return AssignResult::Conv("record2struct");
    }
    if let (TyS::Result(dinner), TyS::Result(sinner)) = (&**dst, &**src) {
        let r = assignable(sinner, dinner, ctx, allow_unbox);
        return match r {
            AssignResult::No(_) => AssignResult::No("result 元素类型不一致"),
            _ => AssignResult::Yes,
        };
    }
    if let (TyS::Map(dk, dv), TyS::Map(sk, sv)) = (&**dst, &**src) {
        if dk == sk && dv == sv {
            return AssignResult::Yes;
        }
        return AssignResult::No("map 键值类型不一致");
    }
    if let (TyS::Array(de, dl), TyS::Array(se, sl)) = (&**dst, &**src) {
        if de == se && dl == sl && dl.is_some() {
            return AssignResult::Yes;
        }
        if dl.is_none() && sl.is_none() && de == se {
            return AssignResult::Yes;
        }
        if dl.is_none() && sl.is_some() {
            return AssignResult::No("定长数组不能直接赋给可变长数组（C-16），请逐元素复制");
        }
        return AssignResult::No("数组类型不一致");
    }
    if let (TyS::Tuple(dt), TyS::Tuple(st)) = (&**dst, &**src) {
        if dt.len() == st.len() && dt.iter().zip(st.iter()).all(|(a, b)| a == b) {
            return AssignResult::Yes;
        }
        return AssignResult::No("元组元素类型不一致");
    }
    if let (TyS::Func(..), TyS::Func(..)) = (&**dst, &**src) {
        if dst == src {
            return AssignResult::Yes;
        }
        return AssignResult::No("函数签名不一致");
    }
    if let (TyS::ArenaRef(di), TyS::ArenaRef(si)) = (&**dst, &**src) {
        return if di == si {
            AssignResult::Yes
        } else {
            AssignResult::No("ArenaRef 元素类型不一致")
        };
    }
    // float32 → float 隐式扩大（无损，C 层 float 自动提升为 double）
    if matches!(&**src, TyS::Float32) && matches!(&**dst, TyS::Float) {
        return AssignResult::Conv("f32_to_f64");
    }
    if is_numeric(src) && is_numeric(dst) {
        return AssignResult::No("");
    }
    AssignResult::No("类型不匹配")
}

pub const BOXABLE_SCALARS: [&str; 6] = ["bool", "int", "float", "float32", "decimal", "string"];

pub fn boxable(t: &Ty) -> bool {
    match &**t {
        TyS::Bool | TyS::Int(..) | TyS::Float | TyS::Float32 | TyS::Decimal | TyS::Str => true,
        TyS::Array(..) | TyS::Map(..) => true,
        TyS::Named(..) => true, // class 实例（struct 由 sema 再拦）
        _ => false,
    }
}

pub fn unboxable(t: &Ty) -> bool {
    match &**t {
        TyS::Bool | TyS::Int(..) | TyS::Float | TyS::Float32 | TyS::Decimal | TyS::Str => true,
        TyS::Array(..) | TyS::Map(..) | TyS::Nullable(_) => true,
        TyS::Named(..) => true,
        _ => false,
    }
}
