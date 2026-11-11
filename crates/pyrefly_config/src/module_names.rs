/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! A module name for each listed source file, given with `--module-names`. A build system can
//! import a file under a name its path doesn't give, e.g. Buck's `base_module` makes
//! `lib/src/platform.py` the module `platform_api.platform`. The map lets Pyrefly find such
//! modules without the build system, and lets reports name them.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context as _;
use dupe::Dupe as _;
use pyrefly_build::handle::Handle;
use pyrefly_build::source_db::LiveSourceDatabase;
use pyrefly_build::source_db::SourceDatabase;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use pyrefly_python::module_path::ModuleStyle;
use pyrefly_python::sys_info::SysInfo;
use pyrefly_util::arc_id::ArcId;
use pyrefly_util::fs_anyhow;
use pyrefly_util::interned_path::InternedPath;
use starlark_map::small_map::SmallMap;

#[derive(Default)]
pub struct ModuleNames {
    /// The working directory that the file paths are relative to.
    cwd: PathBuf,
    /// Module names by file path, as given: relative to `cwd`, with forward slashes.
    by_file: SmallMap<String, ModuleName>,
    /// Absolute paths of the files that each module name is given to. A name can have several
    /// files, e.g. vendored copies of a library.
    by_module: SmallMap<ModuleName, Vec<InternedPath>>,
}

/// A summary: the map can hold millions of entries, and configs that hold it are logged.
impl fmt::Debug for ModuleNames {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModuleNames")
            .field("cwd", &self.cwd)
            .field("files", &self.by_file.len())
            .finish_non_exhaustive()
    }
}

impl ModuleNames {
    /// Read a JSON object mapping file paths, relative to `cwd`, to module names.
    pub fn load(path: &Path, cwd: &Path) -> anyhow::Result<Self> {
        let names: HashMap<String, String> =
            serde_json::from_str(&fs_anyhow::read_to_string(path)?)
                .with_context(|| format!("Invalid module names file `{}`", path.display()))?;
        Ok(Self::new(names, cwd))
    }

    pub fn new(names: HashMap<String, String>, cwd: &Path) -> Self {
        let mut by_file = SmallMap::with_capacity(names.len());
        let mut by_module: SmallMap<ModuleName, Vec<InternedPath>> = SmallMap::new();
        for (file, name) in names {
            let name = ModuleName::from_str(&name);
            by_module
                .entry(name)
                .or_default()
                .push(InternedPath::new(cwd.join(&file)));
            by_file.insert(file, name);
        }
        // Sorted so that the choice among equally near files doesn't depend on hash order.
        by_module.values_mut().for_each(|paths| paths.sort());
        Self {
            cwd: cwd.to_path_buf(),
            by_file,
            by_module,
        }
    }

    /// The module name of a file, given by its path relative to the working directory.
    pub fn module_of_file(&self, file: &str) -> Option<ModuleName> {
        self.by_file.get(file).copied()
    }

    /// The module name of a file, given by its absolute path. A file outside the working
    /// directory is listed by its absolute path.
    pub fn module_of_path(&self, path: &Path) -> Option<ModuleName> {
        let listed = path.strip_prefix(&self.cwd).unwrap_or(path).to_str()?;
        self.module_of_file(&listed.replace('\\', "/"))
    }

    pub fn contains_module(&self, module: ModuleName) -> bool {
        self.by_module.contains_key(&module)
    }

    /// The file given the name `module`, if any. Among several such files, the one sharing the
    /// longest path prefix with the importing file `origin`: a vendored copy is most likely
    /// imported from code next to it. `style_filter` restricts the files to stubs or sources.
    pub fn find(
        &self,
        module: ModuleName,
        origin: Option<&Path>,
        style_filter: Option<ModuleStyle>,
    ) -> Option<&Path> {
        let candidates =
            self.by_module.get(&module)?.iter().filter(|path| {
                style_filter.is_none_or(|style| ModuleStyle::of_path(path) == style)
            });
        let shared_prefix = |path: &Path| {
            origin.map_or(0, |origin| {
                path.components()
                    .zip(origin.components())
                    .take_while(|(a, b)| a == b)
                    .count()
            })
        };
        // `max_by_key` keeps the last of equal maxima; reversing keeps the first file in path
        // order.
        candidates
            .rev()
            .max_by_key(|path| shared_prefix(path))
            .map(|path| path.as_path())
    }
}

