use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::core::server::{ContentKind, ServerType};
use crate::error::Result;
use crate::sources::{PackageSource, SourceId};

const DEFAULT_BASE_URL: &str = "https://api.modrinth.com/v2";

#[derive(Debug, Clone)]
pub struct ModrinthClient {
    http: Client,
    base_url: String,
    #[allow(dead_code)]
    loader_cache: Arc<Mutex<Option<Vec<Tag>>>>,
    #[allow(dead_code)]
    project_type_cache: Arc<Mutex<Option<Vec<Tag>>>>,
}

impl ModrinthClient {
    pub fn new() -> Result<Self> {
        let http = Client::builder()
            .user_agent(format!(
                "minecli/{} (github.com/vincentsaluzzo/mine-cli)",
                env!("CARGO_PKG_VERSION")
            ))
            .build()?;
        let base_url = std::env::var("MINECLI_MODRINTH_BASE_URL")
            .unwrap_or_else(|_| DEFAULT_BASE_URL.to_owned());
        Ok(Self {
            http,
            base_url,
            loader_cache: Arc::new(Mutex::new(None)),
            project_type_cache: Arc::new(Mutex::new(None)),
        })
    }

    pub fn http_client(&self) -> &Client {
        &self.http
    }

    pub fn search(&self, params: &SearchParams) -> Result<SearchResponse> {
        let mut facets = Vec::new();
        if let Some(version) = &params.minecraft_version {
            facets.push(vec![format!("versions:{version}")]);
        }
        if let Some(kind) = params.kind {
            facets.push(vec![format!("project_type:{kind}")]);
        }
        if let Some(loader) = &params.loader {
            facets.push(vec![format!("categories:{loader}")]);
        }
        if params.server_side_only {
            facets.push(vec![
                "server_side:required".to_owned(),
                "server_side:optional".to_owned(),
            ]);
        }

        let facets_json = serde_json::to_string(&facets)?;
        let limit = params.limit.to_string();
        let mut request = self
            .http
            .get(format!("{}/search", self.base_url))
            .query(&[("query", params.query.as_str()), ("limit", limit.as_str())]);

        if !facets.is_empty() {
            request = request.query(&[("facets", facets_json.as_str())]);
        }

        Ok(request.send()?.error_for_status()?.json()?)
    }

    pub fn get_project(&self, project: &str) -> Result<Project> {
        Ok(self
            .http
            .get(format!("{}/project/{project}", self.base_url))
            .send()?
            .error_for_status()?
            .json()?)
    }

    pub fn get_project_versions(
        &self,
        project: &str,
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<Vec<ProjectVersion>> {
        let loaders_json = serde_json::to_string(loaders)?;
        let versions_json = serde_json::to_string(game_versions)?;
        let mut request = self
            .http
            .get(format!("{}/project/{project}/version", self.base_url));

        if !loaders.is_empty() {
            request = request.query(&[("loaders", loaders_json.as_str())]);
        }
        if !game_versions.is_empty() {
            request = request.query(&[("game_versions", versions_json.as_str())]);
        }

        Ok(request.send()?.error_for_status()?.json()?)
    }

    pub fn get_version(&self, version_id: &str) -> Result<ProjectVersion> {
        Ok(self
            .http
            .get(format!("{}/version/{version_id}", self.base_url))
            .send()?
            .error_for_status()?
            .json()?)
    }

    pub fn get_version_from_hash(
        &self,
        hash: &str,
        algorithm: &str,
    ) -> Result<Option<ProjectVersion>> {
        let response = self
            .http
            .get(format!("{}/version_file/{hash}", self.base_url))
            .query(&[("algorithm", algorithm)])
            .send()?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        Ok(Some(response.error_for_status()?.json()?))
    }

    #[allow(dead_code)]
    pub fn list_loaders(&self) -> Result<Vec<Tag>> {
        if let Some(tags) = self.loader_cache.lock().expect("loader cache").clone() {
            return Ok(tags);
        }

        let tags: Vec<Tag> = self
            .http
            .get(format!("{}/tag/loader", self.base_url))
            .send()?
            .error_for_status()?
            .json()?;
        *self.loader_cache.lock().expect("loader cache") = Some(tags.clone());
        Ok(tags)
    }

    #[allow(dead_code)]
    pub fn list_project_types(&self) -> Result<Vec<Tag>> {
        if let Some(tags) = self
            .project_type_cache
            .lock()
            .expect("project type cache")
            .clone()
        {
            return Ok(tags);
        }

        let tags: Vec<Tag> = self
            .http
            .get(format!("{}/tag/project_type", self.base_url))
            .send()?
            .error_for_status()?
            .json()?;
        *self.project_type_cache.lock().expect("project type cache") = Some(tags.clone());
        Ok(tags)
    }
}

impl PackageSource for ModrinthClient {
    fn source_id(&self) -> SourceId {
        SourceId::Modrinth
    }
}

pub trait ProjectSource {
    fn get_project(&self, project: &str) -> Result<Project>;

