/// Resolve terminal session status from current DB status + process exit code.
///
/// Shared by Claude/Grok (and any engine without bridge-error special cases).
pub fn resolve_final_session_status(
    current_status: Option<&str>,
    exit_code: Option<i32>,
) -> &'static str {
    match (current_status, exit_code) {
        (Some("stopping"), _) => "exited",
        (_, Some(0)) => "exited",
        _ => "failed",
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_final_session_status;

    #[test]
    fn keeps_successful_executions_automation_eligible() {
        assert_eq!(
            resolve_final_session_status(Some("running"), Some(0)),
            "exited"
        );
        assert_eq!(resolve_final_session_status(None, Some(0)), "exited");
    }

    #[test]
    fn preserves_stopping_as_exited_without_success() {
        assert_eq!(
            resolve_final_session_status(Some("stopping"), Some(143)),
            "exited"
        );
    }

    #[test]
    fn marks_failed_processes_failed() {
        assert_eq!(
            resolve_final_session_status(Some("running"), Some(1)),
            "failed"
        );
        assert_eq!(
            resolve_final_session_status(Some("running"), None),
            "failed"
        );
    }
}
