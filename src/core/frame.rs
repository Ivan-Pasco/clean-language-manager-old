use crate::core::{compatibility, config::Config, download::Downloader, github::GitHubClient};
use crate::error::{CleenError, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// PID file location for the server
fn get_pid_file_path() -> PathBuf {
    std::env::temp_dir().join("cleen-frame-server.pid")
}

const FRAME_REPO_OWNER: &str = "Ivan-Pasco";
const FRAME_REPO_NAME: &str = "cleen-framework-old";

/// Filename for the per-frame-version manifest that records which plugin
/// versions belong to a given meta-bundle. Lives at
/// `~/.cleen/versions/frame/<frame_version>/plugins.json` as a flat map of
/// `plugin_name -> plugin_version`. Used by `use_frame_version`,
/// `uninstall_frame_version`, and the install re-entry path to re-assert
/// per-plugin `.active-version` pins — the single source of truth for
/// runtime plugin selection (see HOST_BRIDGE.md "Plugin Pin Resolution").
const FRAME_PLUGINS_MANIFEST: &str = "plugins.json";

/// Write the plugin manifest for a frame meta-bundle.
fn write_frame_plugins_manifest(
    version_dir: &Path,
    plugins: &BTreeMap<String, String>,
) -> Result<()> {
    let path = version_dir.join(FRAME_PLUGINS_MANIFEST);
    let content = serde_json::to_string_pretty(plugins).map_err(|e| CleenError::IoError {
        message: format!("could not serialize frame plugins manifest: {e}"),
    })?;
    // The manifest may already exist from a previous install and carry
    // `com.apple.provenance`; the eviction matches the pattern used for
    // sibling files in this module.
    crate::utils::fs::evict_locked_file(&path)?;
    crate::utils::fs::atomic_write(&path, content.as_bytes(), None)?;
    Ok(())
}

/// Read the plugin manifest for an installed frame meta-bundle. Returns an
/// empty map when the manifest is missing — older installs predate it and
/// will simply skip the re-pin step.
fn read_frame_plugins_manifest(version_dir: &Path) -> BTreeMap<String, String> {
    let path = version_dir.join(FRAME_PLUGINS_MANIFEST);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return BTreeMap::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Re-assert every per-plugin `.active-version` recorded in the frame
/// meta-bundle's manifest. Returns the list of `(plugin, version)` entries
/// the manifest expected but which are not present on disk so callers can
/// surface a single combined error rather than aborting on the first miss.
fn reactivate_frame_plugins(config: &Config, frame_version: &str) -> Result<Vec<(String, String)>> {
    let version_dir = get_frame_version_dir(config, frame_version);
    let manifest = read_frame_plugins_manifest(&version_dir);
    let mut missing = Vec::new();
    for (plugin_name, plugin_version) in manifest {
        if !crate::plugin::is_plugin_installed(config, &plugin_name, &plugin_version) {
            missing.push((plugin_name, plugin_version));
            continue;
        }
        crate::plugin::activate_plugin_version_root(config, &plugin_name, &plugin_version)?;
    }
    Ok(missing)
}

/// Install Frame CLI
pub fn install_frame(version: Option<&str>, skip_compatibility_check: bool) -> Result<()> {
    let config = Config::load()?;

    let github_client = GitHubClient::new(config.github_api_token.clone());

    // Determine version to install. Resolve "latest" via /releases/latest
    // rather than /releases[0] — the paginated list endpoint returns
    // `{"message": "..."}` on rate-limit and gets deserialized as
    // `invalid type: map, expected a sequence`. See CLEEN-FRAME-INSTALL-BROKEN.
    let frame_version = if let Some(v) = version {
        if v.eq_ignore_ascii_case("latest") {
            println!("Fetching latest Frame release...");
            match github_client.get_latest_release(FRAME_REPO_OWNER, FRAME_REPO_NAME) {
                Ok(release) => {
                    let latest = release.tag_name.trim_start_matches('v').to_string();
                    println!("Resolved latest version: {latest}");
                    latest
                }
                Err(e) => {
                    println!("Unable to fetch latest Frame release from GitHub: {e}");
                    println!(
                        "   Repository: https://github.com/{FRAME_REPO_OWNER}/{FRAME_REPO_NAME}/releases"
                    );
                    return Ok(());
                }
            }
        } else {
            v.trim_start_matches('v').to_string()
        }
    } else {
        // Auto-detect compatible version based on active compiler
        let compiler_version = config.active_version.as_ref().ok_or_else(|| {
            println!("No compiler is currently active.");
            println!("   Frame requires a Clean Language compiler.");
            println!();
            println!("To install a compiler first:");
            println!("   cleen install latest");
            println!("   cleen install 0.14.0");
            CleenError::NoCompilerForFrame
        })?;

        let matrix = compatibility::CompatibilityMatrix::new();
        match matrix.find_compatible_frame_version(compiler_version) {
            Some(v) => {
                println!("Found compatible Frame version: {v}");
                v
            }
            None => {
                println!("No compatible Frame version found for compiler {compiler_version}");
                println!("   Frame requires compiler >= 0.14.0");
                println!();
                println!("To upgrade your compiler:");
                println!("   cleen install latest");
                return Err(CleenError::FrameIncompatible {
                    frame_version: "1.0.0+".to_string(),
                    required_compiler: "0.14.0".to_string(),
                    current_compiler: compiler_version.clone(),
                });
            }
        }
    };

    // Check if version is already installed
    let version_dir = get_frame_version_dir(&config, &frame_version);
    if version_dir.exists() {
        println!("Frame version {frame_version} is already installed");

        // Ensure this version is set as active
        let mut config = Config::load()?;
        if config.frame_version.as_deref() != Some(&frame_version) {
            config.frame_version = Some(frame_version.clone());
            config.save()?;
            let _ = update_frame_symlink(&config, &frame_version);
            println!("  Activated Frame version {frame_version}");
        }

        // The contract is: "after this command returns 0, every per-plugin
        // pin matches the meta-bundle". Just having `version_dir` on disk
        // is not enough — per-plugin `.active-version` markers may have
        // drifted (e.g. via `cleen plugin use`). Re-assert every pin
        // recorded in this meta-bundle's manifest. Older installs predate
        // the manifest and the call is a no-op for them.
        let missing = reactivate_frame_plugins(&config, &frame_version)?;
        if !missing.is_empty() {
            eprintln!();
            eprintln!("⚠️  Frame {frame_version} expects plugin versions that are not installed:");
            for (name, version) in &missing {
                eprintln!("   • {name} v{version}");
            }
            eprintln!();
            eprintln!("   Run `cleen frame install {frame_version}` to reinstall, or");
            eprintln!("   `cleen plugin install <name>@<version>` to fix individually.");
        }

        return Ok(());
    }

    println!("Installing Frame version: {frame_version}");

    // Check compiler compatibility unless skipped
    if !skip_compatibility_check {
        if let Some(compiler_version) = &config.active_version {
            compatibility::check_frame_compatibility(compiler_version, &frame_version)?;
            println!("Compatible with compiler {compiler_version}");
        } else {
            return Err(CleenError::NoCompilerForFrame);
        }
    }

    // Find the specified version release. Prefer the targeted
    // /releases/tags/<tag> endpoint (single Release object, not paginated) so
    // an explicit pinned version keeps working even when /releases would
    // require pagination to reach it.
    let tag_name = format!("v{}", frame_version.trim_start_matches('v'));
    let release =
        match github_client.get_release_by_tag(FRAME_REPO_OWNER, FRAME_REPO_NAME, &tag_name) {
            Ok(r) => r,
            Err(e) => {
                println!("Unable to fetch Frame release {tag_name}: {e}");
                // Only fetch the paginated list to help the user pick a valid
                // tag — a failure here is informational, not fatal.
                if let Ok(releases) = github_client.get_releases(FRAME_REPO_OWNER, FRAME_REPO_NAME)
                {
                    println!("Available Frame versions (recent):");
                    for r in &releases {
                        println!("  - {}", r.tag_name.trim_start_matches('v'));
                    }
                }
                return Err(CleenError::FrameVersionNotFound {
                    frame_version: frame_version.clone(),
                });
            }
        };

    // Find appropriate asset: try platform-specific binary first, then plugin tarball
    let platform_suffix = get_platform_suffix();

    let platform_asset = release.assets.iter().find(|asset| {
        let name_lower = asset.name.to_lowercase();
        let matches_platform = name_lower.contains(&platform_suffix.to_lowercase())
            || name_lower.contains("universal")
            || name_lower.contains("any");
        let is_archive = name_lower.ends_with(".tar.gz") || name_lower.ends_with(".zip");
        matches_platform && is_archive
    });

    let plugin_asset = release
        .assets
        .iter()
        .find(|asset| asset.name.starts_with("frame-plugins-") && asset.name.ends_with(".tar.gz"));

    let (asset, is_plugin_tarball) = if let Some(a) = platform_asset {
        (a, false)
    } else if let Some(a) = plugin_asset {
        (a, true)
    } else {
        println!("Available assets:");
        for asset in &release.assets {
            println!("  - {}", asset.name);
        }
        return Err(CleenError::BinaryNotFound {
            name: format!("Frame asset for version {frame_version}"),
        });
    };

    println!("Found asset: {}", asset.name);

    // Create temporary download directory
    let temp_dir = std::env::temp_dir().join(format!("cleen-frame-{frame_version}"));
    std::fs::create_dir_all(&temp_dir)?;

    // Download the asset
    let download_path = temp_dir.join(&asset.name);
    println!("Downloading {}...", asset.name);

    let downloader = Downloader::new();
    downloader
        .download_file(&asset.browser_download_url, &download_path)
        .map_err(|_e| CleenError::DownloadError {
            url: asset.browser_download_url.clone(),
        })?;

    if is_plugin_tarball {
        // Plugin tarball: extract to a temp staging directory first
        let staging_dir = temp_dir.join("staging");
        std::fs::create_dir_all(&staging_dir)?;

        println!("Extracting plugins...");
        downloader
            .extract_archive(&download_path, &staging_dir)
            .map_err(|_e| CleenError::ExtractionError {
                path: download_path.clone(),
            })?;

        let plugins_dir = config.get_plugins_dir();
        std::fs::create_dir_all(&plugins_dir)?;

        // Iterate over extracted plugin directories, read plugin.toml for version/name,
        // and move files into versioned subdirectories
        let mut config = Config::load()?;
        let mut installed_plugin_names: Vec<String> = Vec::new();
        // Plugin name -> version that actually landed. Written to the frame
        // version dir as `plugins.json` after the loop so `use`, `uninstall`,
        // and the install re-entry path can re-assert per-plugin pins
        // without needing to re-extract the tarball.
        let mut installed_plugin_versions: BTreeMap<String, String> = BTreeMap::new();
        let mut incompatible_plugins: Vec<(String, String, String, String)> = Vec::new();
        // Plugin name -> reason it failed to install. Used to keep the
        // overall install alive when an individual plugin hits an
        // unevictable kernel pin (e.g. a fully-locked plugin root on
        // macOS Sequoia where even sibling rename is rejected). Without
        // this, one bad plugin would abort the entire frame install and
        // every plugin later in iteration order would never land.
        let mut failed_plugins: Vec<(String, String)> = Vec::new();
        for entry in std::fs::read_dir(&staging_dir)? {
            let entry = entry?;
            let src_path = entry.path();

            if !src_path.is_dir() {
                continue;
            }

            let folder_name = match src_path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Read plugin.toml to get the canonical plugin name and version, and
            // to check whether the active compiler satisfies the plugin's
            // declared min_compiler_version.
            let manifest_path = src_path.join("plugin.toml");
            let (plugin_name, plugin_version) = if manifest_path.exists() {
                match crate::plugin::manifest::PluginManifest::load(&manifest_path) {
                    Ok(manifest) => {
                        if let Err(e) =
                            crate::plugin::check_plugin_compatibility(&config, &manifest)
                        {
                            if let crate::error::CleenError::PluginIncompatible {
                                name,
                                required,
                                current,
                            } = &e
                            {
                                incompatible_plugins.push((
                                    name.clone(),
                                    manifest.plugin.version.clone(),
                                    required.clone(),
                                    current.clone(),
                                ));
                            } else {
                                eprintln!(
                                    "Warning: compatibility check failed for {folder_name}: {e}"
                                );
                            }
                        }
                        (manifest.plugin.name, manifest.plugin.version)
                    }
                    Err(_) => {
                        eprintln!(
                            "Warning: Could not parse plugin.toml for {folder_name}, using folder name and frame version"
                        );
                        (folder_name.clone(), frame_version.clone())
                    }
                }
            } else {
                eprintln!("Warning: No plugin.toml found for {folder_name}, using folder name and frame version");
                (folder_name.clone(), frame_version.clone())
            };

            // Install this plugin in a closure so a fatal error on one
            // plugin (e.g. an unevictable kernel pin on its root dir
            // entry) does not abort the whole frame install — the
            // remaining plugins still land. The failure is recorded and
            // surfaced as a warning at the end.
            let install_one = || -> Result<()> {
                // Evict the plugin root upfront when its own dir entry
                // carries `com.apple.provenance` — that lock rejects
                // creation of new child entries (the next
                // `create_dir_all` would fail with EPERM for any
                // brand-new version like a fresh `2.12.25`). The helper
                // preserves existing versioned subdirs via rename-back.
                // No-op when the root is clean.
                let plugin_root = plugins_dir.join(&plugin_name);
                crate::utils::fs::evict_locked_plugin_root(&plugin_root)?;

                // Create versioned subdirectory:
                // ~/.cleen/plugins/<plugin>/<version>/. If the
                // destination already holds files inherited from a
                // previous install that carry `com.apple.provenance`,
                // evict the locked version dir to a sibling graveyard
                // so the fresh copy lands on a clean inode.
                let version_dest = plugin_root.join(&plugin_version);
                crate::utils::fs::evict_locked_dir(&version_dest)?;
                std::fs::create_dir_all(&version_dest).map_err(|e| CleenError::IoError {
                    message: format!(
                        "could not create plugin version dir {}: {e}",
                        version_dest.display()
                    ),
                })?;

                for file_entry in std::fs::read_dir(&src_path)? {
                    let file_entry = file_entry?;
                    let file_src = file_entry.path();
                    let file_name = file_entry.file_name();
                    let file_dst = version_dest.join(&file_name);

                    if file_src.is_dir() {
                        if file_dst.exists() {
                            std::fs::remove_dir_all(&file_dst).map_err(|e| {
                                CleenError::IoError {
                                    message: format!(
                                        "could not remove existing plugin dir {}: {e}",
                                        file_dst.display()
                                    ),
                                }
                            })?;
                        }
                        crate::utils::fs::copy_dir_recursive(&file_src, &file_dst)?;
                    } else {
                        std::fs::copy(&file_src, &file_dst).map_err(|e| CleenError::IoError {
                            message: format!(
                                "could not write plugin file {}: {e}",
                                file_dst.display()
                            ),
                        })?;
                    }
                }

                crate::plugin::activate_plugin_version_root(&config, &plugin_name, &plugin_version)
            };

            match install_one() {
                Ok(()) => {
                    installed_plugin_names.push(plugin_name.clone());
                    installed_plugin_versions.insert(plugin_name.clone(), plugin_version.clone());
                    // `activate_plugin_version_root` inside `install_one`
                    // already wrote `<name>/.active-version` — the single
                    // source of truth (see HOST_BRIDGE.md "Plugin Pin
                    // Resolution"). No config bookkeeping needed.
                    println!("  Installed {plugin_name} v{plugin_version}");
                    // Aggregate heartbeat fires once after the full
                    // install loop completes — see the send_install()
                    // call after write_frame_plugins_manifest below.
                }
                Err(e) => {
                    eprintln!("  Failed to install {plugin_name} v{plugin_version}: {e}");
                    failed_plugins.push((plugin_name.clone(), e.to_string()));
                }
            }
        }

        // Clean up known renamed plugin folders.
        // Only removes folders for plugins that were renamed — not independently installed plugins.
        let known_renames: &[(&str, &str)] = &[("frame.httpserver", "frame.server")];
        for (old_name, new_name) in known_renames {
            let old_path = plugins_dir.join(old_name);
            if old_path.exists() && installed_plugin_names.contains(&new_name.to_string()) {
                if let Err(e) = std::fs::remove_dir_all(&old_path) {
                    eprintln!("Warning: Could not remove stale plugin {old_name}: {e}");
                } else {
                    // Removing the old plugin dir takes its `.active-version`
                    // with it — that's the only pin record.
                    println!("  Removed stale plugin: {old_name} (renamed to {new_name})");
                }
            }
        }

        // Mark the version as installed in frame-versions dir.
        // The marker file may carry inherited `com.apple.provenance` from a
        // prior install — evict it before writing so the rename can land.
        std::fs::create_dir_all(&version_dir).map_err(|e| CleenError::IoError {
            message: format!(
                "could not create frame version dir {}: {e}",
                version_dir.display()
            ),
        })?;
        let installed_marker = version_dir.join(".installed");
        crate::utils::fs::evict_locked_file(&installed_marker)?;
        std::fs::write(
            &installed_marker,
            format!("plugins-only: {frame_version}\n"),
        )
        .map_err(|e| CleenError::IoError {
            message: format!(
                "could not write frame install marker {}: {e}",
                installed_marker.display()
            ),
        })?;

        // Record which plugin versions ship with this meta-bundle so
        // `cleen frame use`, `cleen frame uninstall`, and the re-entry
        // path of `cleen frame install` can re-assert per-plugin pins.
        write_frame_plugins_manifest(&version_dir, &installed_plugin_versions)?;

        // Plugin files were copied (not atomically replaced) into per-plugin
        // version dirs and the activation root. Strip macOS provenance so the
        // next `cleen frame install` can overwrite plugin.wasm in place.
        crate::utils::fs::strip_macos_xattrs_recursive(&plugins_dir);

        // Activation succeeded for every plugin that's listed in
        // installed_plugin_names. The eviction graveyards left behind by
        // `evict_locked_plugin_root` (at the plugins-dir level) and
        // `evict_locked_dir` (inside each per-plugin dir) have served
        // their purpose — the kernel pin travelled with the rename and
        // the new entries were populated from the staging dir, not from
        // the graveyards. Prune them now so they don't accumulate run
        // after run.
        let mut total_pruned = 0usize;
        let mut total_freed = 0u64;
        let (root_count, root_bytes) = crate::utils::fs::prune_graveyards(&plugins_dir);
        total_pruned += root_count;
        total_freed += root_bytes;
        for plugin_name in &installed_plugin_names {
            let (c, b) = crate::utils::fs::prune_graveyards(&plugins_dir.join(plugin_name));
            total_pruned += c;
            total_freed += b;
        }
        if total_pruned > 0 {
            println!(
                "  Pruned {total_pruned} eviction graveyard(s), freed {}",
                crate::commands::cleanup::format_size(total_freed)
            );
        }

        // Clean up temporary files
        std::fs::remove_dir_all(&temp_dir)?;

        // Update config with Frame version and save
        config.frame_version = Some(frame_version.clone());
        config.save()?;

        println!("Successfully installed Frame plugins version {frame_version}");
        println!("   Plugins location: {}", plugins_dir.display());

        // One aggregate heartbeat describing the whole active toolchain
        // (compiler + every pinned plugin). Server matches
        // installed.compiler + installed.plugins against fix_released
        // bugs for this project_hash and returns advanced_bugs; the
        // helper prints one line per bug that just moved to
        // fix_installed for this user.
        crate::core::heartbeat::send_install();

        if !failed_plugins.is_empty() {
            eprintln!();
            eprintln!("⚠️  Plugins that failed to install (other plugins succeeded):");
            for (name, reason) in &failed_plugins {
                eprintln!("   • {name}: {reason}");
            }
            eprintln!();
            eprintln!("   The most common cause on macOS Sequoia is an unevictable");
            eprintln!("   kernel pin on the plugin root dir entry. Remove it manually:");
            eprintln!("     rm -rf ~/.cleen/plugins/<name>");
            eprintln!("   then re-run `cleen frame install latest`.");
        }

        if !incompatible_plugins.is_empty() {
            eprintln!();
            eprintln!("⚠️  Compiler / plugin version mismatch:");
            for (name, plugin_v, required, current) in &incompatible_plugins {
                eprintln!(
                    "   • {name} v{plugin_v} requires compiler >= {required} (active: {current})"
                );
            }
            eprintln!();
            eprintln!("   `cln build` will fail with PLUGIN-REGISTRY-DRIFT until either:");
            eprintln!("     - the compiler is upgraded:   cleen install latest");
            eprintln!("     - an older Frame version is activated:   cleen frame use <version>");
        }
    } else {
        // Platform binary: extract to frame-versions dir
        std::fs::create_dir_all(&version_dir)?;

        if asset.name.ends_with(".tar.gz") || asset.name.ends_with(".zip") {
            println!("Extracting archive...");
            downloader
                .extract_archive(&download_path, &version_dir)
                .map_err(|_e| CleenError::ExtractionError {
                    path: download_path.clone(),
                })?;
        } else {
            let binary_name = if cfg!(windows) { "frame.exe" } else { "frame" };
            let target_path = version_dir.join(binary_name);
            std::fs::copy(&download_path, &target_path)?;
        }

        // Find the extracted binary and ensure it's executable
        let binary_path = find_frame_binary_in_dir(&version_dir)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&binary_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&binary_path, perms)?;
        }

        // Belt-and-braces strip for the direct-binary branch above.
        crate::utils::fs::strip_macos_xattrs_recursive(&version_dir);

        // Clean up temporary files
        std::fs::remove_dir_all(&temp_dir)?;

        // Validate the installed binary
        if let Err(e) = validate_frame_binary(&binary_path) {
            eprintln!("Warning: Installed Frame binary may have issues: {e}");
        }

        // Update config with Frame version
        let mut config = Config::load()?;
        config.frame_version = Some(frame_version.clone());
        config.save()?;

        // Update Frame symlink
        update_frame_symlink(&config, &frame_version)?;

        println!("Successfully installed Frame version {frame_version}");
        println!("   Binary location: {binary_path:?}");

        crate::core::heartbeat::send_install();
    }

    println!();

    // Auto-install Clean Server if not already installed
    let config = Config::load()?;
    if config.server_version.is_none() {
        println!("Installing Clean Server (required for running Frame applications)...");
        println!();
        if let Err(e) = crate::core::server::install_server(None) {
            println!("Warning: Could not auto-install Clean Server: {e}");
            println!("   You can install it manually with: cleen server install");
        }
        println!();
    }

    // Hint, never prompt: the user opted out of a destructive cleanup by
    // not running `cleen cleanup --plugins` themselves, so just surface
    // the headroom and let them decide.
    if let Some((count, bytes)) = crate::commands::cleanup::plugin_cleanup_summary(&config) {
        println!(
            "💡 {count} inactive plugin version(s) using {} — run `cleen cleanup --plugins` to free space.",
            crate::commands::cleanup::format_size(bytes)
        );
        println!();
    }

    Ok(())
}

