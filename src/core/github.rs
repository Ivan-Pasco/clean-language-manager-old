use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Release {
    pub tag_name: String,
    pub name: String,
    pub prerelease: bool,
    pub draft: bool,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug, Deserialize)]
struct GithubError {
    message: String,
}

pub struct GitHubClient {
    #[allow(dead_code)]
    github_token: Option<String>,
}

const USER_AGENT: &str = concat!("cleen/", env!("CARGO_PKG_VERSION"));

/// Parse a GitHub API response.
///
/// Two failure modes are treated as user-facing errors instead of raw serde
/// failures:
///
/// 1. Non-2xx status codes — GitHub returns a JSON error object of the shape
///    `{"message": "...", "documentation_url": "..."}`. Attempting to
///    deserialize that as `Release` fails with "missing field `tag_name`" and
///    as `Vec<Release>` fails with "invalid type: map, expected a sequence".
///    Both messages hide the real problem (rate limit, 404, auth). Surface
///    the `message` field instead.
/// 2. 2xx with an unexpected body shape — surface a shape hint rather than a
///    raw serde error.
fn parse_github_response<T: serde::de::DeserializeOwned>(
    status_code: Option<i32>,
    body: &str,
) -> Result<T> {
    let is_success = matches!(status_code, Some(200..=299));

    if !is_success {
        if let Ok(err) = serde_json::from_str::<GithubError>(body) {
            let status = status_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "?".to_string());
            return Err(anyhow::anyhow!(
                "GitHub API returned HTTP {status}: {}",
                err.message
            ));
        }
        let status = status_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".to_string());
        let snippet: String = body.chars().take(200).collect();
        return Err(anyhow::anyhow!(
            "GitHub API returned HTTP {status} with unrecognized body: {snippet}"
        ));
    }

    match serde_json::from_str::<T>(body) {
        Ok(value) => Ok(value),
        Err(_) => {
            if let Ok(err) = serde_json::from_str::<GithubError>(body) {
                return Err(anyhow::anyhow!(
                    "GitHub API returned an error message with HTTP 2xx: {}",
                    err.message
                ));
            }
            let snippet: String = body.chars().take(200).collect();
            Err(anyhow::anyhow!(
                "Could not parse GitHub API response: {snippet}"
            ))
        }
    }
}

fn curl_with_status(url: &str) -> Result<(Option<i32>, String)> {
    let output = Command::new("curl")
        .arg("-sS")
        .arg("-w")
        .arg("\n%{http_code}")
        .arg("-H")
        .arg(format!("User-Agent: {USER_AGENT}"))
        .arg("-H")
        .arg("Accept: application/vnd.github+json")
        .arg(url)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "curl failed (exit {:?}): {}",
            output.status.code(),
            stderr.trim()
        ));
    }

    let full = String::from_utf8(output.stdout)?;
    let (body, status_code) = match full.rfind('\n') {
        Some(idx) => {
            let (b, s) = full.split_at(idx);
            let s = s.trim_start_matches('\n').trim();
            (b.to_string(), s.parse::<i32>().ok())
        }
        None => (full, None),
    };
    Ok((status_code, body))
}

/// True when a user-supplied version argument asks for the newest release.
/// Shared by `cleen install`, `cleen frame install` and `cleen server install`
/// so all three accept `latest` the same way.
pub fn is_latest_request(version: &str) -> bool {
    version.eq_ignore_ascii_case("latest")
}

/// Strip the `v` prefix from a release tag (`v1.9.117` -> `1.9.117`).
pub fn tag_to_version(tag: &str) -> String {
    tag.trim_start_matches('v').to_string()
}

impl GitHubClient {
    pub fn new(github_token: Option<String>) -> Self {
        Self { github_token }
    }

    pub fn get_releases(&self, repo_owner: &str, repo_name: &str) -> Result<Vec<Release>> {
        let url = format!("https://api.github.com/repos/{repo_owner}/{repo_name}/releases");
        let (status, body) = curl_with_status(&url)?;
        parse_github_response::<Vec<Release>>(status, &body)
    }

    pub fn get_latest_release(&self, repo_owner: &str, repo_name: &str) -> Result<Release> {
        let url = format!("https://api.github.com/repos/{repo_owner}/{repo_name}/releases/latest");
        let (status, body) = curl_with_status(&url)?;
        parse_github_response::<Release>(status, &body)
    }

    /// Tag of the newest published release (`/releases/latest`), e.g. `v1.9.117`.
    pub fn get_latest_tag(&self, repo_owner: &str, repo_name: &str) -> Result<String> {
        Ok(self.get_latest_release(repo_owner, repo_name)?.tag_name)
    }

