//! Fallback for platforms without an implementation (e.g. macOS): every action
//! is logged and rejected, nothing panics.

use crate::{Action, ActionError, ActionExecutor, log_outcome};

#[derive(Debug, Default)]
pub struct UnsupportedExecutor;

impl ActionExecutor for UnsupportedExecutor {
    fn execute(&self, action: &Action) -> Result<(), ActionError> {
        let result = Err(ActionError::Unsupported {
            action: action.kind(),
            platform: std::env::consts::OS,
        });
        log_outcome(action, &result);
        result
    }
}
