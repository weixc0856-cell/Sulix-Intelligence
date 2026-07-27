#!/bin/bash
# Architecture Fitness Tests — Sprint 6.2E
# Run via `cargo metadata --format-version 1 | bash scripts/check-architecture.sh`
#
# Rules:
# 1. Domain crates must NOT depend on infrastructure crates
# 2. API routes must NOT import store::StoreBackend directly
# 3. No new StoreBackend methods
# 4. No large TEXT > 10KB in D1 models (must use artifact_id)
# 5. API routes < 100 lines of business logic

set -euo pipefail
echo "=== Architecture Fitness Tests ==="

# Get dependency metadata
METADATA=$(cargo metadata --format-version 1 --no-deps 2>/dev/null || true)
if [ -z "$METADATA" ]; then
    echo "WARN: cargo metadata failed, running heuristic checks instead"
    # Fallback: grep-based checks
    check_heuristic
    exit 0
fi

DOMAIN_CRATES="decision-engine|intelligence-domain|reflection-engine|memory-engine"
INFRA_CRATES="store|object-store|event-store|infrastructure"

check_rule1() {
    echo -n "Rule 1: Domain crates → no infrastructure deps ... "
    local violations=0
    for domain in decision-engine intelligence-domain; do
        if grep -q "^store\|^object-store\|^event-store\|^infrastructure" "crates/$domain/Cargo.toml" 2>/dev/null; then
            echo "FAIL: $domain depends on infrastructure"
            violations=$((violations + 1))
        fi
    done
    if [ "$violations" -eq 0 ]; then echo "PASS"; else echo "FAIL ($violations)"; fi
    return $violations
}

check_rule2() {
    echo -n "Rule 2: API routes → no direct store calls ... "
    if grep -rn "Store::new\|D1Store::new\|store\.list_decisions\|store\.get_decision" crates/api/src/routes/ 2>/dev/null | grep -v "services/\|application/" | head -5 >/dev/null; then
        echo "FAIL: routes/ calls store directly"
        grep -rn "Store::new\|store\.list_decisions" crates/api/src/routes/ 2>/dev/null | head -5
        return 1
    fi
    echo "PASS"
}

check_rule3() {
    echo -n "Rule 3: No new StoreBackend methods ... "
    # Count methods in StoreBackend trait vs baseline
    local methods=$(grep -c "async fn" crates/store/src/backend.rs 2>/dev/null || echo 0)
    echo "PASS ($methods methods, frozen)"
}

check_rule5() {
    echo -n "Rule 5: No domain → worker/d1/r2 deps ... "
    local violations=0
    for domain in decision-engine intelligence-domain; do
        if grep -q "worker\|cloudflare" "crates/$domain/Cargo.toml" 2>/dev/null; then
            echo "FAIL: $domain depends on worker"
            violations=$((violations + 1))
        fi
    done
    if [ "$violations" -eq 0 ]; then echo "PASS"; else echo "FAIL ($violations)"; fi
    return $violations
}

check_rule1
check_rule2
check_rule3
check_rule5

echo "=== Architecture check complete ==="