/// List installed Frame CLI versions
pub fn list_frame_versions(config: &Config) -> Result<Vec<String>> {
    let frame_dir = config.get_frame_versions_dir();

    if !frame_dir.exists() {
        return Ok(Vec::new());
    }

    let mut versions = Vec::new();

    for entry in std::fs::read_dir(&frame_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            if let Some(version) = path.file_name().and_then(|n| n.to_str()) {
                versions.push(version.to_string());
            }
        }
    }

    versions.sort();
    Ok(versions)
}

/// Switch to a specific Frame CLI version
pub fn use_frame_version(version: &str) -> Result<()> {
    let mut config = Config::load()?;

    // Check if version is installed
    let version_dir = get_frame_version_dir(&config, version);
    if !version_dir.exists() {
        return Err(CleenError::FrameVersionNotFound {
            frame_version: version.to_string(),
        });
    }

    // Check compatibility with current compiler
    if let Some(compiler_version) = &config.active_version {
        if let Err(e) = compatibility::check_frame_compatibility(compiler_version, version) {
            eprintln!("⚠️  Warning: {e}");
            eprintln!("   Frame CLI may not work correctly with the current compiler.");
            eprintln!();
            eprintln!("Options:");
            eprintln!("  - Use a compatible compiler version");
            eprintln!("  - Continue anyway (not recommended)");
            eprintln!();
        }
    }

    // Update config
    config.frame_version = Some(version.to_string());
    config.save()?;

    // Update symlink
    update_frame_symlink(&config, version)?;

    // Re-assert every per-plugin `.active-version` recorded in this
    // meta-bundle's manifest so the runtime plugin set matches the frame
    // version that was just selected. Missing per-plugin versions are
    // surfaced but do not abort — the user needs to know the bundle is
    // incomplete rather than have `cleen frame use` fail mysteriously.
    let missing = reactivate_frame_plugins(&config, version)?;

    println!("✅ Switched to Frame CLI version {version}");

    if !missing.is_empty() {
        eprintln!();
        eprintln!("⚠️  Frame {version} expects plugin versions that are not installed:");
        for (name, plugin_version) in &missing {
            eprintln!("   • {name} v{plugin_version}");
        }
        eprintln!();
        eprintln!("   Re-run `cleen frame install {version}` to restore them, or");
        eprintln!("   install individual plugins with `cleen plugin install <name>@<version>`.");
    }

    Ok(())
}

