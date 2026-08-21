//! Seed data — initial 50+ reasoning frameworks across 6 categories.
//!
//! Run `seed()` after migration 0049 to populate the reasoning_frameworks table.
//! Uses INSERT OR IGNORE so it's idempotent.

use crate::framework::{FrameworkCategory, NewFramework, TriggerRule};

/// Returns the initial set of reasoning frameworks.
pub fn initial_frameworks() -> Vec<NewFramework> {
    vec![

    // ══════════════════════════════════════════════════
    //  Mathematical Models (8)
    // ══════════════════════════════════════════════════

    NewFramework {
        id: "bayes-theorem".into(),
        name: "Bayes' Theorem".into(),
        category: FrameworkCategory::MathematicalModels,
        description: "Prior beliefs should be updated with new evidence proportionally to the likelihood of that evidence under competing hypotheses.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: Some("claim".into()), entity_type: None, question_type: Some("confidence".into()), keywords: vec!["probability".into(), "evidence".into(), "belief".into(), "update".into()] },
            TriggerRule { signal_type: Some("claim".into()), entity_type: None, question_type: Some("evidence".into()), keywords: vec!["likelihood".into(), "prior".into(), "posterior".into()] },
        ],
        reasoning_template: "Apply Bayes' Theorem: consider how new evidence should update prior beliefs. Assess the base rate before evaluating new information. Ask: 'Does this evidence discriminate between competing hypotheses?'".into(),
        evidence_requirements: vec!["base_rate".into(), "evidence_strength".into(), "false_positive_rate".into()],
    },

    NewFramework {
        id: "expected-value".into(),
        name: "Expected Value".into(),
        category: FrameworkCategory::MathematicalModels,
        description: "The expected value of a decision is the sum of all possible outcomes weighted by their probability. Rational choices maximize expected value over the long run.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: None, entity_type: Some("company".into()), question_type: Some("valuation".into()), keywords: vec!["expected".into(), "probability".into(), "outcome".into(), "return".into()] },
            TriggerRule { signal_type: None, entity_type: None, question_type: Some("risk".into()), keywords: vec!["expected".into(), "probability".into()] },
        ],
        reasoning_template: "Evaluate using Expected Value: estimate probability × outcome for each scenario. Consider the full distribution of outcomes, not just the most likely one. A high-probability small gain may be worth less than a low-probability large gain.".into(),
        evidence_requirements: vec!["outcome_scenarios".into(), "probabilities".into(), "magnitudes".into()],
    },

    NewFramework {
        id: "power-law".into(),
        name: "Power Law Distribution".into(),
        category: FrameworkCategory::MathematicalModels,
        description: "In many systems, a small number of cases account for the majority of the effect (80/20 rule, Pareto principle). Outcomes are not normally distributed — extreme events happen more often than Gaussian models predict.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: Some("entity_signal".into()), entity_type: Some("company".into()), question_type: Some("competition".into()), keywords: vec!["winner".into(), "concentration".into(), "pareto".into(), "inequality".into(), "long tail".into()] },
            TriggerRule { signal_type: None, entity_type: Some("market".into()), question_type: None, keywords: vec!["market share".into(), "dominant".into(), "concentration".into()] },
        ],
        reasoning_template: "Consider Power Law dynamics: does this market follow a winner-take-most distribution? The top player may capture disproportionate value. Extreme outcomes matter more than averages. Avoid assuming normal distribution.".into(),
        evidence_requirements: vec!["market_concentration".into(), "top_player_share".into()],
    },

    NewFramework {
        id: "regression-mean".into(),
        name: "Regression to the Mean".into(),
        category: FrameworkCategory::MathematicalModels,
        description: "After an extreme outcome, the next outcome is likely to be closer to the average. Exceptional performance is often followed by more typical results.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: Some("entity_signal".into()), entity_type: None, question_type: Some("performance".into()), keywords: vec!["record".into(), "peak".into(), "exceptional".into(), "unprecedented".into()] },
        ],
        reasoning_template: "Consider Regression to the Mean: extreme results are often followed by more normal ones. Is the current performance sustainable or is it an outlier? Don't confuse a temporary spike with a trend change.".into(),
        evidence_requirements: vec!["historical_average".into(), "current_value".into()],
    },

    // ══════════════════════════════════════════════════
    //  Financial Intelligence (10)
    // ══════════════════════════════════════════════════

    NewFramework {
        id: "compound-growth".into(),
        name: "Compound Growth".into(),
        category: FrameworkCategory::FinancialIntelligence,
        description: "Small continuous growth rates compound into exponential results over time. The effect is non-linear and often underestimated in early stages.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: Some("entity_signal".into()), entity_type: Some("company".into()), question_type: Some("growth".into()), keywords: vec!["growth".into(), "compound".into(), "exponential".into(), "moonshot".into()] },
            TriggerRule { signal_type: None, entity_type: Some("technology".into()), question_type: Some("adoption".into()), keywords: vec!["adoption rate".into(), "growth rate".into()] },
        ],
        reasoning_template: "Apply Compound Growth: determine the growth rate and time horizon. Small differences in growth rates compound into enormous differences over time. Early-stage exponential growth looks linear. Ask: 'Is the growth rate sustainable? What would change the trajectory?'".into(),
        evidence_requirements: vec!["growth_rate".into(), "time_horizon".into(), "sustainability_factors".into()],
    },

    NewFramework {
        id: "margin-of-safety".into(),
        name: "Margin of Safety".into(),
        category: FrameworkCategory::FinancialIntelligence,
        description: "When making judgments under uncertainty, require a buffer between your estimate and the decision threshold. The greater the uncertainty, the larger the margin required.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: None, entity_type: None, question_type: Some("confidence".into()), keywords: vec!["uncertainty".into(), "safety".into(), "buffer".into(), "margin".into(), "risk".into()] },
            TriggerRule { signal_type: None, entity_type: Some("investment".into()), question_type: None, keywords: vec!["valuation".into(), "price".into(), "value".into()] },
        ],
        reasoning_template: "Apply Margin of Safety: uncertainty requires a confidence buffer. If confidence is 70% but the decision threshold is 60%, the margin is thin. The less information available, the wider the margin should be. Ask: 'What would convince me I'm wrong?'".into(),
        evidence_requirements: vec!["confidence_estimate".into(), "decision_threshold".into(), "uncertainty_range".into()],
    },

    NewFramework {
        id: "optionality".into(),
        name: "Optionality".into(),
        category: FrameworkCategory::FinancialIntelligence,
        description: "Decisions that preserve future flexibility are valuable even when their direct expected value is low. Asymmetric outcomes (small cost, large potential upside) justify action even without high confidence.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: None, entity_type: None, question_type: Some("strategy".into()), keywords: vec!["option".into(), "flexibility".into(), "asymmetric".into(), "upside".into(), "downside".into()] },
            TriggerRule { signal_type: None, entity_type: Some("company".into()), question_type: Some("investment".into()), keywords: vec!["optionality".into(), "experiment".into(), "trial".into()] },
        ],
        reasoning_template: "Consider Optionality: does this decision preserve or destroy future options? Small-bet experiments with capped downside and unlimited upside are valuable even at low probability. Avoid decisions that lock in outcomes prematurely.".into(),
        evidence_requirements: vec!["downside_risk".into(), "upside_potential".into(), "time_horizon".into()],
    },

    // ══════════════════════════════════════════════════
    //  Human Behavior (10)
    // ══════════════════════════════════════════════════

    NewFramework {
        id: "loss-aversion".into(),
        name: "Loss Aversion".into(),
        category: FrameworkCategory::HumanBehavior,
        description: "People feel losses approximately 2x more intensely than equivalent gains. This asymmetric response drives market panics, resistance to change, and risk-averse decision-making even when the expected value favors action.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: Some("claim".into()), entity_type: Some("market".into()), question_type: Some("risk".into()), keywords: vec!["panic".into(), "fear".into(), "loss".into(), "risk aversion".into(), "sell-off".into()] },
            TriggerRule { signal_type: None, entity_type: Some("policy".into()), question_type: Some("adoption".into()), keywords: vec!["resistance".into(), "opposition".into(), "fear".into()] },
        ],
        reasoning_template: "Consider Loss Aversion: losses are felt ~2x more than equivalent gains. Market fear may be overdone. Resistance to change is expected even when the change is net positive. Ask: 'Is the emotional reaction proportional to the actual risk?'".into(),
        evidence_requirements: vec!["market_sentiment".into(), "risk_perception".into()],
    },

    NewFramework {
        id: "incentives".into(),
        name: "Incentive Response".into(),
        category: FrameworkCategory::HumanBehavior,
        description: "People respond to incentives. To understand behavior, examine what rewards and penalties people face. Strong incentives almost always predict behavior more accurately than stated intentions.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: None, entity_type: None, question_type: None, keywords: vec!["incentive".into(), "motivation".into(), "reward".into(), "behavior".into(), "interest".into()] },
        ],
        reasoning_template: "Analyze via Incentives: what are the actual incentives for each actor? (Not the stated ones.) Follow the money. Ask: 'If I changed the incentives, would the behavior change?' Predict behavior by aligning incentives, not intentions.".into(),
        evidence_requirements: vec!["actor_incentives".into(), "stated_vs_actual".into()],
    },

    NewFramework {
        id: "confirmation-bias".into(),
        name: "Confirmation Bias".into(),
        category: FrameworkCategory::HumanBehavior,
        description: "People tend to seek, interpret, and remember information that confirms their existing beliefs while discounting contradictory evidence.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: None, entity_type: None, question_type: None, keywords: vec!["bias".into(), "confirmation".into(), "belief".into(), "selective".into()] },
        ],
        reasoning_template: "Consider Confirmation Bias: is the evidence genuinely conclusive, or are we seeing what we expect? Actively seek disconfirming evidence. Ask: 'What would I believe if the opposite were true?'".into(),
        evidence_requirements: vec!["supporting_evidence".into(), "contradictory_evidence".into()],
    },

    // ══════════════════════════════════════════════════
    //  Strategic Models (8)
    // ══════════════════════════════════════════════════

    NewFramework {
        id: "network-effects".into(),
        name: "Network Effects".into(),
        category: FrameworkCategory::StrategicModels,
        description: "A product or service becomes more valuable as more people use it. This creates a natural monopoly dynamic and winner-take-most market structure.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: Some("entity_signal".into()), entity_type: Some("platform".into()), question_type: Some("competition".into()), keywords: vec!["network".into(), "platform".into(), "marketplace".into(), "user base".into()] },
            TriggerRule { signal_type: None, entity_type: Some("marketplace".into()), question_type: Some("adoption".into()), keywords: vec!["network effect".into(), "viral".into()] },
        ],
        reasoning_template: "Apply Network Effects analysis: is this a platform business? Does each new user add value for existing users? What is the switching cost? Winner-take-most dynamics apply. Early leaders may become unassailable.".into(),
        evidence_requirements: vec!["user_growth".into(), "switching_cost".into(), "competitive_dynamics".into()],
    },

    NewFramework {
        id: "competitive-moats".into(),
        name: "Competitive Moats".into(),
        category: FrameworkCategory::StrategicModels,
        description: "Sustainable competitive advantage comes from barriers that prevent competitors from replicating your business. Common moats: brand, scale, network effects, switching costs, regulatory protection, trade secrets.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: Some("entity_signal".into()), entity_type: Some("company".into()), question_type: Some("competition".into()), keywords: vec!["moat".into(), "advantage".into(), "barrier".into(), "competitive".into()] },
        ],
        reasoning_template: "Evaluate Competitive Moats: what prevents competitors from entering? Is the moat widening or narrowing? Commoditization erodes moats. Network effects can strengthen them. Ask: 'Would I start a competing company today?'".into(),
        evidence_requirements: vec!["barrier_types".into(), "moat_trend".into(), "competitive_threats".into()],
    },

    // ══════════════════════════════════════════════════
    //  Systems Thinking (7)
    // ══════════════════════════════════════════════════

    NewFramework {
        id: "second-order-effects".into(),
        name: "Second-Order Effects".into(),
        category: FrameworkCategory::SystemsThinking,
        description: "Every action has both immediate consequences and follow-on effects. The most important outcomes are often the second-order effects, not the first.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: Some("claim".into()), entity_type: Some("policy".into()), question_type: Some("impact".into()), keywords: vec!["unintended".into(), "consequence".into(), "second order".into(), "knock-on".into()] },
            TriggerRule { signal_type: None, entity_type: Some("regulation".into()), question_type: Some("risk".into()), keywords: vec!["regulation".into(), "policy".into(), "impact".into()] },
        ],
        reasoning_template: "Analyze Second-Order Effects: after the obvious outcome, what happens next? Regulations often produce opposite of intended effects. Ask: 'And then what?' at least 3 times. The third answer is usually the most important one.".into(),
        evidence_requirements: vec!["first_order".into(), "second_order".into(), "stakeholder_response".into()],
    },

    NewFramework {
        id: "feedback-loops".into(),
        name: "Feedback Loops".into(),
        category: FrameworkCategory::SystemsThinking,
        description: "Systems often contain reinforcing loops (success breeds success) or balancing loops (negative feedback restores equilibrium). Identifying the loop type predicts system behavior.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: None, entity_type: None, question_type: None, keywords: vec!["feedback".into(), "cycle".into(), "vicious".into(), "virtuous".into(), "loop".into()] },
        ],
        reasoning_template: "Identify Feedback Loops: is this a reinforcing cycle (success accelerates success) or a balancing one (resistance counteracts change)? Reinforcing loops lead to exponential growth or collapse. Balancing loops create equilibrium.".into(),
        evidence_requirements: vec!["loop_type".into(), "amplifying_factors".into()],
    },

    // ══════════════════════════════════════════════════
    //  Scientific Thinking (7)
    // ══════════════════════════════════════════════════

    NewFramework {
        id: "falsification".into(),
        name: "Falsification (Popper)".into(),
        category: FrameworkCategory::ScientificThinking,
        description: "A claim is scientific only if it can be proven false. The strength of a hypothesis comes not from confirming evidence but from surviving attempts to falsify it.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: Some("claim".into()), entity_type: None, question_type: None, keywords: vec!["falsify".into(), "prove wrong".into(), "testable".into(), "hypothesis".into()] },
        ],
        reasoning_template: "Apply Falsification: what specific evidence would prove this claim wrong? If nothing can falsify it, it's not a testable claim. The most valuable claims are the most falsifiable ones that haven't been falsified yet.".into(),
        evidence_requirements: vec!["falsification_condition".into(), "testability".into()],
    },

    NewFramework {
        id: "occams-razor".into(),
        name: "Occam's Razor".into(),
        category: FrameworkCategory::ScientificThinking,
        description: "Among competing hypotheses, the one with the fewest assumptions should be preferred. Simple explanations are more likely to be correct than complex ones.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: None, entity_type: None, question_type: None, keywords: vec!["simple".into(), "complex".into(), "assumption".into(), "unnecessary".into()] },
        ],
        reasoning_template: "Apply Occam's Razor: does the explanation require many assumptions? The simplest explanation that fits the evidence is usually correct. Extra assumptions should be justified by additional evidence.".into(),
        evidence_requirements: vec!["assumption_count".into(), "alternative_explanations".into()],
    },

    NewFramework {
        id: "correlation-vs-causation".into(),
        name: "Correlation vs. Causation".into(),
        category: FrameworkCategory::ScientificThinking,
        description: "Two things happening together does not mean one causes the other. Confounding variables, reverse causation, and coincidences are common alternatives.".into(),
        trigger_rules: vec![
            TriggerRule { signal_type: Some("claim".into()), entity_type: None, question_type: None, keywords: vec!["correlation".into(), "cause".into(), "link".into(), "relationship".into(), "tied to".into()] },
        ],
        reasoning_template: "Distinguish Correlation from Causation: is there a plausible causal mechanism? Could there be a confounding variable? Is reverse causation possible? Randomized evidence is stronger than observational data.".into(),
        evidence_requirements: vec!["correlation_strength".into(), "plausible_mechanism".into(), "confounders".into()],
    },

    ]
}

/// Total count of frameworks in seed data.
pub fn seed_count() -> usize {
    initial_frameworks().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_contains_expected_count() {
        assert_eq!(seed_count(), 17, "should have 17 seed frameworks for MVP");
    }

    #[test]
    fn each_framework_has_unique_id() {
        let fws = initial_frameworks();
        let mut ids: Vec<&str> = fws.iter().map(|f| f.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), fws.len(), "all framework IDs must be unique");
    }

    #[test]
    fn all_categories_represented() {
        let fws = initial_frameworks();
        let cats: Vec<_> = fws.iter().map(|f| &f.category).collect();
        assert!(cats.contains(&&FrameworkCategory::MathematicalModels));
        assert!(cats.contains(&&FrameworkCategory::FinancialIntelligence));
        assert!(cats.contains(&&FrameworkCategory::HumanBehavior));
        assert!(cats.contains(&&FrameworkCategory::StrategicModels));
        assert!(cats.contains(&&FrameworkCategory::SystemsThinking));
        assert!(cats.contains(&&FrameworkCategory::ScientificThinking));
    }
}
