//! Domain-aligned Repository traits — **aggregate persistence only** (save / find).
//!
//! Each Repository has 2-3 methods and represents the write-side boundary for
//! a single aggregate root.  Read operations belong in [`super::query`].

pub mod article_repo;
pub mod decision_repo;
pub mod entity_repo;
pub mod evaluation_repo;
pub mod feed_repo;
pub mod outcome_repo;
pub mod signal_repo;

pub use article_repo::ArticleRepository;
pub use decision_repo::DecisionRepository;
pub use entity_repo::EntityRepository;
pub use evaluation_repo::EvaluationRepository;
pub use feed_repo::FeedRepository;
pub use outcome_repo::OutcomeRepository;
pub use signal_repo::SignalRepository;