/// Uninstall a specific Frame CLI version
pub fn uninstall_frame_version(version: &str) -> Result<()> {
    let mut config = Config::load()?;

    // Check if version is installed
    let version_dir = get_frame_version_dir(&config, version);
    if !version_dir.exists() {
        return Err(CleenError::FrameVersionNotFound {
            frame_version: version.to_string(),
        });
    }

    // Read the bundle's plugin manifest BEFORE we delete the version dir,
    // so we know which per-plugin pins were owned by this meta-bundle and
    // can re-point them to a still-installed version (or drop the marker
    // so the compiler falls back to highest-installed-semver).
    let owned_plugin_versions = read_frame_plugins_manifest(&version_dir);

    // Check if this is the active version
    if config.frame_version.as_deref() == Some(version) {
        println!("⚠️  This is the currently active Frame CLI version.");
        println!("   Clearing active Frame version...");
        config.frame_version = None;
        config.save()?;

        // Remove symlink (handles broken links too)
        let shim_path = config.get_frame_shim_path();
        crate::utils::fs::remove_path_if_exists(&shim_path)?;
    }

    // Repoint per-plugin pins owned by the bundle being removed.
    for (plugin_name, bundle_version) in &owned_plugin_versions {
        let Some(current_pin) = crate::core::config::read_active_version(&config, plugin_name)
        else {
            continue;
        };
        if current_pin != *bundle_version {
            continue;
        }
        // The active pin belongs to the bundle going away. Try to re-pin
        // to the highest installed version with `plugin.wasm`, else drop
        // the marker so the compiler falls through to its semver-max
        // fallback (see HOST_BRIDGE.md "Plugin Pin Resolution").
        let candidates = crate::plugin::get_plugin_versions(&config, plugin_name)
            .unwrap_or_default()
            .into_iter()
            .filter(|v| v != bundle_version)
            .filter(|v| config.get_plugin_wasm_path(plugin_name, v).exists())
            .collect::<Vec<_>>();
        if let Some(latest) = candidates.first() {
            if let Err(e) =
                crate::plugin::activate_plugin_version_root(&config, plugin_name, latest)
            {
                eprintln!(
                    "⚠️  Could not re-pin {plugin_name} to {latest} after removing {version}: {e}"
                );
            }
        } else {
            let marker = config.get_plugin_dir(plugin_name).join(".active-version");
            let _ = crate::utils::fs::remove_path_if_exists(&marker);
        }
    }

    // Remove version directory
    std::fs::remove_dir_all(&version_dir)?;

    println!("✅ Uninstalled Frame CLI version {version}");

    Ok(())
}

