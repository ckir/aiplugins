//! The published figure (spec §7), rendered into a marked README region.
//!
//! Generated, never hand-written. `scripts/check-qwen-marketplace.sh` sets the
//! precedent and states the reasoning: "nothing fails when a copy goes stale —
//! the marketplace keeps installing, it just advertises the wrong thing." A
//! stale footprint number is the same failure and gets the same treatment.
//!
//! **What is published here is BYTES, and that is a deliberate, stated
//! compromise.** Fork 6(b) makes an exact token count authoritative, and the
//! exact counter does not exist yet, so every `tokens` field in every document
//! is `None`. §4.3.1 is explicit that the estimator's output is "never
//! committed, never published", so an approximate token figure is not available
//! either — publishing one would hand a reader a number that looks exact and
//! is not. Bytes are what has actually been measured, so bytes are what is
//! published, and the region says so in as many words.
//!
//! Every rendering path here refuses a document whose probe did not succeed.
//! That document carries no tiers, and a tier-less document rendered into a
//! table becomes a `0` — advertising the cheapest plugin in the marketplace
//! because its binary would not start. It is the same false zero the gate
//! refuses, arriving on the one surface a USER reads.

use serde_json::Value;

const BEGIN: &str = "<!-- footprint:begin -->";
const END: &str = "<!-- footprint:end -->";

#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    /// The document has no tiers, so there is no figure to publish.
    #[error("{plugin} was not measured ({status}); refusing to publish a figure for it")]
    NotMeasured { plugin: String, status: String },

    /// §7: "a missing or unpaired marker is a hard error naming the file, never
    /// a silent no-op". A checker that quietly passes on a README where it could
    /// not find its region is exactly the stale-copy failure §7 prevents.
    #[error("{path} has no `{BEGIN}` / `{END}` region; add the marker pair by hand once, then this regenerates between them")]
    NoRegion { path: String },

    #[error("{path} has `{END}` before `{BEGIN}`")]
    InvertedRegion { path: String },

    #[error("{path} has more than one `{BEGIN}` / `{END}` region; which one is the figure?")]
    AmbiguousRegion { path: String },
}

/// The disclosures, carried by BOTH surfaces.
///
/// Duplicated into the comparison table on purpose. A reader comparing plugins
/// is making the install decision §7 exists to inform and is the least likely to
/// have opened the per-plugin page, so caveats that live only there are caveats
/// most of the audience never sees.
const DISCLOSURE: &str = "\
**Bytes, not tokens.** This is the exact serialised size of what the host loads, \
measured by probing the plugin's own MCP server and reading its skill and agent \
frontmatter. It is not a token count and does not convert to one at a fixed \
rate — treat it as a figure you can compare between plugins, not as a context \
budget.

**≥, because hook output is excluded.** Measuring it would mean executing the \
plugin's hooks against a synthetic event, which this tool will not do. A plugin \
whose hooks emit a large preamble costs more than the figure shown.";

/// The per-plugin region: both measured tiers, resident leading.
///
/// An OpenCode document carries a third tier — the `plugin.ts` setup module,
/// loaded in-process with the host on every request — and its region carries
/// a third row for it. A document without one renders exactly the two-row
/// table it always has, so regenerating a Claude Code README changes nothing.
pub fn per_plugin_region(document: &Value) -> Result<String, PublishError> {
    let (resident, invocation, setup) = tiers(document)?;

    let table = match setup {
        Some(setup) => format!(
            "| Tier | Bytes | When you pay it |\n\
             |---|---|---|\n\
             | Resident | ≥ {resident} | Every request, for the whole session |\n\
             | Invocation | ≥ {invocation} | Only when that skill or agent runs |\n\
             | Setup | ≥ {setup} | Every request — `plugin.ts` loads in-process with the host |\n",
            resident = thousands(resident),
            invocation = thousands(invocation),
            setup = thousands(setup),
        ),
        None => format!(
            "| Tier | Bytes | When you pay it |\n\
             |---|---|---|\n\
             | Resident | ≥ {resident} | Every request, for the whole session |\n\
             | Invocation | ≥ {invocation} | Only when that skill or agent runs |\n",
            resident = thousands(resident),
            invocation = thousands(invocation),
        ),
    };

    let setup_lead = match setup {
        Some(setup) => format!(
            " Its `plugin.ts` setup module adds **≥ {} bytes**, loaded in-process with the \
             host on every request and counted separately below so the TypeScript tier stays \
             comparable across releases.",
            thousands(setup)
        ),
        None => String::new(),
    };

    Ok(format!(
        "### Context footprint\n\n\
         Installing this plugin adds **≥ {resident} bytes** to every request, for the \
         whole session. A further **≥ {invocation} bytes** load only when one of its \
         skills or agents is actually invoked.{setup_lead}\n\n\
         {table}\n\
         {DISCLOSURE}\n\n\
         Generated by `tools/plugin-footprint`; regenerate with `just footprint-regen`.",
        resident = thousands(resident),
        invocation = thousands(invocation),
    ))
}

