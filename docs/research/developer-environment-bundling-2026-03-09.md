# Developer Environment Bundling Research

Date: 2026-03-09
Task: `#812`

## Goal

Find a realistic path to make taurhaus + CLI tools feel "one-click" or close to it on Windows, macOS, and Linux without fighting the product's current architecture.

## Current Product Constraints

taurhaus is not a generic editor. It already assumes:

- tmux is the execution substrate for launched CLI sessions
- Windows uses WSL2 for daemon/process work
- Windows Terminal is the default built-in terminal target on Windows
- iTerm2 is the default built-in terminal target on macOS
- Linux currently relies on custom terminal commands rather than a built-in activator

Relevant local references:

- [ARCHITECTURE.md](../../ARCHITECTURE.md)
- [docs/features/command-center.md](../features/command-center.md)
- [CONTRIBUTING.md](../../CONTRIBUTING.md)
- [src/lib/FirstRunWizard.svelte](../../src/lib/FirstRunWizard.svelte)

That means the right answer is not "ship a random terminal image." The setup path has to respect native terminals, tmux, WSL on Windows, and local CLI installs.

## Executive Recommendation

Recommended primary approach:

1. Ship a native `taurhaus bootstrap` flow plus a first-run setup wizard.
2. Make it package-manager-backed and idempotent on each OS.
3. Install tools and drop managed config fragments instead of overwriting user dotfiles.
4. Offer an optional "taurhaus managed environment" mode for people who want the full opinionated setup.
5. Keep Docker/devcontainers as a secondary path for disposable demos, contributor onboarding, and CI-like reproducibility, not as the default runtime model.

Recommended secondary approach:

1. Publish a repo-level devcontainer / Codespaces path for contributors and trial users who want a preconfigured environment fast.
2. Use it as a fallback and remote onboarding lane, not as the main taurhaus desktop experience.

Not recommended as the default:

- Docker-only local runtime
- Nix/Home Manager as the first-run path for all users
- direct mutation of existing `.zshrc`, `.tmux.conf`, or terminal settings without a managed boundary

## Approach Options

## Option A: Native bootstrap command plus managed config fragments

Shape:

- app installer installs taurhaus
- first run detects platform/tooling state
- user opts into "Set up recommended environment"
- taurhaus runs a native bootstrap orchestrator
- bootstrap installs missing dependencies and writes taurhaus-managed config fragments

What it would own:

- prerequisite detection
- package-manager dispatch
- tmux install/config
- shell install/config
- CLI tool installers or install links
- font install prompt
- terminal profile import/registration where supported

Pros:

- fits taurhaus's actual native architecture
- works with current tmux and terminal integration model
- can be incremental and idempotent
- supports "configure only missing pieces" instead of wiping existing setups
- keeps users in their local environment, which matters for agent tools and secrets

Cons:

- highest implementation surface area
- each platform needs different privilege/error handling
- config merge rules must be very conservative
- third-party CLI installers may have license/distribution constraints

Assessment:

This is the best default path.

## Option B: Post-install scripts only

Shape:

- installer or app download ships a shell / PowerShell script
- user runs the script manually
- script installs dependencies and config

Pros:

- simpler to build than a full in-app bootstrap flow
- easy to iterate
- good stepping stone to the richer solution

Cons:

- weaker UX
- trust/friction problem around "run this long script"
- poor visibility and recovery if a step fails halfway through
- still needs all the same platform logic eventually

Assessment:

Useful as an initial implementation layer behind `taurhaus bootstrap`, but not good enough as the long-term user-facing experience.

## Option C: Bundle a full managed environment mode

Shape:

- taurhaus offers a "managed environment" option
- it owns dedicated config files, terminal profiles, fonts, and CLI installs under a taurhaus namespace
- users can opt out and keep existing setup untouched

Pros:

- best chance of an appealing out-of-box experience
- avoids merge hell with unknown user dotfiles
- easier to support and debug than arbitrary user environments

Cons:

- still needs native installers and package-manager glue
- can feel heavy-handed if enabled by default
- needs explicit rules for how taurhaus-managed config interacts with user config

Assessment:

This should be the opinionated mode layered on top of Option A. It is likely the best answer for users who want "make it look good and just work."

## Option D: Docker image / container-first environment

Shape:

- ship a Docker image with tmux, shell tooling, and CLI tools preinstalled
- taurhaus attaches work to that environment or launches inside it

Pros:

