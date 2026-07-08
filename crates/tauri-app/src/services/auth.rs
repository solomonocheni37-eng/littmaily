use crate::error::AppError;

/// The `service` parameter acts as a namespace in the OS keychain, allowing us to
/// store separate passwords for IMAP, SMTP, CalDAV, and CardDAV under the same `account_id`.
pub fn get_password(account_id: &str, service: &str) -> Result<String, AppError> {
    let entry = keyring::Entry::new(service, account_id)
        .map_err(|e| AppError::System(format!("Keyring init error: {}", e)))?;
    entry
        .get_password()
        .map_err(|e| AppError::Auth(format!("Keychain access failed: {}", e)))
}

pub fn set_password(account_id: &str, service: &str, password: &str) -> Result<(), AppError> {
    let entry = keyring::Entry::new(service, account_id)
        .map_err(|e| AppError::System(format!("Keyring init error: {}", e)))?;
    entry
        .set_password(password)
        .map_err(|e| AppError::System(format!("Keychain write failed: {}", e)))
}

pub fn delete_password(account_id: &str, service: &str) -> Result<(), AppError> {
    let entry = keyring::Entry::new(service, account_id)
        .map_err(|e| AppError::System(format!("Keyring init error: {}", e)))?;
    let _ = entry.delete_credential();
    Ok(())
}
