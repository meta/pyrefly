/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::path::PathBuf;

use dupe::Dupe;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_name::ModuleNameWithKind;
use pyrefly_python::module_path::ModulePath;
use pyrefly_python::sys_info::SysInfo;
use static_interner::Intern;
use static_interner::Interner;

static SITE_PACKAGE_PATH_INTERNER: Interner<Vec<PathBuf>> = Interner::new();

/// The site-package path of a Python environment. Interned so that copying, comparing and
/// hashing it is cheap. There is about one value per Python environment, so the memory that
/// interning never frees is negligible.
#[derive(Clone, Dupe, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SitePackagePath(Intern<Vec<PathBuf>>);

impl SitePackagePath {
    pub fn new(paths: Vec<PathBuf>) -> Self {
        Self(SITE_PACKAGE_PATH_INTERNER.intern(paths))
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.0
    }

    /// Every site-package path interned so far.
    pub fn all() -> impl Iterator<Item = Self> {
        SITE_PACKAGE_PATH_INTERNER.iter().map(Self)
    }
}

/// A handle to a Python module, containing its name, path, and system information.
#[derive(Debug, Clone, Dupe, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Handle {
    module: ModuleNameWithKind,
    path: ModulePath,
    sys_info: SysInfo,
    /// The site-package path of the Python environment this module is checked in, when that is an
    /// importer's environment rather than its own config's. One file can therefore have several
    /// handles.
    importer_site_package_path: Option<SitePackagePath>,
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

    /// A handle for another module with the same `SysInfo`, checked in the Python environment of
    /// its own config.
    pub fn for_module(&self, module: ModuleName, path: ModulePath) -> Self {
        Self::new(module, path, self.sys_info.dupe())
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

    /// This module, checked in the Python environment with site-package path `path`, or in the
    /// Python environment of its own config if `path` is `None`.
    pub fn with_importer_site_package_path(self, path: Option<SitePackagePath>) -> Self {
        Self {
            importer_site_package_path: path,
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

    pub fn importer_site_package_path(&self) -> Option<SitePackagePath> {
        self.importer_site_package_path
    }

    /// Returns true if this handle's module name was created using fallback heuristics.
    /// When true, the module name is not reliable for determining project structure.
    pub fn is_fallback(&self) -> bool {
        self.module.is_fallback()
    }
}
