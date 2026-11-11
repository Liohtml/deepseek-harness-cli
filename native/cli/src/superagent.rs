//! `dsh superagent <verb>`: installs and operates the dsh-superagent bundle in its own `superagent` profile.
//! The bundle source is a pinned checkout under `$DSH_HOME/superagent/src`; service verbs delegate to its script.
use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::runtime;

/// Verbs handled here; any other `dsh superagent …` invocation launches the profile itself.
pub const VERBS: [&str; 5] = ["install", "start", "stop", "restart", "status"];

const REPOSITORY: &str = "https://github.com/deepseek-ai/dsh-superagent-prototype.git";
/// Reviewed bundle commit. Update it together with a compatibility check against this fork's runtime.
const REVISION: &str = "8c32a9dc5d72c033816c6a8110b37f40cf94fba4";
const PROFILE: &str = "superagent";
const PORT: u16 = 3082;

type Failure = Box<dyn std::error::Error>;

fn home() -> Result<PathBuf, Failure> {
    if let Some(home) = env::var_os("DSH_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    let user = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or("set DSH_HOME or HOME to locate the DSH home directory")?;
    Ok(PathBuf::from(user).join(".dsh"))
}

fn source(home: &Path) -> PathBuf {
    home.join("superagent").join("src")
}

/// Render `value` as a single-quoted YAML scalar.
fn yaml_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Machine configuration of a new profile: loopback panel port and the absolute state directory.
fn profile_patch(state: &Path) -> Result<String, Failure> {
    let state = state
        .to_str()
        .ok_or("the DSH home path must be valid UTF-8 for the superagent profile")?;
    Ok(format!(
        "- id: webserver\n  config:\n    host: 127.0.0.1\n    port: {PORT}\n- id: dsh-superagent\n  config:\n    stateDir: {}\n",
        yaml_quote(state)
    ))
}

fn step(program: &str, args: &[&OsStr], cwd: Option<&Path>) -> Result<(), Failure> {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let status = command
        .status()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed with {status}").into())
    }
}

fn output(program: &str, args: &[&OsStr]) -> Result<String, Failure> {
    let result = Command::new(program)
        .args(args)
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if !result.status.success() {
        return Err(format!("{program} failed with {}", result.status).into());
    }
    Ok(String::from_utf8_lossy(&result.stdout).trim().to_owned())
}

fn checkout(source: &Path) -> Result<(), Failure> {
    let dir = source.as_os_str();
    if !source.join(".git").exists() {
        if let Some(parent) = source.parent() {
            fs::create_dir_all(parent)?;
        }
        step("git", &["clone".as_ref(), REPOSITORY.as_ref(), dir], None)?;
    } else {
        step(
            "git",
            &["-C".as_ref(), dir, "fetch".as_ref(), "origin".as_ref()],
            None,
        )?;
    }
    step(
        "git",
        &[
            "-C".as_ref(),
            dir,
            "checkout".as_ref(),
            "--detach".as_ref(),
            REVISION.as_ref(),
        ],
        None,
    )?;
    let head = output(
        "git",
        &["-C".as_ref(), dir, "rev-parse".as_ref(), "HEAD".as_ref()],
    )?;
    if head != REVISION {
        return Err(format!("superagent checkout is at {head}, expected {REVISION}").into());
    }
    Ok(())
}

/// Node.js 24 names its Linux main thread `MainThread`, which `ps` reports as the command name.
/// The pinned service script recognizes only `node` and `dsh`, so it would report a running service as stopped.
fn accept_node_thread_name(script: &Path) -> Result<(), Failure> {
    const KNOWN: &str = "name in ('node', 'dsh')";
    const WIDENED: &str = "name in ('node', 'dsh', 'MainThread')";
    let text = fs::read_to_string(script)?;
    if text.contains(WIDENED) {
        return Ok(());
    }
    if !text.contains(KNOWN) {
        return Err(format!(
            "{} no longer contains the expected process check; review the pinned revision",
            script.display()
        )
        .into());
    }
    fs::write(script, text.replace(KNOWN, WIDENED))?;
    Ok(())
}