- strongest reproducibility for the toolchain itself
- easier to keep versions pinned
- good for demos, workshops, onboarding sandboxes, and CI-like contributor environments

Cons:

- poor fit for native terminal integration
- awkward host/container boundary for tmux attach/focus behavior
- awkward secret handling and OS keychain integration
- filesystem and performance trade-offs on macOS and Windows
- does not solve "nice native terminal" unless taurhaus also manages a host terminal anyway

Assessment:

Good as an optional lane, bad as the default taurhaus experience.

## Option E: Declarative environment (Nix / Home Manager)

Shape:

- publish a declarative environment spec
- users opt into Nix or Home Manager to reproduce the toolchain and shell config

Pros:

- best long-term reproducibility
- good for power users and contributors
- clean versioned config story

Cons:

- too much conceptual overhead for mainstream first-run onboarding
- introduces its own ecosystem and trust boundary
- still does not solve all terminal-profile UX issues by itself

Assessment:

Offer as an optional advanced path only.

## Platform Analysis

## Windows

Windows is the hardest platform, but also the clearest architecturally because taurhaus already assumes WSL2.

Implications:

- WSL2 is not optional for the current taurhaus developer/runtime model
- CLI tools and tmux should live inside WSL
- terminal experience should center on Windows Terminal as the first-class managed target

Recommended Windows flow:

1. Detect whether WSL2 is present and healthy.
2. If missing, guide or invoke `wsl --install` with explicit restart/admin handling.
3. Ensure a supported distro exists.
4. Inside WSL, install tmux, zsh, starship or chosen prompt, and taurhaus-related CLI prerequisites.
5. Register a taurhaus Windows Terminal profile or fragment that launches into the correct WSL environment.
6. Offer per-user font installation for the recommended monospace font.

Why Windows Terminal should be first:

- taurhaus already integrates with it
- Microsoft documents settings JSON and profile-fragment extension points
- it matches the current `wt.exe` + `wsl.exe ... tmux attach` flow

Main risks:

- admin and reboot boundaries around WSL install
- multiple distros / default distro ambiguity
- Windows-to-WSL path and secret boundary complexity

## macOS

macOS is the easiest polished-native target.

Recommended macOS flow:

1. Detect Homebrew.
2. Use `brew bundle` or equivalent generated install plan for tmux, shell helpers, fonts, and optional terminal casks.
3. Default to iTerm2 for the managed experience because Dynamic Profiles are documented and import-friendly.
4. Fall back to Terminal.app or custom if the user declines iTerm2.
5. Install shell/theme pieces through managed fragments rather than replacing the user's login files.

Why iTerm2 is the best first managed target:

- taurhaus already supports it
- Dynamic Profiles provide a documented way to drop in profile JSON
- profile-based import is safer than mutating opaque app state

Main risks:

- Homebrew may be absent or installed in different prefixes
- shell config can already be highly customized
- GUI app permission prompts can interrupt "smooth" automation

## Linux

Linux should be supported, but with a lighter-touch strategy than Windows or macOS.

Recommended Linux flow:

1. Detect distro and package manager (`apt`, `dnf`, `pacman`, maybe `zypper`).
2. Install tmux, zsh, git, curl, and other system prerequisites through the system package manager.
3. Configure taurhaus-managed tmux and shell fragments.
4. Offer terminal-profile snippets for popular terminals, but do not promise deep one-click integration across every Linux desktop.

Why Linux should be lighter-touch:

- distro and terminal fragmentation is real
- taurhaus currently uses custom terminal commands on Linux anyway
- a strong package-manager + config-fragment story is more realistic than deep GUI-terminal automation

Main risks:

- distro fragmentation
- Wayland/X11 differences
- per-terminal config schemas vary widely

## Configuration Bundling Strategy

The safest rule is:

- install tools directly
- inject config by managed include/fragments
- never overwrite user-owned primary dotfiles unless they explicitly choose full managed mode

Recommended config model:

### tmux

- ship a taurhaus-managed tmux include file
- append one include line to `.tmux.conf` only if needed, or generate a dedicated taurhaus session config
- own mouse, colors, status bar, prefix, and sensible session defaults there

### shell

- ship a taurhaus-managed shell fragment
- source it from `.zshrc` only after explicit consent
- keep aliases, prompt integration, environment variables, and CLI helper functions inside that fragment

### terminal profiles

