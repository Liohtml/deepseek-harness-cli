//! Full-screen terminal ownership, keyboard dispatch, and bounded runtime shutdown.
use crate::{
    editor::Editor,
    runtime::{Event, Frame, Input, Runtime},
    ui::{self, App, Phase},
};
use crossterm::{
    cursor::Show,
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event as KeyEvent, KeyCode,
        KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    ffi::OsString,
    io,
    time::{Duration, Instant},
};

pub use ui::safe_text;

struct Screen {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}
impl Screen {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        // Restore even if terminal initialization fails after raw mode was enabled.
        let result = (|| {
            let mut output = io::stdout();
            execute!(output, EnterAlternateScreen, EnableBracketedPaste)?;
            Ok(Self {
                terminal: Terminal::new(CrosstermBackend::new(output))?,
            })
        })();
        if result.is_err() {
            restore();
        }
        result
    }
    fn draw(&mut self, app: &App) -> io::Result<()> {
        self.terminal.draw(|frame| ui::render(frame, app))?;
        Ok(())
    }
}
fn restore() {
    let _ = execute!(
        io::stdout(),
        DisableBracketedPaste,
        LeaveAlternateScreen,
        Show
    );
    let _ = terminal::disable_raw_mode();
}
impl Drop for Screen {
    fn drop(&mut self) {
        restore();
    }
}

pub fn run(resume: Option<OsString>, demo: bool) -> Result<u8, Box<dyn std::error::Error>> {
    let mut app = if demo {
        App::preview(0)
    } else {
        App::new(safe_text(&std::env::current_dir()?.to_string_lossy()))
    };
    let mut screen = Screen::enter()?;
    screen.draw(&app)?;
    let mut runtime = if demo {
        None
    } else {
        Some(Runtime::start(resume)?)
    };
    let result = interact(&mut runtime, &mut screen, &mut app);
    app.phase = Phase::Closing;
    app.prompt = None;
    let _ = screen.draw(&app);
    if result.is_err()
        && let Some(runtime) = &mut runtime
    {
        let _ = runtime.send(Input::Close);
    }
    let status = if let Some(runtime) = &mut runtime {
        runtime.finish()
    } else {
        Ok(0)
    };
    drop(screen);
    if result.is_err() || status.is_err() {
        for line in app
            .diagnostics
            .iter()
            .skip(app.diagnostics.len().saturating_sub(12))
        {
            eprintln!("{line}");
        }
    }
    result?;
    let status = status?;
    if !app.session.is_empty() && !demo {
        println!("Session: {}", app.session);
    }
    Ok(status)
}

fn submit(runtime: &mut Option<Runtime>, app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let Some(prompt) = app.prompt.take() else {
        return Ok(());
    };
    let text = app.editor.text();
    if prompt.history {
        if text.trim().is_empty() {
            app.prompt = Some(prompt);
            return Ok(());
        }
        match text.trim() {
            "/help" => {
                app.show_help = true;
                app.editor = Editor::default();
                app.prompt = Some(prompt);
                return Ok(());
            }
            "/diagnostics" => {
                app.show_diagnostics = !app.show_diagnostics;
                app.scroll = 0;
                app.editor = Editor::default();
                app.prompt = Some(prompt);
                return Ok(());
            }
            "/clear" => {
                app.messages.clear();
                app.scroll = 0;
                app.editor = Editor::default();
                app.prompt = Some(prompt);
                return Ok(());
            }
            _ => {}
        }
    }
    app.submitted(&prompt, &text);
    if let Some(runtime) = runtime {
        runtime.send(Input::Answer {
            id: prompt.id,
            text: &text,
        })?;
    } else {
        app.note("UI preview only. Start dsh without --demo to work with the agent.".into());
        app.apply(Frame::Prompt {
            id: prompt.id + 1,
            text: "dsh> ".into(),
            history: true,
        });
    }
    Ok(())
}

