use std::path::{Path, PathBuf};

use crate::components::statusline::VcsInfo;
use crate::config::editor::FilePickerConfig;

/// Find workspace root starting from the current working directory.
/// Returns `(workspace_root, is_cwd_fallback)`.
pub fn find_workspace() -> (PathBuf, bool) {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    find_workspace_in(current_dir)
}

/// Find workspace root starting from `dir` and walking up ancestor directories.
/// Looks for marker directories/files: `.git`, `.svn`, `.jj`, `.strand`, or `.helix`.
/// If no marker is found, returns `(dir, true)` indicating fallback.
pub fn find_workspace_in(dir: impl AsRef<Path>) -> (PathBuf, bool) {
    let dir = dir.as_ref();
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    for ancestor in dir.ancestors() {
        if ancestor.join(".git").exists()
            || ancestor.join(".svn").exists()
            || ancestor.join(".jj").exists()
            || ancestor.join(".strand").exists()
            || ancestor.join(".helix").exists()
        {
            return (ancestor.to_path_buf(), false);
        }
    }
    (dir, true)
}

/// Represents an active project workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    root: PathBuf,
    name: String,
}

impl Workspace {
    /// Create a workspace rooted at `root`.
    pub fn new(root: PathBuf) -> Self {
        let root = root.canonicalize().unwrap_or(root);
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Strand".to_string());
        Self { root, name }
    }

    /// Discover workspace from an optional path (file or directory) or fallback to CWD.
    pub fn from_path_or_cwd(path: Option<&Path>) -> Self {
        let (root, _) = match path {
            Some(p) => {
                if p.is_dir() {
                    find_workspace_in(p)
                } else if let Some(parent) = p.parent() {
                    if parent.as_os_str().is_empty() {
                        find_workspace()
                    } else {
                        find_workspace_in(parent)
                    }
                } else {
                    find_workspace()
                }
            }
            None => find_workspace(),
        };
        Self::new(root)
    }

    /// Absolute canonical path to the workspace root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Human-readable workspace name (typically directory basename).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Check if a path is contained within this workspace.
    pub fn contains_file(&self, path: &Path) -> bool {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        path.starts_with(&self.root)
    }

    /// Compute relative path of `path` from workspace root.
    /// If outside workspace, returns canonical/original path.
    pub fn relative_path(&self, path: &Path) -> PathBuf {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        path.strip_prefix(&self.root)
            .map(|p| p.to_path_buf())
            .unwrap_or(path)
    }

    /// Query VCS status for this workspace (detects branch from `.git/HEAD`).
    pub fn vcs_info(&self) -> VcsInfo {
        let git_dir = self.root.join(".git");
        let head_path = if git_dir.is_dir() {
            Some(git_dir.join("HEAD"))
        } else if git_dir.is_file() {
            // Git worktree or submodule: `.git` contains "gitdir: <path>"
            if let Ok(content) = std::fs::read_to_string(&git_dir) {
                content
                    .lines()
                    .find(|l| l.starts_with("gitdir:"))
                    .map(|l| {
                        let rel_path = l["gitdir:".len()..].trim();
                        let resolved = self.root.join(rel_path);
                        resolved.join("HEAD")
                    })
            } else {
                None
            }
        } else {
            None
        };

        let branch = head_path.and_then(|hp| {
            std::fs::read_to_string(hp).ok().map(|content| {
                let trimmed = content.trim();
                if let Some(branch_ref) = trimmed.strip_prefix("ref: refs/heads/") {
                    branch_ref.to_string()
                } else if trimmed.len() >= 7 {
                    trimmed[..7].to_string()
                } else {
                    trimmed.to_string()
                }
            })
        });

        VcsInfo {
            branch,
            added: 0,
            modified: 0,
            deleted: 0,
        }
    }

    /// Walk project directory respecting `.gitignore` and `FilePickerConfig`.
    pub fn walk_files(&self, config: &FilePickerConfig) -> Vec<PathBuf> {
        let mut builder = ignore::WalkBuilder::new(&self.root);
        builder
            .hidden(config.hidden)
            .parents(config.parents)
            .ignore(config.ignore)
            .follow_links(config.follow_symlinks)
            .git_ignore(config.git_ignore)
            .git_global(config.git_global)
            .git_exclude(config.git_exclude);

        if let Some(depth) = config.max_depth {
            builder.max_depth(Some(depth));
        }

        builder.sort_by_file_name(|a, b| a.cmp(b));

        let mut files = Vec::new();
        for entry in builder.build().filter_map(Result::ok) {
            if entry.file_type().is_some_and(|ft| ft.is_file()) {
                files.push(entry.into_path());
            }
        }
        files
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_find_in_current_repo() {
        let (root, is_cwd) = find_workspace();
        assert!(!is_cwd, "Strand repo has .git, should find workspace");
        assert!(root.join("Cargo.toml").exists());
    }

    #[test]
    fn test_workspace_vcs_info() {
        let ws = Workspace::from_path_or_cwd(None);
        let vcs = ws.vcs_info();
        assert!(vcs.branch.is_some(), "Should detect git branch in Strand repo");
    }

    #[test]
    fn test_workspace_relative_path() {
        let ws = Workspace::from_path_or_cwd(None);
        let cargo_toml = ws.root().join("Cargo.toml");
        let rel = ws.relative_path(&cargo_toml);
        assert_eq!(rel, Path::new("Cargo.toml"));
    }

    #[test]
    fn test_workspace_walk_files() {
        let ws = Workspace::from_path_or_cwd(None);
        let config = FilePickerConfig::default();
        let files = ws.walk_files(&config);
        assert!(!files.is_empty(), "Should find project files");
        assert!(
            files.iter().any(|f| f.ends_with("Cargo.toml")),
            "Cargo.toml must be in walk_files"
        );
        assert!(
            files.iter().any(|f| f.ends_with("src/main.rs")),
            "src/main.rs must be in walk_files"
        );
        // Ensure .git directory files are ignored by default git_ignore
        assert!(
            !files.iter().any(|f| f.to_string_lossy().contains("/.git/")),
            ".git internal files must be ignored"
        );
    }
}