fn runtime_step(args: &[&OsStr], quiet: bool) -> Result<(), Failure> {
    let mut command = runtime::command()?;
    command.args(args);
    if quiet {
        command.stdout(Stdio::null());
    }
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("dsh runtime failed with {status}").into())
    }
}

fn install() -> Result<u8, Failure> {
    let home = home()?;
    let source = source(&home);
    let profile = home.join("profiles").join(PROFILE);
    checkout(&source)?;
    accept_node_thread_name(&source.join("scripts").join("dsh-superagent"))?;
    step(
        "pnpm",
        &["install".as_ref(), "--frozen-lockfile".as_ref()],
        Some(&source),
    )?;
    step("pnpm", &["build".as_ref()], Some(&source))?;
    // Creating the profile from the Web template replaces an existing one, so only a missing profile is created.
    let created = !profile.exists();
    if created {
        runtime_step(
            &[
                PROFILE.as_ref(),
                "--from-default-profile".as_ref(),
                "web".as_ref(),
                "--dump-config".as_ref(),
            ],
            true,
        )?;
        let state = home.join("state").join(PROFILE);
        fs::create_dir_all(&state)?;
        fs::write(profile.join("cordis.patch.yml"), profile_patch(&state)?)?;
    }
    let mut link = OsString::from("link:");
    link.push(&source);
    runtime_step(
        &[
            "plugin".as_ref(),
            "--profile".as_ref(),
            PROFILE.as_ref(),
            "add".as_ref(),
            &link,
        ],
        false,
    )?;
    println!(
        "dsh-superagent {} is installed in profile `{PROFILE}`{}.",
        &REVISION[..10],
        if created {
            ""
        } else {
            " (existing profile configuration kept)"
        }
    );
    println!(
        "Next: give the agent its own GitHub CLI login, then run `dsh superagent start` and open http://127.0.0.1:{PORT}."
    );
    println!(
        "Accounts, policy and execution settings: {}",
        source.join("SETUP.md").display()
    );
    Ok(0)
}

/// PATH with this executable's directory first, so the service script starts the same `dsh`.
fn service_path() -> Option<OsString> {
    let exe = env::current_exe().ok()?;
    let mut entries = vec![exe.parent()?.to_path_buf()];
    entries.extend(env::split_paths(&env::var_os("PATH")?));
    env::join_paths(entries).ok()
}

fn service(args: Vec<OsString>) -> Result<u8, Failure> {
    let script = source(&home()?).join("scripts").join("dsh-superagent");
    if !script.is_file() {
        return Err("dsh-superagent is not installed; run `dsh superagent install` first".into());
    }
    let mut command = Command::new("python3");
    command.arg(script).args(args);
    if let Some(path) = service_path() {
        command.env("PATH", path);
    }
    let status = command
        .status()
        .map_err(|error| format!("cannot run python3: {error}"))?;
    Ok(status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1))
}

/// Run one management verb; `args` starts with the verb.
pub fn run(args: Vec<OsString>) -> Result<u8, Failure> {
    match args.first().and_then(|verb| verb.to_str()) {
        Some("install") if args.len() == 1 => install(),
        Some("install") => Err("dsh superagent install takes no options".into()),
        _ => service(args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_patch_quotes_the_state_directory() {
        let patch = profile_patch(Path::new("/home/it's me/.dsh/state/superagent")).unwrap();
        assert!(patch.contains("stateDir: '/home/it''s me/.dsh/state/superagent'\n"));
        assert!(patch.contains("host: 127.0.0.1\n    port: 3082\n"));
    }

    #[test]
    fn source_lives_under_the_dsh_home() {
        assert_eq!(source(Path::new("/h")), PathBuf::from("/h/superagent/src"));
    }
}
