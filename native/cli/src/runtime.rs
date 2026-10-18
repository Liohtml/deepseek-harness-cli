//! Shell-free profile launch and bounded ownership of the Node child.
use serde::{Deserialize, Serialize};
use std::{
    env,
    ffi::OsString,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Frame {
    Hello {
        version: u32,
    },
    Output {
        text: String,
    },
    Prompt {
        id: u64,
        text: String,
        history: bool,
    },
    Cancel {
        id: u64,
    },
    Error {
        message: String,
    },
    Session {
        cwd: String,
        id: String,
        provider: Option<String>,
        model: Option<String>,
    },
    AssistantStart,
    AssistantDelta {
        text: String,
    },
    Assistant {
        text: String,
    },
    Tool {
        id: String,
        name: String,
        arguments: String,
    },
    ToolResult {
        id: String,
        failed: bool,
        text: String,
    },
    Close,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Input<'a> {
    Answer { id: u64, text: &'a str },
    Interrupt,
    Close,
}

pub enum Event {
    Frame(Frame),
    Diagnostic(String),
    Failure(String),
    Eof,
}

fn command() -> Result<Command, Box<dyn std::error::Error>> {
    let runtime = env::var_os("DSH_RUNTIME_JS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../apps/cli/lib/bin.js"
            ))
        });
    if !runtime.is_file() {
        return Err(format!("runtime entry missing: {}. Build this fork with pnpm install && pnpm run build, then reinstall; or set DSH_RUNTIME_JS to its built apps/cli/lib/bin.js", runtime.display()).into());
    }
    let mut command = Command::new(env::var_os("DSH_NODE").unwrap_or_else(|| "node".into()));
    command.arg(runtime);
    Ok(command)
}

pub fn forward(args: Vec<OsString>) -> Result<u8, Box<dyn std::error::Error>> {
    let mut command = command()?;
    command.args(args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec().into())
    }
    #[cfg(not(unix))]
    {
        Ok(command
            .status()?
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(1))
    }
}

pub struct Runtime {
    child: Child,
    input: Option<ChildStdin>,
    pub events: Receiver<Event>,
}

impl Runtime {
    pub fn start(resume: Option<OsString>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut command = command()?;
        command.args(["headless", "--terminal-bridge"]);
        if let Some(id) = resume {
            command.arg("--session-id").arg(id);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let input = child.stdin.take();
        let output = child.stdout.take().expect("piped stdout");
        let diagnostics = child.stderr.take().expect("piped stderr");
        let (sender, events) = mpsc::channel();
        let errors = sender.clone();
        thread::spawn(move || {
            for line in BufReader::new(diagnostics).lines() {
                match line {
                    Ok(line) => {
                        if errors.send(Event::Diagnostic(line)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = errors.send(Event::Failure(format!("runtime stderr: {error}")));
                        break;
                    }
                }
            }
        });
        thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let frame = match line {
                    Ok(line) => match serde_json::from_str(&line) {
                        Ok(frame) => Event::Frame(frame),
                        Err(error) => Event::Failure(format!("invalid terminal protocol: {error}")),
                    },
                    Err(error) => Event::Failure(format!("runtime stdout: {error}")),
                };
                if sender.send(frame).is_err() {
                    return;
                }
            }
            let _ = sender.send(Event::Eof);
        });
        Ok(Self {
            child,
            input,
            events,
        })
    }

    pub fn send(&mut self, input: Input<'_>) -> Result<(), Box<dyn std::error::Error>> {
        let pipe = self.input.as_mut().ok_or("runtime input already closed")?;
        serde_json::to_writer(&mut *pipe, &input)?;
        pipe.write_all(b"\n")?;
        pipe.flush()?;
        Ok(())
    }

    pub fn finish(&mut self) -> Result<u8, Box<dyn std::error::Error>> {
        self.input.take();
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(status) = self.child.try_wait()? {
                return Ok(status
                    .code()
                    .and_then(|code| u8::try_from(code).ok())
                    .unwrap_or(1));
            }
            if Instant::now() >= deadline {
                return Err(
                    "runtime did not shut down within 8 seconds; session flush may be incomplete"
                        .into(),
                );
            }
            thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_requires_known_frames_and_typed_prompt_ids() {
        assert!(matches!(
            serde_json::from_str::<Frame>(r#"{"type":"hello","version":2}"#).unwrap(),
            Frame::Hello { version: 2 }
        ));
        assert!(
            serde_json::from_str::<Frame>(
                r#"{"type":"prompt","id":"1","text":"yes?","history":false}"#
            )
            .is_err()
        );
        assert!(serde_json::from_str::<Frame>(r#"{"type":"unexpected"}"#).is_err());
        assert_eq!(
            serde_json::to_string(&Input::Answer {
                id: 2,
                text: "a\nb"
            })
            .unwrap(),
            r#"{"type":"answer","id":2,"text":"a\nb"}"#
        );
    }
}
