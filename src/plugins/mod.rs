use crate::config::Config;
use crate::errors::Error::PluginNotInstalled;
use crate::git::Git;
use crate::plugins::asdf_plugin::AsdfPlugin;
use crate::plugins::vfox_plugin::VfoxPlugin;
use crate::toolset::install_state;
use crate::ui::multi_progress_report::MultiProgressReport;
use crate::ui::progress_report::SingleReport;
use async_trait::async_trait;
use clap::Command;
use eyre::{Result, eyre};
use regex::Regex;
pub use script_manager::{Script, ScriptManager};
use std::path::{Path, PathBuf};
use std::sync::LazyLock as Lazy;
use std::vec;
use std::{
    fmt::{Debug, Display},
    sync::Arc,
};

pub mod asdf_plugin;
pub mod core;
pub mod mise_plugin_toml;
pub mod script_manager;
pub mod vfox_plugin;

#[derive(Debug, Clone, Copy, PartialEq, strum::EnumString, strum::Display)]
pub enum PluginType {
    Asdf,
    Vfox,
    VfoxBackend,
}

/// Represents where a plugin comes from (remote URL or local filesystem path)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginLocation {
    /// Remote plugin specified by URL
    Remote(String),
    /// Local plugin specified by filesystem path
    Local(PathBuf),
}

