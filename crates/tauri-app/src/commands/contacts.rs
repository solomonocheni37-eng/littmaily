use crate::error::AppError;
use crate::state::AppState;
use storage::models::ContactRecord;
use storage::repository::ContactRepository;
use tauri::State;

/// Fetches all contacts for an account across all discovered address books.
#[tauri::command]
#[specta::specta]
pub async fn get_contacts(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Vec<ContactRecord>, AppError> {
    let pool = state
        .pool
        .get()
        .ok_or_else(|| AppError::System("Database is still initializing...".into()))?;
    let contact_repo = ContactRepository::new(pool);
    let address_books = contact_repo
        .get_address_books_for_account(&account_id)
        .await?;

    let mut all_contacts = Vec::new();
    for book in address_books {
        all_contacts.extend(contact_repo.get_contacts_for_address_book(book.id).await?);
    }
    Ok(all_contacts)
}
