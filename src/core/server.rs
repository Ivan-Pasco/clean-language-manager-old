use crate::core::{
    config::Config,
    download::Downloader,
    github::{self, GitHubClient},
};
use crate::error::{CleenError, Result};
use crate::utils::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SERVER_REPO_OWNER: &str = "Ivan-Pasco";
const SERVER_REPO_NAME: &str = "clean-server-old";

/// Get the directory where server versions are installed
fn get_server_versions_dir(config: &Config) -> PathBuf {
    config.cleen_dir.join("server")
}

/// Get the directory for a specific server version
fn get_server_version_dir(config: &Config, version: &str) -> PathBuf {
    get_server_versions_dir(config).join(version)
}

/// File name of the server binary on this platform
fn server_binary_name() -> &'static str {
    if cfg!(windows) {
        "clean-server.exe"
    } else {
        "clean-server"
    }
}

/// Get the path to the active server binary
fn get_server_binary_path(config: &Config) -> Option<PathBuf> {
    config
        .server_version
        .as_ref()
        .map(|v| get_server_version_dir(config, v).join(server_binary_name()))
}

/// Resolve the version argument of `cleen server install`.
///
/// `None` and `latest` (any case) resolve through `fetch_latest_tag` — the
/// same `/releases/latest` lookup the compiler and Frame installs use. Any
/// other value is taken as an explicit version, with a leading `v` removed.
fn resolve_server_version<F>(requested: Option<&str>, fetch_latest_tag: F) -> Result<String>
where
    F: FnOnce() -> anyhow::Result<String>,
{
    match requested {
        Some(v) if !github::is_latest_request(v) => Ok(github::tag_to_version(v)),
        _ => {
            let tag = fetch_latest_tag().map_err(|e| CleenError::GitHubError {
                message: format!(
                    "Unable to fetch the latest Clean Server release from \
                     https://github.com/{SERVER_REPO_OWNER}/{SERVER_REPO_NAME}/releases: {e}"
                ),
            })?;
            Ok(github::tag_to_version(&tag))
        }
    }
}

/// Point `<cleen_dir>/bin/clean-server` at the binary of an installed version.
///
/// The old entry is never written into: a running server keeps its inode,
/// and macOS kills a process whose executable is overwritten in place. On
/// Unix a symlink is created at a temp name and renamed over the old entry;
/// on Windows the old entry is removed and the binary copied.
fn activate_server_binary(cleen_dir: &Path, version: &str) -> Result<PathBuf> {
    let name = server_binary_name();
    let target = cleen_dir.join("server").join(version).join(name);
    if !target.is_file() {
        return Err(CleenError::ServerVersionNotInstalled {
            version: version.to_string(),
        });
    }

    let bin_dir = cleen_dir.join("bin");
    fs::ensure_dir_exists(&bin_dir)?;
    let link = bin_dir.join(name);

    #[cfg(unix)]
    {
        if fs::atomic_replace_symlink(&link, &target).is_err() {
            // A provenance-locked regular file cannot be renamed over on
            // macOS Sequoia; move the bin dir aside and retry.
            fs::evict_locked_shims(&bin_dir, &[name])?;
            fs::atomic_replace_symlink(&link, &target)?;
        }
    }
    #[cfg(windows)]
    {
        fs::remove_path_if_exists(&link)?;
        std::fs::copy(&target, &link)?;
    }

    Ok(link)
}

