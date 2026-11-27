use anyhow::Result;
use clap::{Parser, Subcommand};

// Use the library modules
use cleen::{commands, core};

#[derive(Parser)]
#[clap(name = "cleen")]
#[clap(about = "Clean Language version manager")]
#[clap(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install a specific version of Clean Language
    Install {
        /// Version to install (e.g., 1.2.3, latest)
        version: String,
        /// Also install Frame CLI
        #[clap(long)]
        with_frame: bool,
        /// Skip Frame CLI prompt
        #[clap(long)]
        no_frame: bool,
    },
    /// Install the version specified in .cleanlanguage/.cleanversion file
    Sync,
    /// List installed versions
    List {
        /// List Frame CLI versions
        #[clap(long)]
        frame: bool,
    },
    /// List available versions from GitHub
    Available,
    /// Switch to a specific version globally
    Use {
        /// Version to use globally
        version: String,
        /// Use Frame CLI version instead
        #[clap(long)]
        frame: bool,
    },
    /// Set project-specific version (creates .cleanlanguage/.cleanversion file)
    Local {
        /// Version to use in this project
        version: String,
    },
    /// Uninstall a specific version
    Uninstall {
        /// Version to uninstall
        version: String,
        /// Uninstall Frame CLI version instead
        #[clap(long)]
        frame: bool,
        /// Force uninstall even if Frame depends on it
        #[clap(long)]
        force: bool,
    },
    /// Initialize shell configuration
    Init,
    /// Check and repair environment setup
    Doctor {
        /// Check Frame CLI installation
        #[clap(long)]
        frame: bool,
    },
    /// Check for Clean Language compiler updates
    Update,
    /// Update cleen itself to the latest version
    SelfUpdate,
    /// Sign up for Clean Language news and security alerts (optional)
    Register {
        /// Print the sign-up link instead of opening the browser
        #[clap(long)]
        print: bool,
        /// Sign up from the terminal with this email (no browser needed)
        #[clap(long, requires = "name", conflicts_with = "print")]
        email: Option<String>,
        /// Your name (used with --email)
        #[clap(long, requires = "email")]
        name: Option<String>,
        /// Phone number, optional (used with --email)
        #[clap(long, requires = "email")]
        phone: Option<String>,
        /// Also get release notes by email (used with --email)
        #[clap(long, requires = "email")]
        releases: bool,
        /// Also get the monthly news email (used with --email)
        #[clap(long, requires = "email")]
        news: bool,
    },
    /// Clean up old compiler and plugin versions
    Cleanup {
        /// Actually remove versions (without this flag, shows what would be removed)
        #[clap(long)]
        confirm: bool,
        /// Number of old versions to keep (default: 3)
        #[clap(long, default_value = "3")]
        keep: usize,
        /// Clean up plugins instead of compiler versions
        #[clap(long)]
        plugins: bool,
        /// Prune `*.locked-*` eviction graveyards under ~/.cleen/plugins/
        #[clap(long)]
        graveyards: bool,
    },
    /// Frame CLI management
    Frame {
        #[clap(subcommand)]
        command: FrameCommands,
    },
    /// Plugin management
    Plugin {
        #[clap(subcommand)]
        command: PluginCommands,
    },
    /// Clean Server management (executes compiled WASM applications)
    Server {
        #[clap(subcommand)]
        command: ServerCommands,
    },
    /// Run tests in a Clean Language project
    Test {
        /// Specific file to test (e.g., app/data/models/User.cln)
        file: Option<String>,
        /// Filter tests by name or file path pattern
        #[clap(short, long)]
        filter: Option<String>,
        /// Show verbose output including compilation details
        #[clap(short, long)]
        verbose: bool,
        /// Show timing information for each test
        #[clap(short, long)]
        timing: bool,
    },
}

#[derive(Subcommand)]
enum FrameCommands {
    /// Create a new Frame project
    New {
        /// Name of the project to create
        name: String,
        /// Project template: api, web, or minimal (default: api)
        #[clap(short, long, default_value = "api")]
        template: String,
        /// Port for development server (default: 3000)
        #[clap(short, long, default_value = "3000")]
        port: u16,
    },
    /// Scan and discover project files (dry-run for build)
    Scan {
        /// Project directory to scan (default: current directory)
        #[clap(default_value = ".")]
        project: String,
        /// Output format: text or json
        #[clap(short, long, default_value = "text")]
        format: String,
        /// Show verbose output including file paths
        #[clap(short, long)]
        verbose: bool,
    },
    /// Build a Frame project for production
    Build {
        /// Input file or project directory (default: current directory)
        #[clap(default_value = ".")]
        input: String,
        /// Output directory (default: dist/)
        #[clap(short, long, default_value = "dist")]
        output: String,
        /// Optimization level: 0, 1, 2, 3, s, z (default: 2)
        #[clap(short = 'O', long, default_value = "2")]
        optimize: String,
    },
    /// Start a development server for a Frame application
    Serve {
        /// Input file to serve (.cln source file with endpoints)
        #[clap(default_value = "app/api/main.cln")]
        input: String,
        /// Port to listen on (default: 3000)
        #[clap(short, long, default_value = "3000")]
        port: u16,
        /// Host to bind to (default: 127.0.0.1)
        #[clap(long, default_value = "127.0.0.1")]
        host: String,
        /// Enable debug output
        #[clap(short, long)]
        debug: bool,
    },
    /// Stop a running Frame development server
    Stop,
    /// Install Frame runtime
    Install {
        /// Version to install (optional, auto-detects compatible version)
        version: Option<String>,
    },
    /// List installed Frame versions
    List,
    /// Switch to a specific Frame version
    Use {
        /// Version to use
        version: String,
    },
    /// Uninstall a Frame version
    Uninstall {
        /// Version to uninstall
        version: String,
    },
}

