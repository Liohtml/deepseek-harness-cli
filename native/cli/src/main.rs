//! Native TTY frontend. Agent execution stays inside the supported DSH headless profile.
mod editor;
mod runtime;
mod superagent;
mod terminal;
mod ui;

use std::{env, ffi::OsString, io::IsTerminal, path::PathBuf, process::ExitCode};

const HELP: &str = "DSH — terminal coding agent (Rust frontend)

Usage: dsh [--cwd PATH] [--resume SESSION] [policy options]
       dsh <profile> [options]

  dsh                         Start a coding session in the current project
  dsh --resume SESSION        Continue a persisted session in its project
  dsh --cwd PATH              Open a session in another project
  dsh --permission-mode NAME  Select a configured permission preset
  dsh --approval-policy MODE  Set approval behavior: ask or never
  dsh --add-dir PATH          Allow writes to another absolute directory (repeatable)
  dsh headless \"run tests\"    Run one task and exit (also accepts piped input)
  dsh web                     Start the existing web profile
  dsh superagent install      Install the dsh-superagent bundle in its own profile
  dsh superagent start|stop|restart|status
                              Operate the installed superagent service
  dsh --demo                  Preview the UI without a model or API key
  dsh --help                  Show this help
  dsh --version               Show the native frontend version

In a session: /login, /logout, /status, /model, /check, /sessions, /help.
/login opens hidden API key input. Never pass keys as command arguments.
Ctrl+P opens commands; /check sends a short billable API request.
Ctrl+C stops the current turn; at the prompt it exits. Ctrl+D exits.
Alt+Enter inserts a newline. F1 shows shortcuts, F2 opens diagnostics.
PgUp/PgDn scroll; Ctrl+T expands tool details.

Requires the built fork's Node.js runtime. DSH_RUNTIME_JS overrides its entry
file; DSH_NODE overrides the Node executable. Set DEEPSEEK_API_KEY in your
environment or project .env. No web server is needed.
";

#[derive(Debug, PartialEq)]
enum Invocation {
    Help,
    Version,
    Demo,
    Terminal {
        cwd: Option<PathBuf>,
        resume: Option<OsString>,
        /// Policy flags passed unchanged to the headless runtime.
        policy: Vec<OsString>,
    },
    /// Management verb and its options for the superagent service.
    Superagent(Vec<OsString>),
    Profile(Vec<OsString>),
}

fn parse(args: Vec<OsString>) -> Result<Invocation, String> {
    match args.first().and_then(|arg| arg.to_str()) {
        Some("--help" | "-h") => return Ok(Invocation::Help),
        Some("--demo") => return Ok(Invocation::Demo),
        Some("--version" | "-V") => return Ok(Invocation::Version),
        Some("--cwd" | "--resume" | "--permission-mode" | "--approval-policy" | "--add-dir")
        | None => {}
        Some("superagent")
            if args
                .get(1)
                .and_then(|verb| verb.to_str())
                .is_some_and(|verb| superagent::VERBS.contains(&verb)) =>
        {
            return Ok(Invocation::Superagent(args.into_iter().skip(1).collect()));
        }
        _ => return Ok(Invocation::Profile(args)),
    }
    let mut cwd = None;
    let mut resume = None;
    let mut policy = Vec::new();
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
            Some("--permission-mode" | "--approval-policy" | "--add-dir") => {
                policy.push(flag);
                policy.push(value);
            }
            _ => {
                return Err(format!(
                    "unknown or repeated option: {}",
                    flag.to_string_lossy()
                ));
            }
        }
    }
    Ok(Invocation::Terminal {
        cwd,
        resume,
        policy,
    })
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
        Invocation::Demo => {
            if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
                print!("{}", ui::preview_text(100, 32));
                return Ok(0);
            }
            terminal::run(None, Vec::new(), true)
        }
        Invocation::Superagent(args) => superagent::run(args),
        Invocation::Profile(args) => runtime::forward(args),
        Invocation::Terminal {
            cwd,
            resume,
            policy,
        } => {
            if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
                return Err(
                    "open dsh in a real terminal (not a captured shell command); use dsh --demo for a non-interactive UI preview. For pipes use dsh headless \"task\" or dsh headless -"
                        .into(),
                );
            }
            if let Some(cwd) = cwd {
                env::set_current_dir(cwd)?;
            }
            terminal::run(resume, policy, false)
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
                resume: None,
                policy: vec![]
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
                resume: Some("session-1".into()),
                policy: vec![]
            }
        );
        assert!(parse(args(&["--resume"])).is_err());
        assert!(parse(args(&["--cwd", "a", "--cwd", "b"])).is_err());
    }

    #[test]
    fn policy_options_reach_the_runtime_in_order() {
        assert_eq!(
            parse(args(&[
                "--permission-mode",
                "read-only",
                "--add-dir",
                "/a",
                "--add-dir",
                "/b"
            ]))
            .unwrap(),
            Invocation::Terminal {
                cwd: None,
                resume: None,
                policy: args(&[
                    "--permission-mode",
                    "read-only",
                    "--add-dir",
                    "/a",
                    "--add-dir",
                    "/b"
                ])
            }
        );
        assert!(parse(args(&["--approval-policy"])).is_err());
    }

    #[test]
    fn superagent_verbs_are_managed_and_other_arguments_launch_the_profile() {
        assert_eq!(
            parse(args(&["superagent", "status", "--url", "http://x"])).unwrap(),
            Invocation::Superagent(args(&["status", "--url", "http://x"]))
        );
        let launch = args(&["superagent", "--from-default-profile", "web"]);
        assert_eq!(parse(launch.clone()).unwrap(), Invocation::Profile(launch));
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
