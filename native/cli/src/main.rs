//! Native TTY frontend. Agent execution stays inside the supported DSH headless profile.
mod runtime;
mod terminal;

use std::{env, ffi::OsString, io::IsTerminal, path::PathBuf, process::ExitCode};

const HELP: &str = "DSH — terminal coding agent (Rust frontend)

Usage: dsh [--cwd PATH] [--resume SESSION]
       dsh <profile> [options]

  dsh                         Start a coding session in the current project
  dsh --resume SESSION        Continue a persisted session in its project
  dsh --cwd PATH              Open a session in another project
  dsh headless \"run tests\"    Run one task and exit (also accepts piped input)
  dsh web                     Start the existing web profile
  dsh --help                  Show this help
  dsh --version               Show the native frontend version

In a session: /help, /session, /multiline, /exit.
Ctrl+C stops the current turn; at the prompt it exits. Ctrl+D exits.
Arrow keys edit input and browse in-memory prompt history.

Requires the built fork's Node.js runtime. DSH_RUNTIME_JS overrides its entry
file; DSH_NODE overrides the Node executable. Set DEEPSEEK_API_KEY in your
environment or project .env. No web server is needed.
";

#[derive(Debug, PartialEq)]
enum Invocation {
    Help,
    Version,
    Terminal {
        cwd: Option<PathBuf>,
        resume: Option<OsString>,
    },
    Profile(Vec<OsString>),
}

fn parse(args: Vec<OsString>) -> Result<Invocation, String> {
    match args.first().and_then(|arg| arg.to_str()) {
        Some("--help" | "-h") => return Ok(Invocation::Help),
        Some("--version" | "-V") => return Ok(Invocation::Version),
        Some("--cwd" | "--resume") | None => {}
        _ => return Ok(Invocation::Profile(args)),
    }
    let mut cwd = None;
    let mut resume = None;
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        let value = args.next().filter(|arg| !arg.is_empty()).ok_or_else(|| {
            format!(
                "{} requires a value; see dsh --help",
                flag.to_string_lossy()
            )
        })?;
        match flag.to_str() {
            Some("--cwd") if cwd.is_none() => cwd = Some(PathBuf::from(value)),
            Some("--resume") if resume.is_none() => resume = Some(value),
            _ => {
                return Err(format!(
                    "unknown or repeated option: {}",
                    flag.to_string_lossy()
                ));
            }
        }
    }
    Ok(Invocation::Terminal { cwd, resume })
}

fn run() -> Result<u8, Box<dyn std::error::Error>> {
    match parse(env::args_os().skip(1).collect())? {
        Invocation::Help => {
            print!("{HELP}");
            Ok(0)
        }
        Invocation::Version => {
            println!("dsh {} (Rust frontend)", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        Invocation::Profile(args) => runtime::forward(args),
        Invocation::Terminal { cwd, resume } => {
            if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
                return Err(
                    "dsh needs a terminal; for pipes use dsh headless \"task\" or dsh headless -"
                        .into(),
                );
            }
            if let Some(cwd) = cwd {
                env::set_current_dir(cwd)?;
            }
            terminal::run(resume)
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("dsh: {}", terminal::safe_text(&error.to_string()));
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn no_arguments_start_a_session() {
        assert_eq!(
            parse(vec![]).unwrap(),
            Invocation::Terminal {
                cwd: None,
                resume: None
            }
        );
    }

    #[test]
    fn session_options_preserve_spaces() {
        assert_eq!(
            parse(args(&[
                "--cwd",
                "project with spaces",
                "--resume",
                "session-1"
            ]))
            .unwrap(),
            Invocation::Terminal {
                cwd: Some("project with spaces".into()),
                resume: Some("session-1".into())
            }
        );
        assert!(parse(args(&["--resume"])).is_err());
        assert!(parse(args(&["--cwd", "a", "--cwd", "b"])).is_err());
    }

    #[test]
    fn explicit_profiles_are_forwarded_verbatim() {
        let original = args(&["headless", "--session-id", "x", "a task; $(literal)"]);
        assert_eq!(
            parse(original.clone()).unwrap(),
            Invocation::Profile(original)
        );
    }
}
