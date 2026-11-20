use std::path::Path;
use std::path::PathBuf;

use brainstone_skills::Catalog;

#[test]
fn discovers_valid_skills_and_skips_invalid_ones_with_diagnostics() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog");

    let catalog = Catalog::load(&root);

    let valid = catalog.get("report-filing").expect("valid skill");
    assert_eq!(valid.name(), "report-filing");
    assert_eq!(
        valid.description(),
        "Files finished reports into long-term storage. Use when a request involves filing a report."
    );
    assert!(catalog.get("missing-description").is_none());
    assert_eq!(catalog.diagnostics().len(), 1);
    assert_eq!(catalog.diagnostics()[0].code(), "invalid_skill_frontmatter");
}

#[test]
fn a_missing_root_is_an_empty_catalog() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/absent");

    let catalog = Catalog::load(&root);

    assert!(catalog.get("anything").is_none());
    assert_eq!(catalog.diagnostics(), []);
}

#[test]
fn a_directory_name_mismatch_warns_but_remains_compatible() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mismatch");

    let catalog = Catalog::load(&root);

    assert!(catalog.get("actual-name").is_some());
    assert_eq!(catalog.diagnostics().len(), 1);
    assert_eq!(catalog.diagnostics()[0].code(), "skill_name_mismatch");
}

#[test]
fn repairs_unquoted_colons_used_by_other_clients() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/colon");

    let catalog = Catalog::load(&root);

    let skill = catalog.get("colon-skill").expect("compatible skill");
    assert_eq!(
        skill.description(),
        "Use this skill when: filing is requested"
    );
    assert_eq!(catalog.diagnostics(), []);
}

#[test]
fn missing_name_uses_the_directory_and_records_a_warning() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/missing_name");

    let catalog = Catalog::load(&root);

    assert!(catalog.get("directory-name").is_some());
    assert_eq!(catalog.diagnostics().len(), 1);
    assert_eq!(catalog.diagnostics()[0].code(), "skill_name_defaulted");
}

#[test]
fn a_root_scan_stops_at_its_directory_limit() {
    let root = TempRoot::new();
    for index in 0..=2000 {
        std::fs::create_dir(root.0.join(format!("skill-{index}"))).unwrap();
    }

    let catalog = Catalog::load(&root.0);

    assert!(catalog.get("anything").is_none());
    assert_eq!(catalog.diagnostics().len(), 1);
    assert_eq!(catalog.diagnostics()[0].code(), "skill_scan_limit");
}

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let unique = format!("brainstone-skills-{}", std::process::id());
        let path = std::env::temp_dir().join(unique);
        drop(std::fs::remove_dir_all(&path));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.0));
    }
}
