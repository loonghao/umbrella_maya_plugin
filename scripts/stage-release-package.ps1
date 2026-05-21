param(
    [Parameter(Mandatory = $true)]
    [string]$MayaVersion,

    [Parameter(Mandatory = $true)]
    [ValidateSet("windows", "linux", "macos")]
    [string]$Platform
)

$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$CargoToml = Get-Content (Join-Path $Root "Cargo.toml") -Raw
$VersionMatch = [regex]::Match($CargoToml, '(?m)^version\s*=\s*"([^"]+)"')
if (-not $VersionMatch.Success) {
    throw "Failed to read package version from Cargo.toml"
}
$Version = $VersionMatch.Groups[1].Value

$Target = switch ($Platform) {
    "windows" { "x86_64-pc-windows-msvc" }
    "linux" { "x86_64-unknown-linux-gnu" }
    "macos" {
        if ([System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -eq "Arm64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        }
    }
}

$AssetName = "UmbrellaMayaPlugin-$Version-maya$MayaVersion-$Platform"
$StageRoot = Join-Path $Root "dist/release/$AssetName"
$Archive = Join-Path $Root "dist/release/$AssetName.zip"
$ModulePackage = Join-Path $Root "dist/modules/$AssetName"

if (Test-Path $StageRoot) {
    Remove-Item -LiteralPath $StageRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $StageRoot | Out-Null

if (-not (Test-Path $ModulePackage)) {
    throw "Maya module package not found: $ModulePackage"
}
Copy-Item -LiteralPath $ModulePackage -Destination (Join-Path $StageRoot "maya-module") -Recurse

$CliName = if ($Platform -eq "windows") { "umbrella-maya.exe" } else { "umbrella-maya" }
$CliPath = Join-Path $Root "target/release/$CliName"
if (Test-Path $CliPath) {
    New-Item -ItemType Directory -Path (Join-Path $StageRoot "cli") | Out-Null
    Copy-Item -LiteralPath $CliPath -Destination (Join-Path $StageRoot "cli/$CliName")
}

$PythonExt = Get-ChildItem -Path (Join-Path $Root "dist/python/$Target") -Filter "umbrella_maya.*" -ErrorAction SilentlyContinue | Select-Object -First 1
if ($PythonExt) {
    New-Item -ItemType Directory -Path (Join-Path $StageRoot "python") | Out-Null
    Copy-Item -LiteralPath $PythonExt.FullName -Destination (Join-Path $StageRoot "python/$($PythonExt.Name)")
}

Set-Content -Path (Join-Path $StageRoot "README_INSTALL.txt") -Value @"
Umbrella Maya Plugin $Version

Contents:
- maya-module/: installable Maya module package for Maya $MayaVersion on $Platform
- python/: PyO3 Python extension for mayapy/Python automation
- cli/: umbrella-maya command line scanner/cleaner

For Maya installation, copy maya-module/UmbrellaMayaPlugin.mod and the
maya-module/UmbrellaMayaPlugin directory into a Maya modules directory.
"@

if (Test-Path $Archive) {
    Remove-Item -LiteralPath $Archive -Force
}
Compress-Archive -Path (Join-Path $StageRoot "*") -DestinationPath $Archive -Force
Write-Host $Archive
