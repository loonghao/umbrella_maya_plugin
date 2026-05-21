param(
    [Parameter(Mandatory = $true)]
    [string]$MayaVersion,

    [Parameter(Mandatory = $true)]
    [ValidateSet("windows", "linux", "macos")]
    [string]$Platform,

    [switch]$RequireReleaseArchive
)

$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")

$PluginExtension = switch ($Platform) {
    "windows" { ".mll" }
    "linux" { ".so" }
    "macos" { ".bundle" }
}

$RuntimeLibrary = switch ($Platform) {
    "windows" { "umbrella_maya_plugin.dll" }
    "linux" { "libumbrella_maya_plugin.so" }
    "macos" { "libumbrella_maya_plugin.dylib" }
}

$PluginBinary = "umbrella_maya$PluginExtension"

function Assert-FileMatch {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RootPath,

        [Parameter(Mandatory = $true)]
        [string]$Pattern,

        [Parameter(Mandatory = $true)]
        [string]$Description
    )

    if (-not (Test-Path -LiteralPath $RootPath)) {
        throw "$Description root does not exist: $RootPath"
    }

    $Match = Get-ChildItem -LiteralPath $RootPath -Recurse -File -Filter $Pattern -ErrorAction Stop | Select-Object -First 1
    if (-not $Match) {
        throw "$Description missing '$Pattern' under $RootPath"
    }

    Write-Host "[ok] $Description contains $($Match.FullName)"
}

$FlatArtifacts = Join-Path $Root "dist/maya$MayaVersion-$Platform"
Assert-FileMatch -RootPath $FlatArtifacts -Pattern $PluginBinary -Description "Flat Maya artifact"
Assert-FileMatch -RootPath $FlatArtifacts -Pattern $RuntimeLibrary -Description "Flat Maya artifact"

$ModulePackages = Get-ChildItem -LiteralPath (Join-Path $Root "dist/modules") -Directory -Filter "UmbrellaMayaPlugin-*-maya$MayaVersion-$Platform" -ErrorAction Stop
if (-not $ModulePackages) {
    throw "Maya module package missing for Maya $MayaVersion $Platform"
}

$ModuleRoot = $ModulePackages[0].FullName
Assert-FileMatch -RootPath $ModuleRoot -Pattern "UmbrellaMayaPlugin.mod" -Description "Maya module package"
Assert-FileMatch -RootPath $ModuleRoot -Pattern $PluginBinary -Description "Maya module package"
Assert-FileMatch -RootPath $ModuleRoot -Pattern $RuntimeLibrary -Description "Maya module package"

if ($RequireReleaseArchive) {
    $ReleaseArchive = Get-ChildItem -LiteralPath (Join-Path $Root "dist/release") -File -Filter "UmbrellaMayaPlugin-*-maya$MayaVersion-$Platform.zip" -ErrorAction Stop | Select-Object -First 1
    if (-not $ReleaseArchive) {
        throw "Release archive missing for Maya $MayaVersion $Platform"
    }

    $TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("umbrella-release-check-" + [System.Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $TempRoot | Out-Null
    try {
        Expand-Archive -LiteralPath $ReleaseArchive.FullName -DestinationPath $TempRoot -Force
        Assert-FileMatch -RootPath $TempRoot -Pattern $PluginBinary -Description "Release archive"
        Assert-FileMatch -RootPath $TempRoot -Pattern $RuntimeLibrary -Description "Release archive"
    }
    finally {
        Remove-Item -LiteralPath $TempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
