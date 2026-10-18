//! Presentation state and deterministic Ratatui rendering. Runtime messages never contain terminal commands.
use crate::{editor::Editor, runtime::Frame};
use ratatui::{
    Frame as Screen,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use std::{collections::VecDeque, path::Path, time::Instant};

pub const BG: Color = Color::Rgb(16, 21, 28);
const PANEL: Color = Color::Rgb(23, 30, 40);
const BORDER: Color = Color::Rgb(44, 57, 72);
const TEXT: Color = Color::Rgb(228, 237, 245);
const MUTED: Color = Color::Rgb(133, 151, 173);
const BLUE: Color = Color::Rgb(103, 183, 255);
const GREEN: Color = Color::Rgb(141, 214, 180);
const AMBER: Color = Color::Rgb(241, 194, 123);
const RED: Color = Color::Rgb(245, 143, 143);

pub const COMMANDS: &[(&str, &str)] = &[
    ("/help", "Keyboard shortcuts and commands"),
    ("/session", "Session identity and resume command"),
    ("/diagnostics", "Inspect runtime diagnostics"),
    ("/clear", "Clear the display; keep session context"),
    ("/multiline", "Compose a block; a single dot submits"),
    ("/exit", "Save and close the session"),
];

pub fn safe_text(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control() || *ch == '\n' || *ch == '\t')
        .collect::<String>()
        .replace('\t', "    ")
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Phase {
    Starting,
    Ready,
    Working,
    Question,
    Closing,
}

pub struct Prompt {
    pub id: u64,
    pub text: String,
    pub history: bool,
}

pub enum Kind {
    User,
    Assistant,
    Note,
    Request,
    Tool {
        id: String,
        name: String,
        arguments: String,
        result: Option<bool>,
    },
}

pub struct Message {
    pub kind: Kind,
    pub text: String,
}

pub struct App {
    pub cwd: String,
    pub session: String,
    pub provider: String,
    pub model: String,
    pub phase: Phase,
    pub prompt: Option<Prompt>,
    pub editor: Editor,
    pub messages: Vec<Message>,
    pub diagnostics: VecDeque<String>,
    pub history: VecDeque<String>,
    pub history_index: usize,
    pub show_help: bool,
    pub show_diagnostics: bool,
    pub expanded_tools: bool,
    pub scroll: usize,
    pub palette_index: usize,
    pub demo: bool,
    pub started: Instant,
    pub turn_started: Instant,
    streaming: Option<usize>,
}

impl App {
    pub fn new(cwd: String) -> Self {
        Self {
            cwd,
            session: String::new(),
            provider: String::new(),
            model: String::new(),
            phase: Phase::Starting,
            prompt: None,
            editor: Editor::default(),
            messages: vec![],
            diagnostics: VecDeque::new(),
            history: VecDeque::new(),
            history_index: 0,
            show_help: false,
            show_diagnostics: false,
            expanded_tools: false,
            scroll: 0,
            palette_index: 0,
            demo: false,
            started: Instant::now(),
            turn_started: Instant::now(),
            streaming: None,
        }
    }
    pub fn apply(&mut self, frame: Frame) {
        match frame {
            Frame::Session {
                cwd,
                id,
                provider,
                model,
            } => {
                self.cwd = safe_text(&cwd);
                self.session = safe_text(&id);
                self.provider = safe_text(&provider.unwrap_or_default());
                self.model = safe_text(&model.unwrap_or_default());
            }
            Frame::Output { text } => self.note(text),
            Frame::AssistantStart => self.streaming = None,
            Frame::AssistantDelta { text } => {
                let text = safe_text(&text);
                if let Some(index) = self.streaming {
                    self.messages[index].text.push_str(&text);
                } else if !text.is_empty() {
                    self.streaming = Some(self.messages.len());
                    self.messages.push(Message {
                        kind: Kind::Assistant,
                        text,
                    });
                }
            }
            Frame::Assistant { text } => {
                let text = safe_text(&text);
                if let Some(index) = self.streaming.take() {
                    self.messages[index].text = text;
                } else if !text.is_empty() {
                    self.messages.push(Message {
                        kind: Kind::Assistant,
                        text,
                    });
                }
            }
            Frame::Tool {
                id,
                name,
                arguments,
            } => {
                self.messages.push(Message {
                    kind: Kind::Tool {
                        id,
                        name: safe_text(&name),
                        arguments: safe_text(&arguments),
                        result: None,
                    },
                    text: String::new(),
                });
            }
            Frame::ToolResult { id, failed, text } => {
                for message in self.messages.iter_mut().rev() {
                    if let Kind::Tool {
                        id: call_id,
                        result,
                        ..
                    } = &mut message.kind
                        && call_id == &id
                    {
                        *result = Some(failed);
                        message.text = safe_text(&text);
                        break;
                    }
                }
            }
            Frame::Prompt { id, text, history } => {
                self.phase = if history {
                    Phase::Ready
                } else {
                    Phase::Question
                };
                self.editor = Editor::default();
                self.palette_index = 0;
                self.history_index = self.history.len();
                let text = safe_text(&text);
                if !history && text.trim() != "..." {
                    self.messages.push(Message {
                        kind: Kind::Request,
                        text: text.trim().to_owned(),
                    });
                }
                self.prompt = Some(Prompt { id, text, history });
            }
            Frame::Cancel { id } => {
                if self.prompt.as_ref().is_some_and(|prompt| prompt.id == id) {
                    self.prompt = None;
                    self.editor = Editor::default();
                    self.phase = Phase::Working;
                }
            }
            Frame::Hello { .. } | Frame::Error { .. } | Frame::Close => {}
        }
    }
    pub fn note(&mut self, text: String) {
        if !text.trim().is_empty() {
            self.messages.push(Message {
                kind: Kind::Note,
                text: safe_text(text.trim()),
            });
        }
    }
    pub fn diagnose(&mut self, text: String) {
        self.diagnostics.push_back(safe_text(&text));
        if self.diagnostics.len() > 500 {
            self.diagnostics.pop_front();
        }
    }
    pub fn history(&mut self, backwards: bool) {
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
    pub fn palette(&self) -> Vec<(&'static str, &'static str)> {
        let text = self.editor.text();
        if !self.prompt.as_ref().is_some_and(|prompt| prompt.history)
            || !text.starts_with('/')
            || text.contains(char::is_whitespace)
        {
            return vec![];
        }
        COMMANDS
            .iter()
            .copied()
            .filter(|(name, _)| name.starts_with(&text))
            .collect()
    }
    pub fn submitted(&mut self, prompt: &Prompt, text: &str) {
        self.messages.push(Message {
            kind: if prompt.history {
                Kind::User
            } else {
                Kind::Note
            },
            text: safe_text(text),
        });
        if prompt.history
            && !text.trim().is_empty()
            && self.history.back().map(String::as_str) != Some(text)
        {
            self.history.push_back(text.to_owned());
            if self.history.len() > 100 {
                self.history.pop_front();
            }
        }
        self.phase = Phase::Working;
        self.turn_started = Instant::now();
        self.scroll = 0;
        self.editor = Editor::default();
        self.palette_index = 0;
    }
    pub fn preview(stage: usize) -> Self {
        let mut app = Self::new("~/projects/deepseek-harness-cli".into());
        app.demo = true;
        app.apply(Frame::Session {
            cwd: app.cwd.clone(),
            id: "session-preview".into(),
            provider: Some("deepseek-official".into()),
            model: Some("deepseek-v4-flash".into()),
        });
        if stage > 0 {
            app.messages.push(Message {
                kind: Kind::User,
                text: "Find why the session tests fail and make a focused fix.".into(),
            });
            app.apply(Frame::Assistant { text: "I'll inspect the failing tests and the session lifecycle, then run the relevant checks.".into() });
            app.apply(Frame::Tool {
                id: "read".into(),
                name: "read_file".into(),
                arguments: r#"{"path":"src/session/store.rs"}"#.into(),
            });
            app.apply(Frame::ToolResult {
                id: "read".into(),
                failed: false,
                text: "Read 124 lines".into(),
            });
            app.apply(Frame::Tool {
                id: "test".into(),
                name: "bash".into(),
                arguments: r#"{"command":"cargo test session::tests"}"#.into(),
            });
            app.apply(Frame::ToolResult {
                id: "test".into(),
                failed: false,
                text: "12 passed; 0 failed".into(),
            });
            app.apply(Frame::Assistant { text: "## Session restore is fixed\nThe writer now flushes before the session lock is released.\n\n```rust\nstore.flush().await?;\ndrop(session_lock);\n```\n\n**Validation:** all 12 session tests passed.\nThe change is limited to `src/session/store.rs`.".into() });
        }
        app.apply(Frame::Prompt {
            id: 1,
            text: if stage == 2 {
                "Allow bash?\nRun cargo test --workspace in this project.\n[y/N] "
            } else {
                "dsh> "
            }
            .into(),
            history: stage != 2,
        });
        app
    }
}

fn span(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(color))
}
fn bold(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(
        text.into(),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}
fn inline(text: &str) -> Vec<Span<'static>> {
    let mut result = vec![];
    let mut remaining = text;
    while !remaining.is_empty() {
        let marker = remaining
            .find('`')
            .map(|i| (i, "`", BLUE))
            .into_iter()
            .chain(remaining.find("**").map(|i| (i, "**", TEXT)))
            .min_by_key(|value| value.0);
        let Some((index, token, color)) = marker else {
            result.push(span(remaining, TEXT));
            break;
        };
        let after = &remaining[index + token.len()..];
        let Some(end) = after.find(token) else {
            result.push(span(remaining, TEXT));
            break;
        };
        result.push(span(&remaining[..index], TEXT));
        result.push(bold(&after[..end], color));
        remaining = &after[end + token.len()..];
    }
    result
}

fn markdown(text: &str) -> Vec<Line<'static>> {
    let mut code = false;
    text.lines()
        .map(|line| {
            if let Some(language) = line.strip_prefix("```") {
                code = !code;
                return if code {
                    Line::from(vec![
                        span("  ┌─ ", BORDER),
                        span(
                            if language.is_empty() {
                                "code"
                            } else {
                                language
                            },
                            MUTED,
                        ),
                    ])
                } else {
                    Line::from(span("  └─", BORDER))
                };
            }
            if code {
                let color = if line.starts_with('+') {
                    GREEN
                } else if line.starts_with('-') {
                    RED
                } else {
                    BLUE
                };
                return Line::from(vec![span("  │ ", BORDER), span(line, color)]);
            }
            if line.starts_with('#') {
                return Line::from(bold(line.trim_start_matches('#').trim(), TEXT));
            }
            let mut spans = vec![span("  ", TEXT)];
            spans.extend(inline(line));
            Line::from(spans)
        })
        .collect()
}