- Windows Terminal: install a profile fragment or managed profile JSON
- iTerm2: install Dynamic Profiles
- Linux: generate optional snippets for selected terminals rather than trying to own all of them

### fonts

- treat fonts as an explicit step
- install only on opt-in
- keep fallback-safe terminal settings if the preferred font is unavailable

### merge policy

- detect existing advanced setups and switch to "minimal integration" mode
- default to non-destructive behavior
- offer "show planned changes" before applying them

## Existing Art

## VS Code Dev Containers

Useful lesson:

- environment definition belongs in code (`devcontainer.json`, Features, image config)
- dependency installation can be layered and repeatable

What to borrow:

- declarative environment description
- reusable setup modules
- prebuild-friendly structure

What not to copy directly:

- devcontainers assume containerized workspaces, while taurhaus depends heavily on native tmux and host terminals

## GitHub Codespaces

Useful lesson:

- repo-scoped preconfigured environments are powerful when paired with dotfiles support
- the environment and personalization layers are separate concerns

What to borrow:

- keep project toolchain config separate from personal shell/theme preferences
- allow a "remote/disposable" path for fast trials and contributors

## Cursor

Useful lesson:

- CLI installation can be made extremely low-friction with a simple installer path
- shell command installation is a first-class onboarding step, not an afterthought

What to borrow:

- reduce the number of manual installation decisions
- make CLI availability testable during onboarding

## Gitpod / remote workspace model

Useful lesson:

- a remote or disposable environment is valuable for fast evaluation and contributor onboarding
- it is best treated as a parallel mode, not the only mode

What to borrow:

- "quickstart sandbox" thinking
- environment prebuilds for zero-to-running demos

## Nix / Home Manager

Useful lesson:

- declarative setup is excellent for reproducibility and power users
- it is a better advanced track than a default onboarding path

What to borrow:

- versioned environment definitions
- optional power-user support story

## Open-source bootstrap projects worth studying

## chezmoi

Why it is relevant:

- strong cross-platform dotfile management story
- explicitly designed for applying config to new machines
- better fit than ad hoc shell scripts when taurhaus needs safe, idempotent config ownership

Best lesson:

- separate source-of-truth config from the applied files on disk

## Dotbot

Why it is relevant:

- very simple bootstrap model
- good mental model for linking files, running setup steps, and keeping installs idempotent

Best lesson:

- a bootstrap system does not need to be huge to be useful if it has clear ownership and repeatable actions

## Devbox

Why it is relevant:

- useful middle ground between package-manager chaos and full Nix adoption
- project-scoped environment definition with shell hooks and scripts

Best lesson:

- project environment and user environment can be related without being the same thing

These are not all direct fits for taurhaus, but they are good references for config ownership, idempotence, and portable setup descriptions.

## First-Run Experience Proposal

Target flow:

1. Install taurhaus.
2. On first launch, run a fast environment scan.
3. Present one clear recommendation:
   - "Use my existing setup"
   - "Set up recommended taurhaus environment"
   - "Use remote/devcontainer environment"
4. Show the exact plan before applying changes.
5. Apply only the selected layers:
   - prerequisites
   - tmux
   - shell/theme
   - terminal profile
   - font
   - CLI tools
6. Run verification:
   - daemon available
   - tmux launch works
   - preferred terminal opens correctly
   - at least one supported CLI tool is installed
7. Land the user in a visually polished starter session, not an unstyled shell.

Design requirements for that experience:

- default to an actually pleasant theme/profile
- avoid surprise edits to existing dotfiles
- expose "advanced / I already have my own setup" early
- keep rollback and "undo managed config" possible
- prefer progress UI with recoverable failures over a silent long-running script

## Recommended Product Direction

## Primary recommendation: Hybrid native bootstrap

Build a hybrid system with three layers:

### Layer 1: detection and planning

- detect OS, package managers, terminal availability, shell, WSL, fonts, and existing config sophistication
- produce a plan instead of immediately mutating anything

### Layer 2: installation and managed integration

- install missing prerequisites through the native package-manager path
- install taurhaus-managed config fragments
- register terminal profiles where the platform supports clean import

### Layer 3: experience presets

- `minimal`: only install missing prerequisites and a tmux include
- `recommended`: install managed tmux + shell + terminal profile + font
- `advanced`: emit or point to declarative/devcontainer/Nix options

Why this is the best fit:

- matches taurhaus's real native runtime model
- can preserve existing user setups
- allows a polished path without forcing it on everyone
- keeps Docker and declarative setups as complements instead of pretending they solve the whole problem

