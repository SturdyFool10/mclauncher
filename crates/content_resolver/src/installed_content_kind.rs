#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InstalledContentKind {
    Mods,
    ResourcePacks,
    ShaderPacks,
    DataPacks,
}

impl InstalledContentKind {
    pub const ALL: [InstalledContentKind; 4] = [
        InstalledContentKind::Mods,
        InstalledContentKind::ResourcePacks,
        InstalledContentKind::ShaderPacks,
        InstalledContentKind::DataPacks,
    ];

    pub fn label(self) -> &'static str {
        match self {
            InstalledContentKind::Mods => "Mods",
            InstalledContentKind::ResourcePacks => "Resource Packs",
            InstalledContentKind::ShaderPacks => "Shader Packs",
            InstalledContentKind::DataPacks => "Data Packs",
        }
    }

    pub fn folder_name(self) -> &'static str {
        match self {
            InstalledContentKind::Mods => "mods",
            InstalledContentKind::ResourcePacks => "resourcepacks",
            InstalledContentKind::ShaderPacks => "shaderpacks",
            InstalledContentKind::DataPacks => "datapacks",
        }
    }

    pub fn content_type_key(self) -> &'static str {
        match self {
            InstalledContentKind::Mods => "mod",
            InstalledContentKind::ResourcePacks => "resource pack",
            InstalledContentKind::ShaderPacks => "shader",
            InstalledContentKind::DataPacks => "data pack",
        }
    }

    pub fn modrinth_project_type(self) -> &'static str {
        match self {
            InstalledContentKind::Mods => "mod",
            InstalledContentKind::ResourcePacks => "resourcepack",
            InstalledContentKind::ShaderPacks => "shader",
            InstalledContentKind::DataPacks => "datapack",
        }
    }
}
