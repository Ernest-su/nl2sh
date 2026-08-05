/// Removes terminal control sequences that could move the cursor, clear the
/// screen, switch buffers, or inject terminal titles. SGR color sequences are
/// retained because they do not alter TUI structure.
pub fn filter_unsafe_ansi(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != 0x1b {
            if bytes[i] == b'\r' {
                out.push(b'\n');
            } else if bytes[i] == b'\n' || bytes[i] == b'\t' || bytes[i] >= 0x20 {
                out.push(bytes[i]);
            }
            i += 1;
            continue;
        }
        if i + 1 >= bytes.len() {
            break;
        }
        match bytes[i + 1] {
            b'[' => {
                let start = i;
                i += 2;
                while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                    i += 1;
                }
                if i < bytes.len() {
                    if bytes[i] == b'm' {
                        out.extend_from_slice(&bytes[start..=i]);
                    }
                    i += 1;
                }
            }
            b']' => {
                i += 2;
                while i < bytes.len() {
                    if bytes[i] == 0x07 {
                        i += 1;
                        break;
                    }
                    if bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'\\') {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            }
            _ => {
                i = (i + 2).min(bytes.len());
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_text_and_color_but_removes_screen_controls() {
        assert_eq!(
            filter_unsafe_ansi("a\x1b[2Jb\x1b[31mc\x1b[0m"),
            "ab\x1b[31mc\x1b[0m"
        );
        assert_eq!(filter_unsafe_ansi("\x1b]0;owned\x07safe"), "safe");
    }
}
