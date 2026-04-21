use std::path::Path;
use globset::{Glob, GlobSet, GlobSetBuilder};

pub enum ExcludeRule {
    FilePattern(GlobSet),
    DirPattern(GlobSet),
    NoExt,
    General(GlobSet),
}

pub struct ExcludeFilter {
    rules: Vec<ExcludeRule>,
}

impl ExcludeFilter {
    pub fn from_patterns(patterns: &[String]) -> anyhow::Result<Self> {
        let mut rules = Vec::new();

        let mut file_builder = GlobSetBuilder::new();
        let mut dir_builder = GlobSetBuilder::new();
        let mut general_builder = GlobSetBuilder::new();
        let mut has_file = false;
        let mut has_dir = false;
        let mut has_general = false;
        let mut has_no_ext = false;

        for p in patterns {
            if p == "!no-ext" {
                has_no_ext = true;
            } else if let Some(pat) = p.strip_prefix("file:") {
                file_builder.add(Glob::new(pat)?);
                has_file = true;
            } else if let Some(pat) = p.strip_prefix("dir:") {
                dir_builder.add(Glob::new(pat)?);
                has_dir = true;
            } else {
                general_builder.add(Glob::new(p)?);
                has_general = true;
            }
        }

        if has_file {
            rules.push(ExcludeRule::FilePattern(file_builder.build()?));
        }
        if has_dir {
            rules.push(ExcludeRule::DirPattern(dir_builder.build()?));
        }
        if has_no_ext {
            rules.push(ExcludeRule::NoExt);
        }
        if has_general {
            rules.push(ExcludeRule::General(general_builder.build()?));
        }

        Ok(Self { rules })
    }

    pub fn should_skip_dir(&self, path: &Path) -> bool {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let path_str = path.to_string_lossy();
        for rule in &self.rules {
            match rule {
                ExcludeRule::DirPattern(gs) => {
                    if gs.is_match(name) || gs.is_match(path_str.as_ref()) {
                        return true;
                    }
                }
                ExcludeRule::General(gs) => {
                    if gs.is_match(name) || gs.is_match(path_str.as_ref()) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    pub fn should_skip_file(&self, path: &Path) -> bool {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let path_str = path.to_string_lossy();
        for rule in &self.rules {
            match rule {
                ExcludeRule::FilePattern(gs) => {
                    if gs.is_match(name) || gs.is_match(path_str.as_ref()) {
                        return true;
                    }
                }
                ExcludeRule::NoExt => {
                    if path.extension().is_none() {
                        return true;
                    }
                }
                ExcludeRule::General(gs) => {
                    if gs.is_match(name) || gs.is_match(path_str.as_ref()) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_file_prefix() {
        let f = ExcludeFilter::from_patterns(&["file:foo.txt".to_string()]).unwrap();
        assert!(f.should_skip_file(&PathBuf::from("/some/dir/foo.txt")));
        assert!(!f.should_skip_file(&PathBuf::from("/some/dir/bar.txt")));
        assert!(!f.should_skip_dir(&PathBuf::from("/some/dir/foo.txt")));
    }

    #[test]
    fn test_dir_prefix() {
        let f = ExcludeFilter::from_patterns(&["dir:node_modules".to_string()]).unwrap();
        assert!(f.should_skip_dir(&PathBuf::from("/project/node_modules")));
        assert!(!f.should_skip_dir(&PathBuf::from("/project/src")));
        assert!(!f.should_skip_file(&PathBuf::from("/project/node_modules/index.js")));
    }

    #[test]
    fn test_no_ext() {
        let f = ExcludeFilter::from_patterns(&["!no-ext".to_string()]).unwrap();
        assert!(f.should_skip_file(&PathBuf::from("/some/Makefile")));
        assert!(!f.should_skip_file(&PathBuf::from("/some/file.txt")));
    }

    #[test]
    fn test_general() {
        let f = ExcludeFilter::from_patterns(&["*.tmp".to_string()]).unwrap();
        assert!(f.should_skip_file(&PathBuf::from("/some/file.tmp")));
        assert!(f.should_skip_dir(&PathBuf::from("/some/cache.tmp")));
        assert!(!f.should_skip_file(&PathBuf::from("/some/file.txt")));
    }
}
