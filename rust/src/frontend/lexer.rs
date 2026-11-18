//! 词法分析（规格书 §3，L-1…L-24）。
//!
//! 要点：
//!   * 位置一律由**字符索引**换算（码点计列），`\r\n` 先规范化。
//!   * 数值只做形状校验（L-11），范围检查留给常量求值。
//!   * 字符串在词法层片段化为 Text / Interp（L-18），插值片段保留原文与绝对位置，
//!     由解析器再词法化——这样才能处理 `"{m["k"]}"` 这种嵌套。
//!
//! Rust 版把源码转成 `Vec<char>` 再用索引操作：列号按码点算是语言规定（D-05），
//! 直接按字节索引会在中文注释/字符串上算错列，而错列会让诊断位置与规格书不符。

use crate::diag::{Diags, Note};

pub const KEYWORDS: &[&str] = &[
    "var", "const", "class", "struct", "interface", "extend",
    "implement", "override", "parent", "this", "enum", "type",
    "if", "else", "for", "while", "in", "match", "return",
    "try", "catch", "finally", "throw", "defer", "import", "public", "protected", "private",
    "static", "new", "as", "break", "continue", "constructor",
    "extern", "unsafe",
    "true", "false", "null",
];

/// 保留类型名：词法上是 IDENT，语义上是根模块内建类型（E3018 用）。
pub const RESERVED_TYPES: &[&str] = &[
    "bool", "int", "int8", "int16", "int32", "uint", "uint8", "uint16",
    "uint32", "float", "decimal", "string", "object", "void", "result",
    "error", "func", "map", "Self", "range", "Rounding", "Arena", "ArenaRef",
];

/// 长运算符必须在前（最大匹配）。
pub const OPS: &[&str] = &[
    "..=", "..", "?.", "??", "<<", ">>", "<=", ">=", "==", "!=",
    "&&", "||", "+=", "-=", "*=", "/=", "%=", "=>", "++", "--",
    "+", "-", "*", "/", "%", "&", "|", "^", "~", "!", "<", ">",
    "=", "?", ":", ".", ",", ";", "(", ")", "[", "]", "{", "}", "@",
];

#[derive(Clone, Debug, PartialEq)]
pub enum StrPart {
    Text(String),
    /// 插值片段：源文本 + 绝对起点 + 可选小数位格式
    Interp {
        src: String,
        line: u32,
        col: u32,
        fmt: Option<u32>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokVal {
    Nil,
    Int { value: i64, raw: String },
    Float { value: f64, raw: String },
    Dec { coeff: i128, scale: i32, raw: String },
    Str(Vec<StrPart>),
    Ident(String),
    /// 关键字 / 运算符 / 属性名，值就是词本身
    Text(String),
}

/// 词法单元。`kind` 用字符串（"ID"/"KW"/"INT"…，与 --dump=ast 的词记法一致）；
/// `line/col` 为起点、`eline/ecol` 为终点（码点计列）；`i0/i1` 是源码字符索引，
/// parser 对插值片段做再词法化时用它切片。
#[derive(Clone, Debug)]
pub struct Tok {
    pub kind: String,
    pub val: TokVal,
    pub line: u32,
    pub col: u32,
    pub eline: u32,
    pub ecol: u32,
    pub i0: usize,
    pub i1: usize,
}

impl Tok {
    pub fn is(&self, kind: &str) -> bool {
        self.kind == kind
    }
    pub fn text(&self) -> &str {
        match &self.val {
            TokVal::Text(s) | TokVal::Ident(s) => s,
            TokVal::Int { raw, .. } | TokVal::Float { raw, .. } | TokVal::Dec { raw, .. } => raw,
            _ => "",
        }
    }
    pub fn line(&self) -> u32 {
        self.line
    }
    pub fn col(&self) -> u32 {
        self.col
    }
}

/// 规范化换行与 BOM。
pub fn normalize(src: &str) -> String {
    let s = src.replace("\r\n", "\n").replace('\r', "\n");
    if let Some(rest) = s.strip_prefix('\u{feff}') {
        rest.to_string()
    } else {
        s
    }
}

/// 字符 → Tok 流。`src` 先转成 `Vec<char>` 按码点索引——列号规定按码点计，
/// 直接按字节索引会在中文内容上算错列。
pub struct Lexer<'a> {
    src: Vec<char>,
    path: String,
    d: &'a mut Diags,
    n: usize,
    i: usize,
    line_starts: Vec<usize>,
    base_line: u32,
    base_col: u32,
    toks: Vec<Tok>,
}

impl<'a> Lexer<'a> {
    pub fn new(src: Vec<char>, path: &str, d: &'a mut Diags, base_line: u32, base_col: u32) -> Self {
        let n = src.len();
        let mut line_starts = vec![0usize];
        for (idx, ch) in src.iter().enumerate() {
            if *ch == '\n' {
                line_starts.push(idx + 1);
            }
        }
        Lexer {
            src,
            path: path.to_string(),
            d,
            n,
            i: 0,
            line_starts,
            base_line,
            base_col,
            toks: Vec::new(),
        }
    }

