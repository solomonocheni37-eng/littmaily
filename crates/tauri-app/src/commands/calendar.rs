use crate::error::AppError;
use crate::state::AppState;
use storage::models::CalendarEventRecord;
use storage::repository::CalendarRepository;
use tauri::State;

/// Fetches all calendar events for an account across all discovered calendars.
#[tauri::command]
#[specta::specta]
pub async fn get_calendar_events(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Vec<CalendarEventRecord>, AppError> {
    let pool = state
        .pool
        .get()
        .ok_or_else(|| AppError::System("Database is still initializing...".into()))?;
    let cal_repo = CalendarRepository::new(pool);
    let calendars = cal_repo.get_calendars_for_account(&account_id).await?;

    let mut all_events = Vec::new();
    for cal in calendars {
        all_events.extend(cal_repo.get_events_for_calendar(cal.id).await?);
    }
    Ok(all_events)
}
