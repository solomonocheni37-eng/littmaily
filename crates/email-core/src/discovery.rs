use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use reqwest::Client;
use roxmltree::Document;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub enum Encryption {
    None,
    StartTls,
    Tls, // Implicit SSL/TLS
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub encryption: Encryption,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProviderConfig {
    pub imap: ServerConfig,
    pub smtp: ServerConfig,
}

/// Discovers IMAP and SMTP settings for a given email address using a 4-step fallback chain:
/// 1. Mozilla Autoconfig DB (covers major providers like Gmail, Outlook).
/// 2. Domain's own autoconfig XML (RFC 8314 / Thunderbird style).
/// 3. DNS SRV Records (RFC 6186).
/// 4. Educated guess (standard `imap.`/`smtp.` subdomains).
pub async fn discover_provider(email: &str) -> Result<ProviderConfig, String> {
    let domain = email.split('@').nth(1).ok_or("Invalid email address")?;

    // 1. Mozilla Autoconfig DB (Covers Gmail, Outlook, Yahoo, etc.)
    if let Ok(config) = try_mozilla_autoconfig(domain).await {
        return Ok(config);
    }

    // 2. Domain's own autoconfig (RFC 8314 / Thunderbird style)
    if let Ok(config) = try_domain_autoconfig(domain).await {
        return Ok(config);
    }

    // 3. DNS SRV Records (RFC 6186 / RFC 8314)
    if let Ok(config) = try_srv_records(domain).await {
        return Ok(config);
    }

    // 4. Educated Guess (Fallback)
    guess_common_subdomains(domain)
}

async fn try_mozilla_autoconfig(domain: &str) -> Result<ProviderConfig, String> {
    let url = format!("https://autoconfig.thunderbird.net/v1.1/{}", domain);
    fetch_and_parse_xml(&url).await
}

async fn try_domain_autoconfig(domain: &str) -> Result<ProviderConfig, String> {
    let urls = [
        format!("https://autoconfig.{}/mail/config-v1.1.xml", domain),
        format!(
            "https://{}/.well-known/autoconfig/mail/config-v1.1.xml",
            domain
        ),
    ];
    for url in urls {
        if let Ok(config) = fetch_and_parse_xml(&url).await {
            return Ok(config);
        }
    }
    Err("Domain autoconfig not found".into())
}

async fn fetch_and_parse_xml(url: &str) -> Result<ProviderConfig, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let text = resp.text().await.map_err(|e| e.to_string())?;
    parse_autoconfig_xml(&text)
}

fn parse_autoconfig_xml(xml: &str) -> Result<ProviderConfig, String> {
    let doc = Document::parse(xml).map_err(|e| e.to_string())?;
    let mut imap = None;
    let mut smtp = None;

    for node in doc
        .descendants()
        .filter(|n| n.has_tag_name("incomingServer"))
    {
        if node.attribute("type") == Some("imap") {
            if let Some(server) = parse_server_node(&node) {
                imap = Some(server);
                break;
            }
        }
    }

    for node in doc
        .descendants()
        .filter(|n| n.has_tag_name("outgoingServer"))
    {
        if node.attribute("type") == Some("smtp") {
            if let Some(server) = parse_server_node(&node) {
                smtp = Some(server);
                break;
            }
        }
    }

    match (imap, smtp) {
        (Some(i), Some(s)) => Ok(ProviderConfig { imap: i, smtp: s }),
        _ => Err("Missing IMAP or SMTP configuration in XML".into()),
    }
}

fn parse_server_node(node: &roxmltree::Node) -> Option<ServerConfig> {
    let host = node
        .children()
        .find(|n| n.has_tag_name("hostname"))?
        .text()?;
    let port_str = node.children().find(|n| n.has_tag_name("port"))?.text()?;
    let port = port_str.parse::<u16>().ok()?;
    let socket_type = node
        .children()
        .find(|n| n.has_tag_name("socketType"))?
        .text()?;
    let encryption = match socket_type {
        "SSL" | "TLS" => Encryption::Tls,
        "STARTTLS" => Encryption::StartTls,
        _ => Encryption::None,
    };
    Some(ServerConfig {
        host: host.to_string(),
        port,
        encryption,
    })
}