impl PluginLocation {
    /// Returns true if this is a local plugin
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local(_))
    }

    /// Returns true if this is a remote plugin
    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Remote(_))
    }

    /// Returns the URL if this is a remote plugin
    pub fn as_remote(&self) -> Option<&str> {
        match self {
            Self::Remote(url) => Some(url.as_str()),
            Self::Local(_) => None,
        }
    }

    /// Returns the path if this is a local plugin
    pub fn as_local(&self) -> Option<&Path> {
        match self {
            Self::Local(path) => Some(path.as_path()),
            Self::Remote(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum PluginEnum {
    Asdf(Arc<AsdfPlugin>),
    Vfox(Arc<VfoxPlugin>),
    VfoxBackend(Arc<VfoxPlugin>),
}

impl PluginEnum {
    pub fn name(&self) -> &str {
        match self {
            PluginEnum::Asdf(plugin) => plugin.name(),
            PluginEnum::Vfox(plugin) => plugin.name(),
            PluginEnum::VfoxBackend(plugin) => plugin.name(),
        }
    }

    pub fn path(&self) -> PathBuf {
        match self {
            PluginEnum::Asdf(plugin) => plugin.path(),
            PluginEnum::Vfox(plugin) => plugin.path(),
            PluginEnum::VfoxBackend(plugin) => plugin.path(),
        }
    }

    pub fn get_plugin_type(&self) -> PluginType {
        match self {
            PluginEnum::Asdf(_) => PluginType::Asdf,
            PluginEnum::Vfox(_) => PluginType::Vfox,
            PluginEnum::VfoxBackend(_) => PluginType::VfoxBackend,
        }
    }

    pub fn get_remote_url(&self) -> eyre::Result<Option<String>> {
        match self {
            PluginEnum::Asdf(plugin) => plugin.get_remote_url(),
            PluginEnum::Vfox(plugin) => plugin.get_remote_url(),
            PluginEnum::VfoxBackend(plugin) => plugin.get_remote_url(),
        }
    }

    pub fn set_remote_url(&self, url: String) {
        match self {
            PluginEnum::Asdf(plugin) => plugin.set_remote_url(url),
            PluginEnum::Vfox(plugin) => plugin.set_remote_url(url),
            PluginEnum::VfoxBackend(plugin) => plugin.set_remote_url(url),
        }
    }

    pub fn current_abbrev_ref(&self) -> eyre::Result<Option<String>> {
        match self {
            PluginEnum::Asdf(plugin) => plugin.current_abbrev_ref(),
            PluginEnum::Vfox(plugin) => plugin.current_abbrev_ref(),
            PluginEnum::VfoxBackend(plugin) => plugin.current_abbrev_ref(),
        }
    }

    pub fn current_sha_short(&self) -> eyre::Result<Option<String>> {
        match self {
            PluginEnum::Asdf(plugin) => plugin.current_sha_short(),
            PluginEnum::Vfox(plugin) => plugin.current_sha_short(),
            PluginEnum::VfoxBackend(plugin) => plugin.current_sha_short(),
        }
    }

    pub fn external_commands(&self) -> eyre::Result<Vec<Command>> {
        match self {
            PluginEnum::Asdf(plugin) => plugin.external_commands(),
            PluginEnum::Vfox(plugin) => plugin.external_commands(),
            PluginEnum::VfoxBackend(plugin) => plugin.external_commands(),
        }
    }

    pub fn execute_external_command(&self, command: &str, args: Vec<String>) -> eyre::Result<()> {
        match self {
            PluginEnum::Asdf(plugin) => plugin.execute_external_command(command, args),
            PluginEnum::Vfox(plugin) => plugin.execute_external_command(command, args),
            PluginEnum::VfoxBackend(plugin) => plugin.execute_external_command(command, args),
        }
    }

    pub async fn update(&self, pr: &dyn SingleReport, gitref: Option<String>) -> eyre::Result<()> {
        match self {
            PluginEnum::Asdf(plugin) => plugin.update(pr, gitref).await,
            PluginEnum::Vfox(plugin) => plugin.update(pr, gitref).await,
            PluginEnum::VfoxBackend(plugin) => plugin.update(pr, gitref).await,
        }
    }

    pub async fn uninstall(&self, pr: &dyn SingleReport) -> eyre::Result<()> {
        match self {
            PluginEnum::Asdf(plugin) => plugin.uninstall(pr).await,
            PluginEnum::Vfox(plugin) => plugin.uninstall(pr).await,
            PluginEnum::VfoxBackend(plugin) => plugin.uninstall(pr).await,
        }
    }

    pub async fn install(&self, config: &Arc<Config>, pr: &dyn SingleReport) -> eyre::Result<()> {
        match self {
            PluginEnum::Asdf(plugin) => plugin.install(config, pr).await,
            PluginEnum::Vfox(plugin) => plugin.install(config, pr).await,
            PluginEnum::VfoxBackend(plugin) => plugin.install(config, pr).await,
        }
    }

    pub fn is_installed(&self) -> bool {
        match self {
            PluginEnum::Asdf(plugin) => plugin.is_installed(),
            PluginEnum::Vfox(plugin) => plugin.is_installed(),
            PluginEnum::VfoxBackend(plugin) => plugin.is_installed(),
        }
    }

    pub fn is_installed_err(&self) -> eyre::Result<()> {
        match self {
            PluginEnum::Asdf(plugin) => plugin.is_installed_err(),
            PluginEnum::Vfox(plugin) => plugin.is_installed_err(),
            PluginEnum::VfoxBackend(plugin) => plugin.is_installed_err(),
        }
    }

    pub async fn ensure_installed(
        &self,
        config: &Arc<Config>,
        mpr: &MultiProgressReport,
        force: bool,
        dry_run: bool,
    ) -> eyre::Result<()> {
        match self {
            PluginEnum::Asdf(plugin) => plugin.ensure_installed(config, mpr, force, dry_run).await,
            PluginEnum::Vfox(plugin) => plugin.ensure_installed(config, mpr, force, dry_run).await,
            PluginEnum::VfoxBackend(plugin) => {
                plugin.ensure_installed(config, mpr, force, dry_run).await
            }
        }
    }
}

impl PluginType {
    pub fn from_full(full: &str) -> eyre::Result<Self> {
        match full.split(':').next() {
            Some("asdf") => Ok(Self::Asdf),
            Some("vfox") => Ok(Self::Vfox),
            Some("vfox-backend") => Ok(Self::VfoxBackend),
            _ => Err(eyre!("unknown plugin type: {full}")),
        }
    }

    pub fn plugin(&self, short: String, custom_path: Option<PathBuf>) -> PluginEnum {
        let path = custom_path.unwrap_or_else(|| install_state::get_plugin_path(&short));
        match self {
            PluginType::Asdf => PluginEnum::Asdf(Arc::new(AsdfPlugin::new(short, path))),
            PluginType::Vfox => PluginEnum::Vfox(Arc::new(VfoxPlugin::new(short, path))),
            PluginType::VfoxBackend => {
                PluginEnum::VfoxBackend(Arc::new(VfoxPlugin::new(short, path)))
            }
        }
    }
}

pub static VERSION_REGEX: Lazy<regex::Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(^Available versions:|-src|-dev|-latest|-stm|[-\\.]rc|-milestone|-alpha|-beta|[-\\.]pre|-next|([abc])[0-9]+|snapshot|SNAPSHOT|master)"
    )
        .unwrap()
});

