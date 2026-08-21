#!/usr/bin/env bash
# Layered dependency guard — decoupling plan P1 (2026-08-21).
#
# cargo-deny bans are workspace-global: they cannot express per-consumer
# dependency edges. To stop NEW coupling from the controlled layer crates to
# infrastructure crates while decoupling P3/P4/P5 is in flight, this guard
# reads each crate's *declared* dependency list (cargo metadata --no-deps —
# no build, no network) and asserts it stays inside the grandfathered set.
#
# The grandfathered set below is the *current* dual-track coupling. As
# decoupling progresses, delete lines here; the guard gets tighter and the
# set must reach zero by Sprint 5 (see docs/architecture/final-architecture-v2.md
# §4 and the decoupling plan). Adding a NEW banned dep to a controlled crate
# fails CI immediately.
#
# Usage: bash scripts/check-layered-deps.sh
set -euo pipefail

# Infrastructure-layer crates that must not appear as declared deps of the
# controlled layer (beyond the grandfathered entries below).
BANNED=(store vectorize embedding event-store object-store)

# Controlled layer crates — the 7 intermediate-layer crates under scrutiny.
# Each must not declare a banned dep beyond its GRANDFATHERED entries.
CONTROLLED=(signal-engine reflection-engine memory-engine ai-pipeline context-engine agent-engine claim-engine)

# Controlled layer crates → grandfathered couplings (format crateName:dep).
# Remove a line once that coupling is migrated out (P3/P4/P5).
GRANDFATHERED=(
  signal-engine:store
  signal-engine:vectorize
  signal-engine:embedding
  signal-engine:event-store
  reflection-engine:store
  reflection-engine:event-store
  memory-engine:store
  memory-engine:event-store
  ai-pipeline:store
  context-engine:store
  context-engine:object-store
  agent-engine:store
  claim-engine:store
)

# Parse once: crate name → declared dep names (includes optional deps).
declare -A DEPS
while IFS='|' read -r crate deps; do
  DEPS["$crate"]="$deps"
done < <(
  cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[] | (.name + "|" + ("," + ([.dependencies[].name] | join(",")) + ","))'
)

# Keep track so we can also flag grandfather entries that are now removable.
declare -A GRANDFATHERED_INDEX
for g in "${GRANDFATHERED[@]}"; do GRANDFATHERED_INDEX["$g"]=1; done

violations=0
stale=0

# Per controlled crate.
for crate in "${CONTROLLED[@]}"; do
  declared="${DEPS[$crate]:-}"
  for banned in "${BANNED[@]}"; do
    if [[ "$declared" == *",$banned,"* ]]; then
      if [[ -z "${GRANDFATHERED_INDEX["$crate:$banned"]:-}" ]]; then
        echo "  ✗ NEW coupling: $crate declares banned dep '$banned' (not grandfathered)"
        violations=$((violations + 1))
      fi
    else
      if [[ -n "${GRANDFATHERED_INDEX["$crate:$banned"]:-}" ]]; then
        echo "  ✓ removable: $crate no longer declares '$banned' — remove from GRANDFATHERED"
        stale=$((stale + 1))
      fi
    fi
  done
done

if [[ "$violations" -gt 0 ]]; then
  echo ""
  echo "Layered dependency guard FAILED: $violations new coupling(s) found."
  echo "Do NOT add store/vectorize/embedding/event-store/object-store to controlled crates."
  echo "Migrate couplings out (decoupling P3/P4/P5) instead of extending them."
  exit 1
fi

echo "Layered dependency guard OK: no new coupling (${#GRANDFATHERED[@]} grandfathered, $stale removable)."