/// Update Frame CLI symlink to point to the specified version
fn update_frame_symlink(config: &Config, version: &str) -> Result<()> {
    let binary_path = get_frame_binary_path(config, version);
    let shim_path = config.get_frame_shim_path();

    // Use symlink-aware removal so a dangling symlink left over from a
    // previously-uninstalled version doesn't trip `symlink()` with EEXIST.
    crate::utils::fs::remove_path_if_exists(&shim_path)?;

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&binary_path, &shim_path)?;
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(&binary_path, &shim_path)?;
    }

    Ok(())
}

/// Get the directory for a specific Frame CLI version
fn get_frame_version_dir(config: &Config, version: &str) -> PathBuf {
    config.get_frame_versions_dir().join(version)
}

/// Get the binary path for a specific Frame CLI version
fn get_frame_binary_path(config: &Config, version: &str) -> PathBuf {
    let binary_name = if cfg!(windows) { "frame.exe" } else { "frame" };
    get_frame_version_dir(config, version).join(binary_name)
}

/// Find Frame binary in a directory
fn find_frame_binary_in_dir(dir: &Path) -> Result<PathBuf> {
    let binary_name = if cfg!(windows) { "frame.exe" } else { "frame" };

    // Look for binary in the root directory first
    let direct_path = dir.join(binary_name);
    if direct_path.exists() {
        return Ok(direct_path);
    }

    // Search recursively for the binary
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            if let Ok(found) = find_frame_binary_in_dir(&path) {
                return Ok(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(binary_name) {
            return Ok(path);
        }
    }

    Err(CleenError::BinaryNotFound {
        name: binary_name.to_string(),
    })
}

/// Validate that the Frame CLI binary works
fn validate_frame_binary(binary_path: &Path) -> std::result::Result<(), String> {
    use std::process::Command;

    // Test 1: Check if binary exists
    if !binary_path.exists() {
        return Err("Binary file does not exist".to_string());
    }

    // Test 2: Try to run --version
    let version_output = Command::new(binary_path).args(["--version"]).output();

    match version_output {
        Ok(output) => {
            if !output.status.success() {
                return Err(format!(
                    "Binary failed to execute: exit code {}",
                    output.status.code().unwrap_or(-1)
                ));
            }

            let version_text = String::from_utf8_lossy(&output.stdout);
            if !version_text.to_lowercase().contains("frame") {
                return Err("Binary does not appear to be Frame CLI".to_string());
            }
        }
        Err(e) => {
            return Err(format!("Failed to execute binary: {e}"));
        }
    }

    Ok(())
}

/// Get platform suffix for downloads
fn get_platform_suffix() -> String {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };

    format!("{os}-{arch}")
}

