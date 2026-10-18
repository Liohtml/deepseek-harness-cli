//! Unicode-aware editing with explicit line layout shared by the composer and cursor.
use unicode_width::UnicodeWidthChar;

#[derive(Default)]
pub struct Editor {
    pub chars: Vec<char>,
    pub cursor: usize,
}

impl Editor {
    pub fn insert(&mut self, text: &str) {
        for ch in text
            .chars()
            .filter(|ch| !ch.is_control() || *ch == '\n' || *ch == '\t')
        {
            if self.chars.len() >= 64 * 1024 {
                break;
            }
            self.chars.insert(self.cursor, ch);
            self.cursor += 1;
        }
    }
    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }
    pub fn replace(&mut self, text: &str) {
        self.chars.clear();
        self.cursor = 0;
        self.insert(text);
    }
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }
    pub fn layout(&self, width: usize) -> (Vec<String>, (usize, usize)) {
        let width = width.max(1);
        let mut lines = vec![String::new()];
        let mut column = 0;
        let mut cursor = (0, 0);
        for (index, ch) in self.chars.iter().enumerate() {
            let ch = if *ch == '\t' { ' ' } else { *ch };
            let cells = ch.width().unwrap_or(0);
            if ch != '\n' && column + cells > width {
                lines.push(String::new());
                column = 0;
            }
            if index == self.cursor {
                cursor = (column, lines.len() - 1);
            }
            if ch == '\n' {
                lines.push(String::new());
                column = 0;
            } else {
                lines.last_mut().unwrap().push(ch);
                column += cells;
            }
        }
        if self.cursor == self.chars.len() {
            if column >= width {
                lines.push(String::new());
                column = 0;
            }
            cursor = (column, lines.len() - 1);
        }
        (lines, cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_editing_and_multiline_paste_keep_the_cursor_aligned() {
        let mut editor = Editor::default();
        editor.insert("Grüße\n世界");
        editor.backspace();
        assert_eq!(editor.text(), "Grüße\n世");
        assert_eq!(editor.layout(5).1, (2, 1));
        editor.cursor = 2;
        editor.insert("!");
        assert_eq!(editor.text(), "Gr!üße\n世");
        assert_eq!(editor.layout(5).1, (3, 0));
    }
    #[test]
    fn wide_characters_wrap_before_the_cursor() {
        let mut editor = Editor::default();
        editor.insert("你好x");
        assert_eq!(editor.layout(4), (vec!["你好".into(), "x".into()], (1, 1)));
    }
}
