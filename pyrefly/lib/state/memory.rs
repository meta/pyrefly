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
    use std::path::PathBuf;

    use super::*;

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
}