/// Install Clean Server
pub fn install_server(version: Option<&str>) -> Result<()> {
    let mut config = Config::load()?;

    // Determine version to install (`latest` or no argument -> newest release)
    let github_client = GitHubClient::new(config.github_api_token.clone());
    if version.is_none_or(github::is_latest_request) {
        println!("Fetching latest Clean Server version...");
    }
    let server_version = resolve_server_version(version, || {
        github_client.get_latest_tag(SERVER_REPO_OWNER, SERVER_REPO_NAME)
    })?;

    println!("Installing Clean Server version: {server_version}");

    // Check if version is already installed
    let version_dir = get_server_version_dir(&config, &server_version);
    if version_dir.exists() {
        println!("✓ Clean Server {server_version} is already installed");

        // Set as active if no version is active
        if config.server_version.is_none() {
            activate_server_binary(&config.cleen_dir, &server_version)?;
            config.server_version = Some(server_version.clone());
            config.save()?;
            println!("✓ Set {server_version} as active version");
        }
        return Ok(());
    }

    // Fetch the release by tag (single, unpaginated lookup)
    println!("Fetching Clean Server release v{server_version}...");
    let tag_name = format!("v{server_version}");
    let release = github_client
        .get_release_by_tag(SERVER_REPO_OWNER, SERVER_REPO_NAME, &tag_name)
        .map_err(|_| CleenError::ServerVersionNotFound {
            version: server_version.clone(),
        })?;

    // Determine platform-specific asset name
    let asset_name = get_platform_asset_name();
    let asset = release
        .assets
        .iter()
        .find(|a| a.name.contains(&asset_name))
        .ok_or_else(|| CleenError::ServerAssetNotFound {
            version: server_version.clone(),
            platform: asset_name.clone(),
        })?;

    println!("Downloading {asset_name}...");

    // Create version directory
    std::fs::create_dir_all(&version_dir)?;

    // Download the asset
    let downloader = Downloader::new();
    let download_path = version_dir.join(&asset.name);
    downloader.download_file(&asset.browser_download_url, &download_path)?;

    // Extract if it's a compressed file
    if asset.name.ends_with(".tar.gz") || asset.name.ends_with(".zip") {
        println!("Extracting...");
        extract_archive(&download_path, &version_dir)?;
        std::fs::remove_file(&download_path)?;
    }

    // Make binary executable on Unix
    #[cfg(unix)]
    {
        let binary_path = version_dir.join(server_binary_name());
        if binary_path.exists() {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&binary_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&binary_path, perms)?;
        }
    }

    println!("✅ Clean Server {server_version} installed successfully!");

    // Set as active version if none is set
    if config.server_version.is_none() {
        activate_server_binary(&config.cleen_dir, &server_version)?;
        config.server_version = Some(server_version.clone());
        config.save()?;
        println!("✓ Set {server_version} as active version");
    }

    Ok(())
}