    fn get_project_versions(
        &self,
        project: &str,
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<Vec<ProjectVersion>>;

    fn get_version(&self, version_id: &str) -> Result<ProjectVersion>;

    fn get_version_from_hash(&self, hash: &str, algorithm: &str) -> Result<Option<ProjectVersion>>;
}

impl ProjectSource for ModrinthClient {
    fn get_project(&self, project: &str) -> Result<Project> {
        self.get_project(project)
    }

    fn get_project_versions(
        &self,
        project: &str,
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<Vec<ProjectVersion>> {
        self.get_project_versions(project, loaders, game_versions)
    }

    fn get_version(&self, version_id: &str) -> Result<ProjectVersion> {
        self.get_version(version_id)
    }

    fn get_version_from_hash(&self, hash: &str, algorithm: &str) -> Result<Option<ProjectVersion>> {
        self.get_version_from_hash(hash, algorithm)
    }
}

#[derive(Debug, Clone)]
pub struct SearchParams {
    pub query: String,
    pub minecraft_version: Option<String>,
    pub loader: Option<String>,
    pub kind: Option<ContentKind>,
    pub server_side_only: bool,
    pub limit: usize,
}

impl SearchParams {
    pub fn for_server(
        query: String,
        minecraft_version: Option<String>,
        server_type: Option<ServerType>,
        kind: Option<ContentKind>,
        limit: usize,
        server_side_only: bool,
    ) -> Self {
        let loader = match kind {
            Some(kind) => server_type
                .and_then(|server_type| server_type.modrinth_loader(kind))
                .map(ToOwned::to_owned),
            None => server_type.and_then(|server_type| match server_type {
                ServerType::Vanilla | ServerType::Unknown => None,
                ServerType::Purpur | ServerType::Folia => Some("paper".to_owned()),
                ServerType::Bukkit => Some("spigot".to_owned()),
                server_type => Some(server_type.as_str().to_owned()),
            }),
        };

        Self {
            query,
            minecraft_version,
            loader,
            kind,
            server_side_only,
            limit,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponse {
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub project_type: String,
    pub downloads: u64,
    pub server_side: String,
    #[serde(default)]
    pub client_side: String,
    #[serde(default)]
    pub versions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub project_type: String,
    pub server_side: String,
    #[serde(default)]
    pub client_side: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectVersion {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    #[serde(default)]
    pub changelog: Option<String>,
    pub version_type: ReleaseChannel,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<VersionDependency>,
    #[serde(default)]
    pub files: Vec<ModrinthFile>,
}

impl ProjectVersion {
    pub fn primary_file(&self) -> Option<&ModrinthFile> {
        self.files
            .iter()
            .find(|file| file.primary)
            .or_else(|| self.files.first())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionDependency {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    pub dependency_type: DependencyType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModrinthFile {
    #[serde(default)]
    pub hashes: BTreeMap<String, String>,
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum ReleaseChannel {
    Release,
    Beta,
    Alpha,
}

impl ReleaseChannel {
    pub fn allows(self, version_type: Self) -> bool {
        match self {
            Self::Release => version_type == Self::Release,
            Self::Beta => matches!(version_type, Self::Release | Self::Beta),
            Self::Alpha => true,
        }
    }
}

impl std::fmt::Display for ReleaseChannel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Release => formatter.write_str("release"),
            Self::Beta => formatter.write_str("beta"),
            Self::Alpha => formatter.write_str("alpha"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Tag {
    pub name: String,
}

pub fn select_version<'a>(
    versions: &'a [ProjectVersion],
    requested_version: Option<&str>,
    channel: ReleaseChannel,
) -> Option<&'a ProjectVersion> {
    if let Some(requested_version) = requested_version {
        return versions.iter().find(|version| {
            version.id == requested_version || version.version_number == requested_version
        });
    }

    versions
        .iter()
        .find(|version| channel.allows(version.version_type) && version.primary_file().is_some())
}

pub fn version_matches_server(
    version: &ProjectVersion,
    minecraft_version: &str,
    loader: Option<&str>,
) -> bool {
    version
        .game_versions
        .iter()
        .any(|version| version == minecraft_version)
        && loader.is_none_or(|loader| version.loaders.iter().any(|item| item == loader))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::core::server::{ContentKind, ServerType};
    use crate::sources::modrinth::{
        ModrinthFile, ProjectVersion, ReleaseChannel, SearchParams, SearchResponse, select_version,
        version_matches_server,
    };

    fn version(id: &str, version_number: &str, version_type: ReleaseChannel) -> ProjectVersion {
        ProjectVersion {
            id: id.to_owned(),
            project_id: "project".to_owned(),
            name: version_number.to_owned(),
            version_number: version_number.to_owned(),
            changelog: None,
            version_type,
            game_versions: vec!["1.21.5".to_owned()],
            loaders: vec!["fabric".to_owned()],
            dependencies: vec![],
            files: vec![ModrinthFile {
                hashes: BTreeMap::new(),
                url: "https://example.com/file.jar".to_owned(),
                filename: "file.jar".to_owned(),
                primary: true,
                size: 1,
            }],
        }
    }

    #[test]
    fn selects_latest_allowed_release_channel() {
        let versions = vec![
            version("alpha", "3.0.0", ReleaseChannel::Alpha),
            version("release", "2.0.0", ReleaseChannel::Release),
            version("old", "1.0.0", ReleaseChannel::Release),
        ];

        let selected = select_version(&versions, None, ReleaseChannel::Release).unwrap();

        assert_eq!(selected.id, "release");
    }

    #[test]
    fn selects_explicit_version_number() {
        let versions = vec![version("id-1", "1.0.0", ReleaseChannel::Release)];

        let selected = select_version(&versions, Some("1.0.0"), ReleaseChannel::Release).unwrap();

        assert_eq!(selected.id, "id-1");
    }

    #[test]
    fn checks_version_compatibility() {
        let selected = version("id-1", "1.0.0", ReleaseChannel::Release);

        assert!(version_matches_server(&selected, "1.21.5", Some("fabric")));
        assert!(!version_matches_server(&selected, "1.20.1", Some("fabric")));
        assert!(!version_matches_server(&selected, "1.21.5", Some("forge")));
    }

    #[test]
    fn builds_search_params_from_server_context() {
        let params = SearchParams::for_server(
            "voice chat".to_owned(),
            Some("1.21.5".to_owned()),
            Some(ServerType::Fabric),
            Some(ContentKind::Mod),
            5,
            true,
        );

        assert_eq!(params.minecraft_version.as_deref(), Some("1.21.5"));
        assert_eq!(params.loader.as_deref(), Some("fabric"));
        assert_eq!(params.kind, Some(ContentKind::Mod));
        assert!(params.server_side_only);
        assert_eq!(params.limit, 5);
    }

    #[test]
    fn search_params_can_include_non_server_side_projects() {
        let params = SearchParams::for_server(
            "map".to_owned(),
            Some("1.21.5".to_owned()),
            Some(ServerType::Purpur),
            None,
            5,
            false,
        );

        assert_eq!(params.loader.as_deref(), Some("paper"));
        assert!(!params.server_side_only);
    }

    #[test]
    fn parses_search_response_fixture() {
        let fixture = include_str!("../../tests/fixtures/modrinth/search_response.json");

        let response: SearchResponse = serde_json::from_str(fixture).unwrap();

        assert_eq!(response.hits.len(), 1);
        assert_eq!(response.hits[0].slug, "fabric-api");
    }
}