/// Start a Frame development server
///
/// This function:
/// 1. Compiles the .cln source file to WASM using the Clean Language compiler
/// 2. Starts the frame-runtime with the compiled WASM file
pub fn serve_application(input: &str, port: u16, host: &str, debug: bool) -> Result<()> {
    let config = Config::load()?;

    // Check if a server is already running
    let pid_file = get_pid_file_path();
    if pid_file.exists() {
        if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                // Check if process is still running
                #[cfg(unix)]
                {
                    let status = Command::new("kill").args(["-0", &pid.to_string()]).output();
                    if status.map(|o| o.status.success()).unwrap_or(false) {
                        println!("⚠️  A Frame server is already running (PID: {pid})");
                        println!("   Use 'cleen frame stop' to stop it first");
                        return Ok(());
                    }
                }

                #[cfg(windows)]
                {
                    // On Windows, just check if PID file exists and warn
                    // TODO: Implement proper Windows process checking
                    println!("⚠️  A Frame server may be running (PID: {pid})");
                    println!("   Use 'cleen frame stop' to stop it first");
                    return Ok(());
                }
            }
        }
        // Clean up stale PID file
        let _ = std::fs::remove_file(&pid_file);
    }

    // Verify input file exists
    let input_path = Path::new(input);
    if !input_path.exists() {
        return Err(CleenError::FileNotFound {
            path: input.to_string(),
        });
    }

    // Find the Clean Language compiler
    let cln_path = config.get_shim_path();
    if !cln_path.exists() {
        println!("⚠️  Clean Language compiler not found");
        println!("   Install it with: cleen install latest");
        return Err(CleenError::NoActiveVersion);
    }

    // Find the frame-runtime
    // It should be installed alongside Frame CLI or in the framework's runtime
    let runtime_path = find_frame_runtime(&config)?;

    // Create output WASM path in temp directory
    let wasm_path = std::env::temp_dir().join("cleen-serve-app.wasm");

    // Compile the source file
    println!("📦 Compiling {}...", input);
    let compile_output = Command::new(&cln_path)
        .args(["compile", input, "-o"])
        .arg(&wasm_path)
        .arg("--plugins")
        .output()
        .map_err(|e| CleenError::CompilationFailed {
            message: format!("Failed to run compiler: {e}"),
        })?;

    if !compile_output.status.success() {
        let stderr = String::from_utf8_lossy(&compile_output.stderr);
        println!("❌ Compilation failed:");
        println!("{stderr}");
        return Err(CleenError::CompilationFailed {
            message: stderr.to_string(),
        });
    }

    println!("✅ Compilation successful");

    // Set environment variables for the server
    let mut cmd = Command::new(&runtime_path);
    cmd.arg(&wasm_path);
    cmd.env("FRAME_PORT", port.to_string());
    cmd.env("FRAME_HOST", host);

    if debug {
        cmd.env("RUST_LOG", "debug");
    }

    println!();
    println!("🚀 Starting Frame development server...");
    println!("   Listening on http://{}:{}", host, port);
    println!();
    println!("   Press Ctrl+C to stop the server");
    println!();

    // Run the server in foreground (blocks until Ctrl+C)
    let mut child = cmd.spawn().map_err(|e| CleenError::ServerStartFailed {
        message: format!("Failed to start frame-runtime: {e}"),
    })?;

    // Save PID
    let pid = child.id();
    std::fs::write(&pid_file, pid.to_string())?;

    // Wait for the process (this blocks)
    let status = child.wait().map_err(|e| CleenError::ServerStartFailed {
        message: format!("Server exited with error: {e}"),
    })?;

    // Clean up PID file
    let _ = std::fs::remove_file(&pid_file);

    if !status.success() {
        println!("⚠️  Server exited with status: {:?}", status.code());
    } else {
        println!("Server stopped");
    }

    Ok(())
}

