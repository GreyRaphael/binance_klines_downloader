use reqwest::Client;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;

pub struct NotifyLayer {
    client: Client,
    gotify_url: String,
    gotify_token: String,
    ntfy_url: String,
    ntfy_token: String,
}

impl NotifyLayer {
    pub fn new(gotify_url: &str, gotify_token: &str, ntfy_url: &str, ntfy_token: &str) -> Self {
        Self {
            client: Client::new(),
            gotify_url: gotify_url.trim_end_matches('/').to_string(),
            gotify_token: gotify_token.to_string(),
            ntfy_url: ntfy_url.trim_end_matches('/').to_string(),
            ntfy_token: ntfy_token.to_string(),
        }
    }

    fn gotify_enabled(&self) -> bool {
        !self.gotify_url.is_empty() && !self.gotify_token.is_empty()
    }

    fn ntfy_enabled(&self) -> bool {
        !self.ntfy_url.is_empty()
    }

    fn gotify_priority(level: &Level) -> u8 {
        match *level {
            Level::ERROR => 10,
            Level::WARN => 7,
            Level::INFO => 5,
            Level::DEBUG => 2,
            Level::TRACE => 0,
        }
    }

    fn ntfy_priority(level: &Level) -> u8 {
        match *level {
            Level::ERROR => 5,
            Level::WARN => 4,
            Level::INFO => 3,
            Level::DEBUG => 2,
            Level::TRACE => 1,
        }
    }

    fn send_gotify(&self, title: &str, message: &str, priority: u8) {
        let client = self.client.clone();
        let url = format!(
            "{}/message?token={}",
            self.gotify_url, self.gotify_token
        );
        let title = title.to_string();
        let message = message.to_string();

        tokio::spawn(async move {
            if let Err(e) = client
                .post(&url)
                .form(&[
                    ("title", title.as_str()),
                    ("message", message.as_str()),
                    ("priority", &priority.to_string()),
                ])
                .send()
                .await
            {
                tracing::error!("Gotify send failed: {:#}", e);
            }
        });
    }

    fn send_ntfy(&self, title: &str, message: &str, priority: u8) {
        let client = self.client.clone();
        let url = self.ntfy_url.clone();
        let token = self.ntfy_token.clone();
        let title = title.to_string();
        let message = message.to_string();

        tokio::spawn(async move {
            let mut req = client.post(&url).header("Title", title.as_str());
            req = req.header("Priority", priority.to_string());
            if !token.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", token));
            }
            if let Err(e) = req.body(message).send().await {
                tracing::error!("Ntfy send failed: {:#}", e);
            }
        });
    }
}

impl<S> Layer<S> for NotifyLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();
        let level = metadata.level();

        if *level > Level::INFO {
            return;
        }

        let mut fields = StringVisitor::new();
        event.record(&mut fields);

        let title = format!("[{}] {}", level, metadata.target());
        let message = fields.0;

        if self.gotify_enabled() {
            let priority = Self::gotify_priority(level);
            self.send_gotify(&title, &message, priority);
        }

        if self.ntfy_enabled() {
            let priority = Self::ntfy_priority(level);
            self.send_ntfy(&title, &message, priority);
        }
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
