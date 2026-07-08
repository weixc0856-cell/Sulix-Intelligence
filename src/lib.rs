//! Sulix Intelligence — 个人创业者的 AI 战略情报助手
//!
//! 管线：RSS 抓取 → 去重 → 全文提取 → 主题聚类 → 影响分析 → 咨询简报

pub mod agent;

pub mod archive;
pub mod catalog;
pub mod client;
pub mod clusterer;
pub mod config;
pub mod db;
pub mod domain;
pub mod engine;
pub mod enricher;
pub mod entity;
pub mod event_log;
pub mod fetcher;
pub mod hermes;
pub mod llm;
pub mod pipeline;
pub mod publishing;
pub mod question_engine;
pub mod renderer;
pub mod schema;
pub mod source;
pub mod storage;
pub mod twitter;
