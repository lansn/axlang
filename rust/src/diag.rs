//! 诊断收集与格式化（规格书 D-05）。
//!
//! 文本模式：1 行头 + 1 行源码 + 1 行脱字符。
//! JSON 模式：JSONL，字段固定、顺序固定。
//! 排序：(path, line, col, code)——UTF-8 字节序与码点序一致，可直接按字符串比较。

use std::collections::HashMap;
use std::fmt::Write as _;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// 附注：要么是纯文字，要么带自己的位置。
#[derive(Clone, Debug)]
pub enum Note {
    Msg(String),
    At(String, u32, u32),
}

/// 一条诊断。`code` 是对外诊断码（E2001…，一经发布不改字），
/// `rule` 是触发规则的内部名（JSON 输出用），`notes` 是附注行，
/// `suggestion` 是可选的修复建议。
#[derive(Clone, Debug)]
pub struct Diag {
    pub code: String,
    pub msg: String,
    pub path: String,
    pub line: u32,
    pub col: u32,
    pub eline: u32,
    pub ecol: u32,
    pub rule: String,
    pub severity: Severity,
    pub notes: Vec<Note>,
    pub suggestion: Option<String>,
}

impl Diag {
    fn key(&self) -> (&str, u32, u32, &str) {
        (&self.path, self.line, self.col, &self.code)
    }
}

/// 诊断池：收集、按 (path, line, col, code) 去重与排序、text/JSONL 两种输出。
/// `sources` 缓存源码文本，供文本模式的脱字符行渲染。
pub struct Diags {
    pub items: Vec<Diag>,
    /// (path, line, col, code) → items 下标：同一位置同一码只保留第一条，
    /// 避免解析恢复在同一 token 上反复报错（"同一个错误循环显示多次"）。
    seen: HashMap<(String, u32, u32, String), usize>,
    /// 最近一次真正入池的诊断下标——set_note / set_suggestion 挂到它上面；
    /// 若该诊断因去重被丢弃，则挂到它原本所在的那条上。
    last: Option<usize>,
    pub sources: HashMap<String, String>,
    pub error_count: usize,
    /// 诊断文案语言（默认英文；`--lang` / `AXC_LANG` / `axc.toml` 决定）
    pub lang: crate::diag_msg::Lang,
}

impl Diags {
    pub fn new() -> Self {
        Diags {
            items: Vec::new(),
            seen: HashMap::new(),
            last: None,
            sources: HashMap::new(),
            error_count: 0,
            lang: crate::diag_msg::Lang::En,
        }
    }

    pub fn add_source(&mut self, path: &str, text: &str) {
        self.sources.insert(path.to_string(), text.to_string());
    }

    /// 回滚到前 n 条诊断（解析回溯用）：截断 items、重建去重表、复位 last。
    pub fn truncate_to(&mut self, n: usize) {
        self.items.truncate(n);
        self.seen.clear();
        for (i, d) in self.items.iter().enumerate() {
            self.seen
                .insert((d.path.clone(), d.line, d.col, d.code.clone()), i);
        }
        self.last = self.items.len().checked_sub(1);
    }

    pub fn has_error(&self) -> bool {
        self.error_count > 0
    }

