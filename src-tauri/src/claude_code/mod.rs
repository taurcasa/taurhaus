//! Claude Code filesystem data access (ADR-019).
//!
//! Claude Code stores per-project data at `~/.claude/projects/<slug>/`.
//! This module resolves project paths to their Claude Code data directories
//! and provides parsers for memory files, team configs, and task lists.
//!
//! v1: resolver + basic parsers. Full UI integration in v1.1.

pub mod memory;
pub mod resolver;
pub mod teams;
