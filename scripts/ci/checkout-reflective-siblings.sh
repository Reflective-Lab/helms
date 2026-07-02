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
# In CI, GITHUB_WORKSPACE (/home/runner/work/helms/helms) plays the role
# of bedrock-platform/helms, so its parent acts as bedrock-platform and
# its grandparent acts as the reflective root. Adapted from
# mosaic-extensions/arbiter-policy/scripts/ci/checkout-reflective-siblings.sh.
set -euo pipefail

workspace="${GITHUB_WORKSPACE:-$(git rev-parse --show-toplevel)}"

checkout_reflective_repo() {
  local repo="$1"
  local relative_path="$2"
  local dest="${workspace}/${relative_path}"

  if [[ -d "$dest/.git" ]]; then
    echo "ok: ${relative_path} already checked out"
    return
  fi

  if [[ -e "$dest" ]]; then
    echo "error: ${dest} exists but is not a git checkout" >&2
    exit 1
  fi

  mkdir -p "$(dirname "$dest")"
  echo "==> checkout Reflective-Lab/${repo} -> ${relative_path}"
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
checkout_reflective_repo runtime-runway ../../runtime-runway
# commerce-rails backs runtime-runway's workspace.dependencies entry.
checkout_reflective_repo commerce-rails ../../commerce-rails

# atelier-showcase crates address the platform as ../bedrock-platform/<repo>.
# In CI the workspace parent already plays that role, so expose it under the
# canonical name too. Locally the real bedrock-platform dir exists -> no-op.
bedrock_alias="${workspace}/../../bedrock-platform"
if [[ ! -e "$bedrock_alias" ]]; then
  echo "==> symlink bedrock-platform -> $(cd "${workspace}/.." && pwd)"
  ln -s "$(cd "${workspace}/.." && pwd)" "$bedrock_alias"
fi
