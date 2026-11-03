#!/usr/bin/env bash
# Re-run a CI run once when GitHub never gave its jobs a runner.
#
#   GH_TOKEN=... GITHUB_REPOSITORY=owner/repo bash scripts/rerun-runner-failures.sh RUN_ID
#   bash scripts/rerun-runner-failures.sh --selftest
#
# Called by .github/workflows/ci-rerun-runner-failures.yml for a finished CI
# run (attempt 1 only). GitHub's macOS pool sometimes cancels a job with the
# annotation "The job was not acquired by Runner of type hosted even after
# multiple attempts" (measured on run 41495365746); that is capacity, not a
# failing change, and left alone it blocks the pull request until a human
# re-runs it.
#
# The rule that keeps this from hiding real failures: re-run ONLY when EVERY
# failed or cancelled job in the run carries that annotation. One real red job
# anywhere in the run means nothing is re-run, so a flaky test cannot be turned
# green by this script.
set -euo pipefail

# The annotation text GitHub writes when a job never got a runner.
MARKER='not acquired by Runner'

# decide RUN_ID: print "rerun" and re-run the failed jobs, or print "leave" why.
decide() {
    local run=$1 repo=${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is not set} ids id msgs attempt total=0 acq=0
    # The workflow only fires for attempt 1, but the jobs endpoint below answers
    # for the LATEST attempt. If anyone re-ran the run between the event and now,
    # that is not the attempt that triggered us, and re-running again would be a
    # second retry. Re-read the attempt and leave a run that has moved on.
    attempt=$(gh api "repos/$repo/actions/runs/$run" | jq -r '.run_attempt')
    if [ "$attempt" != "1" ]; then
        echo "leave: run $run is already on attempt $attempt; only attempt 1 is retried"
        return 0
    fi
    ids=$(gh api "repos/$repo/actions/runs/$run/jobs?per_page=100" |
        jq -r '.jobs[] | select(.conclusion == "failure" or .conclusion == "cancelled") | .id')
    for id in $ids; do
        total=$((total + 1))
        msgs=$(gh api "repos/$repo/check-runs/$id/annotations?per_page=100" | jq -r '.[].message')
        if printf '%s\n' "$msgs" | grep -qF "$MARKER"; then acq=$((acq + 1)); fi
    done
    if [ "$total" -eq 0 ]; then
        echo "leave: run $run has no failed or cancelled job"
    elif [ "$acq" -ne "$total" ]; then
        echo "leave: $((total - acq)) of $total failed jobs in run $run are not runner-acquisition failures"
    else
        echo "rerun: all $total failed jobs in run $run never got a runner"
        gh run rerun "$run" --failed --repo "$repo"
    fi
}

selftest() {
    local me
    # (global: the EXIT trap runs after this function's locals are gone)
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    me=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)/$(basename "${BASH_SOURCE[0]}")

    check() { # check NAME CONDITION-RESULT(0 = pass) DETAIL
        if [ "$2" -eq 0 ]; then echo "ok $1"; else echo "FAIL $1: $3" >&2; exit 1; fi
    }

    # A stub `gh` that serves canned JSON for the two read endpoints and records
    # every `run rerun` call. Scenario files in $tmp/case: run.json, jobs.json,
    # annotations-<id>.json (a missing one makes the stub fail like a 500).
    mkdir -p "$tmp/bin" "$tmp/case"
    cat > "$tmp/bin/gh" <<'STUB'
#!/usr/bin/env bash
case "$*" in
    "api repos/o/r/actions/runs/7") cat "$CASE/run.json" || exit 1 ;;
    "api repos/o/r/actions/runs/7/jobs"*) cat "$CASE/jobs.json" ;;
    "api repos/o/r/check-runs/"*"/annotations"*)
        id=${2#repos/o/r/check-runs/}; id=${id%%/*}
        cat "$CASE/annotations-$id.json" || exit 1 ;;
    "run rerun "*) echo "$*" >> "$CASE/reruns" ;;
    *) echo "stub gh: unexpected call: $*" >&2; exit 2 ;;
esac
STUB
    chmod +x "$tmp/bin/gh"

    # scenario NAME EXPECT(rerun|leave) JOBS-JSON [ID=MESSAGE ...]
    scenario() {
        local name=$1 expect=$2 jobs=$3 out rc=0 pair ok
        shift 3
        rm -rf "${tmp:?}/case"; mkdir -p "$tmp/case"
        printf '%s' "$jobs" > "$tmp/case/jobs.json"
        printf '{"run_attempt":%s}' "${ATTEMPT:-1}" > "$tmp/case/run.json"
        for pair in "$@"; do
            printf '[{"message":"%s"}]' "${pair#*=}" > "$tmp/case/annotations-${pair%%=*}.json"
        done
        out=$(PATH="$tmp/bin:$PATH" CASE="$tmp/case" GITHUB_REPOSITORY=o/r bash "$me" 7 2>&1) || rc=$?
        ok=1
        case $expect in
            rerun) if [ "$rc" -eq 0 ] && [ "$(cat "$tmp/case/reruns" 2>/dev/null)" = "run rerun 7 --failed --repo o/r" ]; then ok=0; fi ;;
            leave) if [ "$rc" -eq 0 ] && [ ! -f "$tmp/case/reruns" ]; then ok=0; fi ;;
            error) if [ "$rc" -ne 0 ] && [ ! -f "$tmp/case/reruns" ]; then ok=0; fi ;;
        esac
        check "$name" "$ok" "expected $expect; got rc=$rc reruns=$(cat "$tmp/case/reruns" 2>/dev/null) out=$out"
    }

    local acq='The job was not acquired by Runner of type hosted even after multiple attempts'
    local real='Process completed with exit code 1.'
    local cancelled='{"jobs":[{"id":101,"conclusion":"cancelled"},{"id":102,"conclusion":"success"}]}'

    scenario "a cancelled job that never got a runner is re-run" rerun "$cancelled" "101=$acq"
    scenario "a real failure is left for a human" leave '{"jobs":[{"id":101,"conclusion":"failure"}]}' "101=$real"
    scenario "one real failure beside an acquisition failure re-runs nothing" leave \
        '{"jobs":[{"id":101,"conclusion":"cancelled"},{"id":103,"conclusion":"failure"}]}' "101=$acq" "103=$real"
    scenario "two acquisition failures are re-run together" rerun \
        '{"jobs":[{"id":101,"conclusion":"cancelled"},{"id":103,"conclusion":"cancelled"}]}' "101=$acq" "103=$acq"
    scenario "a run with nothing failed is left alone" leave '{"jobs":[{"id":101,"conclusion":"success"}]}'
    ATTEMPT=2 scenario "a run already re-run by someone else is not retried a second time" leave "$cancelled" "101=$acq"
    scenario "an unreadable annotation list re-runs nothing and fails loudly" error "$cancelled"
    echo "all rerun-runner-failures self-tests passed"
}

if [ "${1:-}" = "--selftest" ]; then
    selftest
    exit $?
fi
[ $# -eq 1 ] || { echo "usage: $0 RUN_ID | --selftest" >&2; exit 2; }
case $1 in *[!0-9]*) echo "rerun-runner-failures: run id must be numeric" >&2; exit 2 ;; esac
decide "$1"
