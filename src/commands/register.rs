//! `cleen register` — optional sign-up for Clean Language news and
//! security alerts, plus a one-time email question after a successful install.
//!
//! The manager stores no personal data. By default it opens the sign-up
//! page on account.cleanlanguage.dev, which owns all user information
//! (name, email, phone, subscriptions, consent). The only thing passed in
//! the URL is where the visit came from and the cleen version.
//!
//! `cleen register --email … --name …` signs up from the terminal instead:
//! it sends what the person typed to `POST /api/v1/register` on the same
//! service and keeps nothing locally. Nothing is subscribed until the
//! person clicks the confirmation email.
//!
//! After the first successful install, cleen asks once for an email
//! (Enter skips), only in an interactive terminal, never in CI.
//! Set `CLEEN_REGISTER_HINT=off` to suppress it entirely.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const REGISTER_URL: &str = "https://account.cleanlanguage.dev/register";
const REGISTER_API_URL: &str = "https://account.cleanlanguage.dev/api/v1/register";
const CURL_TIMEOUT_SECS: u64 = 15;

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

/// What the person typed for a terminal sign-up.
#[derive(Debug, Clone)]
pub struct EmailSignup {
    pub email: String,
    pub name: String,
    pub phone: Option<String>,
    pub releases: bool,
    pub news: bool,
}

#[derive(Debug, Serialize)]
struct SignupBody<'a> {
    email: &'a str,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<&'a str>,
    topics: Topics,
    source: &'static str,
    cleen_version: &'a str,
}

#[derive(Debug, Serialize)]
struct Topics {
    security: bool,
    releases: bool,
    news: bool,
}

#[derive(Debug, Default, Deserialize)]
struct SignupResponse {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    fields: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, PartialEq)]
enum SignupOutcome {
    ConfirmationSent,
    RecentlySent,
}

/// Run `cleen register --email … --name …`: sign up through the account
/// service's API, without a browser.
pub fn register_by_email(signup: EmailSignup) -> Result<()> {
    let payload = signup_payload(&signup, env!("CARGO_PKG_VERSION"))?;
    let (code, body) = post_signup(&payload)?;

    match interpret_response(code, &body)? {
        SignupOutcome::ConfirmationSent => {
            println!(
                "📬 Almost done. We sent a confirmation email to {}.",
                signup.email
            );
            println!("   Click the link in it to start getting security alerts and news.");
        }
        SignupOutcome::RecentlySent => {
            println!(
                "📬 A confirmation email was already sent to {} recently.",
                signup.email
            );
            println!("   Check your inbox (and spam folder) and click the link in it.");
        }
    }

    let mut state = load_state();
    state.opened_at = Some(now_epoch_secs());
    save_state(&state);

    Ok(())
}

fn signup_payload(signup: &EmailSignup, cleen_version: &str) -> Result<String> {
    let phone = signup
        .phone
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty());
    let body = SignupBody {
        email: signup.email.trim(),
        name: signup.name.trim(),
        phone,
        topics: Topics {
            security: true,
            releases: signup.releases,
            news: signup.news,
        },
        source: "cleen",
        cleen_version,
    };
    Ok(serde_json::to_string(&body)?)
}

/// POST the sign-up. The payload goes through stdin, not the command
/// line, so the person's email and phone never show up in a process list.
fn post_signup(payload: &str) -> Result<(u16, String)> {
    use std::io::Write;

    let unreachable = || {
        crate::error::CleenError::RegisterError {
        message: format!(
            "could not reach account.cleanlanguage.dev. Check your connection, or sign up in the browser: {}",
            register_url(env!("CARGO_PKG_VERSION"))
        ),
    }
    };

    let mut child = Command::new("curl")
        .arg("-sS")
        .arg("--max-time")
        .arg(CURL_TIMEOUT_SECS.to_string())
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg(format!("User-Agent: cleen/{}", env!("CARGO_PKG_VERSION")))
        .arg("--data-binary")
        .arg("@-")
        .arg("-w")
        .arg("\n%{http_code}")
        .arg(REGISTER_API_URL)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| unreachable())?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload.as_bytes())
            .map_err(|_| unreachable())?;
    }
    let out = child.wait_with_output().map_err(|_| unreachable())?;
    let combined = String::from_utf8_lossy(&out.stdout);
    let (body, code) = match combined.rsplit_once('\n') {
        Some((body, code)) => (body.to_string(), code.trim().to_string()),
        None => (String::new(), combined.trim().to_string()),
    };
    match code.parse::<u16>() {
        Ok(code) if code != 0 => Ok((code, body)),
        _ => Err(unreachable()),
    }
}

