//! Module resolver for the Mog language module system.
//!
//! Implements Go-style package resolution:
//! - Package = directory convention
//! - `mog.mod` file at project root
//! - Import paths relative to module root
//! - `pub` keyword for exported symbols
//! - Circular import detection

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::ast::Statement;
use crate::types::Type;

// ── Public types ─────────────────────────────────────────────────────

/// A single Mog package (one directory of `.mog` files).
#[derive(Debug, Clone)]
pub struct MogModule {
    /// Import path, e.g. `"math"`.
    pub path: PathBuf,
    /// Short package name (directory basename or declared name).
    pub package_name: String,
    /// Absolute paths of every `.mog` file in the package.
    pub files: Vec<PathBuf>,
    /// Merged AST from all files (populated after parsing).
    pub ast: Option<Statement>,
    /// Publicly exported symbols.
    pub exports: HashMap<String, ExportedSymbol>,
}

/// A symbol exported via `pub`.
#[derive(Debug, Clone)]
pub struct ExportedSymbol {
    pub name: String,
    pub symbol_type: Type,
}

/// The full dependency graph for a Mog project.
#[derive(Debug, Clone)]
pub struct ModuleGraph {
    /// Filesystem root (directory containing `mog.mod`).
    pub root: PathBuf,
    /// Resolved packages keyed by import path.
    pub packages: HashMap<String, MogModule>,
    /// The name declared in `mog.mod` (`module <name>`).
    pub main_package: String,
}

// ── mog.mod parsing ──────────────────────────────────────────────────

