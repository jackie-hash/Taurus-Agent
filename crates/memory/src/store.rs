//! File-based memory storage rooted at `~/.taurus/memory/`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::types::{MemoryEntry, MemoryFrontmatter, MemoryType};

const DEFAULT_MAX_TOTAL_SIZE: usize = 128 * 1024;

/// Filesystem-backed memory store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryStore {
    root: PathBuf,
}

impl MemoryStore {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Default store location: `~/.taurus/memory/`.
    #[must_use]
    pub fn default_store() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        Self::new(home.join(".taurus").join("memory"))
    }

    pub fn init(&self) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        fs::create_dir_all(self.root.join("project"))?;
        fs::create_dir_all(self.root.join("reference"))?;
        let index_path = self.root.join("MEMORY.md");
        if !index_path.exists() {
            fs::write(&index_path, "# 记忆系统\n\n本文件为系统入口。\n")?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------
    // Path helpers
    // -----------------------------------------------------------------

    #[must_use]
    pub fn path_for(&self, memory_type: MemoryType, name: &str, project_slug: Option<&str>) -> PathBuf {
        let filename = format!("{name}.md");
        match memory_type {
            MemoryType::User | MemoryType::Feedback => self.root.join(&filename),
            MemoryType::Project => {
                let slug = project_slug.unwrap_or("default");
                self.root.join("project").join(slug).join(&filename)
            }
            MemoryType::Reference => self.root.join("reference").join(&filename),
        }
    }

    #[must_use]
    pub fn project_dir(&self, slug: &str) -> PathBuf {
        self.root.join("project").join(slug)
    }

    #[must_use]
    pub fn project_index_path(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("MEMORY.md")
    }

    // -----------------------------------------------------------------
    // Read operations
    // -----------------------------------------------------------------

    pub fn read_entry(&self, path: &Path) -> io::Result<Option<MemoryEntry>> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let (frontmatter, body) = parse_frontmatter(trimmed)?;
        Ok(Some(MemoryEntry {
            frontmatter,
            body: body.to_string(),
            file_path: path.to_path_buf(),
        }))
    }

    pub fn read_named(
        &self,
        memory_type: MemoryType,
        name: &str,
        project_slug: Option<&str>,
    ) -> io::Result<Option<MemoryEntry>> {
        let path = self.path_for(memory_type, name, project_slug);
        self.read_entry(&path)
    }

    pub fn list_global_entries(&self) -> io::Result<Vec<PathBuf>> {
        let mut entries = Vec::new();
        self.collect_md_files(&self.root, &mut entries, false)?;
        let ref_dir = self.root.join("reference");
        if ref_dir.is_dir() {
            self.collect_md_files(&ref_dir, &mut entries, false)?;
        }
        Ok(entries)
    }

    pub fn list_project_entries(&self, slug: &str) -> io::Result<Vec<PathBuf>> {
        let dir = self.project_dir(slug);
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        self.collect_md_files(&dir, &mut entries, true)?;
        Ok(entries)
    }

    pub fn list_project_slugs(&self) -> io::Result<Vec<String>> {
        let project_root = self.root.join("project");
        if !project_root.is_dir() {
            return Ok(Vec::new());
        }
        let mut slugs = Vec::new();
        for entry in fs::read_dir(&project_root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    slugs.push(name.to_string());
                }
            }
        }
        slugs.sort();
        Ok(slugs)
    }

    fn collect_md_files(&self, dir: &Path, out: &mut Vec<PathBuf>, skip_index: bool) -> io::Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "md") {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if skip_index && name == "MEMORY.md" {
                    continue;
                }
                out.push(path);
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------
    // Write operations
    // -----------------------------------------------------------------

    pub fn write_entry(
        &self,
        memory_type: MemoryType,
        name: &str,
        description: &str,
        body: &str,
        project_slug: Option<&str>,
    ) -> io::Result<PathBuf> {
        let path = self.path_for(memory_type, name, project_slug);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let type_str = memory_type.to_string();
        let content = format!(
            "---\nname: {name}\ndescription: {description}\ntype: {type_str}\n---\n\n{body}\n"
        );
        fs::write(&path, &content)?;
        Ok(path)
    }

    // -----------------------------------------------------------------
    // System prompt composition
    // -----------------------------------------------------------------

    #[must_use]
    pub fn compose_all_blocks(&self, max_size: Option<usize>) -> String {
        let max = max_size.unwrap_or(DEFAULT_MAX_TOTAL_SIZE);
        let mut blocks = Vec::new();
        let mut total = 0usize;

        let global = self.list_global_entries().unwrap_or_default();
        let mut user_blocks = Vec::new();
        let mut feedback_blocks = Vec::new();
        let mut reference_blocks = Vec::new();

        for path in &global {
            if let Ok(Some(entry)) = self.read_entry(path) {
                let block = format!(
                    "<!-- name: {} | type: {} -->\n{}",
                    entry.frontmatter.name, entry.frontmatter.memory_type, entry.body
                );
                if total + block.len() > max {
                    blocks.push("…(memory truncated)".to_string());
                    break;
                }
                total += block.len();
                match entry.frontmatter.memory_type {
                    MemoryType::User => user_blocks.push(block),
                    MemoryType::Feedback => feedback_blocks.push(block),
                    MemoryType::Reference => reference_blocks.push(block),
                    MemoryType::Project => {}
                }
            }
        }

        if !user_blocks.is_empty() {
            blocks.push(format!(
                "<user_memory>\n{}\n</user_memory>",
                user_blocks.join("\n\n")
            ));
        }
        if !feedback_blocks.is_empty() {
            blocks.push(format!(
                "<feedback_memory>\n{}\n</feedback_memory>",
                feedback_blocks.join("\n\n")
            ));
        }
        if !reference_blocks.is_empty() {
            blocks.push(format!(
                "<reference_memory>\n{}\n</reference_memory>",
                reference_blocks.join("\n\n")
            ));
        }

        blocks.join("\n\n")
    }

    #[must_use]
    pub fn compose_project_blocks(&self, slug: &str, max_size: Option<usize>) -> Option<String> {
        let max = max_size.unwrap_or(DEFAULT_MAX_TOTAL_SIZE);
        let entries = self.list_project_entries(slug).ok()?;
        if entries.is_empty() {
            return None;
        }
        let mut blocks = Vec::new();
        let mut total = 0usize;
        for path in &entries {
            if let Ok(Some(entry)) = self.read_entry(path) {
                let block = format!(
                    "<!-- name: {} | type: {} -->\n{}",
                    entry.frontmatter.name, entry.frontmatter.memory_type, entry.body
                );
                if total + block.len() > max {
                    blocks.push("…(project memory truncated)".to_string());
                    break;
                }
                total += block.len();
                blocks.push(block);
            }
        }
        if blocks.is_empty() {
            return None;
        }
        Some(format!(
            "<project_memory slug=\"{slug}\">\n{}\n</project_memory>",
            blocks.join("\n\n")
        ))
    }

    // -----------------------------------------------------------------
    // Index operations
    // -----------------------------------------------------------------

    pub fn read_global_index(&self) -> io::Result<String> {
        let path = self.root.join("MEMORY.md");
        match fs::read_to_string(&path) {
            Ok(c) => Ok(c),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(e),
        }
    }

    pub fn write_global_index(&self, content: &str) -> io::Result<()> {
        let path = self.root.join("MEMORY.md");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)
    }
}