fn interact(
    runtime: &mut Option<Runtime>,
    screen: &mut Screen,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let mut initialized = app.demo;
    let mut closing: Option<Instant> = None;
    let mut last_draw = Instant::now();
    loop {
        let mut dirty = false;
        if let Some(runtime) = runtime {
            for _ in 0..100 {
                let Ok(event) = runtime.events.try_recv() else {
                    break;
                };
                dirty = true;
                match event {
                    Event::Frame(Frame::Hello { version: 2 }) if !initialized => initialized = true,
                    Event::Frame(Frame::Hello { version }) => return Err(format!("terminal protocol mismatch (received {version}, expected 2). Rebuild the Rust frontend and runtime together; see native/cli/README.md").into()),
                    Event::Frame(_) if !initialized => return Err("runtime omitted terminal handshake".into()),
                    Event::Frame(Frame::Error { message }) | Event::Failure(message) => return Err(message.into()),
                    Event::Frame(Frame::Close) => return Ok(()),
                    Event::Frame(frame @ Frame::Prompt { .. }) => {
                        if closing.is_none() {
                            // Input queued before this request cannot become a permission answer.
                            while event::poll(Duration::ZERO)? { let _ = event::read()?; }
                            app.apply(frame);
                        }
                    }
                    Event::Frame(frame) => app.apply(frame),
                    Event::Diagnostic(text) => app.diagnose(text),
                    Event::Eof => return Err("runtime closed unexpectedly; see diagnostics above".into()),
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
        if event::poll(Duration::from_millis(30))? {
            dirty = true;
            match event::read()? {
                KeyEvent::Key(key) if key.kind != KeyEventKind::Release => {
                    if app.demo
                        && (matches!(key.code, KeyCode::Esc)
                            || key.modifiers.contains(KeyModifiers::CONTROL)
                                && matches!(key.code, KeyCode::Char('c' | 'd')))
                    {
                        return Ok(());
                    }
                    if app.demo
                        && app.editor.chars.is_empty()
                        && let KeyCode::Char(ch @ '1'..='3') = key.code
                    {
                        *app = App::preview(ch as usize - '1' as usize);
                    }
                    if key.code == KeyCode::F(1) {
                        app.show_help = !app.show_help;
                    } else if key.code == KeyCode::Esc {
                        app.show_help = false;
                        app.show_diagnostics = false;
                        app.scroll = 0;
                    } else if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c')
                    {
                        if closing.is_some() {
                            return Err("forced exit; session flush may be incomplete".into());
                        }
                        app.editor = Editor::default();
                        if let Some(runtime) = runtime {
                            if matches!(app.phase, Phase::Starting | Phase::Ready) {
                                runtime.send(Input::Close)?;
                                closing.get_or_insert(Instant::now());
                                app.phase = Phase::Closing;
                                app.prompt = None;
                            } else {
                                runtime.send(Input::Interrupt)?;
                            }
                        }
                    } else if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('d')
                    {
                        if let Some(runtime) = runtime {
                            runtime.send(Input::Close)?;
                        }
                        closing.get_or_insert(Instant::now());
                        app.phase = Phase::Closing;
                        app.prompt = None;
                    } else if app.show_help { /* Modal help does not consume the draft. */
                    } else if key.code == KeyCode::F(2) {
                        app.show_diagnostics = !app.show_diagnostics;
                        app.scroll = 0;
                    } else if key.code == KeyCode::PageUp {
                        app.scroll = app.scroll.saturating_add(10);
                    } else if key.code == KeyCode::PageDown {
                        app.scroll = app.scroll.saturating_sub(10);
                    } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match key.code {
                            KeyCode::Char('t') => app.expanded_tools = !app.expanded_tools,
                            KeyCode::End => app.scroll = 0,
                            KeyCode::Char('a') => app.editor.cursor = 0,
                            KeyCode::Char('e') => app.editor.cursor = app.editor.chars.len(),
                            KeyCode::Char('u') => app.editor = Editor::default(),
                            KeyCode::Char('j') if app.prompt.is_some() => app.editor.insert("\n"),
                            _ => {}
                        }
                    } else if app.prompt.is_some() {
                        let palette = app.palette();
                        match key.code {
                            KeyCode::Enter
                                if key.modifiers.contains(KeyModifiers::ALT)
                                    || key.modifiers.contains(KeyModifiers::SHIFT) =>
                            {
                                app.editor.insert("\n")
                            }
                            KeyCode::Enter => submit(runtime, app)?,
                            KeyCode::Tab if !palette.is_empty() => app
                                .editor
                                .replace(palette[app.palette_index.min(palette.len() - 1)].0),
                            KeyCode::Up if !palette.is_empty() => {
                                app.palette_index = app.palette_index.saturating_sub(1)
                            }
                            KeyCode::Down if !palette.is_empty() => {
                                app.palette_index = (app.palette_index + 1).min(palette.len() - 1)
                            }
                            KeyCode::Char(ch)
                                if !(app.demo
                                    && app.editor.chars.is_empty()
                                    && ('1'..='3').contains(&ch)) =>
                            {
                                app.editor.insert(&ch.to_string());
                                app.palette_index = 0;
                            }
                            KeyCode::Backspace => {
                                app.editor.backspace();
                                app.palette_index = 0;
                            }
                            KeyCode::Delete if app.editor.cursor < app.editor.chars.len() => {
                                app.editor.chars.remove(app.editor.cursor);
                            }
                            KeyCode::Left => {
                                app.editor.cursor = app.editor.cursor.saturating_sub(1)
                            }
                            KeyCode::Right => {
                                app.editor.cursor =
                                    (app.editor.cursor + 1).min(app.editor.chars.len())
                            }
                            KeyCode::Home => app.editor.cursor = 0,
                            KeyCode::End => app.editor.cursor = app.editor.chars.len(),
                            KeyCode::Up => app.history(true),
                            KeyCode::Down => app.history(false),
                            _ => {}
                        }
                    }
                }
                KeyEvent::Paste(text) if app.prompt.is_some() && !app.show_help => {
                    app.editor.insert(&text)
                }
                KeyEvent::Resize(_, _) => {}
                _ => {}
            }
        }
        let animating = matches!(app.phase, Phase::Starting | Phase::Working | Phase::Closing);
        if dirty || animating && last_draw.elapsed() >= Duration::from_millis(100) {
            screen.draw(app)?;
            last_draw = Instant::now();
        }
    }
}
