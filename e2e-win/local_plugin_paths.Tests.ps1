Describe 'Local Plugin Path Resolution' {
    AfterEach {
        # Clean up
        Remove-Item -Recurse -Force "./plugins" -ErrorAction SilentlyContinue
        Remove-Item "mise.toml" -ErrorAction SilentlyContinue
        Remove-Item -Recurse -Force (Join-Path $env:USERPROFILE ".mise-test-plugins") -ErrorAction SilentlyContinue
    }

    It 'resolves relative path' {
        # Create vfox plugin with relative path
        $PluginPath = "./plugins/rel-plugin"
        New-Item -ItemType Directory -Path (Join-Path $PluginPath "hooks") -Force

        Set-Content -Path (Join-Path $PluginPath "metadata.lua") -Value @"
PLUGIN = {}
PLUGIN.name = "rel-plugin"
PLUGIN.version = "0.1.0"
"@

        Set-Content -Path (Join-Path $PluginPath "hooks\available.lua") -Value @"
function PLUGIN:Available(ctx)
    return { { version = "1.0.0" } }
end
"@

        Set-Content -Path "mise.toml" -Value @"
[plugins]
rel-plugin = "./plugins/rel-plugin"
"@

        $output = mise plugin ls | Out-String
        $output | Should -Match "rel-plugin"
    }

    It 'resolves home directory path' {
        # Create plugin in user profile
        $HomePluginPath = Join-Path $env:USERPROFILE ".mise-test-plugins/home-plugin"
        New-Item -ItemType Directory -Path (Join-Path $HomePluginPath "hooks") -Force

        Set-Content -Path (Join-Path $HomePluginPath "metadata.lua") -Value @"
PLUGIN = {}
PLUGIN.name = "home-plugin"
PLUGIN.version = "0.1.0"
"@

        Set-Content -Path (Join-Path $HomePluginPath "hooks\available.lua") -Value @"
function PLUGIN:Available(ctx)
    return { { version = "1.0.0" } }
end
"@

        # Use ~/ for home directory
        Set-Content -Path "mise.toml" -Value @"
[plugins]
home-plugin = "~/.mise-test-plugins/home-plugin"
"@

        $output = mise plugin ls | Out-String
        $output | Should -Match "home-plugin"
    }

    It 'handles error for nonexistent path' {
        Set-Content -Path "mise.toml" -Value @"
[plugins]
missing = "./does-not-exist"
"@

        { mise plugin ls 2>&1 } | Should -Throw "*does not exist*"
    }
}
