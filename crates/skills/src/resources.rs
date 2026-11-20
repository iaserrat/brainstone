use std::path::{Path, PathBuf};
use std::{fs, io};

const MAX_DEPTH: usize = 6;
const MAX_DIRECTORIES: usize = 2000;
const MAX_RESOURCES: usize = 200;

pub(crate) fn list(root: &Path) -> io::Result<(Vec<String>, bool)> {
    let mut state = Scan {
        root,
        pending: vec![(root.to_owned(), 0)],
        files: Vec::new(),
        directories: 0,
    };
    while let Some((directory, depth)) = state.pending.pop() {
        if depth > MAX_DEPTH {
            continue;
        }
        if state.directories == MAX_DIRECTORIES {
            state.files.sort();
            return Ok((state.files, true));
        }
        state.directories += 1;
        state.directory(&directory, depth)?;
        if state.files.len() > MAX_RESOURCES {
            state.files.truncate(MAX_RESOURCES);
            state.files.sort();
            return Ok((state.files, true));
        }
    }
    state.files.sort();
    Ok((state.files, false))
}

struct Scan<'a> {
    root: &'a Path,
    pending: Vec<(PathBuf, usize)>,
    files: Vec<String>,
    directories: usize,
}

impl Scan<'_> {
    fn directory(&mut self, directory: &Path, depth: usize) -> io::Result<()> {
        let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let kind = entry.file_type()?;
            if kind.is_dir() {
                self.pending.push((entry.path(), depth + 1));
            } else if kind.is_file() && entry.file_name() != "SKILL.md" {
                self.files.push(relative(self.root, &entry.path()));
            }
        }
        Ok(())
    }
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("walked path is below root")
        .to_string_lossy()
        .replace('\\', "/")
}
