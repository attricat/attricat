use async_trait::async_trait;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    message::{Mailbox, Message},
    transport::smtp::authentication::Credentials,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MailError {
    #[error("email delivery failed")]
    Delivery,
    #[error("email configuration is invalid")]
    Configuration,
}

/// Delivers account emails. Implementations must never log message contents,
/// because lifecycle URLs contain opaque secrets.
#[async_trait]
pub trait MailDelivery: Send + Sync {
    async fn deliver_password_reset(
        &self,
        recipient: &str,
        reset_url: &str,
    ) -> Result<(), MailError>;
    async fn deliver_workspace_invitation(
        &self,
        recipient: &str,
        invitation_url: &str,
    ) -> Result<(), MailError>;
    async fn deliver_workspace_onboarding(
        &self,
        recipient: &str,
        onboarding_url: &str,
    ) -> Result<(), MailError>;
}

pub struct SmtpMailDelivery {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SmtpTlsMode {
    Disabled,
    StartTls,
    ImplicitTls,
}

impl SmtpTlsMode {
    fn parse(value: &str) -> Result<Self, MailError> {
        match value {
            "disabled" => Ok(Self::Disabled),
            "starttls" => Ok(Self::StartTls),
            "implicit" => Ok(Self::ImplicitTls),
            _ => Err(MailError::Configuration),
        }
    }
}

impl SmtpMailDelivery {
    /// Builds a mail transport that requires encrypted SMTP by default.
    /// Plaintext is an explicit local-relay opt-out, never a production
    /// fallback, so credentials and lifecycle URLs are not downgraded.
    pub fn new(
        host: &str,
        port: u16,
        from: &str,
        username: Option<String>,
        password: Option<String>,
        tls_mode: &str,
    ) -> Result<Self, MailError> {
        let from = from.parse().map_err(|_| MailError::Configuration)?;
        let mode = SmtpTlsMode::parse(tls_mode)?;
        let credentials = match (username, password) {
            (Some(username), Some(password)) if mode != SmtpTlsMode::Disabled => {
                Some(Credentials::new(username, password))
            }
            (None, None) => None,
            // Never silently drop a partial credential and never permit a
            // credential over the local-only plaintext transport.
            _ => return Err(MailError::Configuration),
        };
        let mut builder = match mode {
            SmtpTlsMode::Disabled => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host),
            SmtpTlsMode::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
                .map_err(|_| MailError::Configuration)?,
            SmtpTlsMode::ImplicitTls => AsyncSmtpTransport::<Tokio1Executor>::relay(host)
                .map_err(|_| MailError::Configuration)?,
        }
        .port(port);
        if let Some(credentials) = credentials {
            builder = builder.credentials(credentials);
        }
        Ok(Self {
            transport: builder.build(),
            from,
        })
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{SmtpMailDelivery, SmtpTlsMode};

    #[test]
    fn tls_mode_rejects_downgrade_prone_values() {
        assert!(SmtpTlsMode::parse("starttls").is_ok());
        assert!(SmtpTlsMode::parse("implicit").is_ok());
        assert!(SmtpTlsMode::parse("opportunistic").is_err());
    }

    #[test]
    fn plaintext_smtp_rejects_credentials_and_partial_credentials() {
        assert!(
            SmtpMailDelivery::new(
                "localhost",
                1025,
                "Catalog <mail@example.test>",
                None,
                None,
                "disabled"
            )
            .is_ok()
        );
        assert!(
            SmtpMailDelivery::new(
                "localhost",
                1025,
                "Catalog <mail@example.test>",
                Some("user".into()),
                Some("secret".into()),
                "disabled"
            )
            .is_err()
        );
        assert!(
            SmtpMailDelivery::new(
                "localhost",
                587,
                "Catalog <mail@example.test>",
                Some("user".into()),
                None,
                "starttls"
            )
            .is_err()
        );
        assert!(
            SmtpMailDelivery::new(
                "localhost",
                587,
                "Catalog <mail@example.test>",
                Some("user".into()),
                Some("secret".into()),
                "starttls"
            )
            .is_ok()
        );
    }
}

impl SmtpMailDelivery {
    async fn deliver(
        &self,
        recipient: &str,
        subject: &str,
        instruction: &str,
        action_url: &str,
    ) -> Result<(), MailError> {
        let recipient = recipient.parse().map_err(|_| MailError::Configuration)?;
        let message = Message::builder()
            .from(self.from.clone())
            .to(recipient)
            .subject(subject)
            .body(format!("{instruction}\n\n{action_url}\n"))
            .map_err(|_| MailError::Configuration)?;
        self.transport
            .send(message)
            .await
            .map_err(|_| MailError::Delivery)?;
        Ok(())
    }
}

#[async_trait]
impl MailDelivery for SmtpMailDelivery {
    async fn deliver_password_reset(
        &self,
        recipient: &str,
        reset_url: &str,
    ) -> Result<(), MailError> {
        self.deliver(
            recipient,
            "Reset your Catalog password",
            "Use this link to reset your Catalog password:",
            reset_url,
        )
        .await
    }

    async fn deliver_workspace_invitation(
        &self,
        recipient: &str,
        invitation_url: &str,
    ) -> Result<(), MailError> {
        self.deliver(
            recipient,
            "You've been invited to Catalog",
            "Use this link to accept your workspace invitation:",
            invitation_url,
        )
        .await
    }

    async fn deliver_workspace_onboarding(
        &self,
        recipient: &str,
        onboarding_url: &str,
    ) -> Result<(), MailError> {
        self.deliver(
            recipient,
            "Set up your Catalog workspace account",
            "Use this one-time link to choose a password and join your workspace:",
            onboarding_url,
        )
        .await
    }
}
