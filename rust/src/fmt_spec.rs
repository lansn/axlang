//! `fmt.printf` 格式串的编译期解析（文稿 11.2）。
//!
//! 支持的占位符：`%%` `%d` `%f` `%.Nf` `%s` `%t` `%v`。
//! 其余（含 `%n` `%x` `%o` `%c` `%e` `%g` 等）一律拒绝——编译期报错，
//! 从根上堵死 C 那类"运行时才发现格式串错了 / `%n` 写内存"的坑。
//! sema 与 backend 共用这一份解析，保证"核对什么"和"生成什么"永远一致。

/// 格式串切片。
#[derive(Debug, Clone, PartialEq)]
pub enum FmtPart {
    /// 字面文本（转义已在词法期解码，这里的 `\n` 就是换行符）
    Text(String),
    /// `%%` → 字面 `%`
    Pct,
    /// 占位符：`spec` 取值 d/f/s/t/v；`prec` 是 `%.Nf` 的 N
    /// （只对 `f` 有意义，`%.Nd` 之类由 sema 另报 E4032 精度错）
    Arg { spec: char, prec: Option<u32> },
}

/// 解析格式串。失败返回 `(字节偏移, 非法字符)`，偏移指向 `%` 之后的那个字符。
pub fn parse(s: &str) -> Result<Vec<FmtPart>, (usize, char)> {
    let mut out: Vec<FmtPart> = Vec::new();
    let mut text = String::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'%' {
            let ch = s[i..].chars().next().unwrap();
            text.push(ch);
            i += ch.len_utf8();
            continue;
        }
        if i + 1 >= b.len() {
            return Err((i, '\0')); // 串尾孤 `%`
        }
        let c = b[i + 1] as char;
        match c {
            '%' => {
                if !text.is_empty() {
                    out.push(FmtPart::Text(std::mem::take(&mut text)));
                }
                out.push(FmtPart::Pct);
                i += 2;
            }
            'd' | 's' | 't' | 'v' | 'f' => {
                if !text.is_empty() {
                    out.push(FmtPart::Text(std::mem::take(&mut text)));
                }
                out.push(FmtPart::Arg { spec: c, prec: None });
                i += 2;
            }
            '.' => {
                // `%.Nf`：`.` 后必须有数字，数字后必须是合法占位符字母
                let mut j = i + 2;
                let mut num = String::new();
                while j < b.len() && b[j].is_ascii_digit() {
                    num.push(b[j] as char);
                    j += 1;
                }
                if num.is_empty() {
                    let pos = if j < b.len() { j } else { i + 1 };
                    let ch = if j < b.len() { b[j] as char } else { '\0' };
                    return Err((pos, ch));
                }
                if j >= b.len() {
                    return Err((i + 1, '.'));
                }
                let f = b[j] as char;
                match f {
                    'f' | 'd' | 's' | 't' | 'v' => {
                        // `%.Nd` 之类交给 sema 报"精度只支持 %f"，比这里笼统报非法更准
                        let prec: u32 = num.parse().unwrap_or(u32::MAX);
                        if !text.is_empty() {
                            out.push(FmtPart::Text(std::mem::take(&mut text)));
                        }
                        out.push(FmtPart::Arg { spec: f, prec: Some(prec) });
                        i = j + 1;
                    }
                    _ => return Err((j, f)),
                }
            }
            _ => return Err((i + 1, c)),
        }
    }
    if !text.is_empty() {
        out.push(FmtPart::Text(text));
    }
    Ok(out)
}