fn transcript(app: &App) -> Vec<Line<'static>> {
    let mut lines = vec![Line::default()];
    for message in &app.messages {
        match &message.kind {
            Kind::User => {
                lines.push(Line::from(bold("  YOU", BLUE)));
                lines.extend(
                    message
                        .text
                        .lines()
                        .map(|line| Line::from(span(format!("  {line}"), TEXT))),
                );
            }
            Kind::Assistant => {
                if message.text.is_empty() {
                    continue;
                }
                lines.push(Line::from(bold("  ◆ DSH", GREEN)));
                lines.extend(markdown(&message.text));
            }
            Kind::Tool {
                name,
                arguments,
                result,
                ..
            } => {
                let (symbol, color, status) = match result {
                    Some(false) => ("✓", GREEN, "done"),
                    Some(true) => ("×", RED, "failed"),
                    None => ("◌", BLUE, "running"),
                };
                lines.push(Line::from(vec![
                    span(format!("  {symbol} "), color),
                    bold(name, TEXT),
                    span(format!("  {status}"), color),
                ]));
                let value: Option<serde_json::Value> = serde_json::from_str(arguments).ok();
                let summary = value
                    .as_ref()
                    .and_then(|v| {
                        ["command", "path", "file_path", "pattern"]
                            .iter()
                            .find_map(|key| v.get(*key).and_then(serde_json::Value::as_str))
                    })
                    .unwrap_or(arguments);
                if app.expanded_tools {
                    let arguments = value
                        .as_ref()
                        .and_then(|value| serde_json::to_string_pretty(value).ok())
                        .unwrap_or_else(|| arguments.clone());
                    lines.extend(
                        arguments
                            .lines()
                            .map(|line| Line::from(span(format!("    {line}"), MUTED))),
                    );
                    lines.extend(message.text.lines().map(|line| {
                        Line::from(span(
                            format!("    {line}"),
                            if *result == Some(true) { RED } else { MUTED },
                        ))
                    }));
                } else {
                    lines.push(Line::from(span(
                        format!("    {}", summary.lines().next().unwrap_or("")),
                        MUTED,
                    )));
                    if *result == Some(true) {
                        lines.extend(
                            message
                                .text
                                .lines()
                                .take(3)
                                .map(|line| Line::from(span(format!("    {line}"), RED))),
                        );
                    }
                }
            }
            Kind::Request => {
                lines.push(Line::from(bold("  ◇ YOUR INPUT", AMBER)));
                lines.extend(
                    message
                        .text
                        .lines()
                        .map(|line| Line::from(span(format!("  {line}"), AMBER))),
                );
            }
            Kind::Note => lines.extend(
                message
                    .text
                    .lines()
                    .map(|line| Line::from(span(format!("  {line}"), MUTED))),
            ),
        }
        lines.push(Line::default());
    }
    lines
}