## Secondary recommendation: publish a devcontainer / Codespaces lane

This should exist for:

- contributors
- trial users
- demos
- reproducible research/testing

It should not replace native onboarding because taurhaus still has to integrate with host terminals, tmux, and OS-specific CLI installs.

## Rough Implementation Outline

## Phase 1: planning and detection

1. Add a `taurhaus bootstrap plan` command.
2. Detect platform, package manager, WSL, terminal apps, shell, fonts, CLI tool presence, and config ownership markers.
3. Present a dry-run plan in the first-run wizard and in CLI form.

## Phase 2: managed install path

1. Add `taurhaus bootstrap apply`.
2. Implement package-manager adapters:
   - Windows: PowerShell + WSL orchestration
   - macOS: Homebrew / Brewfile path
   - Linux: distro package-manager adapters
3. Write taurhaus-managed config fragments and profile files.
4. Record bootstrap state so updates can be idempotent.

## Phase 3: experience polish

1. Add profile presets: `minimal`, `recommended`, `managed`.
2. Add preview of terminal/theme/tmux changes before apply.
3. Add health checks: can launch terminal, can attach tmux, CLI tools present, daemon present, fonts available.

## Phase 4: secondary lanes

1. Publish official devcontainer / Codespaces config.
2. Publish optional declarative environment examples.
3. Consider a Docker image for demos, not default use.

## Open Questions

1. Are we legally and operationally allowed to download/install Claude Code, Codex CLI, and Gemini CLI automatically, or do some need link-out / user confirmation flows?
2. Do we want taurhaus to manage API-key setup directly, or only detect missing keys and hand off to provider-specific instructions?
3. On Windows, do we want to require admin privileges for WSL bootstrapping, or keep that step guided-but-manual?
4. Should the managed experience target exactly one terminal per platform first (`windows_terminal`, `iterm2`) and leave others for later?
5. Do we want a taurhaus-owned shell fragment plus one source line, or a fully taurhaus-owned shell profile in managed mode?
6. How much of tmux/theme customization should be universal vs user-selectable?
7. Should the contributor path and the end-user path share one config source of truth, or should devcontainers live separately from native bootstrap assets?
8. Do we want a font bundled with the app, or do we prefer external installation to avoid packaging/licensing complexity?
9. How should rollback work if bootstrap partially succeeds?
10. How much Linux desktop integration do we actually want to support in v1, given the current `custom` terminal model on Linux?

## Bottom Line

The best answer is not "put everything in Docker" and not "make everyone learn Nix."

The best answer is:

- native bootstrap first
- managed config fragments instead of dotfile replacement
- Windows Terminal on Windows, iTerm2 on macOS, lighter-touch package-manager support on Linux
- devcontainer / Codespaces as a secondary reproducible lane
- optional managed mode for users who want the full polished terminal experience

That direction is consistent with taurhaus's existing architecture and gives a realistic path to a much better first-run experience without breaking advanced users' setups.

## Sources

- Microsoft WSL install docs: https://learn.microsoft.com/windows/wsl/install
- Microsoft WSL development guidance: https://learn.microsoft.com/windows/wsl/setup/environment
- Windows Terminal settings and profile docs: https://learn.microsoft.com/windows/terminal/
- Windows Terminal JSON fragment extensions: https://learn.microsoft.com/windows/terminal/json-fragment-extensions
- Homebrew install docs: https://brew.sh/
- Homebrew Bundle docs: https://docs.brew.sh/Brew-Bundle-and-Brewfile
- iTerm2 Dynamic Profiles: https://iterm2.com/documentation-dynamic-profiles.html
- VS Code Dev Containers: https://code.visualstudio.com/docs/devcontainers/containers
- Dev Container Features: https://containers.dev/features
- GitHub Codespaces personalization and dotfiles: https://docs.github.com/en/codespaces/personalizing-your-codespace/personalizing-github-codespaces-for-your-account
- GitHub Codespaces devcontainer introduction: https://docs.github.com/en/codespaces/setting-up-your-project-for-codespaces/introduction-to-dev-containers
- Starship install docs: https://starship.rs/guide/
- Oh My Zsh install docs: https://ohmyz.sh/
- Nix install docs: https://nixos.org/download/
- Home Manager project: https://github.com/nix-community/home-manager
- Cursor install docs: https://cursor.com/install