/// Stop a running Frame development server
pub fn stop_server() -> Result<()> {
    let pid_file = get_pid_file_path();

    if !pid_file.exists() {
        println!("No Frame server is currently running");
        return Ok(());
    }

    let pid_str = std::fs::read_to_string(&pid_file).map_err(|e| CleenError::IoError {
        message: format!("Failed to read PID file: {e}"),
    })?;

    let pid: i32 = pid_str.trim().parse().map_err(|_| CleenError::IoError {
        message: "Invalid PID in file".to_string(),
    })?;

    println!("Stopping Frame server (PID: {pid})...");

    #[cfg(unix)]
    {
        let output = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .output();

        match output {
            Ok(o) if o.status.success() => {
                println!("✅ Server stopped successfully");
            }
            Ok(_) => {
                // Process might already be dead
                println!("⚠️  Process may have already stopped");
            }
            Err(e) => {
                println!("⚠️  Failed to stop server: {e}");
            }
        }
    }

    #[cfg(windows)]
    {
        let output = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output();

        match output {
            Ok(o) if o.status.success() => {
                println!("✅ Server stopped successfully");
            }
            Ok(_) => {
                println!("⚠️  Process may have already stopped");
            }
            Err(e) => {
                println!("⚠️  Failed to stop server: {e}");
            }
        }
    }

    // Clean up PID file
    let _ = std::fs::remove_file(&pid_file);

    Ok(())
}

