//! Engine 模块 — 核心领域引擎
//!
//! - analysis:     Theme analysis, ASI/SVI scoring, causal chain parsing
//! - belief:       BeliefEngineV2 (config-driven core beliefs)
//! - decision:     Thesis → Decision Intelligence (deterministic mapping)
//! - memory:       MemoryEngine (thesis/evidence lifecycle, outcomes, reflections)
//! - premium:      Premium 3-stage report generation (WhatChanged → WhyItMatters → WhatToDo)
//! - registry:     AssessmentRegistry (ASM-XXXX), shared RegistryCore
//! - stability:    Decision stability gating (2-day hysteresis, confidence threshold)
//! - investigation: Investigation generation + report derivation

pub mod analysis;
pub mod belief;
pub mod decision;
pub mod decision_registry;
pub mod investigation;
pub mod investigation_registry;
pub mod memory;
pub mod pipeline_health;
pub mod premium;
pub mod registry;
pub mod stability;
