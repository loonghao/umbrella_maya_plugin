param(
    [string]$MayaVersion = "2024",
    [string]$PackageRoot = "dist\modules",
    [string]$ModulePath = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($ModulePath)) {
    $ModulePath = Join-Path $HOME "Documents\maya\modules"
}

$packagePattern = "UmbrellaMayaPlugin-*-maya$MayaVersion-*"
$package = Get-ChildItem -Path $PackageRoot -Directory -Filter $packagePattern |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

if (-not $package) {
    throw "No package matching '$packagePattern' found under '$PackageRoot'. Run 'vx just package $MayaVersion' first."
}

$sourceModuleFile = Join-Path $package.FullName "UmbrellaMayaPlugin.mod"
$sourceModuleDir = Join-Path $package.FullName "UmbrellaMayaPlugin"

if (-not (Test-Path $sourceModuleFile)) {
    throw "Package is missing UmbrellaMayaPlugin.mod: $($package.FullName)"
}

if (-not (Test-Path $sourceModuleDir)) {
    throw "Package is missing UmbrellaMayaPlugin directory: $($package.FullName)"
}

New-Item -ItemType Directory -Force -Path $ModulePath | Out-Null

$targetModuleDir = Join-Path $ModulePath "UmbrellaMayaPlugin"
if (Test-Path $targetModuleDir) {
    Remove-Item -LiteralPath $targetModuleDir -Recurse -Force
}

Copy-Item -LiteralPath $sourceModuleFile -Destination $ModulePath -Force
Copy-Item -LiteralPath $sourceModuleDir -Destination $ModulePath -Recurse -Force

Write-Host "Installed UmbrellaMayaPlugin module to: $ModulePath"
Write-Host "Open Maya $MayaVersion and load umbrella_maya in Plug-in Manager."
