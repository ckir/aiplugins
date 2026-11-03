#!/usr/bin/env bash
#
# Generate the storefront catalog (catalog.json) for the GitHub Pages site.
#
#   bash scripts/gen-site.sh [--root DIR] OUT
#
# --root DIR  repository root to read (default "."; relative paths resolve
#             against the repository root, so fixtures can stand in for it).
# OUT         catalog file to write; its parent directory is created.
#
# Enumeration comes from agents.json via scripts/lib/registry.sh. Per plugin
# the order is: family_of (suffix), marketplace lookup, version_of (tag),
# install_entries (install table), so each refusal reaches its own error.
# Every refusal is `gen-site: <reason>` on stderr with exit 1. Output is
# canonical (jq -S), LF, newline-terminated. docs/site/families.txt is read by
# the checker only, never here.
set -euo pipefail
cd "$(dirname "$0")/.."

root=.
out=
while [ $# -gt 0 ]; do
    case "$1" in
        --root)
            [ $# -ge 2 ] || { echo "gen-site: --root needs a directory" >&2; exit 1; }
            root=$2
            shift 2
            ;;
        *)
            [ -z "$out" ] || { echo "gen-site: unexpected argument: $1" >&2; exit 1; }
            out=$1
            shift
            ;;
    esac
done
[ -n "$out" ] || { echo "usage: gen-site.sh [--root DIR] OUT" >&2; exit 1; }

REGISTRY="$root/agents.json"
export REGISTRY
set --
# shellcheck disable=SC1091  # sourced relative to the repository root after cd
source scripts/lib/registry.sh

# The jq on a Windows PATH emits CRLF; a stray carriage return would corrupt
# every value captured below.
jqr() {
    jq -r "$@" | tr -d '\r'
}
jqc() {
    jq -c "$@" | tr -d '\r'
}

die() {
    echo "gen-site: $*" >&2
    exit 1
}

# Strip the agent suffix from a plugin key; refuse any other suffix.
family_of() {
    case "$1" in
        *-cc) echo "${1%-cc}" ;;
        *-qwen) echo "${1%-qwen}" ;;
        *-opencode) echo "${1%-opencode}" ;;
        *-agy) echo "${1%-agy}" ;;
        *) die "$1: unknown agent suffix (expected -cc, -qwen, -opencode or -agy)" ;;
    esac
}

# The bare semver of the -v<semver> tag in a release URL; empty if absent.
version_of() {
    if [[ $1 =~ -v([0-9]+\.[0-9]+\.[0-9]+)/ ]]; then
        echo "${BASH_REMATCH[1]}"
    fi
}

# Join the arguments with newlines (no trailing newline).
join_lines() {
    local IFS=$'\n'
    printf '%s' "$*"
}

# One install entry as compact JSON: LABEL, COMMAND.
entry() {
    jq -n -c --arg l "$1" --arg c "$2" '{label: $l, command: $c}' | tr -d '\r'
}

