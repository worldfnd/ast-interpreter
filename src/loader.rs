//! Test loader for a Noir package using the stock stdlib: resolves `Nargo.toml`, parses the
//! sources, and exposes the inputs `nargo::prepare_package` needs.

use std::path::PathBuf;

use fm::FileManager;
use nargo::package::Package;
use nargo::workspace::Workspace;
use nargo_toml::{PackageSelection, get_package_manifest, resolve_workspace_from_toml};
use noirc_frontend::hir::ParsedFiles;

use super::validation_frontend::PackageSource;

/// A loaded single-package Noir project built against the stock stdlib.
pub(crate) struct NoirProject {
    workspace: Workspace,
    file_manager: FileManager,
    parsed_files: ParsedFiles,
}

impl NoirProject {
    /// Load the Noir package rooted at `root` (the directory containing its `Nargo.toml`).
    pub(crate) fn new(root: PathBuf) -> Result<Self, String> {
        let toml_path = get_package_manifest(&root).map_err(|e| format!("manifest: {e}"))?;
        let workspace = resolve_workspace_from_toml(&toml_path, PackageSelection::All, None)
            .map_err(|e| format!("workspace: {e}"))?;
        // Seed the embedded stdlib, then read and parse the package files.
        let mut file_manager = workspace.new_file_manager();
        nargo::insert_all_files_for_workspace_into_file_manager(&workspace, &mut file_manager);
        let parsed_files = nargo::parse_all(&file_manager);
        Ok(Self {
            workspace,
            file_manager,
            parsed_files,
        })
    }
}

impl PackageSource for NoirProject {
    fn file_manager(&self) -> &FileManager {
        &self.file_manager
    }

    fn parsed_files(&self) -> &ParsedFiles {
        &self.parsed_files
    }

    fn get_only_crate(&self) -> &Package {
        assert_eq!(
            self.workspace.members.len(),
            1,
            "expected exactly one package in the project, got {}",
            self.workspace.members.len()
        );
        &self.workspace.members[0]
    }
}

// Parked behind an always-false cfg until the mavros-compiler dependency is available; restore
// `#[cfg(feature = "mavros-oracle")]` then.
#[cfg(any())]
impl PackageSource for mavros_compiler::project::Project {
    fn file_manager(&self) -> &FileManager {
        self.file_manager()
    }

    fn parsed_files(&self) -> &ParsedFiles {
        self.parsed_files()
    }

    fn get_only_crate(&self) -> &Package {
        self.get_only_crate()
    }
}