// ---------------------------------------------------------------------------
// Frontmatter parsing
// ---------------------------------------------------------------------------

fn parse_frontmatter(content: &str) -> io::Result<(MemoryFrontmatter, String)> {
    let mut lines = content.lines();
    let first = lines.next().unwrap_or("").trim().to_string();
    if first != "---" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "memory file missing frontmatter opening delimiter `---`",
        ));
    }

    let mut name = String::new();
    let mut description = String::new();
    let mut type_str = String::new();

    for line in &mut lines {
        let line = line.trim();
        if line == "---" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim();
            match key.trim() {
                "name" => name = value.to_string(),
                "description" => description = value.to_string(),
                "type" => type_str = value.to_string(),
                _ => {}
            }
        }
    }

    let memory_type = MemoryType::parse(&type_str).unwrap_or(MemoryType::User);
    let body = lines.collect::<Vec<_>>().join("\n").trim().to_string();

    Ok((MemoryFrontmatter { name, description, memory_type }, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (MemoryStore, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(tmp.path().to_path_buf());
        store.init().unwrap();
        (store, tmp)
    }

    #[test]
    fn test_parse_frontmatter() {
        let content = "---\nname: test_memory\ndescription: A test memory\ntype: user\n---\n\nBody text here.";
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, "test_memory");
        assert_eq!(fm.description, "A test memory");
        assert_eq!(fm.memory_type, MemoryType::User);
        assert_eq!(body, "Body text here.");
    }

    #[test]
    fn test_write_and_read_entry() {
        let (store, _tmp) = temp_store();
        let path = store
            .write_entry(MemoryType::User, "test", "A test", "Hello world.", None)
            .unwrap();
        let entry = store.read_entry(&path).unwrap().unwrap();
        assert_eq!(entry.frontmatter.name, "test");
        assert_eq!(entry.body, "Hello world.");
    }

    #[test]
    fn test_list_global_entries() {
        let (store, _tmp) = temp_store();
        store.write_entry(MemoryType::User, "u1", "desc", "body", None).unwrap();
        store.write_entry(MemoryType::Feedback, "f1", "desc", "body", None).unwrap();
        store.write_entry(MemoryType::Reference, "r1", "desc", "body", None).unwrap();
        let entries = store.list_global_entries().unwrap();
        assert!(entries.len() >= 3, "got {} entries", entries.len());
    }

    #[test]
    fn test_project_slugs() {
        let (store, _tmp) = temp_store();
        store.write_entry(MemoryType::Project, "m1", "desc", "body", Some("repo-a")).unwrap();
        store.write_entry(MemoryType::Project, "m2", "desc", "body", Some("repo-b")).unwrap();
        let slugs = store.list_project_slugs().unwrap();
        assert!(slugs.contains(&"repo-a".to_string()));
        assert!(slugs.contains(&"repo-b".to_string()));
    }
}