    // ---------- 位置换算 ----------
    pub fn pos(&self, idx: usize) -> (u32, u32) {
        // 等价于 bisect_right(line_starts, idx) - 1
        let li = match self.line_starts.binary_search(&idx) {
            Ok(k) => k,
            Err(k) => k.saturating_sub(1),
        };
        let mut col = (idx - self.line_starts[li]) as u32 + 1;
        if li == 0 {
            col += self.base_col - 1;
        }
        (self.base_line + li as u32, col)
    }

    fn err(
        &mut self,
        code: &str,
        zh: &str,
        i0: usize,
        i1: usize,
        rule: &str,
        args: &[&dyn std::fmt::Display],
    ) {
        let (l, c) = self.pos(i0);
        let (el, ec) = self.pos(i1);
        let msg = crate::diag_msg::tr(self.d.lang, code, zh, args);
        self.d.error(code, msg, &self.path.clone(), l, c, el, ec, rule);
    }

    fn emit(&mut self, kind: &str, val: TokVal, i0: usize, i1: usize) {
        let (l, c) = self.pos(i0);
        let (el, ec) = self.pos(i1);
        self.toks.push(Tok {
            kind: kind.to_string(),
            val,
            line: l,
            col: c,
            eline: el,
            ecol: ec,
            i0,
            i1,
        });
    }

    fn slice(&self, a: usize, b: usize) -> String {
        self.src[a..b].iter().collect()
    }