/// List installed Clean Server versions
pub fn list_versions() -> Result<()> {
    let config = Config::load()?;
    let versions_dir = get_server_versions_dir(&config);

    if !versions_dir.exists() {
        println!("No Clean Server versions installed");
        println!();
        println!("To install Clean Server:");
        println!("   cleen server install");
        return Ok(());
    }

    let mut versions: Vec<String> = std::fs::read_dir(&versions_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .collect();

    if versions.is_empty() {
        println!("No Clean Server versions installed");
        println!();
        println!("To install Clean Server:");
        println!("   cleen server install");
        return Ok(());
    }

    versions.sort_by(|a, b| version_compare(b, a));

    println!("Installed Clean Server versions:");
    for v in &versions {
        let marker = if config.server_version.as_deref() == Some(v) {
            "* "
        } else {
            "  "
        };
        println!("{marker}{v}");
    }

    Ok(())
}

/// Switch to a specific Clean Server version
pub fn use_version(version: &str) -> Result<()> {
    let mut config = Config::load()?;
    let version_dir = get_server_version_dir(&config, version);

    if !version_dir.exists() {
        return Err(CleenError::ServerVersionNotInstalled {
            version: version.to_string(),
        });
    }

    let link = activate_server_binary(&config.cleen_dir, version)?;
    config.server_version = Some(version.to_string());
    config.save()?;

    println!("✓ Now using Clean Server {version}");
    println!(
        "   {} -> {}",
        link.display(),
        version_dir.join(server_binary_name()).display()
    );

    Ok(())
}

/// Uninstall a Clean Server version
pub fn uninstall_version(version: &str) -> Result<()> {
    let mut config = Config::load()?;
    let version_dir = get_server_version_dir(&config, version);

    if !version_dir.exists() {
        return Err(CleenError::ServerVersionNotInstalled {
            version: version.to_string(),
        });
    }

    // Remove the version directory
    std::fs::remove_dir_all(&version_dir)?;

    // Clear active version if this was it
    if config.server_version.as_deref() == Some(version) {
        fs::remove_path_if_exists(&config.get_bin_dir().join(server_binary_name()))?;
        config.server_version = None;
        config.save()?;
        println!("⚠️  Uninstalled active version. Run 'cleen server use <version>' to set a new active version.");
    }

    println!("✓ Uninstalled Clean Server {version}");

    Ok(())
}

/// Run a WASM application with Clean Server
pub fn run_wasm(wasm_file: &str, port: u16, host: &str) -> Result<()> {
    let config = Config::load()?;

    let binary_path = get_server_binary_path(&config).ok_or(CleenError::NoServerInstalled)?;

    if !binary_path.exists() {
        return Err(CleenError::NoServerInstalled);
    }

    let wasm_path = Path::new(wasm_file);
    if !wasm_path.exists() {
        return Err(CleenError::FileNotFound {
            path: wasm_file.to_string(),
        });
    }

    println!("Starting Clean Server...");
    println!("   WASM: {wasm_file}");
    println!("   Listening: http://{host}:{port}");
    println!();

    // Run the server
    let status = Command::new(&binary_path)
        .arg(wasm_file)
        .args(["--port", &port.to_string()])
        .args(["--host", host])
        .status()
        .map_err(|e| CleenError::ServerStartFailed {
            message: format!("Failed to start server: {e}"),
        })?;

    if !status.success() {
        return Err(CleenError::ServerStartFailed {
            message: format!("Server exited with status: {status}"),
        });
    }

    Ok(())
}

/// Show Clean Server status
pub fn show_status() -> Result<()> {
    let config = Config::load()?;

    println!("Clean Server Status");
    println!("===================");
    println!();

    if let Some(version) = &config.server_version {
        let binary_path = get_server_binary_path(&config);
        let exists = binary_path.as_ref().map(|p| p.exists()).unwrap_or(false);

        println!("Active version: {version}");
        println!("Binary exists:  {}", if exists { "Yes" } else { "No" });

        if exists {
            // Try to get version info
            if let Some(path) = binary_path {
                let output = Command::new(&path).arg("--version").output();
                if let Ok(output) = output {
                    if output.status.success() {
                        let version_str = String::from_utf8_lossy(&output.stdout);
                        println!("Version info:   {}", version_str.trim());
                    }
                }
            }
        }
    } else {
        println!("No Clean Server version is active");
        println!();
        println!("To install Clean Server:");
        println!("   cleen server install");
    }

    // List installed versions
    let versions_dir = get_server_versions_dir(&config);
    if versions_dir.exists() {
        let count = std::fs::read_dir(&versions_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .count();

        println!();
        println!("Installed versions: {count}");
    }

    Ok(())
}

/// Get platform-specific asset name pattern
fn get_platform_asset_name() -> String {
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };

    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };

    format!("{os}-{arch}")
}

/// Extract an archive (tar.gz or zip)
fn extract_archive(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let archive_name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if archive_name.ends_with(".tar.gz") {
        // Use tar command
        let status = Command::new("tar")
            .args(["-xzf"])
            .arg(archive_path)
            .args(["-C"])
            .arg(dest_dir)
            .status()
            .map_err(|e| CleenError::IoError {
                message: format!("Failed to extract tar.gz: {e}"),
            })?;

        if !status.success() {
            return Err(CleenError::IoError {
                message: "tar extraction failed".to_string(),
            });
        }
    } else if archive_name.ends_with(".zip") {
        // Use unzip command
        let status = Command::new("unzip")
            .args(["-o"])
            .arg(archive_path)
            .args(["-d"])
            .arg(dest_dir)
            .status()
            .map_err(|e| CleenError::IoError {
                message: format!("Failed to extract zip: {e}"),
            })?;

        if !status.success() {
            return Err(CleenError::IoError {
                message: "unzip extraction failed".to_string(),
            });
        }
    }

    Ok(())
}

