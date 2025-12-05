use crate::backend::backend_type::BackendType;
use crate::cli::args::BackendArg;
use crate::file::display_path;
use crate::git::Git;
use crate::plugins::PluginType;
use crate::plugins::names::normalize_plugin_name;
use crate::{dirs, file, runtime_symlinks};
use eyre::{Ok, Result};
use heck::ToKebabCase;
use itertools::Itertools;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::OnceCell;
use tokio::task::JoinSet;
use versions::Versioning;

/// Normalize a version string for sorting by stripping leading 'v' or 'V' prefix.
/// This ensures "v1.0.0" and "1.0.0" are sorted together correctly.
fn normalize_version_for_sort(v: &str) -> &str {
    v.strip_prefix('v')
        .or_else(|| v.strip_prefix('V'))
        .unwrap_or(v)
}

/// Complete information about an installed or registered plugin.
/// This represents a fully resolved plugin with normalized name, type, and path.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Normalized plugin name (no type prefixes)
    pub name: String,
    /// Plugin type (Asdf, Vfox, or VfoxBackend)
    pub plugin_type: PluginType,
    /// Absolute path to the plugin directory
    pub path: PathBuf,
}

impl PluginInfo {
    /// Returns true if this is a local plugin (outside dirs::PLUGINS).
    /// This is a derived property based on the path.
    pub fn is_local(&self) -> bool {
        !self.path.starts_with(*dirs::PLUGINS)
    }
}

type InstallStatePlugins = BTreeMap<String, Arc<OnceCell<PluginInfo>>>;
type InstallStateTools = BTreeMap<String, InstallStateTool>;
type MutexResult<T> = Result<Arc<T>>;

#[derive(Debug, Clone)]
pub struct InstallStateTool {
    pub short: String,
    pub full: Option<String>,
    pub versions: Vec<String>,
}

static INSTALL_STATE_PLUGINS: Mutex<Option<Arc<InstallStatePlugins>>> = Mutex::new(None);
static INSTALL_STATE_TOOLS: Mutex<Option<Arc<InstallStateTools>>> = Mutex::new(None);

pub(crate) async fn init() -> Result<()> {
    let (plugins, tools) = tokio::join!(
        tokio::task::spawn(async { measure!("init_plugins", { init_plugins().await }) }),
        tokio::task::spawn(async { measure!("init_tools", { init_tools().await }) }),
    );
    plugins??;
    tools??;
    Ok(())
}

/// Detect plugin type by inspecting the plugin directory
pub fn detect_plugin_type(path: &Path) -> Result<PluginType> {
    // Validate upfront to provide clear error messages before attempting
    // to inspect plugin marker files.
    if !path.exists() {
        return Err(eyre::eyre!(
            "Plugin path does not exist: {}",
            display_path(path)
        ));
    }
    if !path.is_dir() {
        return Err(eyre::eyre!(
            "Plugin path is not a directory: {}",
            display_path(path)
        ));
    }

    if path.join("metadata.lua").exists() {
        if has_backend_methods(path) {
            Ok(PluginType::VfoxBackend)
        } else {
            Ok(PluginType::Vfox)
        }
    } else if path.join("bin").join("list-all").exists() {
        Ok(PluginType::Asdf)
    } else {
        Err(eyre::eyre!(
            "Unable to detect plugin type for: {}. \
             Missing metadata.lua (vfox) or bin/list-all (asdf)",
            display_path(path)
        ))
    }
}

async fn init_plugins() -> MutexResult<InstallStatePlugins> {
    if let Some(plugins) = INSTALL_STATE_PLUGINS
        .lock()
        .expect("INSTALL_STATE_PLUGINS lock failed")
        .clone()
    {
        return Ok(plugins);
    }

    let plugin_dir = &*dirs::PLUGINS;
    if !plugin_dir.exists() {
        let plugins = Arc::new(BTreeMap::new());
        *INSTALL_STATE_PLUGINS
            .lock()
            .expect("INSTALL_STATE_PLUGINS lock failed") = Some(plugins.clone());
        return Ok(plugins);
    }

    // Phase 1: Scan and register refs
    let dirs = file::dir_subdirs(plugin_dir)?;
    let mut plugins = BTreeMap::new();

    for d in dirs {
        time!("init_plugins {d}");
        let path = plugin_dir.join(&d);
        if is_banned_plugin(&path) {
            info!("removing banned plugin {d}");
            let _ = file::remove_all(&path);
            continue;
        }

        let cell = Arc::new(OnceCell::new());
        plugins.insert(d.clone(), cell.clone());

        // Phase 2: Spawn resolution task
        tokio::spawn(async move {
            match detect_plugin_type(&path) {
                eyre::Result::Ok(plugin_type) => {
                    let _ = cell.set(PluginInfo {
                        name: d,
                        plugin_type,
                        path,
                    });
                }
                eyre::Result::Err(e) => {
                    debug!("Failed to detect plugin type: {:#}", e);
                }
            }
        });
    }

    let plugins = Arc::new(plugins);
    *INSTALL_STATE_PLUGINS
        .lock()
        .expect("INSTALL_STATE_PLUGINS lock failed") = Some(plugins.clone());
    Ok(plugins)
}

