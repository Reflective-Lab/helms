#!/usr/bin/env bash
# Check out the Reflective-Lab sibling repos that helms consumes via
# relative path dependencies (see Cargo.toml [workspace.dependencies]
# and [patch.crates-io]).
#
# Local layout (helms lives at reflective/bedrock-platform/helms):
#   ../<repo>                        -> bedrock-platform siblings
#   ../../mosaic-extensions/<repo>   -> extension repos
#   ../../<repo>                     -> reflective-root siblings
#
# In CI, ci.yml checks the repo out at bedrock-platform/helms under
# $GITHUB_WORKSPACE so every relative path dep resolves to the same
# lexical location as it does locally (cargo identifies path packages
# lexically — a symlinked alias produces package collisions). The repo
# root is derived from this script's own location so the topology works
# identically in CI and local runs.
set -euo pipefail

workspace="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# REFLECTIVE_SIBLING_REF controls which branch siblings are cloned from (default: main).
# Validate to only allow chars legal in git branch names; reject anything else so a
# malformed value cannot be used for argument injection in the git clone call below.
_raw_ref="${REFLECTIVE_SIBLING_REF:-main}"
if [[ ! "$_raw_ref" =~ ^[a-zA-Z0-9/._-]+$ ]]; then
  echo "warning: REFLECTIVE_SIBLING_REF='${_raw_ref}' contains disallowed characters; using main" >&2
  _raw_ref=main
fi
REFLECTIVE_SIBLING_REF="$_raw_ref"
unset _raw_ref

checkout_reflective_repo() {
  local repo="$1"
  local relative_path="$2"
  local dest="${workspace}/${relative_path}"
  local sibling_ref="${REFLECTIVE_SIBLING_REF:-main}"

  if [[ -d "$dest/.git" ]]; then
    echo "ok: ${relative_path} already checked out"
    return
  fi

  if [[ -e "$dest" ]]; then
    echo "error: ${dest} exists but is not a git checkout" >&2
    exit 1
  fi

  mkdir -p "$(dirname "$dest")"

  if [[ "$sibling_ref" != "main" ]]; then
    echo "==> checkout Reflective-Lab/${repo}@${sibling_ref} -> ${relative_path}"
    if GIT_TERMINAL_PROMPT=0 git clone --depth=1 --branch "$sibling_ref" --quiet \
        "https://github.com/Reflective-Lab/${repo}.git" "$dest" 2>/dev/null; then
      return
    fi
    echo "    (branch '${sibling_ref}' not found; falling back to main)"
    rm -rf "$dest"
  fi

  echo "==> checkout Reflective-Lab/${repo}@main -> ${relative_path}"
  GIT_TERMINAL_PROMPT=0 git clone --depth=1 --quiet "https://github.com/Reflective-Lab/${repo}.git" "$dest"
}

# bedrock-platform siblings (direct path deps: ../<repo>).
checkout_reflective_repo axiom ../axiom
checkout_reflective_repo converge ../converge
checkout_reflective_repo organism ../organism

# Extension repos (path deps: ../../mosaic-extensions/<repo>).
checkout_reflective_repo arbiter-policy ../../mosaic-extensions/arbiter-policy
# embassy-ports is not a direct helms dep, but atelier-showcase's workspace
# references converge-embassy-sec-edgar by path and cargo metadata loads the
# whole atelier workspace when resolving atelier-domain.
checkout_reflective_repo embassy-ports ../../mosaic-extensions/embassy-ports
checkout_reflective_repo ferrox-solvers ../../mosaic-extensions/ferrox-solvers
checkout_reflective_repo manifold-adapters ../../mosaic-extensions/manifold-adapters
checkout_reflective_repo mnemos-knowledge ../../mosaic-extensions/mnemos-knowledge
checkout_reflective_repo prism-analytics ../../mosaic-extensions/prism-analytics

# Reflective-root siblings (path deps: ../../<repo>).
checkout_reflective_repo atelier-showcase ../../atelier-showcase
# arena-tests: atelier's truth-driven-formation scenario path-depends on
# arena-intent-cases, and cargo metadata loads atelier's whole workspace.
checkout_reflective_repo arena-tests ../../arena-tests
checkout_reflective_repo runtime-runway ../../runtime-runway
# commerce-rails backs runtime-runway's workspace.dependencies entry.
checkout_reflective_repo commerce-rails ../../commerce-rails
