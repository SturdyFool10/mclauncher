use installation::GameSetupResult;

use super::RuntimePrepareOperation;

#[derive(Clone, Debug)]
pub(super) struct RuntimePrepareOutcome {
    pub(super) operation: RuntimePrepareOperation,
    pub(super) setup: GameSetupResult,
    pub(super) configured_java: Option<(u8, String)>,
    pub(super) launch: Option<installation::LaunchResult>,
}