# The install array (compact JSON) for one plugin. The install table: one
# case arm per agent id. An agent with no arm is refused.
# install_entries AGENT DIRKEY NAME URL SKILLS_ONLY(0|1) AGENT_DIR
install_entries() {
    local agent=$1 dirkey=$2 name=$3 url=$4 skills_only=$5 agent_dir=$6
    local zip=${url##*/}
    case "$agent" in
        claude-code)
            {
                entry "Add the marketplace" "claude plugin marketplace add ckir/aiplugins"
                entry "Install" "claude plugin install ${name}@aiplugins"
            } | jq -s -c .
            ;;
        qwen)
            # shellcheck disable=SC2088  # the literal ~ is the install text shown to users
            local ext="~/.qwen/extensions/${name}"
            local -a unix=(
                "curl -fsSL -o ${zip} ${url}"
                "mkdir -p ${ext}"
                "unzip -q -o ${zip} -d ${ext}/"
            )
            if [ "$skills_only" = 0 ]; then
                unix+=("find ${ext}/bin -type f ! -name '*.exe' -exec chmod 755 {} +")
            fi
            local win_dir="\"\$env:USERPROFILE\\.qwen\\extensions\\${name}\""
            {
                entry "Linux and macOS" "$(join_lines "${unix[@]}")"
                entry "Windows (PowerShell)" "$(join_lines \
                    "Invoke-WebRequest -Uri ${url} -OutFile ${zip}" \
                    "New-Item -ItemType Directory -Force -Path ${win_dir}" \
                    "Expand-Archive -Path ${zip} -DestinationPath ${win_dir} -Force")"
            } | jq -s -c .
            ;;
        opencode)
            entry "Ask your agent" "Fetch and follow the installation instructions at https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/${agent_dir}/${dirkey}/INSTALL.md, and ask me whether to install project-local or global first." \
                | jq -s -c .
            ;;
        antigravity)
            if [ "$dirkey" = repo-user-guide-agy ]; then
                entry "Copy the skill" "$(join_lines \
                    "curl -fsSL -o /tmp/${zip} ${url}" \
                    "unzip -q -o /tmp/${zip} -d /tmp/${name}" \
                    "mkdir -p ~/.gemini/config/skills/repo-user-guide" \
                    "cp -r /tmp/${name}/skills/repo-user-guide/. ~/.gemini/config/skills/repo-user-guide/")" \
                    | jq -s -c .
            else
                local -a agy=(
                    "curl -fsSL -o /tmp/${zip} ${url}"
                    "unzip -q -o /tmp/${zip} -d /tmp/${name}"
                    "agy plugin install /tmp/${name}"
                )
                if [ "$skills_only" = 0 ]; then
                    agy+=("export PATH=\"\$HOME/.gemini/config/plugins/${name}/bin:\$PATH\"")
                fi
                entry "Install" "$(join_lines "${agy[@]}")" | jq -s -c .
            fi
            ;;
        *) die "${agent}: no install shape; add a row to the install table in scripts/gen-site.sh" ;;
    esac
}

# The footprint object (compact JSON) for DIRKEY, or null when absent.
footprint_of() {
    local file="$root/docs/footprints/$1.json"
    if [ ! -f "$file" ]; then
        echo null
        return
    fi
    jq -n -c --slurpfile f "$file" --slurpfile b "$root/docs/footprints/budgets.json" --arg k "$1" \
        '$f[0] as $fp | {
            resident: $fp.tiers.resident.bytes,
            invocation: $fp.tiers.invocation.bytes,
            setup: ($fp.tiers.setup.bytes // null),
            budget: $b[0][$k]
        }' | tr -d '\r'
}

plugins_file=$(mktemp)
trap 'rm -f "$plugins_file"' EXIT

for agent in $(jqr '.agents[].id' "$REGISTRY"); do
    marketplace="$root/$(registry_field "$agent" marketplace)"
    agent_dir=$(registry_field "$agent" dir)
    for key in $(registry_plugins "$agent"); do
        family=$(family_of "$key")
        name=$(registry_published_name "$agent" "$key")
        # shellcheck disable=SC2016  # jq program, not shell
        item=$(jqc --arg n "$name" '[.plugins[] | select(.name == $n)][0] // empty' "$marketplace")
        [ -n "$item" ] || die "${key}: no marketplace entry named ${name}"
        url=$(jqr '.source.url' <<< "$item")
        version=$(version_of "$url")
        [ -n "$version" ] || die "${key}: no -v<semver> tag in source.url"
        skills_only=0
        if registry_is_skills_only "$agent" "$key"; then skills_only=1; fi
        install=$(install_entries "$agent" "$key" "$name" "$url" "$skills_only" "$agent_dir")
        footprint=$(footprint_of "$key")
        jq -n -c --arg family "$family" --arg agent "$agent" --arg dirKey "$key" \
            --arg name "$name" --arg version "$version" --argjson item "$item" \
            --argjson skillsOnly "$([ "$skills_only" = 1 ] && echo true || echo false)" \
            --argjson install "$install" --argjson footprint "$footprint" \
            '{family: $family, agent: $agent, plugin: {
                dirKey: $dirKey, name: $name, version: $version,
                description: $item.description, skillsOnly: $skillsOnly,
                readme: $item.homepage, install: $install, footprint: $footprint}}' \
            | tr -d '\r' >> "$plugins_file"
    done
done

mkdir -p "$(dirname "$out")"
jq -S -n --slurpfile rows "$plugins_file" '{
    agents: [
        {id: "claude-code", label: "Claude Code"},
        {id: "opencode", label: "OpenCode"},
        {id: "antigravity", label: "Antigravity"},
        {id: "qwen", label: "Qwen"}
    ],
    families: ($rows | group_by(.family) | map({
        key: .[0].family,
        plugins: (map({key: .agent, value: .plugin}) | from_entries)
    }) | sort_by(.key))
}' | tr -d '\r' > "$out"