async fn init_tools() -> MutexResult<InstallStateTools> {
    if let Some(tools) = INSTALL_STATE_TOOLS
        .lock()
        .expect("INSTALL_STATE_TOOLS lock failed")
        .clone()
    {
        return Ok(tools);
    }
    let mut jset = JoinSet::new();
    for dir in file::dir_subdirs(&dirs::INSTALLS)? {
        jset.spawn(async move {
            let backend_meta = read_backend_meta(&dir).unwrap_or_default();
            let short = backend_meta.first().unwrap_or(&dir).to_string();
            let full = backend_meta.get(1).cloned();
            let dir = dirs::INSTALLS.join(&dir);
            let versions = file::dir_subdirs(&dir)
                .unwrap_or_else(|err| {
                    warn!("reading versions in {} failed: {err:?}", display_path(&dir));
                    Default::default()
                })
                .into_iter()
                .filter(|v| !v.starts_with('.'))
                .filter(|v| !runtime_symlinks::is_runtime_symlink(&dir.join(v)))
                .filter(|v| !dir.join(v).join("incomplete").exists())
                .sorted_by_cached_key(|v| {
                    // Normalize version for sorting to handle mixed v-prefix versions
                    // e.g., "v2.0.51" and "2.0.35" should sort by numeric value
                    let normalized = normalize_version_for_sort(v);
                    (Versioning::new(normalized), v.to_string())
                })
                .collect();
            let tool = InstallStateTool {
                short: short.clone(),
                full,
                versions,
            };
            time!("init_tools {short}");
            (short, tool)
        });
    }
    let mut tools = jset
        .join_all()
        .await
        .into_iter()
        .filter(|(_, tool)| !tool.versions.is_empty())
        .collect::<BTreeMap<_, _>>();
    for (short, plugin_cell) in init_plugins().await?.iter() {
        // Try to get plugin info if already resolved (sync check)
        if let Some(plugin_info) = plugin_cell.get() {
            let full = match plugin_info.plugin_type {
                PluginType::Asdf => format!("asdf:{short}"),
                PluginType::Vfox => format!("vfox:{short}"),
                PluginType::VfoxBackend => short.clone(),
            };
            let tool = tools
                .entry(short.clone())
                .or_insert_with(|| InstallStateTool {
                    short: short.clone(),
                    full: Some(full.clone()),
                    versions: Default::default(),
                });
            tool.full = Some(full);
        }
    }
    let tools = Arc::new(tools);
    *INSTALL_STATE_TOOLS
        .lock()
        .expect("INSTALL_STATE_TOOLS lock failed") = Some(tools.clone());
    Ok(tools)
}

fn list_plugin_cells() -> Arc<InstallStatePlugins> {
    INSTALL_STATE_PLUGINS
        .lock()
        .expect("INSTALL_STATE_PLUGINS lock failed")
        .as_ref()
        .expect("INSTALL_STATE_PLUGINS is None")
        .clone()
}

/// Register a plugin reference (creates OnceCell entry)
pub fn register_plugin_ref(name: String) {
    debug!("Registering plugin ref: {}", name);
    let mut plugins_lock = INSTALL_STATE_PLUGINS.lock().expect("lock failed");

    // Initialize map if it doesn't exist yet, or clone existing one
    let existing = plugins_lock.get_or_insert_with(|| Arc::new(BTreeMap::new()));
    let mut new_map: BTreeMap<_, _> = (**existing).clone();

    let was_new = !new_map.contains_key(&name);
    new_map
        .entry(name.clone())
        .or_insert_with(|| Arc::new(OnceCell::new()));
    *plugins_lock = Some(Arc::new(new_map));

    if was_new {
        debug!("Registered new plugin ref: {}", name);
    }
}

/// Resolve a plugin (populate OnceCell with PluginInfo)
pub async fn resolve_plugin(name: &str, info: PluginInfo) -> Result<()> {
    if let Some(cell) = list_plugin_cells().get(name) {
        debug!(
            "Resolving plugin '{}' with type {:?}",
            name, info.plugin_type
        );
        cell.set(info)
            .map_err(|_| eyre::eyre!("Plugin already resolved"))?;
        debug!("Successfully resolved plugin '{}'", name);
    } else {
        warn!(
            "Plugin '{}' not found in registry when trying to resolve",
            name
        );
    }
    Ok(())
}