#[derive(Subcommand)]
enum PluginCommands {
    /// Install a plugin from the registry
    Install {
        /// Plugin name or name@version (e.g., frame.web or frame.web@1.0.0)
        plugin: String,
        /// Install from a local directory instead of registry
        #[clap(long)]
        local: bool,
    },
    /// List installed plugins
    List,
    /// Create a new plugin project
    Create {
        /// Name of the plugin to create
        name: String,
    },
    /// Build the plugin in the current directory
    Build,
    /// Publish the plugin to the registry
    Publish,
    /// Remove an installed plugin
    Remove {
        /// Name of the plugin to remove
        name: String,
    },
    /// Switch to a specific plugin version
    Use {
        /// Plugin name
        name: String,
        /// Version to use
        version: String,
    },
}

#[derive(Subcommand)]
enum ServerCommands {
    /// Install Clean Server (WASM execution runtime)
    Install {
        /// Version to install (optional, uses latest if not specified)
        version: Option<String>,
    },
    /// List installed Clean Server versions
    List,
    /// Switch to a specific Clean Server version
    Use {
        /// Version to use
        version: String,
    },
    /// Uninstall a Clean Server version
    Uninstall {
        /// Version to uninstall
        version: String,
    },
    /// Run a compiled WASM application
    Run {
        /// Path to the WASM file to execute
        wasm_file: String,
        /// Port to listen on (default: 3000)
        #[clap(short, long, default_value = "3000")]
        port: u16,
        /// Host to bind to (default: 127.0.0.1)
        #[clap(long, default_value = "127.0.0.1")]
        host: String,
    },
    /// Show Clean Server version and status
    Status,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Best-effort weekly heartbeat so the errors dashboard can advance bugs
    // from fix_released → fix_installed for long-running projects that
    // haven't run `cleen install` recently. Silent on failure; gated by
    // CLEEN_HEARTBEAT env var. See core::heartbeat.
    core::heartbeat::maybe_send_weekly();

