use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, CleenError>;

#[derive(Error, Debug)]
pub enum CleenError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Version '{version}' not found")]
    VersionNotFound { version: String },

    #[error("Version '{version}' is already installed")]
    VersionAlreadyInstalled { version: String },

    #[error("No version is currently active")]
    #[allow(dead_code)]
    NoActiveVersion,

    #[error("Invalid version format: '{version}'")]
    InvalidVersion { version: String },

    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("GitHub API error: {message}")]
    #[allow(dead_code)]
    GitHubError { message: String },

    #[error("Download failed: {url}")]
    DownloadError { url: String },

    #[error("Extraction failed: {path}")]
    ExtractionError { path: PathBuf },

    #[error("Home directory not found")]
    HomeDirectoryNotFound,

    #[error("Shell configuration error: {message}")]
    ShellError { message: String },

    #[error("Permission denied: {path}")]
    PermissionDenied { path: PathBuf },

    #[error("Binary not found: {name}")]
    BinaryNotFound { name: String },

    #[error("Environment setup error: {message}")]
    #[allow(dead_code)]
    EnvironmentError { message: String },

    #[error("Binary validation error: {message}")]
    ValidationError { message: String },

    #[error("Update error: {message}")]
    UpdateError { message: String },

    #[error("Frame CLI version '{frame_version}' not found")]
    FrameVersionNotFound { frame_version: String },

    #[error("Frame CLI version '{frame_version}' is already installed")]
    #[allow(dead_code)]
    FrameVersionAlreadyInstalled { frame_version: String },

    #[error("Frame CLI {frame_version} requires compiler >= {required_compiler}, but current compiler is {current_compiler}")]
    FrameIncompatible {
        frame_version: String,
        required_compiler: String,
        current_compiler: String,
    },

    #[error(
        "No compiler installed. Frame CLI requires a Clean Language compiler to be installed first"
    )]
    NoCompilerForFrame,

    #[error(
        "Cannot uninstall compiler {compiler_version}: Frame CLI {frame_version} depends on it"
    )]
    #[allow(dead_code)]
    FrameDependsOnCompiler {
        compiler_version: String,
        frame_version: String,
    },

    // Server errors
    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("Compilation failed: {message}")]
    CompilationFailed { message: String },

    #[error("Server failed to start: {message}")]
    ServerStartFailed { message: String },

    #[error("IO error: {message}")]
    IoError { message: String },

    // Project errors
    #[error("Project '{name}' already exists")]
    ProjectAlreadyExists { name: String },

    #[error("Invalid template: '{template}'. Valid templates: api, web, minimal")]
    InvalidTemplate { template: String },

    // Plugin errors
    #[error("Plugin '{name}' not found")]
    PluginNotFound { name: String },

    #[error("Plugin '{name}' version '{version}' not found")]
    PluginVersionNotFound { name: String, version: String },

    #[error("Plugin '{name}' is already installed")]
    PluginAlreadyInstalled { name: String },

    #[error("Plugin manifest not found: {path}")]
    PluginManifestNotFound { path: PathBuf },

    #[error("Invalid plugin manifest: {message}")]
    PluginManifestError { message: String },

    #[error("Plugin build failed: {message}")]
    PluginBuildError { message: String },

    #[error("Plugin '{name}' requires compiler >= {required}, but current is {current}")]
    #[allow(dead_code)]
    PluginIncompatible {
        name: String,
        required: String,
        current: String,
    },

    #[error("Plugin registry error: {message}")]
    PluginRegistryError { message: String },

    #[error("No compiler installed. Plugins require a Clean Language compiler")]
    NoCompilerForPlugin,

    // Clean Server errors
    #[error("Clean Server version '{version}' not found")]
    ServerVersionNotFound { version: String },

    #[error("Clean Server version '{version}' is not installed")]
    ServerVersionNotInstalled { version: String },

    #[error("No Clean Server version is installed. Run 'cleen server install' first")]
    NoServerInstalled,

    #[error("Clean Server asset not found for version '{version}' on platform '{platform}'")]
    ServerAssetNotFound { version: String, platform: String },

    // Test errors
    #[error("Test error: {message}")]
    TestError { message: String },

    // Register errors
    #[error("Sign-up failed: {message}")]
    RegisterError { message: String },
}

impl From<anyhow::Error> for CleenError {
    fn from(error: anyhow::Error) -> Self {
        CleenError::ShellError {
            message: error.to_string(),
        }
    }
}

impl CleenError {
    #[allow(dead_code)]
    pub fn config_error<S: Into<String>>(message: S) -> Self {
        CleenError::ConfigError {
            message: message.into(),
        }
    }

    #[allow(dead_code)]
    pub fn github_error<S: Into<String>>(message: S) -> Self {
        CleenError::GitHubError {
            message: message.into(),
        }
    }

    #[allow(dead_code)]
    pub fn shell_error<S: Into<String>>(message: S) -> Self {
        CleenError::ShellError {
            message: message.into(),
        }
    }

    #[allow(dead_code)]
    pub fn environment_error<S: Into<String>>(message: S) -> Self {
        CleenError::EnvironmentError {
            message: message.into(),
        }
    }
}