/// Find the frame-runtime binary
fn find_frame_runtime(config: &Config) -> Result<PathBuf> {
    // First, check if it's in the active Frame CLI version directory
    if let Some(frame_version) = &config.frame_version {
        let version_dir = config.get_frame_versions_dir().join(frame_version);

        // Look for frame-runtime in the version directory
        let runtime_name = if cfg!(windows) {
            "frame-runtime.exe"
        } else {
            "frame-runtime"
        };

        let runtime_path = version_dir.join(runtime_name);
        if runtime_path.exists() {
            return Ok(runtime_path);
        }

        // Also check in subdirectories
        if let Ok(found) = find_binary_in_dir(&version_dir, runtime_name) {
            return Ok(found);
        }
    }

    // Check if frame-runtime is in PATH
    if let Ok(path) = which::which("frame-runtime") {
        return Ok(path);
    }

    // Check common installation locations
    let home = dirs::home_dir().ok_or(CleenError::BinaryNotFound {
        name: "home directory".to_string(),
    })?;

    let common_paths = [
        home.join(".cleen").join("bin").join("frame-runtime"),
        home.join(".local").join("bin").join("frame-runtime"),
        PathBuf::from("/usr/local/bin/frame-runtime"),
    ];

    for path in common_paths {
        if path.exists() {
            return Ok(path);
        }
    }

    Err(CleenError::BinaryNotFound {
        name: "frame-runtime".to_string(),
    })
}