/// Compare two version strings (semver-like)
fn version_compare(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u32> {
        s.trim_start_matches('v')
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect()
    };

    let va = parse(a);
    let vb = parse(b);

    va.cmp(&vb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn server_repo_is_the_old_toolchain_repo() {
        assert_eq!(SERVER_REPO_OWNER, "Ivan-Pasco");
        assert_eq!(SERVER_REPO_NAME, "clean-server-old");
    }

    // Regression: issue #1 — `cleen server install latest` looked for a
    // release tagged `vlatest` instead of resolving the newest release.
    #[test]
    fn latest_resolves_through_the_latest_release_lookup() {
        let called = Cell::new(false);
        let v = resolve_server_version(Some("latest"), || {
            called.set(true);
            Ok("v1.9.117".to_string())
        })
        .unwrap();
        assert!(called.get());
        assert_eq!(v, "1.9.117");

        let v = resolve_server_version(Some("LATEST"), || Ok("v2.0.0".to_string())).unwrap();
        assert_eq!(v, "2.0.0");
    }

    #[test]
    fn no_version_resolves_to_latest() {
        let v = resolve_server_version(None, || Ok("v1.9.117".to_string())).unwrap();
        assert_eq!(v, "1.9.117");
    }

    #[test]
    fn explicit_version_does_not_query_latest() {
        let v = resolve_server_version(Some("v1.9.115"), || {
            panic!("latest lookup must not run for an explicit version")
        })
        .unwrap();
        assert_eq!(v, "1.9.115");
        let v = resolve_server_version(Some("1.9.115"), || unreachable!()).unwrap();
        assert_eq!(v, "1.9.115");
    }

    #[test]
    fn latest_lookup_failure_is_an_error() {
        assert!(
            resolve_server_version(Some("latest"), || Err(anyhow::anyhow!("offline"))).is_err()
        );
    }

    fn write_server(cleen_dir: &Path, version: &str) {
        let dir = cleen_dir.join("server").join(version);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(server_binary_name()), format!("server {version}")).unwrap();
    }

    fn active_contents(cleen_dir: &Path) -> String {
        std::fs::read_to_string(cleen_dir.join("bin").join(server_binary_name())).unwrap()
    }

    // Regression: issue #2 — `cleen server use` only wrote config.json and
    // never switched ~/.cleen/bin/clean-server.
    #[test]
    fn activate_switches_the_bin_binary_between_versions() {
        let home = tempfile::tempdir().unwrap();
        let cleen_dir = home.path();
        write_server(cleen_dir, "1.9.115");
        write_server(cleen_dir, "1.9.117");

        activate_server_binary(cleen_dir, "1.9.117").unwrap();
        assert_eq!(active_contents(cleen_dir), "server 1.9.117");

        activate_server_binary(cleen_dir, "1.9.115").unwrap();
        assert_eq!(active_contents(cleen_dir), "server 1.9.115");

        activate_server_binary(cleen_dir, "1.9.117").unwrap();
        assert_eq!(active_contents(cleen_dir), "server 1.9.117");
    }

    // A running server binary must never be overwritten in place (macOS
    // kills a process whose executable pages change). The old file, which a
    // running process may still hold, must keep its original contents.
    #[test]
    fn activate_never_writes_into_the_previous_binary() {
        let home = tempfile::tempdir().unwrap();
        let cleen_dir = home.path();
        write_server(cleen_dir, "1.9.117");

        let bin_dir = cleen_dir.join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let active = bin_dir.join(server_binary_name());
        std::fs::write(&active, "hand-copied 1.9.115").unwrap();
        // Stand-in for the running process's open handle on the old inode.
        let held = cleen_dir.join("held-by-running-process");
        std::fs::hard_link(&active, &held).unwrap();

        activate_server_binary(cleen_dir, "1.9.117").unwrap();

        assert_eq!(active_contents(cleen_dir), "server 1.9.117");
        assert_eq!(
            std::fs::read_to_string(&held).unwrap(),
            "hand-copied 1.9.115"
        );
    }

    #[test]
    fn activate_missing_version_fails_and_leaves_bin_alone() {
        let home = tempfile::tempdir().unwrap();
        let cleen_dir = home.path();
        write_server(cleen_dir, "1.9.115");
        activate_server_binary(cleen_dir, "1.9.115").unwrap();

        let err = activate_server_binary(cleen_dir, "9.9.9").unwrap_err();
        assert!(matches!(err, CleenError::ServerVersionNotInstalled { .. }));
        assert_eq!(active_contents(cleen_dir), "server 1.9.115");
    }
}