    // ---------- 主循环 ----------
    pub fn run(&mut self) -> Vec<Tok> {
        while self.i < self.n {
            let ch = self.src[self.i];

            if ch == ' ' || ch == '\t' || ch == '\u{0c}' || ch == '\u{0b}' {
                self.i += 1;
                continue;
            }
            if ch == '\n' {
                let i0 = self.i;
                self.i += 1;
                self.emit("NL", TokVal::Nil, i0, i0 + 1);
                continue;
            }
            if ch == '/' && self.i + 1 < self.n {
                let nxt = self.src[self.i + 1];
                if nxt == '/' {
                    self.i = match self.src[self.i..].iter().position(|c| *c == '\n') {
                        Some(k) => self.i + k,
                        None => self.n,
                    };
                    continue;
                }
                if nxt == '*' {
                    self.block_comment();
                    continue;
                }
            }
            if ch == '"' {
                self.string();
                continue;
            }
            if ch == 'r' && self.i + 1 < self.n && self.src[self.i + 1] == '"' {
                self.i += 1;
                self.raw_string();
                continue;
            }
            // 单引号不是字符串定界符（L-14）
            if ch == '\'' {
                let i0 = self.i;
                let mut j = i0 + 1;
                while j < self.n && self.src[j] != '\'' && self.src[j] != '\n' {
                    j += 1;
                }
                if j < self.n && self.src[j] == '\'' {
                    j += 1;
                }
                let inner = self.slice(i0 + 1, std::cmp::max(i0 + 1, j - 1));
                self.err("E1002", "单引号不是字符串定界符；一个字符就是长度为 1 的 string，请写 \"{0}\"", i0, j, "L-14", &[&inner]);
                self.d
                    .set_suggestion(format!("\"{}\"", inner));
                self.emit(
                    "STR",
                    TokVal::Str(vec![StrPart::Text(inner)]),
                    i0,
                    j,
                );
                self.i = j;
                continue;
            }
            if ch.is_ascii_digit() {
                self.number();
                continue;
            }
            if ch.is_alphabetic() || ch == '_' {
                let i0 = self.i;
                let mut j = i0 + 1;
                while j < self.n && (self.src[j].is_alphanumeric() || self.src[j] == '_') {
                    j += 1;
                }
                let word = self.slice(i0, j);
                if let Some(bad) = word.chars().find(|c| (*c as u32) > 127) {
                    let k = i0 + self.src[i0..j].iter().position(|c| *c == bad).unwrap_or(0);
                    self.err(
                        "E1006",
                        "非法字符 {0}（标识符只允许 [A-Za-z_][A-Za-z0-9_]*）",
                        k,
                        k + 1,
                        "L-22",
                        &[&format!("{:?}", bad)],
                    );
                    self.i = j;
                    continue;
                }
                self.i = j;
                if word == "_" {
                    self.emit("_", TokVal::Nil, i0, j);
                } else if KEYWORDS.contains(&word.as_str()) {
                    self.emit(&word.clone(), TokVal::Text(word), i0, j);
                } else {
                    self.emit("IDENT", TokVal::Ident(word), i0, j);
                }
                continue;
            }
            if ch == '@' {
                let i0 = self.i;
                let j = i0 + 1;
                if j < self.n && (self.src[j].is_alphabetic() || self.src[j] == '_') {
                    let mut k = j;
                    while k < self.n && (self.src[k].is_alphanumeric() || self.src[k] == '_') {
                        k += 1;
                    }
                    let name = self.slice(j, k);
                    self.emit("ATTR", TokVal::Text(name), i0, k);
                    self.i = k;
                } else {
                    self.err("E2001", "`@` 后必须是属性名（`@os` 或 `@arch`）", i0, i0 + 1, "L-23", &[]);
                    self.i += 1;
                }
                continue;
            }
            // 运算符：最长匹配
            let mut matched: Option<&str> = None;
            for op in OPS {
                let ops: Vec<char> = op.chars().collect();
                if self.i + ops.len() <= self.n && self.src[self.i..self.i + ops.len()] == ops[..] {
                    matched = Some(op);
                    break;
                }
            }
            if let Some(op) = matched {
                let i0 = self.i;
                self.i += op.chars().count();
                self.emit(op, TokVal::Text(op.to_string()), i0, self.i);
                continue;
            }
            let i0 = self.i;
            if ch == '\0' {
                self.err("E1006", "源文件中出现 NUL 字符（U+0000）", i0, i0 + 1, "L-3", &[]);
            } else {
                self.err(
                    "E1006",
                    "非法字符 {0}",
                    i0,
                    i0 + 1,
                    "L-1",
                    &[&format!("{:?}", ch)],
                );
            }
            self.i += 1;
        }
        let (l, c) = self.pos(self.n);
        self.toks.push(Tok {
            kind: "EOF".to_string(),
            val: TokVal::Nil,
            line: l,
            col: c,
            eline: l,
            ecol: c,
            i0: self.n,
            i1: self.n,
        });
        std::mem::take(&mut self.toks)
    }

    // ---------- 注释 ----------
    fn block_comment(&mut self) {
        let i0 = self.i;
        let mut depth = 0i32;
        while self.i < self.n {
            if self.here_is(&['/', '*']) {
                depth += 1;
                self.i += 2;
            } else if self.here_is(&['*', '/']) {
                depth -= 1;
                self.i += 2;
                if depth == 0 {
                    return;
                }
            } else if self.src[self.i] == '\n' {
                // L-19：注释内的换行仍产生 NEWLINE
                let k = self.i;
                self.i += 1;
                self.emit("NL", TokVal::Nil, k, k + 1);
            } else {
                self.i += 1;
            }
        }
        self.err("E1003", "块注释未闭合（缺少 `*/`）", i0, std::cmp::min(i0 + 2, self.n), "L-20", &[]);
    }