fn box_style(title: &str, color: Color) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .title(Line::from(bold(format!(" {title} "), color)))
        .style(Style::default().bg(PANEL))
}

pub fn render(frame: &mut Screen<'_>, app: &App) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(BG).fg(TEXT)),
        area,
    );
    if area.width < 28 || area.height < 10 {
        frame.render_widget(
            Paragraph::new("DSH\nEnlarge the terminal to at least 28 × 10.\nCtrl+D exits.")
                .style(Style::default().fg(AMBER))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let margin = if area.width >= 85 { 3 } else { 1 };
    let content = Rect::new(
        margin,
        0,
        area.width.saturating_sub(margin * 2),
        area.height,
    );
    let (input_lines, cursor) = app.editor.layout(content.width.saturating_sub(4) as usize);
    let input_height = (input_lines.len() as u16).clamp(2, 6) + 2;
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(input_height),
        Constraint::Length(2),
    ])
    .split(content);
    header(frame, app, chunks[0]);
    if app.show_diagnostics {
        diagnostics(frame, app, chunks[1]);
    } else if app.messages.is_empty() {
        welcome(frame, app, chunks[1]);
    } else {
        conversation(frame, app, chunks[1]);
    }
    status(frame, app, chunks[2]);
    composer(frame, app, chunks[3], input_lines, cursor);
    footer(frame, app, chunks[4]);
    let palette = app.palette();
    if !palette.is_empty() && !app.show_help {
        let height = (palette.len() as u16 + 2).min(chunks[1].height);
        let rect = Rect::new(
            chunks[3].x,
            chunks[3].y.saturating_sub(height + 1),
            chunks[3].width.min(70),
            height,
        );
        frame.render_widget(Clear, rect);
        let lines: Vec<Line> = palette
            .iter()
            .enumerate()
            .map(|(i, (name, detail))| {
                let selected = i == app.palette_index.min(palette.len() - 1);
                Line::from(vec![
                    bold(
                        format!(" {} {:<14}", if selected { "›" } else { " " }, name),
                        if selected { BLUE } else { TEXT },
                    ),
                    span(*detail, MUTED),
                ])
                .style(Style::default().bg(if selected { BORDER } else { PANEL }))
            })
            .collect();
        frame.render_widget(
            Paragraph::new(lines).block(box_style("Commands · Tab to complete", BLUE)),
            rect,
        );
    }
    if app.show_help {
        help(frame, area);
    }
}

