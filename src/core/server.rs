use crate::core::{config::Config, download::Downloader, github::GitHubClient};
use crate::error::{CleenError, Result};
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

/// Get the path to the active server binary
fn get_server_binary_path(config: &Config) -> Option<PathBuf> {
    config.server_version.as_ref().map(|v| {
        let version_dir = get_server_version_dir(config, v);
        if cfg!(windows) {
            version_dir.join("clean-server.exe")
        } else {
            version_dir.join("clean-server")
        }
    })
}

/// Install Clean Server
pub fn install_server(version: Option<&str>) -> Result<()> {
    let mut config = Config::load()?;

    // Determine version to install
    let server_version = if let Some(v) = version {
        v.to_string()
    } else {
        // Get latest version from GitHub
        let github_client = GitHubClient::new(config.github_api_token.clone());
        println!("Fetching latest Clean Server version...");

        let releases = match github_client.get_releases(SERVER_REPO_OWNER, SERVER_REPO_NAME) {
            Ok(releases) => releases,
            Err(e) => {
                println!("⚠️  Unable to fetch releases from GitHub: {e}");
                println!(
                    "   Repository: https://github.com/{SERVER_REPO_OWNER}/{SERVER_REPO_NAME}/releases"
                );
                return Ok(());
            }
        };

        if releases.is_empty() {
            println!("⚠️  No releases found for Clean Server.");
            println!(
                "   Repository: https://github.com/{SERVER_REPO_OWNER}/{SERVER_REPO_NAME}/releases"
            );
            return Ok(());
        }

        // Get the latest (first) release
        let latest = &releases[0];
        latest.tag_name.trim_start_matches('v').to_string()
    };

    println!("Installing Clean Server version: {server_version}");

    // Check if version is already installed
    let version_dir = get_server_version_dir(&config, &server_version);
    if version_dir.exists() {
        println!("✓ Clean Server {server_version} is already installed");

        // Set as active if no version is active
        if config.server_version.is_none() {
            config.server_version = Some(server_version.clone());
            config.save()?;
            println!("✓ Set {server_version} as active version");
        }
        return Ok(());
    }

    // Fetch releases from GitHub
    let github_client = GitHubClient::new(config.github_api_token.clone());
    println!("Fetching Clean Server releases...");

    let releases = match github_client.get_releases(SERVER_REPO_OWNER, SERVER_REPO_NAME) {
        Ok(releases) => releases,
        Err(e) => {
            println!("⚠️  Unable to fetch releases from GitHub: {e}");
            return Ok(());
        }
    };

    // Find the specified version
    let tag_name = format!("v{}", server_version.trim_start_matches('v'));
    let release = releases
        .iter()
        .find(|r| r.tag_name == tag_name || r.tag_name == server_version)
        .ok_or_else(|| CleenError::ServerVersionNotFound {
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
        let binary_path = version_dir.join("clean-server");
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

    config.server_version = Some(version.to_string());
    config.save()?;

    println!("✓ Now using Clean Server {version}");

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