/// Get plugin info, awaiting resolution with timeout
pub async fn get_plugin_info(name: &str) -> Option<PluginInfo> {
    use tokio::time::{Duration, Instant, sleep};

    let cells = list_plugin_cells();
    let cell = cells.get(name)?;
    let timeout_duration = Duration::from_secs(30); // TODO: make configurable
    let start = Instant::now();
    let poll_interval = Duration::from_millis(10);

    // Poll until initialized or timeout
    loop {
        if let Some(info) = cell.get() {
            return Some(info.clone());
        }

        if start.elapsed() >= timeout_duration {
            warn!(
                "Plugin '{}' resolution timed out after {}s, using fallback",
                name,
                timeout_duration.as_secs()
            );
            // Return default PluginInfo as fallback
            return Some(PluginInfo {
                name: name.to_string(),
                plugin_type: PluginType::Asdf, // Default guess
                path: dirs::PLUGINS.join(name.to_kebab_case()),
            });
        }

        sleep(poll_interval).await;
    }
}

/// Get plugin info synchronously (returns None if not yet resolved)
pub fn get_plugin_info_sync(name: &str) -> Option<PluginInfo> {
    list_plugin_cells().get(name)?.get().cloned()
}

/// List all plugins that have been resolved (sync check)
pub fn list_plugins() -> BTreeMap<String, PluginInfo> {
    list_plugin_cells()
        .iter()
        .filter_map(|(name, cell)| cell.get().map(|info| (name.clone(), info.clone())))
        .collect()
}

fn is_banned_plugin(path: &Path) -> bool {
    if path.ends_with("gradle") {
        let repo = Git::new(path);
        if let Some(url) = repo.get_remote_url() {
            return url == "https://github.com/rfrancis/asdf-gradle.git";
        }
    }
    false
}

fn has_backend_methods(plugin_path: &Path) -> bool {
    // to be a backend plugin, it must have a backend_install.lua file so we don't need to check for other files
    plugin_path
        .join("hooks")
        .join("backend_install.lua")
        .exists()
}

pub fn get_tool_full(short: &str) -> Option<String> {
    list_tools().get(short).and_then(|t| t.full.clone())
}

pub fn get_plugin_type(short: &str) -> Option<PluginType> {
    get_plugin_info_sync(short).map(|info| info.plugin_type)
}

pub fn list_tools() -> Arc<BTreeMap<String, InstallStateTool>> {
    INSTALL_STATE_TOOLS
        .lock()
        .expect("INSTALL_STATE_TOOLS lock failed")
        .as_ref()
        .expect("INSTALL_STATE_TOOLS is None")
        .clone()
}

pub fn backend_type(short: &str) -> Result<Option<BackendType>> {
    let backend_type = list_tools()
        .get(short)
        .and_then(|ist| ist.full.as_ref())
        .map(|full| BackendType::guess(full));
    if let Some(BackendType::Unknown) = backend_type
        && let Some((plugin_name, _)) = short.split_once(':')
        && let Some(PluginType::VfoxBackend) = get_plugin_type(plugin_name)
    {
        return Ok(Some(BackendType::VfoxBackend(plugin_name.to_string())));
    }
    Ok(backend_type)
}

pub fn list_versions(short: &str) -> Vec<String> {
    list_tools()
        .get(short)
        .map(|tool| tool.versions.clone())
        .unwrap_or_default()
}

/// Get the path for a plugin, checking for local plugins first, then falling back to the standard plugins directory.
/// Returns (plugin_name, plugin_path) where plugin_name is the normalized name.
pub fn get_plugin_path_and_name(short: &str) -> (String, PathBuf) {
    // Normalize the plugin name for consistent lookup
    let normalized_name = normalize_plugin_name(short);

    // Single lookup with normalized name (sync check)
    if let Some(info) = get_plugin_info_sync(normalized_name) {
        return (info.name.clone(), info.path.clone());
    }

    // Fall back to standard plugins directory
    (short.to_string(), dirs::PLUGINS.join(short.to_kebab_case()))
}

/// Get the path for a plugin, checking for local plugins first, then falling back to the standard plugins directory.
pub fn get_plugin_path(short: &str) -> PathBuf {
    get_plugin_path_and_name(short).1
}

fn backend_meta_path(short: &str) -> PathBuf {
    dirs::INSTALLS
        .join(short.to_kebab_case())
        .join(".mise.backend")
}

