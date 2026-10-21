// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::config::Smtp;

/// Whether this build can send email, and [smtp] says where through.
pub fn available(smtp: &Smtp) -> bool {
    cfg!(feature = "smtp") && smtp.relay_host().is_some()
}

/// Close enough to tell a typo from an address; the mail server has the final say.
pub fn plausible(address: &str) -> bool {
    let address = address.trim();

    address.len() <= 254
        && !address.chars().any(|c| c.is_whitespace() || c.is_control())
        && address.rsplit_once('@').is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'))
}

#[cfg(feature = "smtp")]
pub async fn send(smtp: &Smtp, to: &str, subject: &str, body: String) -> anyhow::Result<()> {
    use std::time::Duration;

    use anyhow::Context;
    use lettre::message::Mailbox;
    use lettre::message::header::ContentType;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    use crate::config::SmtpSecurity;

    let host = smtp.relay_host().context("email isn't set up; fill in [smtp]")?;
    let from: Mailbox =
        smtp.from.as_deref().unwrap_or_default().trim().parse().context("[smtp] `from` isn't an address")?;
    let to: Mailbox = to.trim().parse().map_err(|_| anyhow::anyhow!("{:?} isn't an email address", to.trim()))?;

    let message = Message::builder().from(from).to(to).subject(subject).header(ContentType::TEXT_PLAIN).body(body)?;

    let mut transport = match smtp.security {
        SmtpSecurity::Starttls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?,
        SmtpSecurity::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(host)?,
        SmtpSecurity::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host),
    }
    .port(smtp.relay_port())
    .timeout(Some(Duration::from_secs(20)));

    if let Some(username) = smtp.username.as_deref().filter(|u| !u.is_empty()) {
        transport = transport.credentials(Credentials::new(username.into(), smtp.password.clone().unwrap_or_default()));
    }

    transport.build().send(message).await.with_context(|| format!("can't send email through {host}"))?;
    Ok(())
}

#[cfg(not(feature = "smtp"))]
pub async fn send(_: &Smtp, _: &str, _: &str, _: String) -> anyhow::Result<()> {
    anyhow::bail!("this tinystream was built without email; rebuild it with the `smtp` feature")
}