    fn here_is(&self, pat: &[char]) -> bool {
        self.i + pat.len() <= self.n && self.src[self.i..self.i + pat.len()] == *pat
    }

    // ---------- 数值 ----------
    fn number(&mut self) {
        let i0 = self.i;
        let mut is_float = false;
        let mut is_dec = false;

        // 0x / 0b / 0o
        if self.src[i0] == '0'
            && i0 + 1 < self.n
            && matches!(self.src[i0 + 1], 'x' | 'X' | 'b' | 'B' | 'o' | 'O')
        {
            let prefix = self.src[i0 + 1].to_ascii_lowercase();
            let base: u32 = match prefix {
                'x' => 16,
                'b' => 2,
                _ => 8,
            };
            let j = i0 + 2;
            let mut k = j;
            while k < self.n && (self.src[k].is_alphanumeric() || self.src[k] == '_') {
                k += 1;
            }
            let body = self.slice(j, k);
            let digit_ok: &str = match prefix {
                'x' => "0123456789abcdefABCDEF",
                'b' => "01",
                _ => "01234567",
            };
            if !body.chars().any(|c| digit_ok.contains(c)) {
                self.err("E1005", "数值字面量缺少数字（`0{0}` 后必须有数字）", i0, k, "L-7", &[&prefix]);
                let raw = self.slice(i0, k);
                self.emit("INT", TokVal::Int { value: 0, raw }, i0, k);
                self.i = k;
                return;
            }
            if body.starts_with('_') || body.ends_with('_') || body.contains("__") {
                self.err("E1005", "下划线只能出现在两个数字之间", i0, k, "L-6", &[]);
            }
            let clean = body.replace('_', "");
            let mut val: i64 = 0;
            for ch in clean.chars() {
                if digit_ok.contains(ch) {
                    val = val.wrapping_mul(base as i64) + ch.to_digit(16).unwrap() as i64;
                } else {
                    self.err(
                        "E1005",
                        "数字 {0} 不属于 {1} 进制",
                        i0,
                        k,
                        "L-7",
                        &[&format!("{:?}", ch), &base],
                    );
                    break;
                }
            }
            let raw = self.slice(i0, k);
            self.emit("INT", TokVal::Int { value: val, raw }, i0, k);
            self.i = k;
            return;
        }

        let mut j = i0;
        while j < self.n && (self.src[j].is_ascii_digit() || self.src[j] == '_') {
            j += 1;
        }
        // 前导零（L-8）
        let head = self.slice(i0, j);
        let hc: Vec<char> = head.chars().collect();
        if hc.len() > 1 && hc[0] == '0' && hc[1] != '_' {
            self.err("E1001", "禁止前导零十进制（八进制请写 `0o17`）", i0, j, "L-8", &[]);
        }
        // 小数部分：只有下一个字符是数字才并入（`1..10` 的关键）
        if j < self.n && self.src[j] == '.' && j + 1 < self.n && self.src[j + 1].is_ascii_digit() {
            is_float = true;
            j += 1;
            while j < self.n && (self.src[j].is_ascii_digit() || self.src[j] == '_') {
                j += 1;
            }
        } else if j < self.n
            && self.src[j] == '.'
            && j + 2 < self.n
            && matches!(self.src[j + 1], 'e' | 'E')
            && (self.src[j + 2].is_ascii_digit()
                || (matches!(self.src[j + 2], '+' | '-')
                    && j + 3 < self.n
                    && self.src[j + 3].is_ascii_digit()))
        {
            self.err("E1005", "`1.e3` 不合法：小数点后必须有数字", i0, j + 1, "L-9", &[]);
        }
        // 指数
        if j < self.n && matches!(self.src[j], 'e' | 'E') {
            let mut k = j + 1;
            if k < self.n && matches!(self.src[k], '+' | '-') {
                k += 1;
            }
            if k < self.n && self.src[k].is_ascii_digit() {
                is_float = true;
                j = k;
                while j < self.n && (self.src[j].is_ascii_digit() || self.src[j] == '_') {
                    j += 1;
                }
            } else {
                self.err("E1005", "指数部分缺少数字", i0, j + 1, "L-9", &[]);
            }
        }
        // decimal 后缀
        if j < self.n && matches!(self.src[j], 'd' | 'D') {
            is_dec = true;
            j += 1;
        }

        let raw = self.slice(i0, j);
        let body: String = if is_dec {
            raw[..raw.len() - 1].to_string()
        } else {
            raw.clone()
        };
        if body.ends_with('_') {
            self.err("E1005", "下划线只能出现在两个数字之间", i0, j, "L-6", &[]);
        }
        let clean0: String = body.trim_end_matches(['d', 'D']).to_string();
        if clean0.contains("__") {
            self.err("E1005", "下划线只能出现在两个数字之间", i0, j, "L-6", &[]);
        }
        let clean = clean0.replace('_', "");
        if is_dec {
            let (coeff, scale) = dec_parts(&clean);
            self.emit("DEC", TokVal::Dec { coeff, scale, raw }, i0, j);
        } else if is_float {
            let v = match clean.parse::<f64>() {
                Ok(v) => v,
                Err(_) => {
                    self.err("E1005", "非法浮点字面量 `{0}`", i0, j, "L-9", &[&raw]);
                    0.0
                }
            };
            self.emit("FLOAT", TokVal::Float { value: v, raw }, i0, j);
        } else {
            let v = match clean.parse::<i64>() {
                Ok(v) => v,
                Err(_) => {
                    self.err("E1005", "非法整数字面量 `{0}`", i0, j, "L-6", &[&raw]);
                    0
                }
            };
            self.emit("INT", TokVal::Int { value: v, raw }, i0, j);
        }
        self.i = j;
    }

