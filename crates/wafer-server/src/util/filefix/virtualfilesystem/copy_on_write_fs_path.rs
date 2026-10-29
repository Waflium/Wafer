use wafer_core::java_string::JavaStr;
use std::cmp::Ordering;
use std::fmt;
use std::io;

use super::copy_on_write_file_system::CopyOnWriteFileSystem;

const SEPARATOR: char = std::path::MAIN_SEPARATOR;

#[derive(Clone)]
pub struct CopyOnWriteFSPath {
    fs: CopyOnWriteFileSystem,
    names: Vec<String>,
    is_absolute: bool,
}

fn parse(segments: &[&str]) -> Vec<String> {
    segments.iter().flat_map(|segment| segment.split(['/', '\\'])).filter(|name| !name.is_empty()).map(str::to_string).collect()
}

fn names_equal(a: &str, b: &str) -> bool {
    a == b || a.to_uppercase() == b.to_uppercase() || a.to_lowercase() == b.to_lowercase()
}

impl CopyOnWriteFSPath {
    pub(crate) fn from_names(fs: CopyOnWriteFileSystem, names: Vec<String>, is_absolute: bool) -> CopyOnWriteFSPath {
        CopyOnWriteFSPath { fs, names, is_absolute }
    }

    pub(crate) fn of(fs: CopyOnWriteFileSystem, first: &str, more: &[&str]) -> CopyOnWriteFSPath {
        let mut is_absolute = false;
        let mut first = first;
        while let Some(rest) = first.strip_prefix('/') {
            is_absolute = true;
            first = rest;
        }
        let mut segments = vec![first];
        segments.extend_from_slice(more);
        CopyOnWriteFSPath { fs, names: parse(&segments), is_absolute }
    }

    pub fn get_file_system(&self) -> &CopyOnWriteFileSystem {
        &self.fs
    }

    pub fn is_absolute(&self) -> bool {
        self.is_absolute
    }

    pub fn get_root(&self) -> Option<CopyOnWriteFSPath> {
        self.is_absolute().then(|| self.fs.root_path())
    }

    pub fn get_file_name(&self) -> Option<CopyOnWriteFSPath> {
        if self.is_root() {
            None
        } else {
            Some(CopyOnWriteFSPath { fs: self.fs.clone(), names: self.names.last().cloned().into_iter().collect(), is_absolute: false })
        }
    }

    pub fn get_parent(&self) -> Option<CopyOnWriteFSPath> {
        if self.is_root() {
            None
        } else if self.names.len() <= 1 {
            self.is_absolute().then(|| self.fs.root_path())
        } else {
            Some(CopyOnWriteFSPath { fs: self.fs.clone(), names: self.names[..self.names.len() - 1].to_vec(), is_absolute: self.is_absolute })
        }
    }

    pub fn get_name_count(&self) -> usize {
        if self.names.is_empty() && !self.is_absolute { 1 } else { self.names.len() }
    }

    pub fn get_name(&self, index: usize) -> CopyOnWriteFSPath {
        if self.is_root() {
            panic!("java.lang.IllegalArgumentException");
        }
        let names = if self.names.is_empty() && index == 0 {
            Vec::new()
        } else {
            vec![self.names.get(index).cloned().unwrap_or_else(|| panic!("java.lang.IllegalArgumentException"))]
        };
        CopyOnWriteFSPath { fs: self.fs.clone(), names, is_absolute: false }
    }

    pub fn subpath(&self, begin_index: usize, end_index: usize) -> CopyOnWriteFSPath {
        CopyOnWriteFSPath { fs: self.fs.clone(), names: self.names[begin_index..end_index].to_vec(), is_absolute: false }
    }

    pub fn starts_with(&self, other: &CopyOnWriteFSPath) -> bool {
        self.is_absolute() == other.is_absolute()
            && self.names.len() >= other.names.len()
            && self.names.iter().zip(&other.names).all(|(a, b)| names_equal(a, b))
    }

    pub fn ends_with(&self, other: &CopyOnWriteFSPath) -> bool {
        if other.is_absolute() {
            self.is_absolute() && self == other
        } else {
            self.names.len() >= other.names.len() && self.names.iter().rev().zip(other.names.iter().rev()).all(|(a, b)| names_equal(a, b))
        }
    }

    pub fn normalize(&self) -> CopyOnWriteFSPath {
        let mut normalized: Vec<String> = Vec::with_capacity(self.names.len());
        for name in &self.names {
            if name == "." {
                continue;
            }
            if name == ".." && normalized.last().is_some_and(|last| last != "..") {
                normalized.pop();
            } else {
                normalized.push(name.clone());
            }
        }
        if self.is_absolute() && normalized.first().is_some_and(|first| first == "." || first == "..") {
            self.fs.root_path()
        } else {
            CopyOnWriteFSPath { fs: self.fs.clone(), names: normalized, is_absolute: self.is_absolute }
        }
    }

    pub fn resolve(&self, other: &CopyOnWriteFSPath) -> CopyOnWriteFSPath {
        if other.is_absolute() {
            return other.clone();
        }
        let mut names = self.names.clone();
        names.extend(other.names.iter().cloned());
        CopyOnWriteFSPath { fs: self.fs.clone(), names, is_absolute: self.is_absolute }
    }

    pub fn resolve_str(&self, other: &str) -> CopyOnWriteFSPath {
        self.resolve(&self.fs.get_path(other, &[]))
    }

    pub fn relativize(&self, other: &CopyOnWriteFSPath) -> CopyOnWriteFSPath {
        if self.is_absolute() != other.is_absolute() {
            panic!("java.lang.IllegalArgumentException: 'other' is different type of Path");
        }
        let common = self.names.iter().zip(&other.names).take_while(|(a, b)| names_equal(a, b)).count();
        let mut names: Vec<String> = vec!["..".to_string(); self.names.len() - common];
        names.extend(other.names[common..].iter().cloned());
        CopyOnWriteFSPath { fs: self.fs.clone(), names, is_absolute: false }
    }

    pub fn to_absolute_path(&self) -> CopyOnWriteFSPath {
        if self.is_absolute() { self.clone() } else { CopyOnWriteFSPath { fs: self.fs.clone(), names: self.names.clone(), is_absolute: true } }
    }

    pub fn to_real_path(&self) -> io::Result<CopyOnWriteFSPath> {
        self.fs.provider().get_real_path(self)
    }

    pub fn compare_to(&self, other: &CopyOnWriteFSPath) -> Ordering {
        self.to_string().java_utf16().cmp(other.to_string().java_utf16())
    }

    pub(crate) fn names(&self) -> &[String] {
        &self.names
    }

    pub(crate) fn is_root(&self) -> bool {
        self.get_name_count() == 0
    }

}

impl fmt::Display for CopyOnWriteFSPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_absolute() {
            f.write_str("/")?;
        }
        for (index, name) in self.names.iter().enumerate() {
            if index > 0 {
                write!(f, "{SEPARATOR}")?;
            }
            f.write_str(name)?;
        }
        Ok(())
    }
}

impl fmt::Debug for CopyOnWriteFSPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl PartialEq for CopyOnWriteFSPath {
    fn eq(&self, other: &CopyOnWriteFSPath) -> bool {
        self.is_absolute == other.is_absolute
            && self.fs == other.fs
            && self.names.len() == other.names.len()
            && self.names.iter().zip(&other.names).all(|(a, b)| names_equal(a, b))
    }
}

impl Eq for CopyOnWriteFSPath {}