fn interpret_response(code: u16, body: &str) -> Result<SignupOutcome> {
    let parsed: SignupResponse = serde_json::from_str(body).unwrap_or_default();
    let fail = |message: String| Err(crate::error::CleenError::RegisterError { message });

    match code {
        200..=299 => match parsed.status.as_deref() {
            Some("recently_sent") => Ok(SignupOutcome::RecentlySent),
            _ => Ok(SignupOutcome::ConfirmationSent),
        },
        400 => {
            if parsed.fields.is_empty() {
                return fail("the account service did not accept the details.".to_string());
            }
            let problems: Vec<String> = parsed
                .fields
                .iter()
                .map(|(field, need)| format!("--{field} must be {need}"))
                .collect();
            fail(problems.join("; "))
        }
        429 => fail("too many sign-up attempts. Please try again in a few minutes.".to_string()),
        503 => match parsed.error.as_deref() {
            Some("closed") => {
                fail("sign-up is closed right now. Please try again later.".to_string())
            }
            _ => fail(
                "the confirmation email could not be sent. Please try again later.".to_string(),
            ),
        },
        other => fail(format!(
            "the account service answered with status {other}. Please try again later."
        )),
    }
}

/// After the first successful install, ask once for an email to send
/// security alerts and early news to. Enter skips. Only in an interactive
/// terminal, never in CI, and never again after it was asked or after
/// `cleen register` was run. A failed sign-up never fails the install.
pub fn maybe_show_hint() {
    let state = load_state();
    let interactive = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
    if !should_show_hint(&state, interactive, &env_lookup) {
        return;
    }

    // Recorded before asking, so the question is asked once even if the
    // person interrupts it.
    let mut state = state;
    state.hint_shown_at = Some(now_epoch_secs());
    save_state(&state);

    println!();
    println!("📬 A new Clean Language toolchain is coming soon.");
    println!("   Get security alerts and early news by email (optional).");

    let Some(email) = ask("   Your email (press Enter to skip)") else {
        println!("   Skipped. You can sign up any time with: cleen register");
        return;
    };
    let Some(name) = ask("   Your name") else {
        println!("   Skipped. You can sign up any time with: cleen register");
        return;
    };

    let signup = EmailSignup {
        email,
        name,
        phone: None,
        releases: false,
        news: false,
    };
    if let Err(e) = register_by_email(signup) {
        println!("   ⚠️  {e}");
        println!("   You can try again any time with: cleen register");
    }
}

/// One line of input; `None` when left empty or the terminal closes.
fn ask(prompt: &str) -> Option<String> {
    dialoguer::Input::<String>::new()
        .with_prompt(prompt)
        .allow_empty(true)
        .interact_text()
        .ok()
        .map(|answer| answer.trim().to_string())
        .filter(|answer| !answer.is_empty())
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
            "https://account.cleanlanguage.dev/register?source=cleen&cleen_version=1.2.3"
        );
    }

    fn signup(phone: Option<&str>, releases: bool, news: bool) -> EmailSignup {
        EmailSignup {
            email: " ana@example.com ".to_string(),
            name: " Ana ".to_string(),
            phone: phone.map(str::to_string),
            releases,
            news,
        }
    }

    #[test]
    fn payload_matches_the_api_shape() {
        let json: serde_json::Value =
            serde_json::from_str(&signup_payload(&signup(None, false, false), "1.2.3").unwrap())
                .unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "email": "ana@example.com",
                "name": "Ana",
                "topics": { "security": true, "releases": false, "news": false },
                "source": "cleen",
                "cleen_version": "1.2.3"
            })
        );
    }

    #[test]
    fn payload_includes_phone_and_chosen_topics() {
        let json: serde_json::Value = serde_json::from_str(
            &signup_payload(&signup(Some("+34 600 000 000"), true, true), "1.2.3").unwrap(),
        )
        .unwrap();
        assert_eq!(json["phone"], "+34 600 000 000");
        assert_eq!(json["topics"]["releases"], true);
        assert_eq!(json["topics"]["news"], true);
    }

    #[test]
    fn blank_phone_is_left_out() {
        let json: serde_json::Value = serde_json::from_str(
            &signup_payload(&signup(Some("  "), false, false), "1.2.3").unwrap(),
        )
        .unwrap();
        assert!(json.get("phone").is_none());
    }

    #[test]
    fn accepted_responses() {
        assert_eq!(
            interpret_response(202, r#"{"status":"confirmation_sent"}"#).unwrap(),
            SignupOutcome::ConfirmationSent
        );
        assert_eq!(
            interpret_response(202, r#"{"status":"recently_sent"}"#).unwrap(),
            SignupOutcome::RecentlySent
        );
    }

    #[test]
    fn invalid_fields_are_named() {
        let err = interpret_response(
            400,
            r#"{"error":"invalid","fields":{"email":"a valid email address","name":"1 to 100 characters"}}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("--email must be a valid email address"),
            "{err}"
        );
        assert!(err.contains("--name must be 1 to 100 characters"), "{err}");
    }

    #[test]
    fn rate_limit_and_unavailable_are_errors() {
        assert!(interpret_response(429, r#"{"error":"too_many"}"#)
            .unwrap_err()
            .to_string()
            .contains("too many"));
        assert!(interpret_response(503, r#"{"error":"closed"}"#)
            .unwrap_err()
            .to_string()
            .contains("closed"));
        assert!(interpret_response(503, r#"{"error":"send_failed"}"#)
            .unwrap_err()
            .to_string()
            .contains("could not be sent"));
        assert!(interpret_response(500, "")
            .unwrap_err()
            .to_string()
            .contains("500"));
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