fn header(frame: &mut Screen<'_>, app: &App, area: Rect) {
    let name = Path::new(&app.cwd)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&app.cwd);
    let title = Line::from(vec![
        bold(" ◆ dsh ", BLUE),
        span(" / ", BORDER),
        span(name, TEXT),
    ]);
    frame.render_widget(
        Paragraph::new(title).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(BORDER)),
        ),
        area,
    );
    let tag = if app.demo {
        "UI PREVIEW · NO MODEL CONNECTED"
    } else {
        "TERMINAL AGENT"
    };
    if area.width > 65 {
        frame.render_widget(
            Paragraph::new(tag)
                .alignment(Alignment::Right)
                .style(Style::default().fg(if app.demo { AMBER } else { MUTED })),
            Rect::new(area.x + area.width / 2, area.y, area.width / 2, 1),
        );
    }
}

fn welcome(frame: &mut Screen<'_>, app: &App, area: Rect) {
    let top = area.y + area.height.saturating_sub(14) / 2;
    let mut lines = vec![
        Line::from(bold("     ▄▀▀▄  ▄▀▀▀  █  █", BLUE)),
        Line::from(bold("     █  █  ▀▀▀▄  █▀▀█", BLUE)),
        Line::from(bold("     ▀▀▀   ▀▀▀   ▀  ▀", BLUE)),
        Line::default(),
        Line::from(bold("     Your project. Your next move.", TEXT)),
        Line::from(span(
            "     Inspect code, make a change, verify the result.",
            MUTED,
        )),
        Line::default(),
        Line::from(vec![span("     Workspace  ", MUTED), span(&app.cwd, TEXT)]),
    ];
    if !app.model.is_empty() {
        lines.push(Line::from(vec![
            span("     Model      ", MUTED),
            span(&app.model, BLUE),
        ]));
    }
    lines.extend([
        Line::default(),
        Line::from(span("     Start with a task", MUTED)),
        Line::from(span("     › Explain how this project is organized", TEXT)),
        Line::from(span("     › Find the cause of a failing test", TEXT)),
        Line::from(span("     › Review my changes before I commit", TEXT)),
    ]);
    if area.width < 60 {
        for line in &mut lines {
            for span in &mut line.spans {
                span.content = span.content.trim_start().to_owned().into();
            }
        }
    }
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        Rect::new(area.x, top, area.width, area.bottom().saturating_sub(top)),
    );
}