pub fn get(short: &str) -> Result<PluginEnum> {
    let (name, full) = short.split_once(':').unwrap_or((short, short));

    // For plugin:tool format, look up the plugin by just the plugin name
    let plugin_lookup_key = if short.contains(':') {
        // Check if the part before the colon is a plugin name
        if install_state::list_plugins().get(name).is_some() {
            name
        } else {
            short
        }
    } else {
        short
    };

    // Get plugin info including optional custom path
    let (plugin_type, custom_path) =
        if let Some((plugin_type, path)) = install_state::list_plugins().get(plugin_lookup_key) {
            (*plugin_type, path.clone())
        } else {
            (PluginType::from_full(full)?, None)
        };

    Ok(plugin_type.plugin(name.to_string(), custom_path))
}

#[allow(unused_variables)]
#[async_trait]
pub trait Plugin: Debug + Send {
    fn name(&self) -> &str;
    fn path(&self) -> PathBuf;
    fn get_remote_url(&self) -> eyre::Result<Option<String>>;
    fn set_remote_url(&self, url: String) {}
    fn current_abbrev_ref(&self) -> eyre::Result<Option<String>>;
    fn current_sha_short(&self) -> eyre::Result<Option<String>>;
    fn is_installed(&self) -> bool {
        true
    }
    fn is_installed_err(&self) -> eyre::Result<()> {
        if !self.is_installed() {
            return Err(PluginNotInstalled(self.name().to_string()).into());
        }
        Ok(())
    }

    async fn ensure_installed(
        &self,
        _config: &Arc<Config>,
        _mpr: &MultiProgressReport,
        _force: bool,
        _dry_run: bool,
    ) -> eyre::Result<()> {
        Ok(())
    }
    async fn update(&self, _pr: &dyn SingleReport, _gitref: Option<String>) -> eyre::Result<()> {
        Ok(())
    }
    async fn uninstall(&self, _pr: &dyn SingleReport) -> eyre::Result<()> {
        Ok(())
    }
    async fn install(&self, _config: &Arc<Config>, _pr: &dyn SingleReport) -> eyre::Result<()> {
        Ok(())
    }
    fn external_commands(&self) -> eyre::Result<Vec<Command>> {
        Ok(vec![])
    }
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute_external_command(&self, _command: &str, _args: Vec<String>) -> eyre::Result<()> {
        unimplemented!(
            "execute_external_command not implemented for {}",
            self.name()
        )
    }
}

impl Ord for PluginEnum {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name().cmp(other.name())
    }
}

