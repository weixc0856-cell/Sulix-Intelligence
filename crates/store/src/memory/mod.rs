//! In-memory [`StoreBackend`](crate::StoreBackend) implementation for tests.
//!
//! Uses `HashMap` / `Vec` instead of D1.  Supports failure injection so
//! pipeline error-handling paths can be exercised without a database.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::{Feed, NewArticle, OutcomeEvent, OutboxEntry, SignalEvent};

mod backend;

/// Per-feed fetch-result entry recorded by `record_fetch_result`.
type FetchResultEntry = (i64, i64, Option<String>, Option<String>);

/// In-memory store with failure-injection flags.
///
/// Uses `RefCell` for interior mutability — safe because the trait is
/// `#[async_trait(?Send)]` and tests run on a single thread.
pub struct MemoryStore {
    pub feeds: HashMap<i64, Feed>,
    pub rules: Vec<String>,

    // RefCell for interior mutability (trait takes &self)
    articles: RefCell<Vec<NewArticle>>,
    next_article_id: RefCell<i64>,
    summaries: RefCell<HashMap<i64, String>>,
    r2_keys: RefCell<HashMap<i64, Option<String>>>,
    pub fetch_results: RefCell<Vec<FetchResultEntry>>,

    // Entity graph state
    entities: RefCell<HashMap<i64, EntityInternal>>,
    next_entity_id: RefCell<i64>,
    article_entity_links: RefCell<Vec<(i64, i64)>>,
    entity_relation_edges: RefCell<Vec<RelationEdge>>,
    artifacts: RefCell<Vec<ArtifactData>>,
    next_artifact_id: RefCell<i64>,

    /// When `true`, `insert_article` returns `Err`.
    pub fail_insert: bool,
    /// When `true`, `active_rule_jsons` returns `Err`.
    pub fail_rules: bool,
    /// When `true`, `set_ai_summary` returns `Err`.
    pub fail_summary: bool,
    /// When `true`, `record_fetch_result` returns `Err`.
    pub fail_fetch_result: bool,
    /// When `true`, `set_raw_content_r2_key` returns `Err`.
    pub fail_r2_key: bool,

    // Signal engine state
    pub signal_events: RefCell<Vec<SignalEvent>>,
    next_signal_event_id: RefCell<i64>,

    // Decision state
    decisions: RefCell<Vec<crate::Decision>>,
    next_decision_id: RefCell<i64>,

    // Outcome state
    outcomes: RefCell<Vec<OutcomeEvent>>,
    next_outcome_id: RefCell<i64>,

    // Evaluation state
    evaluations: RefCell<Vec<crate::DecisionEvaluation>>,

    // Outbox state
    outbox: RefCell<Vec<OutboxEntry>>,
    next_outbox_id: RefCell<i64>,
}

struct EntityInternal {
    id: i64,
    name: String,
    normalized_name: String,
    entity_type: String,
    canonical_id: Option<i64>,
    description: Option<String>,
    metadata: Option<String>,
    created_at: i64,
    updated_at: i64,
}

#[allow(dead_code)]
struct RelationEdge {
    source: i64,
    target: i64,
    rtype: String,
    confidence: f64,
    first_seen: i64,
    last_seen: i64,
}

struct ArtifactData {
    id: i64,
    artifact_type: String,
    entity_id: i64,
    r2_key: String,
    schema_version: String,
    model: Option<String>,
    pipeline_version: String,
    metadata: Option<String>,
    created_at: i64,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            feeds: HashMap::new(),
            articles: RefCell::new(Vec::new()),
            rules: Vec::new(),
            summaries: RefCell::new(HashMap::new()),
            r2_keys: RefCell::new(HashMap::new()),
            fetch_results: RefCell::new(Vec::new()),
            next_article_id: RefCell::new(1),
            entities: RefCell::new(HashMap::new()),
            next_entity_id: RefCell::new(1),
            article_entity_links: RefCell::new(Vec::new()),
            entity_relation_edges: RefCell::new(Vec::new()),
            artifacts: RefCell::new(Vec::new()),
            next_artifact_id: RefCell::new(1),
            fail_insert: false,
            fail_rules: false,
            fail_summary: false,
            fail_fetch_result: false,
            fail_r2_key: false,
            signal_events: RefCell::new(Vec::new()),
            next_signal_event_id: RefCell::new(1),
            decisions: RefCell::new(Vec::new()),
            next_decision_id: RefCell::new(1),
            outcomes: RefCell::new(Vec::new()),
            next_outcome_id: RefCell::new(1),
            evaluations: RefCell::new(Vec::new()),
            outbox: RefCell::new(Vec::new()),
            next_outbox_id: RefCell::new(1),
        }
    }

    /// Builder-style: set the rules that `active_rule_jsons` returns.
    pub fn with_rules(mut self, rules: Vec<String>) -> Self {
        self.rules = rules;
        self
    }

    /// Builder-style: insert a feed into the store.
    pub fn with_feed(mut self, feed: Feed) -> Self {
        self.feeds.insert(feed.id, feed);
        self
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}