    // ---------- 字符串 ----------
    fn string(&mut self) {
        let i0 = self.i;
        // 三引号
        if self.here_is(&['"', '"', '"']) {
            self.i += 3;
            let body_start = self.i;
            let end = self.find_triple(self.i);
            if end.is_none() {
                self.err("E1003", "多行字符串未闭合（缺少 `\"\"\"`）", i0, std::cmp::min(i0 + 3, self.n), "L-15", &[]);
                let tail = self.slice(body_start, self.n);
                self.emit("STR", TokVal::Str(vec![StrPart::Text(tail)]), i0, self.n);
                self.i = self.n;
                return;
            }
            let end = end.unwrap();
            let mut raw = self.slice(body_start, end);
            // L-16：去掉开头第一个换行与结尾最后一个换行
            if raw.starts_with('\n') {
                raw.remove(0);
            }
            if raw.ends_with('\n') {
                raw.pop();
            }
            let parts = self.template_parts(&raw, body_start);
            self.emit("STR", TokVal::Str(parts), i0, end + 3);
            self.i = end + 3;
            return;
        }

        self.i += 1;
        let mut parts: Vec<StrPart> = Vec::new();
        let mut buf = String::new();
        loop {
            if self.i >= self.n {
                self.err("E1003", "字符串未闭合（缺少 `\"`）", i0, std::cmp::min(i0 + 1, self.n), "L-15", &[]);
                break;
            }
            let ch = self.src[self.i];
            if ch == '"' {
                self.i += 1;
                break;
            }
            if ch == '\n' {
                self.err("E1003", "字符串未闭合（换行出现前缺少 `\"`）", i0, std::cmp::min(i0 + 1, self.n), "L-15", &[]);
                break;
            }
            if ch == '\\' {
                let c2 = if self.i + 1 < self.n { self.src[self.i + 1] } else { '\0' };
                let mapped = match c2 {
                    'n' => Some('\n'),
                    't' => Some('\t'),
                    'r' => Some('\r'),
                    '\\' => Some('\\'),
                    '"' => Some('"'),
                    '0' => Some('\0'),
                    _ => None,
                };
                if let Some(m) = mapped {
                    buf.push(m);
                    self.i += 2;
                    continue;
                }
                if c2 == 'u' {
                    let (ok, ch2, ni) = self.unicode_escape(self.i);
                    if ok {
                        buf.push(ch2);
                    }
                    self.i = ni;
                    continue;
                }
                self.err("E1004", "非法转义序列 `\\{0}`（只允许 \\n \\t \\r \\\\ \\\" \\0 与 \\u{{...}}）", self.i, self.i + 2, "L-13", &[&c2]);
                buf.push(c2);
                self.i += 2;
                continue;
            }
            if ch == '{' {
                if self.here_is(&['{', '{']) {
                    buf.push('{');
                    self.i += 2;
                    continue;
                }
                // 插值判定：`{` 后（跳过空白）是标识符 / `this` / `(` 才进入
                // 插值——`{` 开头的字面文本（如 JSON `{"k":1}`）不再被误当
                // 插值解析；要写字面 `{` 插值表达式仍可用 `{{`。
                if self.interp_start() {
                    if !buf.is_empty() {
                        parts.push(StrPart::Text(std::mem::take(&mut buf)));
                    }
                    if let Some(part) = self.interp() {
                        parts.push(part);
                    }
                    continue;
                }
                buf.push('{');
                self.i += 1;
                continue;
            }
            if ch == '}' && self.here_is(&['}', '}']) {
                buf.push('}');
                self.i += 2;
                continue;
            }
            buf.push(ch);
            self.i += 1;
        }
        if !buf.is_empty() {
            parts.push(StrPart::Text(buf));
        }
        self.emit("STR", TokVal::Str(parts), i0, self.i);
    }

