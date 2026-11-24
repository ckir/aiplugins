# GitHub: action pins and repository settings

## Resolving an action's commit SHA

The workflow pins each action as `owner/repo@<40-hex sha> # vX.Y.Z`, because a tag can be moved to
different code. Resolve from the release tag; do not copy a SHA from memory or from the template:

```sh
gh api repos/actions/checkout/git/ref/tags/v7.0.1 -q '.object | "\(.type) \(.sha)"'
# if the type is "tag" (an annotated tag), dereference it to the commit:
gh api repos/actions/checkout/git/tags/<sha-from-above> -q .object.sha
```

Do the same for `taiki-e/install-action`. Use the latest release of each
(`gh api repos/<owner>/<repo>/releases/latest -q .tag_name`) and keep the version comment in step
with the SHA.

`taiki-e/install-action` installs `just`, `cargo-nextest`, `cargo-deny` and `typos` at the versions
the workflow names. It does not list `gitleaks` or `lefthook`, so the workflow downloads each one by
version from its GitHub release and verifies it against the release's checksums file
(`gitleaks_<version>_checksums.txt`, `lefthook_checksums.txt`). The toolchain comes from
`rust-toolchain.toml` through `rustup toolchain install`.

## Settings to offer (the user switches them on)

These are outward-facing account settings: describe them, get a yes, then apply.

1. **Ruleset on the default branch** requiring a pull request and the `check` status check. Without
   it CI is advisory.
2. **Require review from Code Owners.** CODEOWNERS does nothing until this is on. Say this first: an
   author cannot approve their own pull request, so a repository with a single owner will need the
   admin bypass for the owner's own pull requests that touch owned paths. The rule only bites for
   other contributors. Alternatives: a second owner, or leaving it off.
3. **CodeQL default setup** (`gh api -X PATCH repos/<o>/<r>/code-scanning/default-setup -f state=configured`).
   Expect the first scan to raise alerts in existing code; check
   `gh api 'repos/<o>/<r>/code-scanning/alerts?state=open'` afterwards.

Before changing a ruleset through the API, save the current one (`gh api repos/<o>/<r>/rulesets/<id>`),
change only the field asked for, and `PUT` the whole object back, bypass actors included.

## When a merge is refused

`gh pr view <n> --json mergeStateStatus` saying `BLOCKED` with green checks usually means a rule that
cannot be met, not a failing one. Read the rules (`gh api repos/<o>/<r>/rules/branches/<branch>`) and
look for:

- a **code scanning** rule while code scanning is not configured: it waits forever for results that
  never come;
- **"branch must be up to date"** while the base has commits the branch lacks;
- **Code Owner review** with nobody eligible to give it.

Report which rule blocks and let the user choose between fixing the cause and `gh pr merge --admin`.
Bypassing branch protection is their decision, each time.

## Smaller traps

- A pull request that was merged while work continued: check its state before pushing to the same
  branch, and ask before opening a follow-up pull request.
- Before overwriting a pull request description, fetch the current body with
  `gh api repos/<o>/<r>/pulls/<n> -q .body` into a file and confirm the file is not empty.
- CodeQL flags a token generator that takes a random byte modulo the alphabet length as biased
  randomness, rated high, even in tests. The template's `fake_token()` keeps six bits of each byte and
  discards the two values past the end of the alphabet.
