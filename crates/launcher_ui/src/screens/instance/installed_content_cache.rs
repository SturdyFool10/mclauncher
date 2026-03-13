use std::collections::HashMap;
use std::sync::Arc;

use content_resolver::{InstalledContentFile, InstalledContentKind};
use managed_content::InstalledContentIdentity;

#[derive(Clone, Debug, Default)]
pub(super) struct InstalledContentCache {
    pub(super) managed_identities: Option<HashMap<String, InstalledContentIdentity>>,
    pub(super) files_by_tab: HashMap<InstalledContentKind, Arc<[InstalledContentFile]>>,
}

impl InstalledContentCache {
    pub(super) fn clear(&mut self) {
        self.managed_identities = None;
        self.files_by_tab.clear();
    }
}
