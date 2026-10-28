# CLAUDE.md

This file provides guidance when working with code in this repository.

## Important Constraints

- **NEVER** write any reference to Claude Code in any documents, git commits, or any part of the code
- **NEVER** mention Claude Code in git commit messages or any part of the codebase

## Overview

This is the **Clean Language Manager** (`cleen`), a Rust-based version manager for the Clean Language compiler (`cln`). It allows developers to install, switch, and manage multiple versions of Clean Language across macOS, Linux, and Windows systems.

## Project Status

This is a **fully implemented and functional project**. The codebase is complete with all core functionality working.

## Documentation

**Comprehensive documentation is available in the `docs/` folder:**

- **[docs/README.md](docs/README.md)** - Documentation overview and navigation guide
- **[docs/architecture.md](docs/architecture.md)** - Technical architecture and system design
- **[docs/functional-specification.md](docs/functional-specification.md)** - Complete functional requirements and behavior
- **[docs/api-reference.md](docs/api-reference.md)** - Command-line interface documentation
- **[docs/user-guide.md](docs/user-guide.md)** - Practical usage guide and troubleshooting

**When working with this codebase, always reference the docs/ folder for:**
- Understanding the architecture before making changes
- Following established patterns and conventions
- Understanding command behavior and expected outputs
- Troubleshooting and debugging guidance

## Architecture Boundary Rules

**CRITICAL: Read `../foundation/management/ARCHITECTURE_BOUNDARIES.md` before implementing ANY new functionality.**

The manager is a **version manager and orchestrator**. It delegates to other binaries — it does NOT reimplement their logic.

### The Manager MUST NOT:

- Parse or understand Clean Language `.cln` syntax
- Generate Clean Language source code
- Know about framework folder conventions (`pages/`, `api/`, `data/`, `components/`)
- Discover/scan project files for routes, components, or models
- Transform HTML templates or expand component tags
- Implement any part of the build pipeline (codegen, discovery, compilation)
- Implement host bridge functions

### The Manager MUST delegate framework operations:

```rust
// CORRECT — delegate to frame-cli binary
Command::new("frame-cli").args(["build"]).status()?;
Command::new("frame-cli").args(["new", name]).status()?;
Command::new("frame-cli").args(["scan"]).status()?;

// WRONG — implement framework logic directly
codegen::generate_code(&project);  // DO NOT DO THIS
discovery::discover_project(&path); // DO NOT DO THIS
```

### Boundary Violation Check

Before writing ANY new function, ask:
1. Does this function read/parse/generate `.cln` code? → **STOP — belongs in compiler or frame-cli**
2. Does this function know about `pages/`, `api/`, `data/` folders? → **STOP — belongs in frame-cli**
3. Does this function transform HTML or expand templates? → **STOP — belongs in frame-cli**
4. Does this function implement a `_*` host function? → **STOP — belongs in clean-server**

### Known Violations (Pending Extraction)

The following files contain framework code that must be extracted to `clean-framework/frame-cli/`:
- `src/core/codegen.rs` (1,491 lines) — entire file is misplaced
- `src/core/discovery.rs` (745 lines) — entire file is misplaced
- `src/core/frame.rs` (~663 lines) — `create_project()`, `build_project()`, `scan_project()` and template functions

See: `../foundation/management/cross-component-prompts/manager-extract-framework-code.md`

## Current Implementation

The Clean Language Manager is **fully implemented** with the following architecture:

**Implemented Core Components:**
- ✅ **Version Management**: Downloads and installs compiler binaries from GitHub releases
- ✅ **Shim System**: Creates symbolic links to route `cln` commands to active versions
- ✅ **PATH Management**: Automatically configures shell environments
- ✅ **Storage Structure**: Organizes versions in isolated directories (`~/.cleen/versions/<version>/`)

