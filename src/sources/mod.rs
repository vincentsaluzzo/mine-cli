use serde::{Deserialize, Serialize};

pub mod modrinth;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceId {
    Modrinth,
    ModrinthPack,
    Hangar,
    CurseForge,
    LocalFile,
    LocalFolder,
}

impl SourceId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::ModrinthPack => "modrinth-pack",
            Self::Hangar => "hangar",
            Self::CurseForge => "curseforge",
            Self::LocalFile => "local-file",
            Self::LocalFolder => "local-folder",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "modrinth" => Some(Self::Modrinth),
            "modrinth-pack" => Some(Self::ModrinthPack),
            "hangar" => Some(Self::Hangar),
            "curseforge" => Some(Self::CurseForge),
            "local-file" => Some(Self::LocalFile),
            "local-folder" => Some(Self::LocalFolder),
            _ => None,
        }
    }

    pub fn priority(self) -> u8 {
        match self {
            Self::Modrinth => 10,
            Self::ModrinthPack => 15,
            Self::Hangar => 20,
            Self::CurseForge => 30,
            Self::LocalFile | Self::LocalFolder => 90,
        }
    }

    pub fn is_registry(self) -> bool {
        matches!(self, Self::Modrinth | Self::Hangar | Self::CurseForge)
    }
}

impl std::fmt::Display for SourceId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[allow(dead_code)]
pub trait PackageSource {
    fn source_id(&self) -> SourceId;

    fn priority(&self) -> u8 {
        self.source_id().priority()
    }
}

pub fn source_priority(source: &str) -> u8 {
    SourceId::parse(source)
        .map(SourceId::priority)
        .unwrap_or(u8::MAX)
}

pub fn is_registry_source(source: &str) -> bool {
    SourceId::parse(source).is_some_and(SourceId::is_registry)
}

pub fn source_identity(source: &str, project_id: &str) -> String {
    format!("{source}:{project_id}")
}

#[allow(dead_code)]
pub fn sort_sources_by_priority(sources: &mut [SourceId]) {
    sources.sort_by_key(|source| source.priority());
}

#[cfg(test)]
mod tests {
    use crate::sources::{SourceId, sort_sources_by_priority, source_identity};

    #[test]
    fn source_priority_prefers_modrinth_then_hangar_then_curseforge() {
        let mut sources = vec![SourceId::CurseForge, SourceId::Hangar, SourceId::Modrinth];

        sort_sources_by_priority(&mut sources);

        assert_eq!(
            sources,
            vec![SourceId::Modrinth, SourceId::Hangar, SourceId::CurseForge]
        );
    }

    #[test]
    fn source_identity_namespaces_project_ids() {
        assert_eq!(
            source_identity("modrinth", "abc"),
            "modrinth:abc".to_owned()
        );
    }
}
