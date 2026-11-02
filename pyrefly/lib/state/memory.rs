/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::state::load::FileContents;

#[derive(Debug, Clone, Default)]
pub struct MemoryFiles(HashMap<PathBuf, Arc<FileContents>>);

#[derive(Debug, Default, Clone)]
pub struct MemoryFilesOverlay(HashMap<PathBuf, Option<Arc<FileContents>>>);

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
    pub fn set(&mut self, path: PathBuf, contents: Option<Arc<FileContents>>) {
        self.0.insert(path, contents);
    }
}

pub struct MemoryFilesLookup<'a> {
    base: &'a MemoryFiles,
    overlay: &'a MemoryFilesOverlay,
    filesystem_overlays: bool,
}

impl<'a> MemoryFilesLookup<'a> {
    pub fn new(base: &'a MemoryFiles, overlay: &'a MemoryFilesOverlay) -> Self {
        Self {
            base,
            overlay,
            filesystem_overlays: false,
        }
    }

    pub fn with_filesystem_overlays(mut self, enabled: bool) -> Self {
        self.filesystem_overlays = enabled;
        self
    }

    pub fn filesystem_overlays(&self) -> bool {
        self.filesystem_overlays
    }

    pub fn get(&self, path: &Path) -> Option<&'a Arc<FileContents>> {
        match self.overlay.0.get(path) {
            Some(contents) => contents.as_ref(), // Might be a None if deleted
            None => self.base.0.get(path),
        }
    }
}

#[cfg(test)]
mod filesystem_overlay_tests {
    use std::fs;

    use pyrefly_python::module_path::ModulePath;
    use tempfile::TempDir;

    use super::*;
    use crate::state::load::Load;

    #[test]
    fn filesystem_load_requires_opt_in_and_removal_returns_to_disk() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("file.py");
        fs::write(&path, "saved\n").unwrap();
        let mut base = MemoryFiles::default();
        let mut initial = MemoryFilesOverlay::default();
        initial.set(
            path.clone(),
            Some(Arc::new(FileContents::from_source(
                "committed\n".to_owned(),
            ))),
        );
        base.apply_overlay(initial);
        let mut overlay = MemoryFilesOverlay::default();
        let module = ModulePath::filesystem(path.clone());

        for (enabled, expected) in [(false, "saved\n"), (true, "committed\n")] {
            let lookup = MemoryFilesLookup::new(&base, &overlay).with_filesystem_overlays(enabled);
            let (contents, error) = Load::load_from_path(&module, &lookup, None);
            assert!(error.is_none(), "{error:?}");
            assert_eq!(contents, FileContents::from_source(expected.to_owned()));
        }

        overlay.set(path.clone(), None);
        let lookup = MemoryFilesLookup::new(&base, &overlay).with_filesystem_overlays(true);
        let (contents, error) = Load::load_from_path(&module, &lookup, None);
        assert!(error.is_none(), "{error:?}");
        assert_eq!(contents, FileContents::from_source("saved\n".to_owned()));

        fs::remove_file(&path).unwrap();
        let (_, error) = Load::load_from_path(&module, &lookup, None);
        assert!(
            error.is_some(),
            "Missing disk source must not become a successful overlay read"
        );
    }
}
