/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use pyrefly_python::module_path::ModulePath;

use crate::state::load::FileContents;

#[derive(Debug, Clone, Default)]
pub struct MemoryFiles(HashMap<ModulePath, Arc<FileContents>>);

#[derive(Debug, Default, Clone)]
pub struct MemoryFilesOverlay(HashMap<ModulePath, Option<Arc<FileContents>>>);

impl MemoryFiles {
    pub fn apply_overlay(&mut self, overlay: MemoryFilesOverlay) {
        for (path, contents) in overlay.0.into_iter() {
            match contents {
                None => self.0.remove(&path),
                Some(contents) => self.0.insert(path, contents),
            };
        }
    }
}

impl MemoryFilesOverlay {
    pub fn set(&mut self, path: ModulePath, contents: Option<Arc<FileContents>>) {
        self.0.insert(path, contents);
    }
}

pub struct MemoryFilesLookup<'a> {
    base: &'a MemoryFiles,
    overlay: &'a MemoryFilesOverlay,
}

impl<'a> MemoryFilesLookup<'a> {
    pub fn new(base: &'a MemoryFiles, overlay: &'a MemoryFilesOverlay) -> Self {
        Self { base, overlay }
    }

    pub fn get(&self, path: &ModulePath) -> Option<&'a Arc<FileContents>> {
        match self.overlay.0.get(path) {
            Some(contents) => contents.as_ref(), // Might be a None if deleted
            None => self.base.0.get(path),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::*;
    use crate::state::load::Load;

    #[test]
    fn lookup_keeps_module_identities_and_pending_removals_separate() {
        let path = PathBuf::from("same.py");
        let memory = ModulePath::memory(path.clone());
        let filesystem = ModulePath::filesystem(path);
        let memory_contents = Arc::new(FileContents::from_source("memory".to_owned()));
        let filesystem_contents = Arc::new(FileContents::from_source("filesystem".to_owned()));
        let mut base = MemoryFiles::default();
        let mut initial = MemoryFilesOverlay::default();
        initial.set(memory.clone(), Some(Arc::clone(&memory_contents)));
        initial.set(filesystem.clone(), Some(Arc::clone(&filesystem_contents)));
        base.apply_overlay(initial);

        let mut overlay = MemoryFilesOverlay::default();
        let lookup = MemoryFilesLookup::new(&base, &overlay);
        assert_eq!(lookup.get(&memory), Some(&memory_contents));
        assert_eq!(lookup.get(&filesystem), Some(&filesystem_contents));
        overlay.set(filesystem.clone(), None);
        let lookup = MemoryFilesLookup::new(&base, &overlay);
        assert!(lookup.get(&filesystem).is_none());
        assert_eq!(lookup.get(&memory), Some(&memory_contents));

        base.apply_overlay(overlay);
        let empty = MemoryFilesOverlay::default();
        let lookup = MemoryFilesLookup::new(&base, &empty);
        assert!(lookup.get(&filesystem).is_none());
        assert_eq!(lookup.get(&memory), Some(&memory_contents));
    }

    #[test]
    fn exact_overrides_and_removal_preserve_each_variants_normal_loader() {
        let directory = TempDir::new().unwrap();
        let file = directory.path().join("source.py");
        fs::write(&file, "saved\n").unwrap();
        let paths = [
            ModulePath::filesystem(file.clone()),
            ModulePath::memory(file.clone()),
            ModulePath::namespace(file),
            ModulePath::bundled_typeshed(PathBuf::from("builtins.pyi")),
            ModulePath::bundled_typeshed_third_party(PathBuf::from("__overlay_probe__.pyi")),
            ModulePath::bundled_third_party(PathBuf::from("__overlay_probe__.pyi")),
        ];
        let mut base = MemoryFiles::default();
        let empty = MemoryFilesOverlay::default();
        let normal: Vec<_> = paths
            .iter()
            .map(|path| {
                let (contents, error) =
                    Load::load_from_path(path, &MemoryFilesLookup::new(&base, &empty), None);
                (contents, error.map(|error| error.to_string()))
            })
            .collect();
        let mut initial = MemoryFilesOverlay::default();
        for (index, path) in paths.iter().enumerate() {
            initial.set(
                path.clone(),
                Some(Arc::new(FileContents::from_source(format!(
                    "override {index}\n"
                )))),
            );
        }
        base.apply_overlay(initial);
        for (index, path) in paths.iter().enumerate() {
            let (contents, error) =
                Load::load_from_path(path, &MemoryFilesLookup::new(&base, &empty), None);
            assert!(error.is_none(), "{path:?}: {error:?}");
            assert_eq!(
                contents,
                FileContents::from_source(format!("override {index}\n"))
            );
            let mut removed = MemoryFilesOverlay::default();
            removed.set(path.clone(), None);
            let (contents, error) =
                Load::load_from_path(path, &MemoryFilesLookup::new(&base, &removed), None);
            assert_eq!(
                (contents, error.map(|error| error.to_string())),
                normal[index]
            );
        }
    }

    #[test]
    fn removing_an_override_exposes_missing_filesystem_source() {
        let directory = TempDir::new().unwrap();
        let path = ModulePath::filesystem(directory.path().join("missing.py"));
        let mut base = MemoryFiles::default();
        let mut initial = MemoryFilesOverlay::default();
        initial.set(
            path.clone(),
            Some(Arc::new(FileContents::from_source("buffer\n".to_owned()))),
        );
        base.apply_overlay(initial);
        let mut pending = MemoryFilesOverlay::default();
        let (contents, error) =
            Load::load_from_path(&path, &MemoryFilesLookup::new(&base, &pending), None);
        assert!(error.is_none());
        assert_eq!(contents, FileContents::from_source("buffer\n".to_owned()));
        pending.set(path.clone(), None);
        let (_, error) =
            Load::load_from_path(&path, &MemoryFilesLookup::new(&base, &pending), None);
        assert!(
            error.is_some(),
            "Removing an override must expose the missing saved source"
        );
    }
}
