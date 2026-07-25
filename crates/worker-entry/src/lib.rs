use worker::*;

mod jobs;
pub(crate) mod metrics;
mod runtime;
pub(crate) mod services;
pub(crate) mod version;

#[event(fetch)]
async fn fetch(req: Request, env: Env, ctx: Context) -> Result<Response> {
    runtime::http::handle(req, env, ctx).await
}

#[event(scheduled)]
async fn scheduled(event: ScheduledEvent, env: Env, ctx: ScheduleContext) {
    runtime::cron::handle(event, env, ctx).await;
}

#[event(queue)]
async fn queue(batch: MessageBatch<runtime::queue::FetchJob>, env: Env, ctx: Context) -> Result<()> {
    runtime::queue::handle(batch, env, ctx).await
}

/// Extract the best available body text from an RSS/Atom entry.
pub(crate) fn extract_body(entry: &feed_rs::model::Entry) -> String {
    entry
        .summary
        .as_ref()
        .map(|s| s.content.clone())
        .or_else(|| entry.content.as_ref().and_then(|c| c.body.clone()))
        .or_else(|| {
            let texts: Vec<&str> =
                entry.media.iter().filter_map(|m| m.description.as_ref().map(|d| d.content.as_str())).collect();
            if texts.is_empty() {
                None
            } else {
                Some(texts.join("\n"))
            }
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use feed_rs::model::{Content, Entry};

    #[test]
    fn extract_body_from_content() {
        let entry = Entry {
            content: Some(Content { body: Some("content".into()), ..Default::default() }),
            ..Default::default()
        };
        assert_eq!(extract_body(&entry), "content");
    }

    #[test]
    fn extract_body_prefers_summary() {
        let entry = Entry {
            content: Some(Content { body: Some("fallback".into()), ..Default::default() }),
            ..Default::default()
        };
        assert_eq!(extract_body(&entry), "fallback");
    }

    #[test]
    fn extract_body_empty_entry() {
        let entry = Entry::default();
        assert_eq!(extract_body(&entry), "");
    }
}
