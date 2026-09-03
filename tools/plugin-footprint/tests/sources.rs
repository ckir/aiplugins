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

#[test]
fn an_os_artifact_in_a_source_directory_is_skipped_not_fatal() {
    // MEASURED during the capstone review: a `.DS_Store` — which appears the
    // moment a macOS developer opens `skills/` in Finder — took the entire
    // measurement down with "skills/.DS_Store is not where a source belongs".
    // In CI that aborts `footprint-regen` under `set -e`, so the footprint job
    // fails with an error about a skill that does not exist.
    //
    // This is the one silent skip the module tolerates, because an OS artifact
    // is neither a source nor a mistake about a source.
    let fx = Fixture::new("dsstore");
    fx.write("skills/real/SKILL.md", SKILL);
    fx.write("skills/.DS_Store", "\u{0}\u{0}\u{0}Bud1");
    fx.write("agents/.DS_Store", "\u{0}\u{0}\u{0}Bud1");
    fx.write("skills/Thumbs.db", "junk");

    let sources = read_file_sources(fx.path()).expect("an OS artifact must not be fatal");

    assert_eq!(sources.resident.len(), 1);
    assert_eq!(sources.resident[0].id, "real");
}

#[test]
fn a_flat_source_that_is_not_a_md_file_is_an_error_not_a_silent_skip() {
    // MEASURED: `agents/re-analyst.md.bak` and `agents/nested/agent.md` were
    // both dropped from the measurement with nothing failing anywhere. A skill
    // in the wrong shape is already loud (see above); an agent in the wrong
    // shape was not, and the asymmetry is the false zero this crate refuses,
    // arriving through the one directory nobody thought about.
    let fx = Fixture::new("flatbak");
    fx.write("agents/re-analyst.md.bak", SKILL);

    let err = read_file_sources(fx.path()).expect_err("a stray file must be loud");

    assert!(
        err.to_string().contains("agents/<name>.md"),
        "the error must name the shape a source belongs in, got: {err}"
    );
}

#[test]
fn a_directory_where_a_flat_source_belongs_is_an_error_too() {
    // `agents/reviewer/agent.md` — an agent someone structured like a skill.
    // Skipping it drops a real source; the whole point is that this is loud.
    let fx = Fixture::new("flatdir");
    fx.write("agents/reviewer/agent.md", SKILL);

    let err = read_file_sources(fx.path()).expect_err("a nested agent must be loud");

    assert!(err.to_string().contains("agents/<name>.md"), "got: {err}");
}

#[test]
fn the_error_names_the_layout_that_was_violated_not_always_skills() {
    // Every Malformed used to report `skills/<name>/SKILL.md` whatever the
    // directory, because only the nested layout could produce one. Now that a
    // flat layout can too, an error naming the wrong directory would send a
    // contributor to the wrong file.
    let fx = Fixture::new("whichlayout");
    fx.write("commands/stray.txt", "x");

    let err = read_file_sources(fx.path()).expect_err("a stray file must be loud");

    assert!(err.to_string().contains("commands/<name>.md"), "got: {err}");
    assert!(!err.to_string().contains("SKILL.md"), "got: {err}");
}

#[test]
fn a_dot_named_directory_cannot_hide_a_source_from_the_measurement() {
    // The edge the round-1 `.DS_Store` fix created. Skipping every dot-named
    // entry skipped dot-named DIRECTORIES too, so `skills/.sneaky/SKILL.md`
    // would have vanished from the footprint with nothing failing — a way to
    // hide cost, introduced by a fix for a way to fail loudly.
    //
    // OS artifacts are FILES. A dot-named directory goes through the normal
    // check: it is either a skill, or it is malformed, and both are loud. The
    // trade is that a stray `.vscode/` inside `skills/` now hard-fails, which
    // costs seconds to fix and announces itself, versus silently unmeasured
    // bytes which do neither.
    let fx = Fixture::new("dotdir");
    fx.write("skills/.sneaky/SKILL.md", SKILL);

    let sources = read_file_sources(fx.path()).expect("a dot-named skill is still a skill");

    assert_eq!(
        sources.resident.len(),
        1,
        "a dot-named directory must not hide a source: {:?}",
        sources.resident
    );
    assert_eq!(sources.resident[0].id, ".sneaky");
}

#[test]
#[cfg(unix)]
fn a_filename_that_is_not_utf8_is_refused_rather_than_lossily_collided() {
    // `to_string_lossy` maps EVERY invalid byte sequence onto U+FFFD, so two
    // different filenames can arrive as one id. Two sources then share an id and
    // their relative order comes from `read_dir` rather than from the data,
    // churning the committed document between machines — and the whole reason
    // the sort key became (kind, id) was to stop exactly that.
    //
    // `document.rs::strip_root` already refuses to compare lossily for this same
    // reason. Unix-only because Windows filenames cannot hold these bytes.
    use std::os::unix::ffi::OsStrExt;

    let fx = Fixture::new("notutf8");
    std::fs::create_dir_all(fx.path().join("agents")).expect("mkdir");
    let bad = std::ffi::OsStr::from_bytes(b"\xff\xfe.md");
    std::fs::write(fx.path().join("agents").join(bad), SKILL).expect("write");

    let err = read_file_sources(fx.path()).expect_err("a lossy name must be loud");
    assert!(
        err.to_string().contains("UTF-8"),
        "the error must say why, got: {err}"
    );
}

#[test]
fn a_crlf_source_measures_the_same_as_an_lf_one() {
    // THE defect that nearly shipped, found while about to merge. Git stores
    // these files with LF and hands a Windows checkout CRLF, so the SAME commit
    // measures differently on a Windows developer's machine and on Linux CI:
    // `agents/re-analyst.md` was 8790 bytes on disk and 8651 in git, one byte
    // per line apart.
    //
    // The committed documents are generated on one machine and re-measured on
    // the other by §6's freshness layer, which is "regenerate, then require no
    // diff". A platform-dependent byte count makes that layer fail on every run
    // for a reason that has nothing to do with a footprint — and it is exactly
    // what removing the timestamp was meant to guarantee could not happen.
    //
    // Counting the CONTENT rather than the checkout is also the more honest
    // measurement: a user installs a bundle assembled on Linux, so LF is what
    // they actually pay for.
    let lf = Fixture::new("lf");
    lf.write(
        "skills/s/SKILL.md",
        "---\nname: s\ndesc: d\n---\n\nbody\nmore\n",
    );

    let crlf = Fixture::new("crlf");
    crlf.write(
        "skills/s/SKILL.md",
        "---\r\nname: s\r\ndesc: d\r\n---\r\n\r\nbody\r\nmore\r\n",
    );

    let a = read_file_sources(lf.path()).expect("lf reads");
    let b = read_file_sources(crlf.path()).expect("crlf reads");

    assert_eq!(
        a.resident[0].bytes, b.resident[0].bytes,
        "frontmatter must not depend on the checkout's line endings"
    );
    assert_eq!(
        a.invocation[0].bytes, b.invocation[0].bytes,
        "body must not depend on the checkout's line endings"
    );
}