fn conversation(frame: &mut Screen<'_>, app: &App, area: Rect) {
    let lines = transcript(app);
    let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    let total = paragraph.line_count(area.width);
    let bottom = total.saturating_sub(area.height as usize);
    let offset = bottom.saturating_sub(app.scroll).min(u16::MAX as usize) as u16;
    frame.render_widget(paragraph.scroll((offset, 0)), area);
    if app.scroll > 0 {
        frame.render_widget(
            Paragraph::new(" ↑ History · Ctrl+End returns to latest ")
                .style(Style::default().fg(AMBER).bg(PANEL))
                .alignment(Alignment::Right),
            Rect::new(area.x, area.y, area.width, 1),
        );
    }
}

fn diagnostics(frame: &mut Screen<'_>, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(bold("Runtime diagnostics", AMBER)),
        Line::from(span(
            "F2 returns to the conversation · PgUp/PgDn scroll",
            MUTED,
        )),
        Line::default(),
    ];
    if app.diagnostics.is_empty() {
        lines.push(Line::from(span("No runtime diagnostics recorded.", GREEN)));
    }
    lines.extend(
        app.diagnostics
            .iter()
            .map(|line| Line::from(span(line, MUTED))),
    );
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let offset = paragraph
        .line_count(area.width)
        .saturating_sub(area.height as usize)
        .saturating_sub(app.scroll)
        .min(u16::MAX as usize) as u16;
    frame.render_widget(paragraph.scroll((offset, 0)), area);
}

