/// A downloadable file on a Modrinth project version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectVersionFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
}