/// The root region: one row per published plugin, for the install decision.
///
/// The table compares the two tiers every plugin shares. An OpenCode plugin's
/// Setup tier (`plugin.ts`, loaded in-process) is NOT a third column: folding
/// it into Resident would conflate the TypeScript tier the per-plugin page
/// reports separately, and a column no Claude Code plugin can fill turns every
/// existing row into `n/a` noise. When any listed document carries one, a note
/// below the table says so and points at the per-plugin pages — a reader
/// comparing install costs must not take the two columns for an OpenCode
/// plugin's total. With no Setup tier anywhere, the note is absent and the
/// rendering is exactly what it always was.
pub fn comparison_region(documents: &[(String, Value)]) -> Result<String, PublishError> {
    let mut rows: Vec<(String, u64, u64, Option<u64>)> = documents
        .iter()
        .map(|(name, document)| {
            let (resident, invocation, setup) = tiers(document)?;
            Ok((name.clone(), resident, invocation, setup))
        })
        .collect::<Result<_, PublishError>>()?;

    // Sorted by name so the table does not inherit the order the marketplace
    // manifest happens to list plugins in, which would churn the committed
    // README for no reason.
    rows.sort_by(|a, b| a.0.cmp(&b.0));

    let mut table = String::from("| Plugin | Resident bytes | Invocation bytes |\n|---|---|---|\n");
    for (name, resident, invocation, _) in &rows {
        table.push_str(&format!(
            "| `{name}` | ≥ {} | ≥ {} |\n",
            thousands(*resident),
            thousands(*invocation)
        ));
    }

    let setup_note = if rows.iter().any(|(_, _, _, setup)| setup.is_some()) {
        "**Setup, on OpenCode plugins only.** An OpenCode plugin also loads its \
         `plugin.ts` setup module in-process with the host on every request — \
         a third tier the table above does not include. Each plugin's own page \
         reports it as Setup alongside these two tiers.\n\n"
    } else {
        ""
    };

    Ok(format!(
        "{table}\n\
         **Resident** is what the host holds in every request, for the whole session — \
         the MCP tool schemas plus the frontmatter it reads to decide what each skill \
         and agent is for. **Invocation** loads only when one of them actually runs.\n\n\
         {setup_note}\
         {DISCLOSURE}\n\n\
         Generated by `tools/plugin-footprint`; regenerate with `just footprint-regen`."
    ))
}

/// Replace whatever sits between the marker pair in `markdown`.
///
/// `path` is carried only so a failure can name the file, which §7 requires.
pub fn splice(path: &str, markdown: &str, region: &str) -> Result<String, PublishError> {
    let named = || path.to_string();

    if markdown.matches(BEGIN).count() > 1 || markdown.matches(END).count() > 1 {
        return Err(PublishError::AmbiguousRegion { path: named() });
    }

    let (Some(begin), Some(end)) = (markdown.find(BEGIN), markdown.find(END)) else {
        return Err(PublishError::NoRegion { path: named() });
    };
    if end < begin {
        return Err(PublishError::InvertedRegion { path: named() });
    }

    Ok(format!(
        "{}{BEGIN}\n{}\n{}",
        &markdown[..begin],
        region.trim(),
        &markdown[end..]
    ))
}

/// The two measured tiers, or a refusal. The third — Setup — is present only
/// on documents measured from an OpenCode plugin directory.
fn tiers(document: &Value) -> Result<(u64, u64, Option<u64>), PublishError> {
    let plugin = document["plugin"]
        .as_str()
        .unwrap_or("<unnamed>")
        .to_string();
    let status = document["probe"]["status"]
        .as_str()
        .unwrap_or("absent")
        .to_string();

    let bytes = |tier: &str| -> Option<u64> { document["tiers"][tier]["bytes"].as_u64() };

    match (bytes("resident"), bytes("invocation")) {
        (Some(resident), Some(invocation)) => Ok((resident, invocation, bytes("setup"))),
        // Absent tiers are what a failed probe produces, and `unwrap_or(0)` here
        // would publish that as a plugin costing nothing.
        _ => Err(PublishError::NotMeasured { plugin, status }),
    }
}

/// `21631` -> `21,631`. Readability only; the documents keep raw integers.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