/// Parse a `mog.mod` file and return the module name.
///
/// Expected format (one declaration per file):
/// ```text
/// module myproject
/// ```
pub fn parse_mod_file(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("module ") {
            let name = name.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Walk up from `start` looking for a directory that contains `mog.mod`.
pub fn find_module_root(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(start)
    };

    loop {
        if dir.join("mog.mod").is_file() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

// ── Module resolver ──────────────────────────────────────────────────

/// Resolve the full module graph rooted at `root`.
///
/// 1. Reads `mog.mod` to get the project name.
/// 2. Scans immediate subdirectories for `.mog` files; each directory is
///    treated as one package.
/// 3. Detects circular imports (though actual import-edge extraction
///    requires AST parsing, which is stubbed here).
pub fn resolve_modules(root: &Path) -> Result<ModuleGraph, String> {
    // 1. Read mog.mod
    let mod_path = root.join("mog.mod");
    let mod_content = std::fs::read_to_string(&mod_path)
        .map_err(|e| format!("Cannot read {}: {}", mod_path.display(), e))?;
    let main_package = parse_mod_file(&mod_content)
        .ok_or_else(|| format!("No `module` declaration in {}", mod_path.display()))?;

    // 2. Scan subdirectories for packages
    let mut packages: HashMap<String, MogModule> = HashMap::new();

    // Also consider root-level .mog files as the main package
    let root_mog_files = collect_mog_files(root);
    if !root_mog_files.is_empty() {
        packages.insert(
            main_package.clone(),
            MogModule {
                path: root.to_path_buf(),
                package_name: main_package.clone(),
                files: root_mog_files,
                ast: None,
                exports: HashMap::new(),
            },
        );
    }

    let entries = std::fs::read_dir(root)
        .map_err(|e| format!("Cannot read directory {}: {}", root.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Directory entry error: {}", e))?;
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }

        let dir_name = match entry_path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Skip hidden directories and common non-package dirs
        if dir_name.starts_with('.') || dir_name == "target" || dir_name == "node_modules" {
            continue;
        }

        let mog_files = collect_mog_files(&entry_path);
        if mog_files.is_empty() {
            continue;
        }

        packages.insert(
            dir_name.clone(),
            MogModule {
                path: entry_path,
                package_name: dir_name,
                files: mog_files,
                ast: None,
                exports: HashMap::new(),
            },
        );
    }

    // 3. Circular import detection (placeholder — full detection requires
    //    parsing import statements from each file's AST).
    detect_circular_imports(&packages)?;

    Ok(ModuleGraph {
        root: root.to_path_buf(),
        packages,
        main_package,
    })
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Collect all `.mog` files in a single directory (non-recursive).
fn collect_mog_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("mog"))
        .collect();

    files.sort();
    files
}

/// Detect circular imports by scanning each `.mog` file for `import` lines
/// and running a DFS over the resulting dependency graph.
///
/// This is a best-effort text-level scan (looks for `import "<pkg>"` lines)
/// that works without a full parser.  A more precise version can replace
/// this once the AST is populated on each `MogModule`.
fn detect_circular_imports(packages: &HashMap<String, MogModule>) -> Result<(), String> {
    // Build adjacency list from textual import scanning
    let mut edges: HashMap<String, Vec<String>> = HashMap::new();

    for (pkg_name, module) in packages {
        let mut deps = Vec::new();
        for file in &module.files {
            if let Ok(content) = std::fs::read_to_string(file) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    // Match `import "pkg"` or `import ( "pkg1" "pkg2" )`
                    if let Some(rest) = trimmed.strip_prefix("import ") {
                        let rest = rest.trim();
                        if rest.starts_with('(') {
                            // Multi-import block: extract quoted names
                            for part in rest.split('"') {
                                let part = part.trim();
                                if !part.is_empty()
                                    && !part.starts_with('(')
                                    && !part.starts_with(')')
                                {
                                    deps.push(part.to_string());
                                }
                            }
                        } else if rest.starts_with('"') {
                            // Single import
                            if let Some(name) =
                                rest.strip_prefix('"').and_then(|s| s.strip_suffix('"'))
                            {
                                deps.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
        edges.insert(pkg_name.clone(), deps);
    }

    // DFS cycle detection
    let mut visited = HashSet::new();
    let mut in_stack = HashSet::new();

    for pkg in edges.keys() {
        if !visited.contains(pkg.as_str()) {
            dfs_cycle_check(pkg, &edges, &mut visited, &mut in_stack)?;
        }
    }

    Ok(())
}

fn dfs_cycle_check(
    node: &str,
    edges: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    in_stack: &mut HashSet<String>,
) -> Result<(), String> {
    visited.insert(node.to_string());
    in_stack.insert(node.to_string());

    if let Some(deps) = edges.get(node) {
        for dep in deps {
            if in_stack.contains(dep.as_str()) {
                return Err(format!("Circular import detected: {} -> {}", node, dep));
            }
            if !visited.contains(dep.as_str()) {
                dfs_cycle_check(dep, edges, visited, in_stack)?;
            }
        }
    }

    in_stack.remove(node);
    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mod_file_basic() {
        assert_eq!(
            parse_mod_file("module myproject\n"),
            Some("myproject".into())
        );
    }

    #[test]
    fn parse_mod_file_with_comments() {
        let content = "// Mog module file\nmodule hello\n";
        assert_eq!(parse_mod_file(content), Some("hello".into()));
    }

    #[test]
    fn parse_mod_file_empty() {
        assert_eq!(parse_mod_file(""), None);
        assert_eq!(parse_mod_file("// just a comment"), None);
    }

    #[test]
    fn find_module_root_returns_none_for_nonexistent() {
        assert!(find_module_root(Path::new("/nonexistent/path/xyz")).is_none());
    }

    #[test]
    fn cycle_detection_no_cycle() {
        let mut packages = HashMap::new();
        packages.insert(
            "a".to_string(),
            MogModule {
                path: PathBuf::new(),
                package_name: "a".into(),
                files: vec![],
                ast: None,
                exports: HashMap::new(),
            },
        );
        packages.insert(
            "b".to_string(),
            MogModule {
                path: PathBuf::new(),
                package_name: "b".into(),
                files: vec![],
                ast: None,
                exports: HashMap::new(),
            },
        );
        // No files to scan, so no edges, so no cycles
        assert!(detect_circular_imports(&packages).is_ok());
    }
}