impl PartialOrd for PluginEnum {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PluginEnum {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl Eq for PluginEnum {}

impl Display for PluginEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[derive(Debug, Clone)]
pub enum PluginSource {
    /// Git repository with URL and optional ref
    Git {
        url: String,
        git_ref: Option<String>,
    },
    /// Zip file accessible via HTTPS
    Zip { url: String },
}

impl PluginSource {
    pub fn parse(repository: &str) -> Self {
        // Split Parameters
        let url_path = repository
            .split('?')
            .next()
            .unwrap_or(repository)
            .split('#')
            .next()
            .unwrap_or(repository);
        // Check if it's a zip file (ends with -zip)
        if url_path.to_lowercase().ends_with(".zip") {
            return PluginSource::Zip {
                url: repository.to_string(),
            };
        }
        // Otherwise treat as git repository
        let (url, git_ref) = Git::split_url_and_ref(repository);
        PluginSource::Git {
            url: url.to_string(),
            git_ref: git_ref.map(|s| s.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_source_parse_git() {
        // Test parsing Git URL
        let source = PluginSource::parse("https://github.com/user/plugin.git");
        match source {
            PluginSource::Git { url, git_ref } => {
                assert_eq!(url, "https://github.com/user/plugin.git");
                assert_eq!(git_ref, None);
            }
            _ => panic!("Expected a git plugin"),
        }
    }

    #[test]
    fn test_plugin_source_parse_git_with_ref() {
        // Test parsing Git URL with refs
        let source = PluginSource::parse("https://github.com/user/plugin.git#v1.0.0");
        match source {
            PluginSource::Git { url, git_ref } => {
                assert_eq!(url, "https://github.com/user/plugin.git");
                assert_eq!(git_ref, Some("v1.0.0".to_string()));
            }
            _ => panic!("Expected a git plugin"),
        }
    }

    #[test]
    fn test_plugin_source_parse_zip() {
        // Test parsing zip URL
        let source = PluginSource::parse("https://example.com/plugins/my-plugin.zip");
        match source {
            PluginSource::Zip { url } => {
                assert_eq!(url, "https://example.com/plugins/my-plugin.zip");
            }
            _ => panic!("Expected a Zip source"),
        }
    }

    #[test]
    fn test_plugin_source_parse_uppercase_zip_with_query() {
        // Test parsing zip URL with query
        let source =
            PluginSource::parse("https://example.com/plugins/my-plugin.ZIP?version=v1.0.0");
        match source {
            PluginSource::Zip { url } => {
                assert_eq!(
                    url,
                    "https://example.com/plugins/my-plugin.ZIP?version=v1.0.0"
                );
            }
            _ => panic!("Expected a Zip source"),
        }
    }

    #[test]
    fn test_plugin_source_parse_edge_cases() {
        // Test parsing git url which contains `.zip`
        let source = PluginSource::parse("https://example.com/.zip/plugin");
        match source {
            PluginSource::Git { .. } => {}
            _ => panic!("Expected a git plugin"),
        }
    }

    #[test]
    fn test_plugin_location_remote() {
        let location = PluginLocation::Remote("https://github.com/user/plugin.git".to_string());
        assert!(location.is_remote());
        assert!(!location.is_local());
        assert_eq!(
            location.as_remote(),
            Some("https://github.com/user/plugin.git")
        );
        assert_eq!(location.as_local(), None);
    }

    #[test]
    fn test_plugin_location_local() {
        let path = PathBuf::from("/path/to/plugin");
        let location = PluginLocation::Local(path.clone());
        assert!(location.is_local());
        assert!(!location.is_remote());
        assert_eq!(location.as_remote(), None);
        assert_eq!(location.as_local(), Some(path.as_path()));
    }

    #[test]
    fn test_plugin_location_equality() {
        let remote1 = PluginLocation::Remote("https://github.com/user/plugin.git".to_string());
        let remote2 = PluginLocation::Remote("https://github.com/user/plugin.git".to_string());
        let remote3 = PluginLocation::Remote("https://github.com/other/plugin.git".to_string());
        let local1 = PluginLocation::Local(PathBuf::from("/path/to/plugin"));
        let local2 = PluginLocation::Local(PathBuf::from("/path/to/plugin"));
        let local3 = PluginLocation::Local(PathBuf::from("/other/path"));

        assert_eq!(remote1, remote2);
        assert_ne!(remote1, remote3);
        assert_eq!(local1, local2);
        assert_ne!(local1, local3);
        assert_ne!(remote1, local1);
    }

    #[test]
    fn test_plugin_location_clone() {
        let location = PluginLocation::Remote("https://github.com/user/plugin.git".to_string());
        let cloned = location.clone();
        assert_eq!(location, cloned);
    }
}
