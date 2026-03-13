#[derive(Debug, Clone, Copy)]
pub struct VtmpackExportStats {
    pub bundled_mod_files: usize,
    pub config_files: usize,
    pub additional_files: usize,
}
