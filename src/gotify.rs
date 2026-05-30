use reqwest::Client;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;

pub struct GotifyLayer {
    client: Client,
    gotify_url: String,
    gotify_token: String,
}

impl GotifyLayer {
    pub fn new(gotify_url: &str, gotify_token: &str) -> Self {
        Self {
            client: Client::new(),
            gotify_url: gotify_url.trim_end_matches('/').to_string(),
            gotify_token: gotify_token.to_string(),
        }
    }

    fn level_to_priority(level: &Level) -> u8 {
        match *level {
            Level::ERROR => 10,
            Level::WARN => 7,
            Level::INFO => 5,
            Level::DEBUG => 2,
            Level::TRACE => 0,
        }
    }
}

impl<S> Layer<S> for GotifyLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();
        let level = metadata.level();

        // 只推送 info 及以上级别
        if *level > Level::INFO {
            return;
        }

        let mut fields = StringVisitor::new();
        event.record(&mut fields);

        let title = format!("[{}] {}", level, metadata.target());
        let message = fields.0;
        let priority = Self::level_to_priority(level);

        let client = self.client.clone();
        let url = format!(
            "{}/message?token={}",
            self.gotify_url, self.gotify_token
        );

        tokio::spawn(async move {
            let _ = client
                .post(&url)
                .form(&[
                    ("title", title.as_str()),
                    ("message", message.as_str()),
                    ("priority", &priority.to_string()),
                ])
                .send()
                .await;
        });
    }
}

struct StringVisitor(String);

impl StringVisitor {
    fn new() -> Self {
        Self(String::new())
    }
}

impl tracing::field::Visit for StringVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if !self.0.is_empty() {
            self.0.push_str(", ");
        }
        self.0.push_str(&format!("{}: {:?}", field.name(), value));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if !self.0.is_empty() {
            self.0.push_str(", ");
        }
        self.0.push_str(&format!("{}: {}", field.name(), value));
    }
}