    fn find_triple(&self, from: usize) -> Option<usize> {
        let mut j = from;
        while j + 2 < self.n {
            if self.src[j] == '"' && self.src[j + 1] == '"' && self.src[j + 2] == '"' {
                return Some(j);
            }
            j += 1;
        }
        None
    }

    /// at 指向 `\`，下一字符是 u。返回 (ok, char, next_index)。
    fn unicode_escape(&mut self, at: usize) -> (bool, char, usize) {
        let j = at + 2;
        if j >= self.n || self.src[j] != '{' {
            self.err("E1007", "`\\u` 后必须紧跟 `{十六进制}`", at, std::cmp::min(at + 2, self.n), "L-13", &[]);
            return (false, '\0', std::cmp::min(at + 2, self.n));
        }
        let mut k = j;
        let mut found = None;
        while k < self.n {
            if self.src[k] == '}' {
                found = Some(k);
                break;
            }
            k += 1;
        }
        let Some(k) = found else {
            self.err("E1007", "`\\u{...}` 未闭合", at, std::cmp::min(at + 2, self.n), "L-13", &[]);
            return (false, '\0', std::cmp::min(at + 2, self.n));
        };
        let hexs = self.slice(j + 1, k);
        if hexs.is_empty() || hexs.chars().count() > 6 || !hexs.chars().all(|c| c.is_ascii_hexdigit()) {
            self.err("E1007", "`\\u{...}` 需要 1~6 位十六进制数字", at, k + 1, "L-13", &[]);
            return (false, '\0', k + 1);
        }
        let cp = u32::from_str_radix(&hexs, 16).unwrap_or(0x110000);
        if cp > 0x10FFFF || (0xD800..=0xDFFF).contains(&cp) {
            self.err(
                "E1007",
                "非法 Unicode 码点 U+{0}",
                at,
                k + 1,
                "L-13",
                &[&format!("{:04X}", cp)],
            );
            return (false, '\0', k + 1);
        }
        (true, char::from_u32(cp).unwrap_or('\u{fffd}'), k + 1)
    }

