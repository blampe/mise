Describe 'Local Plugin Tests' {
    BeforeEach {
        # Create minimal vfox plugin locally in current directory
        $LocalPluginPath = "./local-plugin"
        New-Item -ItemType Directory -Path (Join-Path $LocalPluginPath "hooks") -Force

        # Create metadata.lua
        Set-Content -Path (Join-Path $LocalPluginPath "metadata.lua") -Value @"
PLUGIN = {}
PLUGIN.name = "local-plugin"
PLUGIN.version = "0.1.0"
PLUGIN.description = "Local test plugin"
"@

        # Create available.lua (lists versions)
        Set-Content -Path (Join-Path $LocalPluginPath "hooks\available.lua") -Value @"
function PLUGIN:Available(ctx)
    return {
        { version = "1.0.0" },
        { version = "2.0.0" },
        { version = "3.0.0" }
    }
end
"@

        # Create pre_install.lua (handles installation)
        Set-Content -Path (Join-Path $LocalPluginPath "hooks\pre_install.lua") -Value @"
function PLUGIN:PreInstall(ctx)
    return { version = ctx.version }
end
"@

        # Create post_install.lua (sets up installed tool)
        Set-Content -Path (Join-Path $LocalPluginPath "hooks\post_install.lua") -Value @"
function PLUGIN:PostInstall(ctx)
    local bin_dir = ctx.path .. "/bin"
    os.execute("mkdir " .. bin_dir)

    local bin_file = bin_dir .. "/local-tool.bat"
    local file = io.open(bin_file, "w")
    if file then
        file:write("@echo off\n")
        file:write("echo local-tool version " .. ctx.version .. "\n")
        file:close()
    end
end
"@

        # Configure mise.toml with relative path (vfox plugin)
        Set-Content -Path "mise.toml" -Value @"
[plugins]
local-plugin = "./local-plugin"

[tools]
"vfox:local-plugin" = "2.0.0"
"@
    }

    AfterEach {
        # Clean up
        Remove-Item -Recurse -Force "./local-plugin" -ErrorAction SilentlyContinue
        Remove-Item "mise.toml" -ErrorAction SilentlyContinue
    }

    It 'lists local plugin' {
        $output = mise plugin ls | Out-String
        $output | Should -Match "local-plugin"
    }

    It 'lists versions from local plugin' {
        $output = mise ls-remote vfox:local-plugin | Out-String
        $output | Should -Match "2.0.0"
    }

    It 'installs tool from local plugin' {
        mise install vfox:local-plugin@2.0.0
        $output = mise ls --installed | Out-String
        $output | Should -Match "local-plugin"
    }
}
