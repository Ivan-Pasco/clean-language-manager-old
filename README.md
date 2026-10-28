# Clean Language Manager

A cross-platform version manager for the Clean Language compiler, similar to rustup for Rust or nvm for Node.js.

## ⚡ Quick Start

### One-Line Installation

**Unix/Linux/macOS:**
```bash
curl -sSL https://github.com/Ivan-Pasco/clean-language-manager-old/releases/latest/download/install.sh | bash
```

**Windows PowerShell:**
```powershell
iwr https://github.com/Ivan-Pasco/clean-language-manager-old/releases/latest/download/install.ps1 | iex
```

### 🚀 **NEW: Fully Automated Setup**
```bash
# Automatic shell detection and PATH configuration
cleen init

# 🔧 Initializing Clean Language Manager
# 
# 📁 Clean Language Manager directories:
#   - Manager directory: "/Users/you/.cleen"
#   - Binary directory: "/Users/you/.cleen/bin"
#   - Versions directory: "/Users/you/.cleen/versions"
#
# 🛣️  Configuring PATH for Clean Language Manager
#
# Detected shell: zsh
# Configuration file: /Users/you/.zshrc
#
# Would you like to automatically add Clean Language Manager to your PATH? (Y/n): 
# ✅ Successfully configured PATH!
#
# 🔄 To apply the changes:
#   1. Restart your terminal, OR
#   2. Run: source /Users/you/.zshrc
#
# Then run 'cleen doctor' to verify your setup.
```

### Complete Setup
```bash
# Verify everything works
cleen doctor

# List available Clean Language versions from GitHub
cleen available

# Install and use a version
cleen install 0.1.2
cleen use 0.1.2

# Now `cln` command is available!
cln --version
```

## Features

### 🚀 **Automated Experience**
- **One-Line Installation**: Fully automated installation with PATH configuration
- **Interactive Setup**: Smart shell detection with automatic config file updates (`cleen init`)
- **Zero Configuration**: Works out-of-the-box with sensible defaults
- **Professional UX**: Clear guidance, error recovery, and helpful feedback

### ⚙️ **Version Management**
- **Multi-version Support**: Install and manage multiple Clean Language compiler versions
- **Easy Switching**: Switch between versions with simple commands (`cleen use <version>`)
- **GitHub Integration**: Direct access to official Clean Language releases (`cleen available`)
- **Isolated Installations**: Each version stored separately with automatic routing

### 🔧 **Platform & Shell Support**
- **Cross-Platform**: Native support for macOS (Intel & Apple Silicon), Linux, and Windows
- **Shell Integration**: Supports bash, zsh, and fish with proper syntax and PATH management
- **Environment Diagnostics**: Built-in health checking and troubleshooting (`cleen doctor`)
- **Smart Error Handling**: Helpful error messages with recovery suggestions

## Installation

### Quick Install (Recommended)

#### Unix/Linux/macOS
```bash
curl -sSL https://github.com/Ivan-Pasco/clean-language-manager-old/releases/latest/download/install.sh | bash
```

#### Windows (PowerShell)
```powershell
iwr https://github.com/Ivan-Pasco/clean-language-manager-old/releases/latest/download/install.ps1 | iex
```

### Manual Installation

1. **Download** the appropriate binary for your platform from [Releases](https://github.com/Ivan-Pasco/clean-language-manager-old/releases/latest):
   - **Linux (x86_64)**: `cleen-linux-x86_64.tar.gz`
   - **macOS (Intel)**: `cleen-macos-x86_64.tar.gz`
   - **macOS (Apple Silicon)**: `cleen-macos-aarch64.tar.gz`
   - **Windows (x86_64)**: `cleen-windows-x86_64.zip`

2. **Extract** the archive:
   ```bash
   # Unix/Linux/macOS
   tar -xzf cleen-*.tar.gz
   
   # Windows
   # Extract using Windows Explorer or 7-Zip
   ```

3. **Move** the binary to a directory in your PATH:
   ```bash
   # Unix/Linux/macOS
   sudo mv cleen /usr/local/bin/
   
   # Or to user directory
   mkdir -p ~/.local/bin
   mv cleen ~/.local/bin/
   ```

4. **Initialize** the environment:
   ```bash
   cleen init
   ```

### From Source

```bash
git clone https://github.com/Ivan-Pasco/clean-language-manager-old.git
cd clean-language-manager
cargo build --release
cp target/release/cleen ~/.local/bin/  # or your preferred location
```

## Usage

### Initial Setup
```bash
# Set up your environment (adds cleen to PATH)
cleen init

# Verify everything is working
cleen doctor
```

### Version Management
```bash
# Install a specific version
cleen install 1.2.3

# List installed versions
cleen list

# Switch to a version (makes it active)
cleen use 1.2.3

# List available versions from GitHub
cleen available

# Uninstall a version
cleen uninstall 1.2.3
```

### Getting Help
```bash
# Show help for all commands
cleen --help

# Show help for a specific command
cleen install --help
```

## Architecture

The manager organizes versions in isolated directories:

```
~/.cleen/
├── bin/cln                    # Shim to active version
├── versions/
│   ├── 1.2.3/cln             # Version-specific binaries
│   └── 1.3.0/cln
└── config.json               # Manager configuration
```

## How It Works

Clean Language Manager stores different compiler versions in isolated directories and uses a shim system to route the `cln` command to the currently active version:

1. **Download**: Fetches compiler binaries from GitHub releases
2. **Install**: Extracts and stores each version in `~/.cleen/versions/<version>/`
3. **Activate**: Creates a symlink/shim in `~/.cleen/bin/cln` pointing to the active version
4. **Route**: When you run `cln`, it automatically uses the active version

## Troubleshooting

### Command Not Found
If you get `command not found: cleen`:
1. Make sure the binary is in your PATH: `echo $PATH`
2. Run `cleen init` to get setup instructions
3. Restart your terminal after updating your shell configuration

### Clean Language Not Working
If `cln` doesn't work after installation:
1. Run `cleen doctor` to diagnose issues
2. Make sure you've activated a version: `cleen use <version>`
3. Check that `~/.cleen/bin` is in your PATH

### Permission Issues
If you get permission errors:
- On Unix systems, make sure the binary is executable: `chmod +x cleen`
- On Windows, you may need to run PowerShell as Administrator

## Development

See [TASKS.md](TASKS.md) for current implementation progress and [CLAUDE.md](CLAUDE.md) for development guidance.

### Building from Source

```bash
# Check compilation
cargo check

# Run with help
cargo run -- --help

# Test a command
cargo run -- doctor

# Build release binary
cargo build --release
```

### Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `cargo test`
5. Submit a pull request

## Related Projects

- [Clean Language Compiler](https://github.com/Ivan-Pasco/clean-language-compiler-old) - The main compiler this tool manages

## License

MIT License