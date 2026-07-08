use crate::discovery::Encryption;
use lettre::{
    Address, AsyncSmtpTransport, AsyncTransport, Tokio1Executor, address::Envelope,
    transport::smtp::authentication::{Credentials, Mechanism},
};

pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub encryption: Encryption,
}

/// Sends a pre-built raw MIME message via SMTP using Lettre's async transport.
/// Automatically configures TLS/STARTTLS based on the discovered `Encryption` type.
pub async fn send_raw_mime(
    config: &SmtpConfig,
    envelope_from: &str,
    envelope_to: &[String],
    raw_mime: &[u8],
) -> Result<(), String> {
    let mut to_addresses = Vec::new();
    for to in envelope_to {
        let addr = to
            .parse::<Address>()
            .map_err(|e| format!("Invalid to address: {}", e))?;
        to_addresses.push(addr);
    }
    let from_addr = envelope_from
        .parse::<Address>()
        .map_err(|e| format!("Invalid from address: {}", e))?;
    let envelope = Envelope::new(Some(from_addr), to_addresses)
        .map_err(|e| format!("Invalid envelope: {}", e))?;

    let creds = Credentials::new(config.username.clone(), config.password.clone());

    // Lettre 0.11 uses specific relay builders that automatically configure the TLS handshake
    // correctly for Implicit TLS (465) vs STARTTLS (587).
    let builder = match config.encryption {
        Encryption::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
            .map_err(|e| format!("Failed to create TLS relay: {}", e))?,
        Encryption::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            .map_err(|e| format!("Failed to create STARTTLS relay: {}", e))?,
        Encryption::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host),
    };

    let transport = builder.port(config.port).credentials(creds).build();
    transport
        .send_raw(&envelope, raw_mime)
        .await
        .map_err(|e| format!("SMTP send failed: {}", e))?;
    Ok(())
}

/// Sends a raw MIME message via SMTP using the XOAUTH2 SASL mechanism.
/// Lettre natively handles the Base64 encoding and SASL challenge-response formatting.
pub async fn send_raw_mime_xoauth2(
    host: &str,
    port: u16,
    email: &str,
    access_token: &str,
    envelope_from: &str,
    envelope_to: &[String],
    raw_mime: &[u8],
) -> Result<(), String> {
    let mut to_addresses = Vec::new();
    for to in envelope_to {
        let addr = to
            .parse::<Address>()
            .map_err(|e| format!("Invalid to address: {}", e))?;
        to_addresses.push(addr);
    }
    let from_addr = envelope_from
        .parse::<Address>()
        .map_err(|e| format!("Invalid from address: {}", e))?;
    let envelope = Envelope::new(Some(from_addr), to_addresses)
        .map_err(|e| format!("Invalid envelope: {}", e))?;

    let creds = Credentials::new(email.to_string(), access_token.to_string());
    let transport = AsyncSmtpTransport::<Tokio1Executor>::relay(host)
        .map_err(|e| format!("Failed to create TLS relay: {}", e))?
        .port(port)
        .credentials(creds)
        .authentication(vec![Mechanism::Xoauth2]) // Instructs Lettre to use the XOAUTH2 SASL mechanism
        .build();

    transport
        .send_raw(&envelope, raw_mime)
        .await
        .map_err(|e| format!("SMTP XOAUTH2 send failed: {}", e))?;
    Ok(())
}