    /// self.i 指向开引号（已跳过 r）。
    fn raw_string(&mut self) {
        let i0 = self.i - 1;
        let mut j = self.i + 1;
        while j < self.n && self.src[j] != '"' {
            j += 1;
        }
        if j >= self.n {
            self.err("E1003", "原始字符串未闭合（缺少 `\"`）", i0, std::cmp::min(i0 + 2, self.n), "L-15", &[]);
            let tail = self.slice(self.i + 1, self.n);
            self.emit("STR", TokVal::Str(vec![StrPart::Text(tail)]), i0, self.n);
            self.i = self.n;
            return;
        }
        let text = self.slice(self.i + 1, j);
        self.emit("STR", TokVal::Str(vec![StrPart::Text(text)]), i0, j + 1);
        self.i = j + 1;
    }

    /// `{` 是否开启插值。判定规则（只为把字面 JSON 文本和插值区分开）：
    /// - `{}`（后紧跟 `}`）→ 字面 `{`；
    /// - `{"k":`（字符串字面量后跟 `:`）→ 字面 `{`（JSON 对象文本形态）；
    /// - 其余一律按插值，包括标识符 / 数字 / `(` / 一元 `- + !` / 非 ASCII
    ///   标识符 / **字符串字面量表达式**（`{"中文".ByteLen()}`）。
    /// 要写字面 `{` 但形状不巧撞上插值时，用 `{{` 转义。
    fn interp_start(&self) -> bool {
        let mut j = self.i + 1;
        while j < self.n && (self.src[j] == ' ' || self.src[j] == '\t') {
            j += 1;
        }
        if j >= self.n {
            return false;
        }
        let c = self.src[j];
        if c == '}' || c == '\\' {
            // `{}` 字面；`{\"...`（JSON 文本里转义的引号）也当字面
            return false;
        }
        if c == '"' && self.json_key_ahead(j) {
            return false;
        }
        true
    }

    /// j 指向字符串字面量的开引号：跳过整个字面量（含转义）后，下一个
    /// 非空白字符是 `:` → 形如 JSON 的 `"key": value`。
    fn json_key_ahead(&self, mut j: usize) -> bool {
        j += 1; // 跳过开引号
        while j < self.n {
            match self.src[j] {
                '\\' => j += 2,
                '"' => {
                    j += 1;
                    break;
                }
                _ => j += 1,
            }
        }
        while j < self.n && (self.src[j] == ' ' || self.src[j] == '\t') {
            j += 1;
        }
        j < self.n && self.src[j] == ':'
    }

    /// self.i 指向 `{`。返回插值片段或 None。
    fn interp(&mut self) -> Option<StrPart> {
        let i0 = self.i;
        let mut j = i0 + 1;
        let mut depth = 1i32;
        while j < self.n {
            let ch = self.src[j];
            if ch == '"' {
                // 跳过嵌套字符串（含转义）
                let mut k = j + 1;
                while k < self.n && self.src[k] != '"' {
                    if self.src[k] == '\\' {
                        k += 2;
                    } else {
                        k += 1;
                    }
                }
                j = k + 1;
                continue;
            }
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            j += 1;
        }
        if j >= self.n {
            self.err("E1008", "插值 `{` 未闭合", i0, std::cmp::min(i0 + 1, self.n), "L-18", &[]);
            self.i = self.n;
            return None;
        }
        let frag = self.slice(i0 + 1, j);
        self.i = j + 1;
        // 格式说明符：仅当末尾是 `:.数字` 时切分（不影响三元中的 `:`）
        let mut fmt = None;
        let mut expr_src = frag.clone();
        if let Some(idx) = frag.rfind(':') {
            let tail = &frag[idx + 1..];
            if tail.len() >= 2 && tail.starts_with('.') && tail[1..].chars().all(|c| c.is_ascii_digit()) {
                fmt = tail[1..].parse::<u32>().ok();
                expr_src = frag[..idx].to_string();
            }
        }
        if expr_src.trim().is_empty() {
            let (l, c) = self.pos(i0);
            let msg = crate::diag_msg::tr(self.d.lang, "E1008", "插值表达式为空", &[]);
            self.d.error("E1008", msg, &self.path.clone(), l, c, l, c + 1, "L-18");
            return None;
        }
        let (l, c) = self.pos(i0 + 1);
        Some(StrPart::Interp {
            src: expr_src,
            line: l,
            col: c,
            fmt,
        })
    }