async fn try_srv_records(domain: &str) -> Result<ProviderConfig, String> {
    let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());
    let imap_srv = format!("_imaps._tcp.{}", domain);
    let smtp_srv = format!("_submission._tcp.{}", domain);
    let mut imap = None;
    let mut smtp = None;
    let dns_timeout = Duration::from_secs(3);

    if let Ok(Ok(response)) = tokio::time::timeout(dns_timeout, resolver.srv_lookup(imap_srv)).await
    {
        if let Some(record) = response.iter().next() {
            let host = record.target().to_string();
            let host = host.trim_end_matches('.').to_string();
            imap = Some(ServerConfig {
                host,
                port: record.port(),
                encryption: Encryption::Tls,
            });
        }
    }

    if let Ok(Ok(response)) = tokio::time::timeout(dns_timeout, resolver.srv_lookup(smtp_srv)).await
    {
        if let Some(record) = response.iter().next() {
            let host = record.target().to_string();
            let host = host.trim_end_matches('.').to_string();
            smtp = Some(ServerConfig {
                host,
                port: record.port(),
                encryption: Encryption::StartTls,
            });
        }
    }

    match (imap, smtp) {
        (Some(i), Some(s)) => Ok(ProviderConfig { imap: i, smtp: s }),
        _ => Err("SRV records incomplete or not found".into()),
    }
}

fn guess_common_subdomains(domain: &str) -> Result<ProviderConfig, String> {
    Ok(ProviderConfig {
        imap: ServerConfig {
            host: format!("imap.{}", domain),
            port: 993,
            encryption: Encryption::Tls,
        },
        smtp: ServerConfig {
            host: format!("smtp.{}", domain),
            port: 465,
            encryption: Encryption::Tls,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_valid_autoconfig_xml_when_parsed_then_extracts_imap_and_smtp() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<clientConfig version="1.1">
<emailProvider id="example.com">
<incomingServer type="imap">
<hostname>imap.example.com</hostname>
<port>993</port>
<socketType>SSL</socketType>
<authentication>password-cleartext</authentication>
</incomingServer>
<outgoingServer type="smtp">
<hostname>smtp.example.com</hostname>
<port>587</port>
<socketType>STARTTLS</socketType>
<authentication>password-cleartext</authentication>
</outgoingServer>
</emailProvider>
</clientConfig>"#;
        let config = parse_autoconfig_xml(xml).unwrap();
        assert_eq!(config.imap.host, "imap.example.com");
        assert_eq!(config.imap.port, 993);
        assert_eq!(config.imap.encryption, Encryption::Tls);
        assert_eq!(config.smtp.host, "smtp.example.com");
        assert_eq!(config.smtp.port, 587);
        assert_eq!(config.smtp.encryption, Encryption::StartTls);
    }

    #[test]
    fn given_missing_smtp_in_xml_when_parsed_then_returns_error() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<clientConfig version="1.1">
<emailProvider id="example.com">
<incomingServer type="imap">
<hostname>imap.example.com</hostname>
<port>993</port>
<socketType>SSL</socketType>
</incomingServer>
</emailProvider>
</clientConfig>"#;
        let result = parse_autoconfig_xml(xml);
        assert!(result.is_err(), "Should fail if SMTP is missing");
    }

    #[test]
    fn given_unknown_domain_when_guessing_then_returns_standard_subdomains() {
        let config = guess_common_subdomains("mycustomdomain.com").unwrap();
        assert_eq!(config.imap.host, "imap.mycustomdomain.com");
        assert_eq!(config.imap.port, 993);
        assert_eq!(config.imap.encryption, Encryption::Tls);
        assert_eq!(config.smtp.host, "smtp.mycustomdomain.com");
        assert_eq!(config.smtp.port, 465);
    }
}