fn status(frame: &mut Screen<'_>, app: &App, area: Rect) {
    let spin = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
        [(app.started.elapsed().as_millis() / 100 % 10) as usize];
    let (text, color) = match app.phase {
        Phase::Starting => (format!(" {spin} Connecting to your workspace…"), BLUE),
        Phase::Ready => (" ● Ready".into(), GREEN),
        Phase::Working => (
            format!(
                " {spin} Working · {}s · Ctrl+C to stop",
                app.turn_started.elapsed().as_secs()
            ),
            BLUE,
        ),
        Phase::Question => (" ◇ Waiting for your answer".into(), AMBER),
        Phase::Closing => (format!(" {spin} Saving session…"), MUTED),
    };
    frame.render_widget(Paragraph::new(span(text, color)), area);
    if !app.diagnostics.is_empty() && area.width > 65 {
        frame.render_widget(
            Paragraph::new(span("Runtime diagnostics · F2 ", AMBER)).alignment(Alignment::Right),
            Rect::new(area.x + area.width / 2, area.y, area.width / 2, area.height),
        );
    }
}

fn composer(
    frame: &mut Screen<'_>,
    app: &App,
    area: Rect,
    lines: Vec<String>,
    cursor: (usize, usize),
) {
    let question = app.prompt.as_ref().filter(|prompt| !prompt.history);
    let (title, color) = if let Some(prompt) = question {
        if prompt.text.contains("[y/N]") {
            ("Permission · y allows once · Enter denies", AMBER)
        } else {
            ("Your answer", AMBER)
        }
    } else {
        ("Message", if app.prompt.is_some() { BLUE } else { BORDER })
    };
    let block = box_style(title, color).padding(ratatui::widgets::Padding::horizontal(1));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if app.editor.chars.is_empty() {
        let hint = if question.is_some() {
            "Type your answer…"
        } else if app.phase == Phase::Starting {
            "Opening your session…"
        } else if app.prompt.is_none() {
            "The agent is working. Ctrl+C stops the current turn."
        } else {
            "What would you like to build?  / for commands"
        };
        frame.render_widget(Paragraph::new(span(hint, MUTED)), inner);
    } else {
        let offset = cursor
            .1
            .saturating_sub(inner.height.saturating_sub(1) as usize);
        frame.render_widget(
            Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>())
                .style(Style::default().fg(TEXT))
                .scroll((offset as u16, 0)),
            inner,
        );
    }
    if app.prompt.is_some() && !app.show_help && inner.width > 0 && inner.height > 0 {
        let offset = cursor
            .1
            .saturating_sub(inner.height.saturating_sub(1) as usize);
        frame.set_cursor_position((
            inner.x + (cursor.0 as u16).min(inner.width.saturating_sub(1)),
            inner.y + (cursor.1 - offset) as u16,
        ));
    }
}

fn footer(frame: &mut Screen<'_>, app: &App, area: Rect) {
    let keys = if app.demo {
        " 1 welcome · 2 conversation · 3 permission · Esc exit"
    } else if area.width > 85 {
        " Enter send   Alt+Enter newline   PgUp/PgDn scroll   Ctrl+T tools   F1 help"
    } else {
        " Enter send · Alt+Enter newline · F1 help"
    };
    let model = if app.model.is_empty() {
        "Connecting…"
    } else {
        &app.model
    };
    let session = app
        .session
        .strip_prefix("session-")
        .unwrap_or(&app.session)
        .chars()
        .take(8)
        .collect::<String>();
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(span(keys, MUTED)),
            Line::from(span(format!(" {model}  ·  {session}"), MUTED)),
        ]),
        area,
    );
}

fn help(frame: &mut Screen<'_>, area: Rect) {
    let width = area.width.saturating_sub(4).min(72);
    let height = area.height.saturating_sub(2).min(22);
    let rect = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    let lines = vec![
        Line::default(),
        Line::from(bold(" Work from your terminal", TEXT)),
        Line::default(),
        Line::from(span(" Enter          Send your message", TEXT)),
        Line::from(span(
            " Alt+Enter      Insert a newline (Ctrl+J also works)",
            TEXT,
        )),
        Line::from(span(" ↑ / ↓          Browse prompt history", TEXT)),
        Line::from(span(
            " PgUp / PgDn    Scroll conversation or diagnostics",
            TEXT,
        )),
        Line::from(span(" Ctrl+End       Return to the latest message", TEXT)),
        Line::from(span(
            " Ctrl+T         Expand or collapse tool details",
            TEXT,
        )),
        Line::from(span(" F2             Open runtime diagnostics", TEXT)),
        Line::from(span(" Ctrl+C         Stop a turn; exit when idle", TEXT)),
        Line::from(span(" Ctrl+D         Save and exit", TEXT)),
        Line::default(),
        Line::from(span(" /session       Show your resume command", BLUE)),
        Line::from(span(
            " /clear         Clear display; keep session context",
            BLUE,
        )),
        Line::from(span(" /exit          Close the session", BLUE)),
        Line::default(),
        Line::from(span(" Esc or F1 closes this panel", MUTED)),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(box_style("DSH · shortcuts", BLUE))
            .wrap(Wrap { trim: false }),
        rect,
    );
}

