use crate::shred;
use std::fs;
use std::path::PathBuf;

pub struct File {
    pub name: String,
    pub size: u64,
    pub path: PathBuf,
}

impl File {
    pub fn wipe(&self) {
        shred::wipe_file(
            &self.path.display().to_string(),
            3,
            true,
            Some(self.size),
            true,
            true,
            true,
            false,
        )
    }
}

pub struct Files(pub Vec<File>);

impl Iterator for Files {
    type Item = File;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop()
    }
}

pub fn walk_dir(path: &str) -> Result<Files, Box<dyn std::error::Error>> {
    let mut files = Files(Vec::new());
    for entry in fs::read_dir(path)? {
        let dir = entry?;
        if dir.path().is_dir() {
            walk_dir(&dir.path().display().to_string())?.for_each(|file| files.0.push(file));
        }
        if dir.path().is_file() {
            files.0.push(File {
                name: dir.path().display().to_string(),
                size: dir.metadata()?.len(),
                path: dir.path(),
            })
        }
    }
    Ok(files)
}