/// Finds the listed modules by their given names, and gives the listed files those names. Meant
/// as a config's `fallback_source_db`, which is asked after the stdlib and site packages, so that
/// a listed file given the name of a stdlib module (e.g. a PyPI backport of `argparse`) doesn't
/// shadow that module.
#[derive(Debug)]
pub struct ModuleNamesSourceDatabase {
    names: ArcId<ModuleNames>,
    /// The Python settings of the config's files.
    sys_info: SysInfo,
}

impl ModuleNamesSourceDatabase {
    pub fn new(names: ArcId<ModuleNames>, sys_info: SysInfo) -> Self {
        Self { names, sys_info }
    }
}

impl SourceDatabase for ModuleNamesSourceDatabase {
    fn may_contain_module(&self, module: ModuleName) -> bool {
        self.names.contains_module(module)
    }

    fn lookup(
        &self,
        module: ModuleName,
        origin: Option<&Path>,
        style_filter: Option<ModuleStyle>,
    ) -> Option<ModulePath> {
        let path = self.names.find(module, origin, style_filter)?;
        Some(ModulePath::filesystem(path.to_path_buf()))
    }

    fn handle_from_module_path(&self, module_path: &ModulePath) -> Option<Handle> {
        let name = self.names.module_of_path(module_path.as_path())?;
        Some(Handle::new(name, module_path.dupe(), self.sys_info.dupe()))
    }

    fn as_live_source_database(&self) -> Option<&dyn LiveSourceDatabase> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(entries: &[(&str, &str)]) -> ModuleNames {
        ModuleNames::new(
            entries
                .iter()
                .map(|(file, name)| (file.to_string(), name.to_string()))
                .collect(),
            Path::new("/repo"),
        )
    }

    #[test]
    fn test_find_by_module_name() {
        let names = names(&[("lib/src/platform.py", "platform_api.platform")]);
        assert_eq!(
            names.find(ModuleName::from_str("platform_api.platform"), None, None),
            Some(Path::new("/repo/lib/src/platform.py")),
        );
        assert_eq!(
            names.find(ModuleName::from_str("lib.src.platform"), None, None),
            None,
            "a file is only found under the name it is given",
        );
        assert_eq!(
            names.module_of_file("lib/src/platform.py"),
            Some(ModuleName::from_str("platform_api.platform")),
        );
        assert_eq!(
            names.module_of_path(Path::new("/repo/lib/src/platform.py")),
            Some(ModuleName::from_str("platform_api.platform")),
        );
    }

    #[test]
    fn test_find_prefers_the_copy_nearest_the_importer() {
        let names = names(&[
            ("a/vendor/vllm/config.py", "vllm.config"),
            ("b/vendor/vllm/config.py", "vllm.config"),
        ]);
        let vllm_config = ModuleName::from_str("vllm.config");
        assert_eq!(
            names.find(vllm_config, Some(Path::new("/repo/b/app/main.py")), None),
            Some(Path::new("/repo/b/vendor/vllm/config.py")),
        );
        assert_eq!(
            names.find(vllm_config, Some(Path::new("/repo/a/main.py")), None),
            Some(Path::new("/repo/a/vendor/vllm/config.py")),
        );
    }

    #[test]
    fn test_find_respects_the_style_filter() {
        let names = names(&[("stubs/numpy/__init__.pyi", "numpy")]);
        let numpy = ModuleName::from_str("numpy");
        assert_eq!(names.find(numpy, None, Some(ModuleStyle::Executable)), None);
        assert_eq!(
            names.find(numpy, None, Some(ModuleStyle::Interface)),
            Some(Path::new("/repo/stubs/numpy/__init__.pyi")),
        );
    }
}