    #[allow(clippy::too_many_arguments)]
    pub fn error(
        &mut self,
        code: &str,
        msg: impl Into<String>,
        path: &str,
        line: u32,
        col: u32,
        eline: u32,
        ecol: u32,
        rule: &str,
    ) {
        let key = (path.to_string(), line, col, code.to_string());
        if let Some(&i) = self.seen.get(&key) {
            // 同一位置同一码已有诊断：丢弃重复条，但记住它，让后续
            // set_note / set_suggestion 仍挂到首条上而不是别的诊断上
            self.last = Some(i);
            return;
        }
        let idx = self.items.len();
        self.items.push(Diag {
            code: code.to_string(),
            msg: msg.into(),
            path: path.to_string(),
            line,
            col,
            eline,
            ecol,
            rule: rule.to_string(),
            severity: Severity::Error,
            notes: Vec::new(),
            suggestion: None,
        });
        self.seen.insert(key, idx);
        self.last = Some(idx);
        self.error_count += 1;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn warn(
        &mut self,
        code: &str,
        msg: impl Into<String>,
        path: &str,
        line: u32,
        col: u32,
        eline: u32,
        ecol: u32,
        rule: &str,
    ) {
        let key = (path.to_string(), line, col, code.to_string());
        if let Some(&i) = self.seen.get(&key) {
            self.last = Some(i);
            return;
        }
        let idx = self.items.len();
        self.items.push(Diag {
            code: code.to_string(),
            msg: msg.into(),
            path: path.to_string(),
            line,
            col,
            eline,
            ecol,
            rule: rule.to_string(),
            severity: Severity::Warning,
            notes: Vec::new(),
            suggestion: None,
        });
        self.seen.insert(key, idx);
        self.last = Some(idx);
    }

    /// 给最近一条诊断补建议（`help:` 行）。
    pub fn set_suggestion(&mut self, text: impl Into<String>) {
        let target = self.last.or_else(|| self.items.len().checked_sub(1));
        if let Some(i) = target {
            if let Some(d) = self.items.get_mut(i) {
                d.suggestion = Some(text.into());
            }
        }
    }

    pub fn set_note(&mut self, note: Note) {
        let target = self.last.or_else(|| self.items.len().checked_sub(1));
        if let Some(i) = target {
            if let Some(d) = self.items.get_mut(i) {
                d.notes.push(note);
            }
        }
    }

    fn sorted(&self) -> Vec<&Diag> {
        let mut v: Vec<&Diag> = self.items.iter().collect();
        v.sort_by(|a, b| a.key().cmp(&b.key()));
        v
    }

    pub fn format_text(&self) -> String {
        let mut out = String::new();
        for d in self.sorted() {
            let _ = writeln!(
                out,
                "{}:{}:{}: {}[{}]: {}",
                d.path,
                d.line,
                d.col,
                d.severity.as_str(),
                d.code,
                d.msg
            );
            if let Some(src) = self.sources.get(&d.path) {
                // 按 \n 切行；只取目标行
                if d.line >= 1 {
                    if let Some(text) = src.split('\n').nth((d.line - 1) as usize) {
                        let text: String = text.chars().filter(|c| *c != '\r').collect();
                        let _ = writeln!(out, "    {}", text);
                        let tlen = text.chars().count() as i64;
                        let cspan = d.ecol as i64 - d.col as i64;
                        let mut width = std::cmp::max(1, std::cmp::min(cspan, std::cmp::max(1, tlen - d.col as i64 + 1)));
                        if d.ecol <= d.col {
                            width = 1;
                        }
                        let mut line = String::from("    ");
                        for _ in 0..(d.col.saturating_sub(1)) {
                            line.push(' ');
                        }
                        for _ in 0..width {
                            line.push('^');
                        }
                        out.push_str(&line);
                        out.push('\n');
                    }
                }
            }
            for n in &d.notes {
                match n {
                    Note::Msg(m) => {
                        let _ = writeln!(out, "    note: {}", m);
                    }
                    Note::At(m, l, c) => {
                        let _ = writeln!(out, "    note: {} ({}:{})", m, l, c);
                    }
                }
            }
            if let Some(s) = &d.suggestion {
                let _ = writeln!(out, "    help: {}", s);
            }
        }
        out
    }

    pub fn format_json(&self) -> String {
        let mut out = String::new();
        for d in self.sorted() {
            let mut s = String::from("{");
            let _ = write!(s, "\"path\":{},", json_str(&d.path));
            let _ = write!(s, "\"line\":{},", d.line);
            let _ = write!(s, "\"col\":{},", d.col);
            let _ = write!(s, "\"end_line\":{},", d.eline);
            let _ = write!(s, "\"end_col\":{},", d.ecol);
            let _ = write!(s, "\"severity\":{},", json_str(d.severity.as_str()));
            let _ = write!(s, "\"code\":{},", json_str(&d.code));
            let _ = write!(s, "\"message\":{},", json_str(&d.msg));
            let _ = write!(s, "\"rule\":{},", json_str(&d.rule));
            s.push_str("\"notes\":[");
            for (i, n) in d.notes.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                let (m, l, c) = match n {
                    Note::Msg(m) => (m.as_str(), d.line, d.col),
                    Note::At(m, l, c) => (m.as_str(), *l, *c),
                };
                let _ = write!(
                    s,
                    "{{\"message\":{},\"line\":{},\"col\":{}}}",
                    json_str(m),
                    l,
                    c
                );
            }
            s.push(']');
            if let Some(sg) = &d.suggestion {
                let _ = write!(
                    s,
                    ",\"suggestions\":[{{\"message\":{},\"replacement\":\"\"}}]",
                    json_str(sg)
                );
            }
            s.push('}');
            out.push_str(&s);
            out.push('\n');
        }
        out
    }
}

impl Default for Diags {
    fn default() -> Self {
        Self::new()
    }
}

/// 最小 JSON 字符串转义（不引 serde：控制字符与引号足够覆盖诊断文本）。
pub fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
