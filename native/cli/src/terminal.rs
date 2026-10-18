//! Scrollback-preserving terminal editing; approval answers never enter prompt history.
use crate::runtime::{Event, Frame, Input, Runtime};
use crossterm::{
    cursor::MoveToColumn,
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event as KeyEvent, KeyCode,
        KeyEventKind, KeyModifiers,
    },
    execute,
    style::Print,
    terminal::{self, Clear, ClearType},
};
use std::{
    collections::VecDeque,
    ffi::OsString,
    io::{self, Write},
    time::{Duration, Instant},
};
use unicode_width::UnicodeWidthChar;

const MAX_INPUT: usize = 64 * 1024;

pub fn safe_text(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control() || *ch == '\n' || *ch == '\t')
        .collect::<String>()
        .replace('\t', "    ")
}

#[derive(Default)]
struct Editor {
    chars: Vec<char>,
    cursor: usize,
}

impl Editor {
    fn insert(&mut self, text: &str) {
        for ch in text
            .chars()
            .filter(|ch| !ch.is_control() || *ch == '\n' || *ch == '\t')
        {
            if self.chars.len() >= MAX_INPUT {
                break;
            }
            self.chars.insert(self.cursor, ch);
            self.cursor += 1;
        }
    }
    fn text(&self) -> String {
        self.chars.iter().collect()
    }
    fn replace(&mut self, text: &str) {
        self.chars.clear();
        self.cursor = 0;
        self.insert(text);
    }
    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }
    fn display(&self, width: usize) -> (String, usize) {
        let chars: Vec<char> = self
            .chars
            .iter()
            .map(|ch| match ch {
                '\n' => '↵',
                '\t' => ' ',
                _ => *ch,
            })
            .collect();
        let cell_width = |ch: &char| ch.width().unwrap_or(0);
        let mut start = self.cursor;
        let mut cursor = 0;
        while start > 0 && cursor + cell_width(&chars[start - 1]) < width {
            start -= 1;
            cursor += cell_width(&chars[start]);
        }
        let mut used = 0;
        let visible = chars[start..]
            .iter()
            .take_while(|ch| {
                used += cell_width(ch);
                used < width
            })
            .collect();
        (visible, cursor)
    }
}

struct Prompt {
    id: u64,
    label: String,
    history: bool,
}

struct Terminal {
    output: io::Stdout,
    prompt: Option<Prompt>,
    editor: Editor,
    history: VecDeque<String>,
    history_index: usize,
}

impl Terminal {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut terminal = Self {
            output: io::stdout(),
            prompt: None,
            editor: Editor::default(),
            history: VecDeque::new(),
            history_index: 0,
        };
        execute!(terminal.output, EnableBracketedPaste)?;
        Ok(terminal)
    }
    fn clear(&mut self) -> io::Result<()> {
        execute!(self.output, MoveToColumn(0), Clear(ClearType::CurrentLine))
    }
    fn print(&mut self, text: &str) -> io::Result<()> {
        self.clear()?;
        execute!(self.output, Print(safe_text(text).replace('\n', "\r\n")))?;
        // Runtime outputs finish lines; keep diagnostics separate from an active answer.
        if self.prompt.is_some() && !text.ends_with('\n') {
            execute!(self.output, Print("\r\n"))?;
        }
        self.redraw()
    }
    fn redraw(&mut self) -> io::Result<()> {
        if let Some(prompt) = &self.prompt {
            let width = terminal::size()?.0.saturating_sub(1) as usize;
            let label: String = prompt.label.chars().take(width / 2).collect();
            let label_width: usize = label.chars().map(|ch| ch.width().unwrap_or(0)).sum();
            let (visible, cursor) = self
                .editor
                .display(width.saturating_sub(label_width).max(1));
            execute!(
                self.output,
                MoveToColumn(0),
                Clear(ClearType::CurrentLine),
                Print(&label),
                Print(visible),
                MoveToColumn((label_width + cursor).min(width) as u16)
            )?;
        }
        self.output.flush()
    }
    fn prompt(&mut self, id: u64, text: String, history: bool) -> io::Result<()> {
        // Discard typeahead before displaying a new request, especially an approval.
        while event::poll(Duration::ZERO)? {
            let _ = event::read()?;
        }
        self.prompt = None;
        self.editor = Editor::default();
        self.history_index = self.history.len();
        self.clear()?;
        let text = safe_text(&text);
        let label = if let Some((details, label)) = text.rsplit_once('\n') {
            self.print(&format!("{details}\n"))?;
            label.to_owned()
        } else {
            text
        };
        self.prompt = Some(Prompt { id, label, history });
        self.redraw()
    }
    fn submit(&mut self, runtime: &mut Runtime) -> Result<(), Box<dyn std::error::Error>> {
        let Some(prompt) = self.prompt.take() else {
            return Ok(());
        };
        let text = self.editor.text();
        self.clear()?;
        execute!(
            self.output,
            Print(&prompt.label),
            Print(safe_text(&text).replace('\n', "\r\n")),
            Print("\r\n")
        )?;
        self.output.flush()?;
        if prompt.history && !text.trim().is_empty() && self.history.back() != Some(&text) {
            self.history.push_back(text.clone());
            if self.history.len() > 100 {
                self.history.pop_front();
            }
        }
        self.editor = Editor::default();
        runtime.send(Input::Answer {
            id: prompt.id,
            text: &text,
        })
    }
    fn history(&mut self, backwards: bool) {
        if !self.prompt.as_ref().is_some_and(|prompt| prompt.history) {
            return;
        }
        self.history_index = if backwards {
            self.history_index.saturating_sub(1)
        } else {
            (self.history_index + 1).min(self.history.len())
        };
        self.editor.replace(
            self.history
                .get(self.history_index)
                .map(String::as_str)
                .unwrap_or(""),
        );
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = execute!(
            self.output,
            DisableBracketedPaste,
            MoveToColumn(0),
            Print("\r\n")
        );
        let _ = terminal::disable_raw_mode();
    }
}

