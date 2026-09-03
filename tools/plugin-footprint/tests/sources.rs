//! Reading a plugin's file-backed resident and invocation sources (spec §4.5).

use plugin_footprint::sources::read_file_sources;
use std::path::{Path, PathBuf};

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "plugin-footprint-sources-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("create fixture");
        Self { dir }
    }

    fn write(&self, relative: &str, contents: &str) -> &Self {
        let path = self.dir.join(relative);
        std::fs::create_dir_all(path.parent().expect("has a parent")).expect("create dirs");
        std::fs::write(path, contents).expect("write file");
        self
    }

    fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

const SKILL: &str = "---\nname: doctor\ndescription: d\n---\n\nbody bytes\n";

#[test]
fn a_skill_contributes_frontmatter_to_resident_and_its_body_to_invocation() {
    let fx = Fixture::new("skill");
    fx.write("skills/doctor/SKILL.md", SKILL);

    let sources = read_file_sources(fx.path()).expect("reads");

    let resident: Vec<_> = sources
        .resident
        .iter()
        .map(|s| (s.kind, s.id.as_str()))
        .collect();
    assert_eq!(resident, vec![("skill_frontmatter", "doctor")]);

    let invocation: Vec<_> = sources
        .invocation
        .iter()
        .map(|s| (s.kind, s.id.as_str()))
        .collect();
    assert_eq!(invocation, vec![("skill_body", "doctor")]);

    // The two halves together are the whole file, and neither includes the
    // `---` delimiter lines.
    let total: u64 = sources.resident[0].bytes + sources.invocation[0].bytes;
    assert_eq!(total as usize, SKILL.len() - "---\n---\n".len());
}

#[test]
fn an_agent_contributes_both_halves_too() {
    let fx = Fixture::new("agent");
    fx.write("agents/re-analyst.md", "---\nname: re-analyst\n---\nbody\n");

    let sources = read_file_sources(fx.path()).expect("reads");

    assert_eq!(sources.resident[0].kind, "agent_frontmatter");
    assert_eq!(sources.resident[0].id, "re-analyst");
    assert_eq!(sources.invocation[0].kind, "agent_body");
}

#[test]
fn a_plugin_with_no_skills_or_agents_has_no_file_sources() {
    // Legitimate: an MCP-only plugin. Not an error, and not a failure to read.
    let fx = Fixture::new("bare");

    let sources = read_file_sources(fx.path()).expect("reads");

    assert!(sources.resident.is_empty());
    assert!(sources.invocation.is_empty());
}

#[test]
fn readme_and_examples_are_not_measured() {
    // The host never loads them; they are documentation for whoever reads the
    // repository. Counting them would overstate what a user actually pays.
    let fx = Fixture::new("docs");
    fx.write("README.md", "# readme\n");
    fx.write("examples/x.local.md", "---\nname: x\n---\nbody\n");

    let sources = read_file_sources(fx.path()).expect("reads");

    assert!(sources.resident.is_empty(), "got {:?}", sources.resident);
    assert!(sources.invocation.is_empty());
}

#[test]
fn a_skill_without_frontmatter_is_all_invocation_and_no_resident() {
    // It contributes no discovery text, so it costs nothing on every request —
    // but its body is still loaded when the skill runs.
    let fx = Fixture::new("nofront");
    fx.write("skills/plain/SKILL.md", "# no frontmatter\n");

    let sources = read_file_sources(fx.path()).expect("reads");

    assert!(sources.resident.is_empty());
    assert_eq!(sources.invocation.len(), 1);
    assert_eq!(
        sources.invocation[0].bytes,
        "# no frontmatter\n".len() as u64
    );
}

#[test]
fn a_mis_structured_skill_is_an_error_not_a_silent_skip() {
    // `skills/doctor.md` instead of `skills/doctor/SKILL.md`. Appending
    // SKILL.md to it yields a path that does not exist, and skipping on that
    // basis drops a real skill's bytes out of the footprint with nothing
    // failing. A measurement that quietly omits a source is the same false zero
    // the probe layers refuse.
    let fx = Fixture::new("misplaced");
    fx.write("skills/doctor.md", SKILL);

    let err = read_file_sources(fx.path()).expect_err("a mis-structured skill must be loud");

    assert!(
        err.to_string().contains("SKILL.md"),
        "the error must say where the file belongs, got: {err}"
    );
}

#[test]
fn a_skill_directory_with_no_skill_file_is_an_error_too() {
    let fx = Fixture::new("emptyskill");
    fx.write("skills/doctor/notes.md", "# notes\n");

    let err = read_file_sources(fx.path()).expect_err("an empty skill directory must be loud");

    assert!(err.to_string().contains("SKILL.md"), "got: {err}");
}

#[test]
fn sources_come_back_sorted_by_id() {
    // The document must not inherit directory iteration order, which differs
    // between filesystems and would churn a committed copy for no reason.
    let fx = Fixture::new("sorted");
    fx.write("skills/zebra/SKILL.md", SKILL);
    fx.write("skills/alpha/SKILL.md", SKILL);

    let sources = read_file_sources(fx.path()).expect("reads");

    let ids: Vec<&str> = sources.resident.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["alpha", "zebra"]);
}

#[test]
fn bytes_are_utf8_length_not_character_count() {
    let fx = Fixture::new("utf8");
    // 'é' is one character, two UTF-8 bytes.
    fx.write("skills/s/SKILL.md", "---\nname: é\n---\n");

    let sources = read_file_sources(fx.path()).expect("reads");

    assert_eq!(sources.resident[0].bytes, "name: é\n".len() as u64);
}