fn migrate_backend_meta_json(dir: &str) {
    let old = dirs::INSTALLS.join(dir).join(".mise.backend.json");
    let migrate = || {
        let json: serde_json::Value = serde_json::from_reader(file::open(&old)?)?;
        if let Some(full) = json.get("id").and_then(|id| id.as_str()) {
            let short = json
                .get("short")
                .and_then(|short| short.as_str())
                .unwrap_or(dir);
            let doc = format!("{short}\n{full}");
            file::write(backend_meta_path(dir), doc.trim())?;
        }
        Ok(())
    };
    if old.exists() {
        if let Err(err) = migrate() {
            debug!("{err:#}");
        }
        if let Err(err) = file::remove_file(&old) {
            debug!("{err:#}");
        }
    }
}

fn read_backend_meta(short: &str) -> Option<Vec<String>> {
    migrate_backend_meta_json(short);
    let path = backend_meta_path(short);
    if path.exists() {
        let body = file::read_to_string(&path)
            .map_err(|err| {
                warn!("{err:?}");
            })
            .unwrap_or_default();
        Some(
            body.lines()
                .filter(|f| !f.is_empty())
                .map(|f| f.to_string())
                .collect(),
        )
    } else {
        None
    }
}

pub fn write_backend_meta(ba: &BackendArg, install_path: &Path) -> Result<()> {
    let full = match ba.full() {
        full if full.starts_with("core:") => ba.full(),
        _ => ba.full_with_opts(),
    };
    let doc = format!("{}\n{}", ba.short, full);
    let meta_path = install_path.join(".mise.backend");
    file::write(meta_path, doc.trim())?;
    Ok(())
}

pub fn incomplete_file_path(short: &str, v: &str) -> PathBuf {
    dirs::CACHE
        .join(short.to_kebab_case())
        .join(v)
        .join("incomplete")
}

pub fn reset() {
    *INSTALL_STATE_PLUGINS
        .lock()
        .expect("INSTALL_STATE_PLUGINS lock failed") = None;
    *INSTALL_STATE_TOOLS
        .lock()
        .expect("INSTALL_STATE_TOOLS lock failed") = None;
}

#[cfg(test)]
mod tests {
    use super::normalize_version_for_sort;
    use itertools::Itertools;
    use versions::Versioning;

    #[test]
    fn test_normalize_version_for_sort() {
        assert_eq!(normalize_version_for_sort("v1.0.0"), "1.0.0");
        assert_eq!(normalize_version_for_sort("V1.0.0"), "1.0.0");
        assert_eq!(normalize_version_for_sort("1.0.0"), "1.0.0");
        assert_eq!(normalize_version_for_sort("latest"), "latest");
    }

    #[test]
    fn test_version_sorting_with_v_prefix() {
        // Test that mixed v-prefix and non-v-prefix versions sort correctly
        let versions = ["v2.0.51", "2.0.35", "2.0.52"];

        // Without normalization - demonstrates the problem
        let sorted_without_norm: Vec<_> = versions
            .iter()
            .sorted_by_cached_key(|v| (Versioning::new(v), v.to_string()))
            .collect();
        println!("Without normalization: {:?}", sorted_without_norm);

        // With normalization - the fix
        let sorted_with_norm: Vec<_> = versions
            .iter()
            .sorted_by_cached_key(|v| {
                let normalized = normalize_version_for_sort(v);
                (Versioning::new(normalized), v.to_string())
            })
            .collect();
        println!("With normalization: {:?}", sorted_with_norm);

        // With the fix, v2.0.51 should sort between 2.0.35 and 2.0.52
        // The highest version should be 2.0.52
        assert_eq!(**sorted_with_norm.last().unwrap(), "2.0.52");

        // v2.0.51 should be second to last
        assert_eq!(**sorted_with_norm.get(1).unwrap(), "v2.0.51");

        // 2.0.35 should be first
        assert_eq!(**sorted_with_norm.first().unwrap(), "2.0.35");
    }

    #[test]
    fn test_detect_plugin_type_asdf_and_vfox() {
        use crate::file::{create_dir_all, remove_all, write};
        use crate::plugins::PluginType;
        use std::path::PathBuf;

        // Test ASDF plugin detection
        let asdf_dir = PathBuf::from("/tmp/test-asdf-plugin");
        create_dir_all(asdf_dir.join("bin")).unwrap();
        write(
            asdf_dir.join("bin").join("list-all"),
            "#!/bin/bash\necho 1.0.0",
        )
        .unwrap();

        let result = super::detect_plugin_type(&asdf_dir).unwrap();
        assert_eq!(result, PluginType::Asdf);

        remove_all(&asdf_dir).unwrap();

        // Test vfox plugin detection
        let vfox_dir = PathBuf::from("/tmp/test-vfox-plugin");
        create_dir_all(&vfox_dir).unwrap();
        write(vfox_dir.join("metadata.lua"), "return {}").unwrap();

        let result = super::detect_plugin_type(&vfox_dir).unwrap();
        assert_eq!(result, PluginType::Vfox);

        remove_all(&vfox_dir).unwrap();
    }
}
