/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::hash::Hash;
use std::hash::Hasher;
use std::path::PathBuf;
use std::sync::Arc;

use dupe::Dupe;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_name::ModuleNameWithKind;
use pyrefly_python::module_path::ModulePath;
use pyrefly_python::sys_info::SysInfo;

/// A handle to a Python module, containing its name, path, and system information.
#[derive(Debug, Clone, Dupe, PartialEq, Eq, PartialOrd, Ord)]
pub struct Handle {
    module: ModuleNameWithKind,
    path: ModulePath,
    sys_info: SysInfo,
    /// The site-package path of the importer's Python environment, when this module is checked in
    /// that environment rather than in its own config's. The same file checked in two environments
    /// therefore has two handles.
    importer_site_package_path: Option<Arc<[PathBuf]>>,
}

impl Hash for Handle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.module.hash(state);
        self.path.hash(state);
        self.sys_info.hash(state);
        // The importer's site-package path is left out, which keeps a handle's hash independent of
        // the Python environment it is checked in. Equal handles still hash equally, and equality
        // still compares the path.
    }
}

impl Handle {
    /// Create a new handle with a guaranteed module name.
    pub fn new(module: ModuleName, path: ModulePath, sys_info: SysInfo) -> Self {
        Self {
            module: ModuleNameWithKind::guaranteed(module),
            path,
            sys_info,
            importer_site_package_path: None,
        }
    }

    /// Create a new handle from a ModuleNameWithKind.
    pub fn from_with_module_name_kind(
        module: ModuleNameWithKind,
        path: ModulePath,
        sys_info: SysInfo,
    ) -> Self {
        Self {
            module,
            path,
            sys_info,
            importer_site_package_path: None,
        }
    }

    /// The same handle, checked in the Python environment with the given site-package path.
    pub fn with_importer_site_package_path(self, path: Arc<[PathBuf]>) -> Self {
        Self {
            importer_site_package_path: Some(path),
            ..self
        }
    }

    /// Get the underlying module name.
    pub fn module(&self) -> ModuleName {
        self.module.name()
    }

    /// Get the module name kind (guaranteed or fallback).
    pub fn module_kind(&self) -> ModuleNameWithKind {
        self.module
    }

    pub fn path(&self) -> &ModulePath {
        &self.path
    }

    pub fn sys_info(&self) -> &SysInfo {
        &self.sys_info
    }

    /// The same module, checked in the Python environment of its own config.
    pub fn without_importer_site_package_path(&self) -> Self {
        Self {
            importer_site_package_path: None,
            ..self.dupe()
        }
    }

    pub fn importer_site_package_path(&self) -> Option<&Arc<[PathBuf]>> {
        self.importer_site_package_path.as_ref()
    }

    /// Returns true if this handle's module name was created using fallback heuristics.
    /// When true, the module name is not reliable for determining project structure.
    pub fn is_fallback(&self) -> bool {
        self.module.is_fallback()
    }
}
