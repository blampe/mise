/// Plugin name normalization utilities.
///
/// This module provides the single source of truth for plugin name normalization.
/// After normalization, code should ONLY use normalized names (no prefixes).
/// 
/// Normalize a plugin name by stripping type prefixes.
///
/// This is the ONLY place names should be normalized. After normalization,
/// the application works exclusively with normalized names.
///
/// # Examples
///
/// ```
/// use mise::plugins::names::normalize_plugin_name;
///
/// assert_eq!(normalize_plugin_name("vfox:node"), "node");
/// assert_eq!(normalize_plugin_name("asdf:python"), "python");
/// assert_eq!(normalize_plugin_name("vfox-backend:npm"), "npm");
/// assert_eq!(normalize_plugin_name("ruby"), "ruby");
/// ```
pub fn normalize_plugin_name(name: &str) -> &str {
    name.strip_prefix("vfox:")
        .or_else(|| name.strip_prefix("vfox-backend:"))
        .or_else(|| name.strip_prefix("asdf:"))
        .unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_plugin_name() {
        // Strip vfox: prefix
        assert_eq!(normalize_plugin_name("vfox:node"), "node");
        assert_eq!(normalize_plugin_name("vfox:golang"), "golang");

        // Strip asdf: prefix
        assert_eq!(normalize_plugin_name("asdf:python"), "python");
        assert_eq!(normalize_plugin_name("asdf:ruby"), "ruby");

        // Strip vfox-backend: prefix
        assert_eq!(normalize_plugin_name("vfox-backend:npm"), "npm");

        // No prefix - return as-is
        assert_eq!(normalize_plugin_name("erlang"), "erlang");
        assert_eq!(normalize_plugin_name("elixir"), "elixir");

        // Nested colons - only strip first prefix
        assert_eq!(normalize_plugin_name("vfox:plugin:tool"), "plugin:tool");
    }
}