**Working Commands:**
```bash
# Build and test (implemented)
cargo build --release
cargo test

# Core functionality (all working)
cleen install <version>     # Install any available version
cleen available            # List GitHub releases
cleen list                # Show installed versions
cleen use <version>       # Switch global version
cleen local <version>     # Set project version
cleen uninstall <version> # Remove version
cleen doctor              # Environment diagnostics
cleen init                # Shell setup
cleen sync                # Install from .cleanversion
```

## Codebase Structure

**Implemented Modules:**
- `src/main.rs` - CLI entry point with clap integration
- `src/commands/` - Complete command implementations (9 commands)
- `src/core/` - Core functionality (version, github, shim, config management)
- `src/utils/` - Cross-platform utilities (fs, shell integration)
- `src/error.rs` - Comprehensive error handling

**Integration Status:**
- ✅ GitHub API integration working (`Ivan-Pasco/clean-language-compiler-old`)
- ✅ Cross-platform binary downloads (Linux, macOS, Windows)
- ✅ Shell environment setup (bash, zsh, fish)
- ✅ Project-specific version files (`.cleanlanguage/.cleanversion`)

## Building and Testing

```bash
# Build (working)
cargo build --release

# Run any command
cargo run -- <command>

# Examples that work right now:
cargo run -- available    # Shows GitHub releases
cargo run -- list        # Shows installed versions  
cargo run -- doctor      # Environment check
```

## Cross-Component Work Policy

**CRITICAL: You are a Team Developer AI.** When you discover something in another component, choose the correct channel based on what you found:

| What you found | Channel | Why |
|---|---|---|
| A **bug** (crash, wrong output, spec violation, regression) | **`report_error` MCP tool** — MANDATORY | Fingerprint dedup, occurrence tracking, automatic user notification on fix, visible on errors.cleanlanguage.dev |
| A **design proposal, directive change, schema/API request, architectural ask** | Markdown file in `../foundation/management/cross-component-prompts/` | Requires discussion, not auto-fix |

**Never** write a markdown file for something that is a bug. Bug reports in markdown are invisible to the dashboard, don't notify users when fixed, and can't be queried via `list_component_bugs`.

### What You CAN Do

- Read files from other components to understand interfaces
- Call `report_error` for bugs found in other components
- Write markdown prompts for design/architecture discussions
- Update your component to work with existing interfaces

### What You MUST NOT Do

- Directly edit code in other components
- Make changes to other components' configuration files
- Write a markdown file for something that is a bug — use `report_error` instead

See `../foundation/management/USER_TYPES_AND_ERROR_REPORTING.md` for the full policy.

## Documentation Sync Protocol

Facts about the language live in `foundation/spec/` (at the project root). Facts about the platform live in `foundation/spec/platform/`. Do not duplicate them here — link to them instead.

**When you make a change in this component, update the corresponding spec file in the same commit:**

| Change type | Update required |
|-------------|-----------------|
| New or changed host bridge function | `foundation/spec/platform/HOST_BRIDGE.md` |
| New or changed execution layer | `foundation/spec/platform/EXECUTION_LAYERS.md` |

The manager delegates to the compiler and framework — it does not define language rules or platform contracts. When a change here affects a spec file, update that spec file in the same commit.

The spec files are the single source of truth. Component documentation explains implementation — it does not redefine language rules.

## Cross-component prompts

The team publishes cross-component prompts, change requests, and handoffs at https://errors.cleanlanguage.dev/prompts.

- `/team-prompt` publishes a prompt. Use it when this session discovered something a session in a **different** component must know before working: a required change there, a discovered contract, a blocker needing a decision from another maintainer, or a session-end handoff whose next reader is in a different repo.
- `/team-prompts-list` fetches open prompts addressed to this component (inferred from `$PWD`). Consider running it at session start if the user asks for status.

Do **not** use these skills for:
- Compiler/plugin/runtime bug reports — those go through `report_error` via the MCP server.
- Same-component notes — use `TASKS.md` or a session-handoff markdown file.
- Chat.

The API token lives at `~/.config/clean-errors/api_token` (mode 600). Team members without it get it from the team vault.