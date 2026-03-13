#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VtmpackProviderMode {
    IncludeCurseForge,
    ExcludeCurseForge,
}

impl VtmpackProviderMode {
    pub fn label(self) -> &'static str {
        match self {
            VtmpackProviderMode::IncludeCurseForge => "Allow CurseForge project IDs",
            VtmpackProviderMode::ExcludeCurseForge => "Exclude CurseForge as a provider",
        }
    }
}
