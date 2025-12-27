use super::tabs::CommandResult;

#[tauri::command]
pub fn frontend_ready() -> CommandResult<()> {
    tracing::info!("Frontend ready");
    CommandResult::ok(())
}
