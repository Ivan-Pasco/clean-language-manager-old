//! `cleen register` — optional sign-up for Clean Language news and
//! security alerts, plus a one-time hint after a successful install.
//!
//! The manager never collects personal data itself. It opens the sign-up
//! page on accounts.cleanlanguage.dev, which owns all user information
//! (name, email, phone, subscriptions, consent). The only thing passed in
//! the URL is where the visit came from and the cleen version.
//!
//! The hint is shown once, only in an interactive terminal, never in CI.
//! Set `CLEEN_REGISTER_HINT=off` to suppress it entirely.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const REGISTER_URL: &str = "https://accounts.cleanlanguage.dev/register";

/// Environment variables that mark a CI or automated build. Any of them
/// being set (to anything) suppresses the hint.
const CI_ENV_VARS: &[&str] = &[
    "CI",
    "CONTINUOUS_INTEGRATION",
    "GITHUB_ACTIONS",
    "GITLAB_CI",
    "BUILDKITE",
    "JENKINS_URL",
    "TF_BUILD",
];

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
struct State {
    #[serde(default)]
    hint_shown_at: Option<u64>,
    #[serde(default)]
    opened_at: Option<u64>,
}

/// Run `cleen register`: explain what it is, open the sign-up page in the
/// browser (unless `print_only` or there is no interactive terminal), and
/// always print the link so it can be copied.
pub fn register(print_only: bool) -> Result<()> {
    let url = register_url(env!("CARGO_PKG_VERSION"));

    println!("📬 Clean Language news and security alerts");
    println!();
    println!("   Sign up to get:");
    println!("   • Security alerts for the versions you use");
    println!("   • Early news about the new Clean Language toolchain");
    println!("   • Release notes (optional)");
    println!();
    println!("   Sign-up is optional and you can unsubscribe at any time.");
    println!();

    let opened = !print_only && std::io::stdout().is_terminal() && open_in_browser(&url);
    if opened {
        println!("🌐 Opened the sign-up page in your browser.");
        println!("   If it did not open, visit:");
    } else {
        println!("👉 Sign up here:");
    }
    println!("   {url}");

    let mut state = load_state();
    state.opened_at = Some(now_epoch_secs());
    save_state(&state);

    Ok(())
}

/// Print the one-time register hint after a successful install, if it
/// has not been shown before and the session is interactive.
pub fn maybe_show_hint() {
    let state = load_state();
    if !should_show_hint(&state, std::io::stdout().is_terminal(), &env_lookup) {
        return;
    }

    println!();
    println!("📬 A new Clean Language toolchain is coming soon.");
    println!("   Get early news and security alerts: cleen register (optional)");

    let mut state = state;
    state.hint_shown_at = Some(now_epoch_secs());
    save_state(&state);
}

fn should_show_hint(state: &State, is_tty: bool, env: &dyn Fn(&str) -> Option<String>) -> bool {
    if !is_tty || state.hint_shown_at.is_some() || state.opened_at.is_some() {
        return false;
    }
    let off = env("CLEEN_REGISTER_HINT")
        .unwrap_or_default()
        .to_lowercase();
    if matches!(off.as_str(), "0" | "off" | "false" | "no") {
        return false;
    }
    !CI_ENV_VARS.iter().any(|var| env(var).is_some())
}

fn register_url(cleen_version: &str) -> String {
    format!("{REGISTER_URL}?source=cleen&cleen_version={cleen_version}")
}

fn env_lookup(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn open_in_browser(url: &str) -> bool {
    let mut command = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(url);
        c
    } else if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "", url]);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn state_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".cleen").join("register.json"))
}

fn load_state() -> State {
    let Some(path) = state_path() else {
        return State::default();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return State::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Best-effort: a failed write only means the hint may show once more.
fn save_state(state: &State) {
    let Some(path) = state_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(&path, content);
    }
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn hint_shows_on_first_interactive_run() {
        assert!(should_show_hint(&State::default(), true, &env_from(&[])));
    }

    #[test]
    fn hint_hidden_when_not_a_terminal() {
        assert!(!should_show_hint(&State::default(), false, &env_from(&[])));
    }

    #[test]
    fn hint_hidden_after_it_was_shown() {
        let state = State {
            hint_shown_at: Some(1),
            opened_at: None,
        };
        assert!(!should_show_hint(&state, true, &env_from(&[])));
    }

    #[test]
    fn hint_hidden_after_register_was_run() {
        let state = State {
            hint_shown_at: None,
            opened_at: Some(1),
        };
        assert!(!should_show_hint(&state, true, &env_from(&[])));
    }

    #[test]
    fn hint_hidden_in_ci() {
        for var in CI_ENV_VARS {
            assert!(
                !should_show_hint(&State::default(), true, &env_from(&[(var, "true")])),
                "hint should be hidden when {var} is set"
            );
        }
    }

    #[test]
    fn hint_hidden_when_turned_off() {
        for value in ["0", "off", "OFF", "false", "no"] {
            assert!(!should_show_hint(
                &State::default(),
                true,
                &env_from(&[("CLEEN_REGISTER_HINT", value)])
            ));
        }
    }

    #[test]
    fn register_url_carries_only_source_and_version() {
        assert_eq!(
            register_url("1.2.3"),
            "https://accounts.cleanlanguage.dev/register?source=cleen&cleen_version=1.2.3"
        );
    }

    #[test]
    fn state_round_trips_through_json() {
        let state = State {
            hint_shown_at: Some(10),
            opened_at: Some(20),
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: State = serde_json::from_str(&json).unwrap();
        assert_eq!(back, state);
    }
}
