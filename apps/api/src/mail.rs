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

impl SmtpMailDelivery {
    pub fn new(
        host: &str,
        port: u16,
        from: &str,
        username: Option<String>,
        password: Option<String>,
    ) -> Result<Self, MailError> {
        let from = from.parse().map_err(|_| MailError::Configuration)?;
        let mut builder = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host).port(port);
        if let (Some(username), Some(password)) = (username, password) {
            builder = builder.credentials(Credentials::new(username, password));
        }
        Ok(Self {
            transport: builder.build(),
            from,
        })
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
