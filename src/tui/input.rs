#[derive(Default)]
pub struct Input {
    pub text: String,
}
impl Input {
    pub fn push(&mut self, c: char) {
        self.text.push(c);
        if matches!(c, 'M' | 'm') {
            self.remove_sgr_mouse_report();
        }
    }
    pub fn backspace(&mut self) {
        self.text.pop();
    }
    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.text)
    }

    fn remove_sgr_mouse_report(&mut self) {
        let Some(start) = self.text.rfind("[<") else {
            return;
        };
        let candidate = &self.text[start + 2..self.text.len().saturating_sub(1)];
        let mut fields = candidate.split(';');
        let valid = (0..3).all(|_| {
            fields
                .next()
                .is_some_and(|field| !field.is_empty() && field.chars().all(|c| c.is_ascii_digit()))
        }) && fields.next().is_none();
        if valid {
            self.text.truncate(start);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Input;

    #[test]
    fn strips_fragmented_sgr_mouse_reports_from_user_input() {
        let mut input = Input {
            text: "保留这些文字".into(),
        };
        for character in "[<35;96;3M".chars() {
            input.push(character);
        }
        assert_eq!(input.text, "保留这些文字");
    }

    #[test]
    fn preserves_normal_bracketed_input() {
        let mut input = Input::default();
        for character in "echo [<not-a-mouse>]".chars() {
            input.push(character);
        }
        assert_eq!(input.text, "echo [<not-a-mouse>]");
    }
}