    /// Fetch a single release by tag. Uses the /releases/tags/<tag> endpoint,
    /// which is not paginated and does not incur an "invalid type: map"
    /// deserialize failure when the response happens to be a single Release
    /// object. `tag` should be the full tag (e.g. "v2.12.127").
    pub fn get_release_by_tag(
        &self,
        repo_owner: &str,
        repo_name: &str,
        tag: &str,
    ) -> Result<Release> {
        let url =
            format!("https://api.github.com/repos/{repo_owner}/{repo_name}/releases/tags/{tag}");
        let (status, body) = curl_with_status(&url)?;
        parse_github_response::<Release>(status, &body)
    }

    #[allow(dead_code)]
    pub fn download_asset(&self, asset: &Asset, dest_path: &std::path::Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let output = Command::new("curl")
            .arg("-L") // Follow redirects
            .arg("-s") // Silent
            .arg("-H")
            .arg(format!("User-Agent: {USER_AGENT}"))
            .arg("-o")
            .arg(dest_path)
            .arg(&asset.browser_download_url)
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to download asset: curl exited with status {:?}",
                output.status.code()
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_RELEASE: &str = r#"{
        "tag_name": "v2.12.127",
        "name": "v2.12.127",
        "prerelease": false,
        "draft": false,
        "assets": []
    }"#;

    const VALID_RELEASE_ARRAY: &str = r#"[
        {
            "tag_name": "v2.12.127",
            "name": "v2.12.127",
            "prerelease": false,
            "draft": false,
            "assets": []
        }
    ]"#;

    const RATE_LIMIT_BODY: &str = r#"{
        "message": "API rate limit exceeded for 203.0.113.5. (But here's the good news: Authenticated requests get a higher rate limit. Check out the documentation for more details.)",
        "documentation_url": "https://docs.github.com/rest/overview/resources-in-the-rest-api#rate-limiting"
    }"#;

    const NOT_FOUND_BODY: &str = r#"{
        "message": "Not Found",
        "documentation_url": "https://docs.github.com/rest"
    }"#;

    #[test]
    fn latest_request_is_case_insensitive() {
        assert!(is_latest_request("latest"));
        assert!(is_latest_request("Latest"));
        assert!(!is_latest_request("1.9.117"));
        assert!(!is_latest_request("vlatest"));
    }

    #[test]
    fn tag_to_version_strips_prefix() {
        assert_eq!(tag_to_version("v1.9.117"), "1.9.117");
        assert_eq!(tag_to_version("1.9.117"), "1.9.117");
    }

    #[test]
    fn parses_valid_single_release() {
        let r: Release = parse_github_response(Some(200), VALID_RELEASE).unwrap();
        assert_eq!(r.tag_name, "v2.12.127");
    }

    #[test]
    fn parses_valid_release_array() {
        let releases: Vec<Release> = parse_github_response(Some(200), VALID_RELEASE_ARRAY).unwrap();
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].tag_name, "v2.12.127");
    }

    // Regression: CLEEN-FRAME-INSTALL-BROKEN (fp da383b1b5fc6).
    // Rate-limited responses used to surface as "invalid type: map, expected a
    // sequence" (Vec target) or "missing field `tag_name`" (Release target).
    // Both must now surface the human-readable message from the response body.
    #[test]
    fn rate_limit_surfaces_message_not_serde_failure_as_vec() {
        let err = parse_github_response::<Vec<Release>>(Some(403), RATE_LIMIT_BODY).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("HTTP 403"), "got: {msg}");
        assert!(msg.contains("rate limit exceeded"), "got: {msg}");
        assert!(
            !msg.contains("invalid type"),
            "should not leak serde error: {msg}"
        );
    }

    #[test]
    fn rate_limit_surfaces_message_not_serde_failure_as_release() {
        let err = parse_github_response::<Release>(Some(403), RATE_LIMIT_BODY).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("HTTP 403"), "got: {msg}");
        assert!(msg.contains("rate limit exceeded"), "got: {msg}");
        assert!(
            !msg.contains("missing field"),
            "should not leak serde error: {msg}"
        );
    }

    #[test]
    fn not_found_surfaces_message() {
        let err = parse_github_response::<Release>(Some(404), NOT_FOUND_BODY).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("HTTP 404"), "got: {msg}");
        assert!(msg.contains("Not Found"), "got: {msg}");
    }

    #[test]
    fn unrecognized_body_snippets_out() {
        let err = parse_github_response::<Release>(Some(500), "internal server error").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("HTTP 500"), "got: {msg}");
        assert!(msg.contains("internal server error"), "got: {msg}");
    }
}