    let result = match cli.command {
        Commands::Install {
            version,
            with_frame,
            no_frame,
        } => commands::install::install_version(&version, with_frame, no_frame)
            .map_err(|e| anyhow::anyhow!(e)),
        Commands::Sync => commands::sync::sync_project_version().map_err(|e| anyhow::anyhow!(e)),
        Commands::List { frame } => {
            commands::list::list_versions(frame).map_err(|e| anyhow::anyhow!(e))
        }
        Commands::Available => commands::available::list_available_versions(),
        Commands::Use { version, frame } => {
            commands::use_version::use_version(&version, frame).map_err(|e| anyhow::anyhow!(e))
        }
        Commands::Local { version } => {
            commands::local::set_local_version(&version).map_err(|e| anyhow::anyhow!(e))
        }
        Commands::Uninstall {
            version,
            frame,
            force,
        } => commands::uninstall::uninstall_version(&version, frame, force)
            .map_err(|e| anyhow::anyhow!(e)),
        Commands::Init => commands::init::init_shell().map_err(|e| anyhow::anyhow!(e)),
        Commands::Doctor { frame } => {
            commands::doctor::check_environment(frame).map_err(|e| anyhow::anyhow!(e))
        }
        Commands::Update => commands::update::check_for_updates().map_err(|e| anyhow::anyhow!(e)),
        Commands::SelfUpdate => {
            commands::update::update_self_auto().map_err(|e| anyhow::anyhow!(e))
        }
        Commands::Register {
            print,
            email,
            name,
            phone,
            releases,
            news,
        } => match email {
            Some(email) => commands::register::register_by_email(commands::register::EmailSignup {
                email,
                name: name.unwrap_or_default(),
                phone,
                releases,
                news,
            }),
            None => commands::register::register(print),
        }
        .map_err(|e| anyhow::anyhow!(e)),
        Commands::Cleanup {
            confirm,
            keep,
            plugins,
            graveyards,
        } => {
            if graveyards {
                if confirm {
                    commands::cleanup::cleanup_graveyards_execute().map_err(|e| anyhow::anyhow!(e))
                } else {
                    commands::cleanup::cleanup_graveyards_dry_run().map_err(|e| anyhow::anyhow!(e))
                }
            } else if plugins {
                if confirm {
                    commands::cleanup::cleanup_plugins_execute().map_err(|e| anyhow::anyhow!(e))
                } else {
                    commands::cleanup::cleanup_plugins_dry_run().map_err(|e| anyhow::anyhow!(e))
                }
            } else if confirm {
                commands::cleanup::cleanup_execute(keep).map_err(|e| anyhow::anyhow!(e))
            } else {
                commands::cleanup::cleanup_dry_run(keep).map_err(|e| anyhow::anyhow!(e))
            }
        }
        Commands::Frame { command } => match command {
            FrameCommands::New {
                name,
                template,
                port,
            } => {
                core::frame::create_project(&name, &template, port).map_err(|e| anyhow::anyhow!(e))
            }
            FrameCommands::Scan {
                project,
                format,
                verbose,
            } => core::frame::scan_project(&project, &format, verbose)
                .map_err(|e| anyhow::anyhow!(e)),
            FrameCommands::Build {
                input,
                output,
                optimize,
            } => core::frame::build_project(&input, &output, &optimize)
                .map_err(|e| anyhow::anyhow!(e)),
            FrameCommands::Serve {
                input,
                port,
                host,
                debug,
            } => core::frame::serve_application(&input, port, &host, debug)
                .map_err(|e| anyhow::anyhow!(e)),
            FrameCommands::Stop => core::frame::stop_server().map_err(|e| anyhow::anyhow!(e)),
            FrameCommands::Install { version } => {
                core::frame::install_frame(version.as_deref(), false)
                    .map_err(|e| anyhow::anyhow!(e))
            }
            FrameCommands::List => {
                let config = core::config::Config::load().map_err(|e| anyhow::anyhow!(e))?;
                let versions =
                    core::frame::list_frame_versions(&config).map_err(|e| anyhow::anyhow!(e))?;

                if versions.is_empty() {
                    println!("No Frame versions installed");
                    println!();
                    println!("To install Frame:");
                    println!("   cleen frame install");
                } else {
                    println!("Installed Frame versions:");
                    for v in &versions {
                        let marker = if config.frame_version.as_deref() == Some(v) {
                            "* "
                        } else {
                            "  "
                        };
                        println!("{marker}{v}");
                    }
                }
                Ok(())
            }
            FrameCommands::Use { version } => {
                core::frame::use_frame_version(&version).map_err(|e| anyhow::anyhow!(e))
            }
            FrameCommands::Uninstall { version } => {
                core::frame::uninstall_frame_version(&version).map_err(|e| anyhow::anyhow!(e))
            }
        },
        Commands::Plugin { command } => match command {
            PluginCommands::Install { plugin, local } => {
                if local {
                    let path = std::path::Path::new(&plugin);
                    commands::plugin::install_local_plugin(path).map_err(|e| anyhow::anyhow!(e))
                } else {
                    commands::plugin::install_plugin(&plugin).map_err(|e| anyhow::anyhow!(e))
                }
            }
            PluginCommands::List => {
                commands::plugin::list_plugins().map_err(|e| anyhow::anyhow!(e))
            }
            PluginCommands::Create { name } => {
                commands::plugin::create_plugin(&name).map_err(|e| anyhow::anyhow!(e))
            }
            PluginCommands::Build => {
                commands::plugin::build_plugin().map_err(|e| anyhow::anyhow!(e))
            }
            PluginCommands::Publish => {
                commands::plugin::publish_plugin().map_err(|e| anyhow::anyhow!(e))
            }
            PluginCommands::Remove { name } => {
                commands::plugin::remove_plugin_command(&name).map_err(|e| anyhow::anyhow!(e))
            }
            PluginCommands::Use { name, version } => {
                commands::plugin::use_plugin_version(&name, &version)
                    .map_err(|e| anyhow::anyhow!(e))
            }
        },
        Commands::Test {
            file,
            filter,
            verbose,
            timing,
        } => commands::test::run_tests(file.as_deref(), filter.as_deref(), verbose, timing)
            .map_err(|e| anyhow::anyhow!(e)),
        Commands::Server { command } => match command {
            ServerCommands::Install { version } => {
                core::server::install_server(version.as_deref()).map_err(|e| anyhow::anyhow!(e))
            }
            ServerCommands::List => core::server::list_versions().map_err(|e| anyhow::anyhow!(e)),
            ServerCommands::Use { version } => {
                core::server::use_version(&version).map_err(|e| anyhow::anyhow!(e))
            }
            ServerCommands::Uninstall { version } => {
                core::server::uninstall_version(&version).map_err(|e| anyhow::anyhow!(e))
            }
            ServerCommands::Run {
                wasm_file,
                port,
                host,
            } => core::server::run_wasm(&wasm_file, port, &host).map_err(|e| anyhow::anyhow!(e)),
            ServerCommands::Status => core::server::show_status().map_err(|e| anyhow::anyhow!(e)),
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }

    Ok(())
}