/// Find a binary in a directory (recursive)
fn find_binary_in_dir(dir: &Path, name: &str) -> Result<PathBuf> {
    let direct_path = dir.join(name);
    if direct_path.exists() {
        return Ok(direct_path);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            if let Ok(found) = find_binary_in_dir(&path, name) {
                return Ok(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            return Ok(path);
        }
    }

    Err(CleenError::BinaryNotFound {
        name: name.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Framework operations (delegated to frame-cli binary)
// ---------------------------------------------------------------------------

/// Find the frame-cli binary in the active frame version directory or PATH
fn find_frame_cli() -> Result<PathBuf> {
    let config = Config::load()?;

    // Check active frame version directory
    if let Some(ref frame_version) = config.frame_version {
        let version_dir = config.get_frame_versions_dir().join(frame_version);
        let cli_name = if cfg!(windows) {
            "frame-cli.exe"
        } else {
            "frame-cli"
        };
        let cli_path = version_dir.join(cli_name);
        if cli_path.exists() {
            return Ok(cli_path);
        }
    }

    // Check PATH
    if let Ok(path) = which::which("frame-cli") {
        return Ok(path);
    }

    eprintln!("Frame CLI is not installed.");
    eprintln!("Frame CLI is required for project creation, building, and scanning.");
    eprintln!();
    eprintln!("To install Frame CLI:");
    eprintln!("   cleen frame install");
    eprintln!("   cleen frame install <version>");

    Err(CleenError::BinaryNotFound {
        name: "frame-cli".to_string(),
    })
}

/// Create a new Frame project (delegates to frame-cli)
pub fn create_project(name: &str, template: &str, port: u16) -> Result<()> {
    let frame_cli = find_frame_cli()?;

    let status = Command::new(&frame_cli)
        .args([
            "new",
            name,
            "--template",
            template,
            "--port",
            &port.to_string(),
        ])
        .status()
        .map_err(|e| CleenError::CompilationFailed {
            message: format!("Failed to run frame-cli: {e}"),
        })?;

    if !status.success() {
        return Err(CleenError::CompilationFailed {
            message: "frame-cli new failed".to_string(),
        });
    }

    Ok(())
}

/// Build a Frame project (delegates to frame-cli)
pub fn build_project(input: &str, output: &str, optimize: &str) -> Result<()> {
    let frame_cli = find_frame_cli()?;

    let status = Command::new(&frame_cli)
        .args(["build", input, "--output", output, "--optimize", optimize])
        .status()
        .map_err(|e| CleenError::CompilationFailed {
            message: format!("Failed to run frame-cli: {e}"),
        })?;

    if !status.success() {
        return Err(CleenError::CompilationFailed {
            message: "frame-cli build failed".to_string(),
        });
    }

    Ok(())
}

/// Scan and discover project files (delegates to frame-cli)
pub fn scan_project(project_dir: &str, format: &str, verbose: bool) -> Result<()> {
    let frame_cli = find_frame_cli()?;

    let mut args = vec!["scan", project_dir, "--format", format];
    if verbose {
        args.push("--verbose");
    }

    let status = Command::new(&frame_cli).args(&args).status().map_err(|e| {
        CleenError::CompilationFailed {
            message: format!("Failed to run frame-cli: {e}"),
        }
    })?;

    if !status.success() {
        return Err(CleenError::CompilationFailed {
            message: "frame-cli scan failed".to_string(),
        });
    }

    Ok(())
}