    /// 把已取出的多行字符串正文再按插值规则切分。
    fn template_parts(&mut self, raw: &str, raw_start: usize) -> Vec<StrPart> {
        let (l, c) = self.pos(raw_start);
        let chars: Vec<char> = raw.chars().collect();
        let path = self.path.clone();
        let mut sub = Lexer::new(chars, &path, &mut *self.d, l, c);
        sub.scan_template_only()
    }

    fn scan_template_only(&mut self) -> Vec<StrPart> {
        let mut parts: Vec<StrPart> = Vec::new();
        let mut buf = String::new();
        let n = self.n;
        let mut i = 0usize;
        while i < n {
            let ch = self.src[i];
            if ch == '\\' {
                let c2 = if i + 1 < n { self.src[i + 1] } else { '\0' };
                let mapped = match c2 {
                    'n' => Some('\n'),
                    't' => Some('\t'),
                    'r' => Some('\r'),
                    '\\' => Some('\\'),
                    '"' => Some('"'),
                    '0' => Some('\0'),
                    _ => None,
                };
                if let Some(m) = mapped {
                    buf.push(m);
                    i += 2;
                    continue;
                }
                if c2 == 'u' {
                    self.i = i;
                    let (ok, ch2, ni) = self.unicode_escape(i);
                    if ok {
                        buf.push(ch2);
                    }
                    i = ni;
                    continue;
                }
                self.err("E1004", "非法转义序列 `\\{0}`", i, i + 2, "L-13", &[&c2]);
                i += 2;
                continue;
            }
            if ch == '{' {
                if i + 1 < n && self.src[i + 1] == '{' {
                    buf.push('{');
                    i += 2;
                    continue;
                }
                if !buf.is_empty() {
                    parts.push(StrPart::Text(std::mem::take(&mut buf)));
                }
                self.i = i;
                if let Some(part) = self.interp() {
                    parts.push(part);
                }
                i = self.i;
                continue;
            }
            if ch == '}' && i + 1 < n && self.src[i + 1] == '}' {
                buf.push('}');
                i += 2;
                continue;
            }
            buf.push(ch);
            i += 1;
        }
        if !buf.is_empty() {
            parts.push(StrPart::Text(buf));
        }
        parts
    }
}

/// `'19.99'` -> `(1999, 2)`。不走 float（S-6）。
pub fn dec_parts(text: &str) -> (i128, i32) {
    let mut exp: i32 = 0;
    let mut mant = text.to_string();
    for e in ["e", "E"] {
        if let Some(pos) = mant.find(e) {
            let (m, es) = mant.split_at(pos);
            exp = es[1..].parse::<i32>().unwrap_or(0);
            mant = m.to_string();
            break;
        }
    }
    let (digits, mut scale) = if let Some(pos) = mant.find('.') {
        let (ip, fp) = mant.split_at(pos);
        let fp = &fp[1..];
        let d = format!("{}{}", if ip.is_empty() { "0" } else { ip }, fp);
        (d, fp.chars().count() as i32 - exp)
    } else {
        (if mant.is_empty() { "0".to_string() } else { mant }, -exp)
    };
    let mut coeff: i128 = digits.parse::<i128>().unwrap_or(0);
    if scale < 0 {
        coeff *= 10i128.pow((-scale) as u32);
        scale = 0;
    }
    (coeff, scale)
}

pub fn lex(src: &str, path: &str, d: &mut Diags) -> Vec<Tok> {
    lex_from(src, path, d, 1, 1)
}

pub fn lex_from(src: &str, path: &str, d: &mut Diags, base_line: u32, base_col: u32) -> Vec<Tok> {
    let norm = normalize(src);
    let chars: Vec<char> = norm.chars().collect();
    let mut lx = Lexer::new(chars, path, d, base_line, base_col);
    lx.run()
}

/// 供诊断使用的 Note 便捷构造（避免调用方直接依赖 diag 的枚举细节）。
pub fn note_at(msg: impl Into<String>, line: u32, col: u32) -> Note {
    Note::At(msg.into(), line, col)
}