pub fn preview_text(width: u16, height: u16) -> String {
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = ratatui::Terminal::new(backend).expect("in-memory terminal");
    terminal
        .draw(|frame| render(frame, &App::preview(0)))
        .expect("render preview");
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(app: &App, width: u16, height: u16) -> String {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn streamed_answer_is_replaced_by_committed_text_without_duplicates() {
        let mut app = App::preview(0);
        app.apply(Frame::AssistantStart);
        app.apply(Frame::AssistantDelta {
            text: "Grü".into()
        });
        app.apply(Frame::AssistantDelta {
            text: "ße 世界".into(),
        });
        assert_eq!(app.messages[0].text, "Grüße 世界");
        app.apply(Frame::Assistant {
            text: "Grüße 世界!".into(),
        });
        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].text, "Grüße 世界!");
        app.apply(Frame::AssistantStart);
        app.apply(Frame::AssistantDelta {
            text: "Next answer".into(),
        });
        assert_eq!(app.messages.len(), 2);
    }

    #[test]
    fn prompts_and_tools_keep_approval_answers_out_of_history() {
        let mut app = App::preview(2);
        let prompt = app.prompt.take().unwrap();
        app.submitted(&prompt, "yes");
        assert!(app.history.is_empty());
        app.apply(Frame::Tool {
            id: "fail".into(),
            name: "bash".into(),
            arguments: "cargo test".into(),
        });
        app.apply(Frame::ToolResult {
            id: "fail".into(),
            failed: true,
            text: "test failed".into(),
        });
        let rendered = screen(&app, 100, 32);
        assert!(rendered.contains("failed"));
        assert!(rendered.contains("test failed"));
        app.apply(Frame::Prompt {
            id: 8,
            text: "Answer".into(),
            history: false,
        });
        app.apply(Frame::Cancel { id: 7 });
        assert!(app.prompt.is_some());
        app.apply(Frame::Cancel { id: 8 });
        assert!(app.prompt.is_none());
    }

    #[test]
    fn layouts_handle_small_terminals_unicode_and_multiline_drafts() {
        for (width, height) in [(20, 8), (28, 10), (40, 18), (80, 24), (120, 40)] {
            for stage in 0..3 {
                let mut app = App::preview(stage);
                app.editor
                    .insert("你好\nGrüße\nthird\nfourth\nfifth\nsixth\nseventh");
                assert!(!screen(&app, width, height).is_empty());
                app.show_help = true;
                screen(&app, width, height);
                app.show_help = false;
                app.show_diagnostics = true;
                app.diagnose("fixture diagnostic".into());
                screen(&app, width, height);
            }
        }
        let app = App::preview(2);
        let rendered = screen(&app, 100, 32);
        assert!(rendered.contains("Waiting for your answer"));
        assert!(rendered.contains("Enter denies"));
        assert!(rendered.contains("NO MODEL CONNECTED"));
    }

    #[test]
    fn command_palette_and_diagnostics_are_visible_without_losing_draft() {
        let mut app = App::preview(0);
        app.editor.insert("/se");
        assert_eq!(
            app.palette(),
            vec![("/session", "Session identity and resume command")]
        );
        assert!(screen(&app, 100, 32).contains("Tab to complete"));
        app.editor.replace("my task");
        app.diagnose("missing module".into());
        app.show_diagnostics = true;
        let rendered = screen(&app, 100, 32);
        assert!(rendered.contains("missing module"));
        assert!(rendered.contains("my task"));
        assert_eq!(safe_text("a\x1b\x07\r\nb"), "a\nb");
    }
}
