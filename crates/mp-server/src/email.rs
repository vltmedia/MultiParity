//! Sending email, through an SMTP server or SendGrid.
//!
//! Used for invites, "forgot password" links, requests to release a file, and telling server
//! admins a backup failed. Without email set up, the server works as before: invite links are
//! copied by hand and admins set new passwords.
//!
//! `.env`:
//!   MP_EMAIL_FROM="MultiParity <multiparity@studio.example>"
//!   SMTP:     MP_SMTP_HOST, MP_SMTP_PORT (587), MP_SMTP_USER, MP_SMTP_PASSWORD,
//!             MP_SMTP_SECURITY = starttls (default) | tls | none
//!   SendGrid: MP_SENDGRID_API_KEY (MP_SENDGRID_URL to point elsewhere, for tests)
//!   MP_EMAIL = smtp | sendgrid | off picks one when both are set.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use lettre::message::{header::ContentType, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde_json::json;

#[derive(Clone)]
pub enum Mailer {
    Smtp { transport: AsyncSmtpTransport<Tokio1Executor>, describe: String },
    SendGrid { key: String, url: String, http: reqwest::Client },
}

#[derive(Clone)]
pub struct Email {
    pub mailer: Mailer,
    pub from: Mailbox,
}

impl std::fmt::Debug for Email {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Email({})", self.describe())
    }
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

impl Email {
    /// From the environment; `None` when email is not set up.
    pub fn from_env() -> Result<Option<Email>> {
        let pick = env("MP_EMAIL").map(|s| s.to_lowercase());
        if pick.as_deref() == Some("off") {
            return Ok(None);
        }
        let smtp = env("MP_SMTP_HOST");
        let sendgrid = env("MP_SENDGRID_API_KEY");
        let kind = match (pick.as_deref(), &smtp, &sendgrid) {
            (Some("smtp"), _, _) => "smtp",
            (Some("sendgrid"), _, _) => "sendgrid",
            (Some(other), _, _) => bail!("MP_EMAIL={other}: use smtp, sendgrid or off"),
            (None, Some(_), _) => "smtp",
            (None, None, Some(_)) => "sendgrid",
            (None, None, None) => return Ok(None),
        };
        let from: Mailbox = env("MP_EMAIL_FROM")
            .context("MP_EMAIL_FROM is required to send email, e.g. \"MultiParity <multiparity@studio.example>\"")?
            .parse()
            .context("MP_EMAIL_FROM is not an email address")?;
        let mailer = if kind == "smtp" {
            let host = smtp.context("MP_SMTP_HOST is required for MP_EMAIL=smtp")?;
            let security = env("MP_SMTP_SECURITY").unwrap_or_else(|| "starttls".into()).to_lowercase();
            let port: u16 = env("MP_SMTP_PORT").map(|p| p.parse()).transpose().context("MP_SMTP_PORT is not a port number")?.unwrap_or(match security.as_str() {
                "tls" => 465,
                "none" => 25,
                _ => 587,
            });
            let tls = match security.as_str() {
                "starttls" => Tls::Required(TlsParameters::new(host.clone())?),
                "tls" => Tls::Wrapper(TlsParameters::new(host.clone())?),
                "none" => Tls::None,
                other => bail!("MP_SMTP_SECURITY={other}: use starttls, tls or none"),
            };
            let mut b = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&host).port(port).tls(tls).timeout(Some(Duration::from_secs(20)));
            if let Some(user) = env("MP_SMTP_USER") {
                b = b.credentials(Credentials::new(user, env("MP_SMTP_PASSWORD").unwrap_or_default()));
            }
            Mailer::Smtp { transport: b.build(), describe: format!("SMTP {host}:{port} ({security})") }
        } else {
            Mailer::SendGrid {
                key: sendgrid.context("MP_SENDGRID_API_KEY is required for MP_EMAIL=sendgrid")?,
                url: env("MP_SENDGRID_URL").unwrap_or_else(|| "https://api.sendgrid.com".into()).trim_end_matches('/').to_string(),
                http: reqwest::Client::builder().timeout(Duration::from_secs(20)).build()?,
            }
        };
        Ok(Some(Email { mailer, from }))
    }

    pub fn describe(&self) -> String {
        match &self.mailer {
            Mailer::Smtp { describe, .. } => describe.clone(),
            Mailer::SendGrid { .. } => "SendGrid".into(),
        }
    }

    pub fn kind(&self) -> &'static str {
        match &self.mailer {
            Mailer::Smtp { .. } => "smtp",
            Mailer::SendGrid { .. } => "sendgrid",
        }
    }

    /// Send one message, plain text with a simple HTML copy.
    pub async fn send(&self, to: &str, subject: &str, text: &str) -> Result<()> {
        let html = to_html(text);
        match &self.mailer {
            Mailer::Smtp { transport, .. } => {
                let to: Mailbox = to.parse().with_context(|| format!("{to} is not an email address"))?;
                let msg = Message::builder()
                    .from(self.from.clone())
                    .to(to)
                    .subject(subject)
                    .multipart(MultiPart::alternative().singlepart(SinglePart::builder().header(ContentType::TEXT_PLAIN).body(text.to_string())).singlepart(SinglePart::builder().header(ContentType::TEXT_HTML).body(html)))?;
                transport.send(msg).await.context("the mail server refused the message")?;
            }
            Mailer::SendGrid { key, url, http } => {
                let from = json!({ "email": self.from.email.to_string(), "name": self.from.name.clone().unwrap_or_else(|| "MultiParity".into()) });
                let body = json!({
                    "personalizations": [{ "to": [{ "email": to }] }],
                    "from": from,
                    "subject": subject,
                    "content": [{ "type": "text/plain", "value": text }, { "type": "text/html", "value": html }],
                });
                let r = http.post(format!("{url}/v3/mail/send")).bearer_auth(key).json(&body).send().await.context("could not reach SendGrid")?;
                if !r.status().is_success() {
                    let status = r.status();
                    let detail = r.text().await.unwrap_or_default();
                    bail!("SendGrid answered {status}: {}", detail.chars().take(300).collect::<String>());
                }
            }
        }
        Ok(())
    }

    /// Send without waiting; failures are logged.
    pub fn send_later(&self, to: String, subject: String, text: String) {
        let me = self.clone();
        tokio::spawn(async move {
            if let Err(e) = me.send(&to, &subject, &text).await {
                tracing::warn!(to = %to, error = %format!("{e:#}"), "email not sent");
            }
        });
    }
}

/// Plain text to minimal HTML: paragraphs, and links made clickable.
fn to_html(text: &str) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let paras: Vec<String> = text
        .split("\n\n")
        .map(|p| {
            let words: Vec<String> = p
                .split(' ')
                .map(|w| if w.starts_with("https://") || w.starts_with("http://") { format!("<a href=\"{0}\">{0}</a>", esc(w)) } else { esc(w) })
                .collect();
            format!("<p>{}</p>", words.join(" ").replace('\n', "<br>"))
        })
        .collect();
    format!("<div style=\"font-family:-apple-system,Segoe UI,sans-serif;font-size:15px;line-height:1.5;color:#1b1324;max-width:560px\">{}</div>", paras.join(""))
}

#[cfg(test)]
mod tests {
    #[test]
    fn html_links_and_escapes() {
        let h = super::to_html("Hi <you>\n\nOpen https://mp.example/x?a=1&b=2 now");
        assert!(h.contains("Hi &lt;you&gt;"));
        assert!(h.contains("<a href=\"https://mp.example/x?a=1&amp;b=2\">"));
    }
}
