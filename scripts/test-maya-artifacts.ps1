param(
    [Parameter(Mandatory = $true)]
    [string]$MayaVersion,

    [Parameter(Mandatory = $true)]
    [ValidateSet("windows", "linux", "macos")]
    [string]$Platform,

    [switch]$RequireReleaseArchive,

    # Overrides the auto-detected dumpbin used for Windows dependency checks.
    [string]$DumpbinPath = ""
)

$ErrorActionPreference = "Stop"

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")

# msvc-kit fingerprints describe selected metadata; capture the actual compiler bytes too.
$ToolchainRecord = Join-Path $Root "build/toolchain.json"
if (Test-Path -LiteralPath $ToolchainRecord) {
    $Toolchain = Get-Content -LiteralPath $ToolchainRecord -Raw | ConvertFrom-Json
    if ($Toolchain.tools.cl) {
        $CompilerHash = Get-FileHash -LiteralPath $Toolchain.tools.cl -Algorithm SHA256
        @{ compiler = $Toolchain.tools.cl; sha256 = $CompilerHash.Hash.ToLowerInvariant() } |
            ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Root "build/compiler-provenance.json") -Encoding UTF8
    }
}

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

function Resolve-Dumpbin {
    $Vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    $SearchRoots = @()
    if (Test-Path -LiteralPath $Vswhere -PathType Leaf) {
        $Installations = & $Vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($LASTEXITCODE -eq 0) {
            $SearchRoots += @($Installations | Where-Object { $_ })
        }
    }
    $SearchRoots += (Join-Path $env:ProgramFiles "Microsoft Visual Studio")
    $SearchRoots = @($SearchRoots | Where-Object { $_ -and (Test-Path -LiteralPath $_) })

    $Dumpbin = $SearchRoots |
        ForEach-Object {
            Get-ChildItem -LiteralPath $_ -Recurse -File -Filter "dumpbin.exe" -ErrorAction SilentlyContinue |
                Where-Object { $_.FullName -match '\\VC\\Tools\\MSVC\\([^\\]+)\\bin\\Hostx64\\x64\\dumpbin\.exe$' } |
                ForEach-Object {
                    [pscustomobject]@{
                        Path = $_.FullName
                        Toolset = [version]($_.FullName -replace '^.*\\VC\\Tools\\MSVC\\([^\\]+)\\.*$', '$1')
                    }
                }
        } |
        Sort-Object -Property Toolset -Descending |
        Select-Object -First 1

    if (-not $Dumpbin) {
        throw "dumpbin.exe not found. Windows artifact checks need a Visual Studio installation with the x64 VC tools, or pass -DumpbinPath explicitly."
    }

    Write-Host "[ok] Detected dumpbin $($Dumpbin.Toolset): $($Dumpbin.Path)"
    return $Dumpbin.Path
}

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

if ($Platform -eq "windows") {
    if ([string]::IsNullOrWhiteSpace($DumpbinPath)) {
        $DumpbinPath = Resolve-Dumpbin
    } elseif (-not (Test-Path -LiteralPath $DumpbinPath -PathType Leaf)) {
        throw "-DumpbinPath does not point at an existing dumpbin executable: $DumpbinPath"
    }

    $Report = @()
    foreach ($BinaryName in @($PluginBinary, $RuntimeLibrary)) {
        $Binary = Get-ChildItem -LiteralPath $ModuleRoot -Recurse -File -Filter $BinaryName | Select-Object -First 1
        $Headers = & $DumpbinPath /headers $Binary.FullName
        if ($LASTEXITCODE -ne 0 -or -not ($Headers -match "8664 machine")) {
            throw "$BinaryName is not a readable x64 PE artifact"
        }
        $Dependents = & $DumpbinPath /dependents $Binary.FullName
        if ($LASTEXITCODE -ne 0) {
            throw "Cannot inspect dependencies of $BinaryName"
        }
        $Dependencies = @($Dependents | ForEach-Object { $_.Trim() } | Where-Object { $_ -match '^[\w.-]+\.dll$' })
        if (-not $Dependencies) {
            throw "No DLL imports found for $BinaryName"
        }
        if ($Dependencies -match '^(msvcp\d+d|vcruntime\d+(?:_\d+)?d|ucrtbased)\.dll$') {
            throw "$BinaryName imports a debug CRT"
        }
        if ($BinaryName -eq $PluginBinary -and $Dependencies -notcontains $RuntimeLibrary) {
            throw "$PluginBinary does not import the packaged Rust runtime $RuntimeLibrary"
        }
        $Report += "$BinaryName`: $($Dependencies -join ', ')"
    }
    $Report | Set-Content -LiteralPath (Join-Path $ModuleRoot "windows-dependencies.txt") -Encoding UTF8
    Write-Host "[ok] Windows package has x64 PE binaries, a packaged Rust import, and no debug CRT imports"
}

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
