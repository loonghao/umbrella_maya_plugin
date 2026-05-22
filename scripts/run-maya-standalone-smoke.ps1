param(
    [string]$MayaVersion = "2024",
    [string]$PackageRoot = "dist\modules",
    [string]$ScenePath = "tests\virus\uifiguration.ma",
    [string]$Mayapy = "",
    [switch]$UnsafeAllowScriptNodes
)

$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")

function Resolve-InputPath([string]$Path) {
    if ([System.IO.Path]::IsPathRooted($Path)) {
        return Resolve-Path -LiteralPath $Path
    }
    return Resolve-Path -LiteralPath (Join-Path $Root $Path)
}

if ([string]::IsNullOrWhiteSpace($Mayapy)) {
    $Candidates = @(
        "C:\Program Files\Autodesk\Maya$MayaVersion\bin\mayapy.exe",
        "/usr/autodesk/maya$MayaVersion/bin/mayapy",
        "/Applications/Autodesk/maya$MayaVersion/Maya.app/Contents/bin/mayapy"
    )
    $Mayapy = $Candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
}

if ([string]::IsNullOrWhiteSpace($Mayapy) -or -not (Test-Path -LiteralPath $Mayapy)) {
    throw "mayapy not found. Pass -Mayapy or run inside a Maya Docker image with mayapy available."
}

$PackageRootPath = Resolve-InputPath $PackageRoot
$Package = Get-ChildItem -LiteralPath $PackageRootPath -Directory -Filter "UmbrellaMayaPlugin-*-maya$MayaVersion-*" |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

if (-not $Package) {
    throw "No Maya module package for Maya $MayaVersion under $PackageRootPath. Run 'vx just package $MayaVersion' first."
}

$PluginExtension = if ($IsWindows -or $env:OS -eq "Windows_NT") { ".mll" } elseif ($IsMacOS) { ".bundle" } else { ".so" }
$PluginPath = Join-Path $Package.FullName "UmbrellaMayaPlugin\plug-ins\umbrella_maya$PluginExtension"
if (-not (Test-Path -LiteralPath $PluginPath)) {
    throw "Plugin binary missing: $PluginPath"
}

$ResolvedScenePath = Resolve-InputPath $ScenePath
$TempScript = Join-Path ([System.IO.Path]::GetTempPath()) ("umbrella-maya-smoke-" + [System.Guid]::NewGuid().ToString("N") + ".py")
$ExecuteScriptNodes = if ($UnsafeAllowScriptNodes) { "True" } else { "False" }

Set-Content -Path $TempScript -Encoding UTF8 -Value @"
import maya.standalone
maya.standalone.initialize(name="umbrella_maya_smoke")

from maya import cmds

plugin_path = r"$PluginPath"
scene_path = r"$ResolvedScenePath"

cmds.loadPlugin(plugin_path)
assert cmds.pluginInfo(plugin_path, query=True, loaded=True)
cmds.umbrellaInfo()
cmds.umbrellaEnable()
cmds.file(scene_path, open=True, force=True, ignoreVersion=True, executeScriptNodes=$ExecuteScriptNodes, prompt=False)
cmds.umbrellaScanScene()
cmds.umbrellaDisable()
maya.standalone.uninitialize()
print("[ok] Maya standalone smoke completed")
"@

try {
    & $Mayapy $TempScript
    if ($LASTEXITCODE -ne 0) {
        throw "mayapy exited with code $LASTEXITCODE"
    }
}
finally {
    Remove-Item -LiteralPath $TempScript -Force -ErrorAction SilentlyContinue
}