pub fn run(resume: Option<OsString>) -> Result<u8, Box<dyn std::error::Error>> {
    let mut terminal = Terminal::enter()?;
    terminal.print("Starting DSH…\n")?;
    let mut runtime = Runtime::start(resume)?;
    let result = interact(&mut runtime, &mut terminal);
    if result.is_err() {
        let _ = runtime.send(Input::Close);
    }
    let status = runtime.finish();
    result?;
    status
}

fn interact(
    runtime: &mut Runtime,
    terminal: &mut Terminal,
) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let mut initialized = false;
    let mut closing: Option<Instant> = None;
    loop {
        // Bound each drain so model output cannot starve Ctrl+C handling.
        for _ in 0..100 {
            let Ok(event) = runtime.events.try_recv() else {
                break;
            };
            match event {
                Event::Frame(Frame::Hello { version: 1 }) if !initialized => initialized = true,
                Event::Frame(Frame::Hello { .. }) => {
                    return Err("unsupported or duplicate terminal handshake".into());
                }
                Event::Frame(_) if !initialized => {
                    return Err("runtime omitted terminal handshake".into());
                }
                Event::Frame(Frame::Output { text }) => terminal.print(&text)?,
                Event::Frame(Frame::Prompt { id, text, history }) => {
                    if closing.is_none() {
                        terminal.prompt(id, text, history)?;
                    }
                }
                Event::Frame(Frame::Cancel { id }) => {
                    if terminal
                        .prompt
                        .as_ref()
                        .is_some_and(|prompt| prompt.id == id)
                    {
                        terminal.prompt = None;
                        terminal.editor = Editor::default();
                        terminal.clear()?;
                    }
                }
                Event::Frame(Frame::Error { message }) | Event::Failure(message) => {
                    return Err(message.into());
                }
                Event::Frame(Frame::Close) => return Ok(()),
                Event::Diagnostic(text) => terminal.print(&format!("{text}\n"))?,
                Event::Eof => {
                    return Err("runtime closed unexpectedly; see diagnostics above".into());
                }
            }
        }
        if !initialized && started.elapsed() > Duration::from_secs(60) {
            return Err(
                "runtime startup exceeded 60 seconds; check its diagnostics and build artifacts"
                    .into(),
            );
        }
        if closing.is_some_and(|time| time.elapsed() > Duration::from_secs(8)) {
            return Err("runtime did not acknowledge terminal close".into());
        }
        if !event::poll(Duration::from_millis(30))? {
            continue;
        }
        match event::read()? {
            KeyEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match key.code {
                        KeyCode::Char('c') => {
                            if closing.is_some() {
                                return Err("forced exit; session flush may be incomplete".into());
                            }
                            terminal.editor = Editor::default();
                            runtime.send(Input::Interrupt)?;
                        }
                        KeyCode::Char('d') => {
                            runtime.send(Input::Close)?;
                            closing.get_or_insert(Instant::now());
                            terminal.prompt = None;
                            terminal.clear()?;
                        }
                        KeyCode::Char('a') => terminal.editor.cursor = 0,
                        KeyCode::Char('e') => terminal.editor.cursor = terminal.editor.chars.len(),
                        KeyCode::Char('u') => terminal.editor = Editor::default(),
                        _ => {}
                    }
                } else if terminal.prompt.is_some() {
                    match key.code {
                        KeyCode::Enter => terminal.submit(runtime)?,
                        KeyCode::Char(ch) => terminal.editor.insert(&ch.to_string()),
                        KeyCode::Backspace => terminal.editor.backspace(),
                        KeyCode::Delete if terminal.editor.cursor < terminal.editor.chars.len() => {
                            terminal.editor.chars.remove(terminal.editor.cursor);
                        }
                        KeyCode::Left => {
                            terminal.editor.cursor = terminal.editor.cursor.saturating_sub(1)
                        }
                        KeyCode::Right => {
                            terminal.editor.cursor =
                                (terminal.editor.cursor + 1).min(terminal.editor.chars.len())
                        }
                        KeyCode::Home => terminal.editor.cursor = 0,
                        KeyCode::End => terminal.editor.cursor = terminal.editor.chars.len(),
                        KeyCode::Up => terminal.history(true),
                        KeyCode::Down => terminal.history(false),
                        _ => {}
                    }
                }
                terminal.redraw()?;
            }
            KeyEvent::Paste(text) if terminal.prompt.is_some() => {
                terminal.editor.insert(&text);
                terminal.redraw()?;
            }
            KeyEvent::Resize(_, _) => terminal.redraw()?,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_unicode_and_preserves_multiline_paste_without_submitting() {
        let mut editor = Editor::default();
        editor.insert("Grüße\n世界");
        editor.backspace();
        assert_eq!(editor.text(), "Grüße\n世");
        editor.cursor = 2;
        editor.insert("!");
        assert_eq!(editor.text(), "Gr!üße\n世");
        assert!(!editor.display(5).0.contains('\n'));
    }
    #[test]
    fn display_fits_wide_characters_and_cursor_in_small_terminals() {
        let mut editor = Editor::default();
        editor.insert("你好abc世界");
        let (text, cursor) = editor.display(5);
        assert_eq!(text, "世界");
        assert_eq!(cursor, 4);
    }
    #[test]
    fn external_controls_cannot_reach_the_terminal() {
        assert_eq!(safe_text("\x1b[2J\x07hello\rworld\n"), "[2Jhelloworld\n");
    }
}
