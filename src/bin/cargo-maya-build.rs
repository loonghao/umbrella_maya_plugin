//! Cargo Maya Build - Pure Rust Cross-platform Maya Plugin Build Tool
//!
//! This tool allows building Maya plugins using the `cargo maya-build` command
//!
//! Usage:
//!   cargo maya-build --platform windows --maya-version 2024
//!   cargo maya-build --all-platforms --all-versions
//!   cargo maya-build --current-platform

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use colored::*;
use serde::{Deserialize, Serialize};
use tokio::fs as async_fs;

#[derive(Parser)]
#[command(about = "Umbrella Maya Plugin Cross-platform Build Tool")]
#[command(name = "cargo-maya-build")]
struct MayaBuildArgs {
    /// Target platform
    #[arg(short, long, value_enum)]
    platform: Option<Platform>,

    /// Maya version
    #[arg(short, long)]
    maya_version: Option<String>,

    /// Build all platforms
    #[arg(long)]
    all_platforms: bool,

    /// Build all Maya versions
    #[arg(long)]
    all_versions: bool,

    /// Build current platform only
    #[arg(long)]
    current_only: bool,

    /// Skip Rust library build
    #[arg(long)]
    skip_rust: bool,

    /// Skip C++ plugin build
    #[arg(long)]
    skip_cpp: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Clean build directories
    #[arg(long)]
    clean: bool,

    /// CMake generator override (portable Windows toolchains use Ninja)
    #[arg(long)]
    cmake_generator: Option<String>,

    /// Optional compatible msvc-kit executable; never installs a toolchain
    #[arg(long, requires_all = ["msvc_version", "sdk_version"])]
    msvc_kit: Option<PathBuf>,

    /// Full installed MSVC version for the optional portable Windows build
    #[arg(long, requires = "msvc_kit")]
    msvc_version: Option<String>,

    /// Full installed Windows SDK version for the optional portable build
    #[arg(long, requires = "msvc_kit")]
    sdk_version: Option<String>,

    /// Existing msvc-kit installation directory
    #[arg(long, requires = "msvc_kit")]
    msvc_dir: Option<PathBuf>,

    /// Host tool architecture; Maya's Windows target remains x64
    #[arg(long, default_value = "x64", value_parser = ["x64", "x86", "arm64"])]
    host_arch: String,
}

#[derive(Clone, Debug, ValueEnum, PartialEq)]
enum Platform {
    Windows,
    Linux,
    MacOS,
}

#[derive(Debug, Serialize, Deserialize)]
struct BuildConfig {
    maya_versions: Vec<String>,
    platforms: HashMap<String, PlatformConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlatformConfig {
    rust_target: String,
    plugin_ext: String,
    lib_ext: String,
    devkit_platform: String,
    /// Generator used when the platform has a stable one. Windows stays `None`
    /// because hosted images move between Visual Studio releases, so the
    /// generator is detected from the installed instances instead.
    cmake_generator: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DevKitConfig {
    devkit: DevKitInfo,
}

#[derive(Debug, Deserialize)]
struct DevKitInfo {
    #[allow(dead_code)]
    base_url: String,
    #[allow(dead_code)]
    supported_versions: Vec<String>,
    #[allow(dead_code)]
    platforms: HashMap<String, String>,
    urls: HashMap<String, HashMap<String, String>>,
    #[allow(dead_code)]
    extraction: ExtractionConfig,
    #[allow(dead_code)]
    structure: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ExtractionConfig {
    #[allow(dead_code)]
    zip_pattern: String,
    #[allow(dead_code)]
    tgz_pattern: String,
    #[allow(dead_code)]
    dmg_pattern: String,
}

#[derive(Debug)]
struct BuildContext {
    project_root: PathBuf,
    dist_dir: PathBuf,
    devkit_dir: PathBuf,
    current_platform: Platform,
    config: BuildConfig,
    devkit_config: Option<DevKitConfig>,
    verbose: bool,
    cmake_generator: Option<String>,
    toolchain: Option<ToolchainQuery>,
}

#[derive(Debug, Deserialize)]
struct ToolchainQuery {
    env_vars: HashMap<String, String>,
    tools: HashMap<String, String>,
    msvc: ToolchainComponent,
    sdk: ToolchainComponent,
    arch: String,
    host_arch: String,
}

#[derive(Debug, Deserialize)]
struct ToolchainComponent {
    version: String,
}

impl BuildContext {
    fn new(verbose: bool) -> Result<Self> {
        let project_root = env::current_dir().context("Failed to get current directory")?;
        let dist_dir = project_root.join("dist");
        let devkit_dir = project_root.join("maya-devkit");

        let current_platform = detect_platform()?;
        let config = create_build_config();
        let devkit_config = load_devkit_config(&project_root);

        Ok(Self {
            project_root,
            dist_dir,
            devkit_dir,
            current_platform,
            config,
            devkit_config,
            verbose,
            cmake_generator: None,
            toolchain: None,
        })
    }

    fn configure_toolchain(&mut self, args: &MayaBuildArgs) -> Result<()> {
        self.cmake_generator = args.cmake_generator.clone();
        let Some(executable) = &args.msvc_kit else {
            return Ok(());
        };
        if self.current_platform != Platform::Windows
            || args.all_platforms
            || args
                .platform
                .as_ref()
                .is_some_and(|p| *p != Platform::Windows)
        {
            bail!("--msvc-kit requires a native Windows build");
        }
        if self
            .cmake_generator
            .as_deref()
            .is_some_and(|g| g != "Ninja")
        {
            bail!("--msvc-kit requires the Ninja generator");
        }
        let msvc = args
            .msvc_version
            .as_deref()
            .context("Missing --msvc-version")?;
        let sdk = args
            .sdk_version
            .as_deref()
            .context("Missing --sdk-version")?;
        if !is_full_version(msvc, 3) || !is_full_version(sdk, 4) {
            bail!("Portable builds require full MSVC and SDK versions");
        }
        let selectors = [
            "--msvc-version",
            msvc,
            "--sdk-version",
            sdk,
            "--arch",
            "x64",
            "--host-arch",
            &args.host_arch,
        ];
        let mut doctor = Command::new(executable);
        doctor
            .args(["doctor", "--format", "json", "--compile"])
            .args(selectors);
        if let Some(directory) = &args.msvc_dir {
            doctor.arg("--dir").arg(directory);
        }
        let output = doctor.output().context("Failed to run msvc-kit doctor")?;
        if !output.status.success() {
            bail!(
                "Selected toolchain failed doctor: {}",
                command_output_summary(&output)
            );
        }
        let doctor_report = output.stdout;
        let mut query = Command::new(executable);
        query.args(["query", "--format", "json"]).args(selectors);
        if let Some(directory) = &args.msvc_dir {
            query.arg("--dir").arg(directory);
        }
        let output = query.output().context("Failed to run msvc-kit query")?;
        if !output.status.success() {
            bail!(
                "Toolchain query failed: {}",
                command_output_summary(&output)
            );
        }
        let mut toolchain: ToolchainQuery = serde_json::from_slice(&output.stdout)
            .context("msvc-kit query returned an incompatible JSON contract")?;
        if toolchain.msvc.version != msvc
            || toolchain.sdk.version != sdk
            || toolchain.arch != "x64"
            || toolchain.host_arch != args.host_arch
        {
            bail!("msvc-kit resolved a different toolchain than requested");
        }
        if toolchain.env_vars.is_empty()
            || !toolchain.tools.contains_key("cl")
            || !toolchain.tools.contains_key("link")
        {
            bail!("msvc-kit query did not resolve the compiler, linker and environment");
        }
        let inherited_path = env::var("PATH").unwrap_or_default();
        let selected_path = toolchain
            .env_vars
            .get("PATH")
            .context("Missing toolchain PATH")?;
        toolchain.env_vars.insert(
            "PATH".to_owned(),
            format!("{};{}", selected_path, inherited_path),
        );
        toolchain.env_vars.insert(
            "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER".to_owned(),
            toolchain.tools["link"].clone(),
        );
        let report_dir = self.project_root.join("build");
        std::fs::create_dir_all(&report_dir)?;
        std::fs::write(report_dir.join("toolchain.json"), &output.stdout)?;
        std::fs::write(report_dir.join("doctor.json"), doctor_report)?;
        self.cmake_generator = Some("Ninja".to_owned());
        self.toolchain = Some(toolchain);
        self.log_success(&format!(
            "Selected MSVC {} / SDK {} for Cargo and Ninja",
            msvc, sdk
        ));
        Ok(())
    }

    fn build_command(&self, program: &str) -> Command {
        let mut command = Command::new(program);
        if let Some(toolchain) = &self.toolchain {
            command.envs(&toolchain.env_vars);
            command.env_remove("CMAKE_GENERATOR_PLATFORM");
            command.env_remove("CMAKE_GENERATOR_TOOLSET");
            command.env_remove("CMAKE_GENERATOR_INSTANCE");
        }
        command
    }

    fn log(&self, message: &str) {
        println!("{}", message);
    }

    fn log_verbose(&self, message: &str) {
        if self.verbose {
            println!("{} {}", "[debug]".blue(), message.dimmed());
        }
    }

    fn log_success(&self, message: &str) {
        println!("{} {}", "[ok]".green(), message.green());
    }

    fn log_error(&self, message: &str) {
        eprintln!("{} {}", "[error]".red(), message.red());
    }

    fn log_warning(&self, message: &str) {
        println!("{} {}", "[warn]".yellow(), message.yellow());
    }

    /// Chooses the CMake generator for a build.
    ///
    /// An explicit `--cmake-generator` override always wins. Non-Windows
    /// platforms keep their stable generator. Windows detects the newest
    /// installed Visual Studio instead of pinning one release, because hosted
    /// images move between Visual Studio versions; with no supported instance
    /// we omit `-G` and let CMake pick its own default.
    fn resolve_cmake_generator(&self, config: &PlatformConfig) -> Option<String> {
        if let Some(generator) = &self.cmake_generator {
            return Some(generator.clone());
        }
        if let Some(generator) = &config.cmake_generator {
            return Some(generator.clone());
        }
        match detect_visual_studio_generator() {
            Some(generator) => {
                self.log_verbose(&format!("Detected CMake generator: {}", generator));
                Some(generator)
            }
            None => {
                self.log_warning(
                    "No supported Visual Studio instance detected; using CMake's default generator",
                );
                None
            }
        }
    }
}

fn detect_platform() -> Result<Platform> {
    cfg_select! {
        windows => Ok(Platform::Windows),
        target_os = "macos" => Ok(Platform::MacOS),
        target_os = "linux" => Ok(Platform::Linux),
        _ => bail!("Unsupported platform: {}", env::consts::OS),
    }
}

fn load_devkit_config(project_root: &Path) -> Option<DevKitConfig> {
    let config_path = project_root.join("maya-devkit-config.toml");
    if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => Some(config),
                Err(e) => {
                    eprintln!("Warning: Failed to parse maya-devkit-config.toml: {}", e);
                    None
                }
            },
            Err(e) => {
                eprintln!("Warning: Failed to read maya-devkit-config.toml: {}", e);
                None
            }
        }
    } else {
        None
    }
}

fn create_build_config() -> BuildConfig {
    let mut platforms = HashMap::new();

    platforms.insert(
        "windows".to_string(),
        PlatformConfig {
            rust_target: "x86_64-pc-windows-msvc".to_string(),
            plugin_ext: ".mll".to_string(),
            lib_ext: ".dll".to_string(),
            devkit_platform: "win".to_string(),
            cmake_generator: None,
        },
    );

    platforms.insert(
        "linux".to_string(),
        PlatformConfig {
            rust_target: "x86_64-unknown-linux-gnu".to_string(),
            plugin_ext: ".so".to_string(),
            lib_ext: ".so".to_string(),
            devkit_platform: "linux".to_string(),
            cmake_generator: Some("Unix Makefiles".to_string()),
        },
    );

    platforms.insert(
        "macos".to_string(),
        PlatformConfig {
            rust_target: "x86_64-apple-darwin".to_string(),
            plugin_ext: ".bundle".to_string(),
            lib_ext: ".dylib".to_string(),
            devkit_platform: "osx".to_string(),
            cmake_generator: Some("Unix Makefiles".to_string()),
        },
    );

    BuildConfig {
        maya_versions: vec![
            "2018".to_string(),
            "2019".to_string(),
            "2020".to_string(),
            "2021".to_string(),
            "2022".to_string(),
            "2023".to_string(),
            "2024".to_string(),
            "2025".to_string(),
            "2026".to_string(),
        ],
        platforms,
    }
}

/// Parses a Visual Studio `installationVersion` such as `17.14.37111.16`.
fn parse_visual_studio_version(version: &str) -> Option<(u32, u32, u32, u32)> {
    let mut parts = version
        .trim()
        .split('.')
        .map(|part| part.trim().parse().ok());
    let major = parts.next()??;
    let minor = parts.next().flatten().unwrap_or(0);
    let patch = parts.next().flatten().unwrap_or(0);
    let build = parts.next().flatten().unwrap_or(0);
    Some((major, minor, patch, build))
}

/// Maps a Visual Studio major version to its CMake generator name.
///
/// Unknown majors return `None` so callers fall back to CMake's own default
/// instead of requesting a generator this mapping cannot name correctly.
fn visual_studio_generator(major: u32) -> Option<String> {
    let year = match major {
        15 => "2017",
        16 => "2019",
        17 => "2022",
        18 => "2026",
        _ => return None,
    };
    Some(format!("Visual Studio {} {}", major, year))
}

/// Picks the newest generator from `vswhere` output, one version per line.
fn newest_visual_studio_generator(vswhere_output: &str) -> Option<String> {
    let mut versions: Vec<(u32, u32, u32, u32)> = vswhere_output
        .lines()
        .filter_map(parse_visual_studio_version)
        .collect();
    versions.sort_unstable_by(|left, right| right.cmp(left));
    versions
        .into_iter()
        .find_map(|(major, _, _, _)| visual_studio_generator(major))
}

/// Reports whether `cmake --help` lists a generator on this host.
fn cmake_supports_generator(cmake_help: &str, generator: &str) -> bool {
    cmake_help.lines().any(|line| {
        // The default generator is prefixed with `*` in `cmake --help` output.
        let candidate = line.trim_start().trim_start_matches('*').trim_start();
        let Some(rest) = candidate.strip_prefix(generator) else {
            return false;
        };
        rest.starts_with(char::is_whitespace) || rest.starts_with('=')
    })
}

/// Detects the newest Visual Studio instance this host can build with.
///
/// Returns `None` when no instance is installed, when `vswhere` is unavailable,
/// or when the local CMake is too old to know the detected generator; in those
/// cases the caller lets CMake choose its own default.
fn detect_visual_studio_generator() -> Option<String> {
    let program_files = env::var("ProgramFiles(x86)")
        .or_else(|_| env::var("ProgramFiles"))
        .ok()?;
    let vswhere = Path::new(&program_files)
        .join("Microsoft Visual Studio")
        .join("Installer")
        .join("vswhere.exe");
    if !vswhere.is_file() {
        return None;
    }
    let install_output = Command::new(&vswhere)
        .args([
            "-products",
            "*",
            "-format",
            "value",
            "-property",
            "installationVersion",
        ])
        .output()
        .ok()?;
    if !install_output.status.success() {
        return None;
    }
    let generator =
        newest_visual_studio_generator(&String::from_utf8_lossy(&install_output.stdout))?;
    let cmake_help = Command::new("cmake").arg("--help").output().ok()?;
    if !cmake_help.status.success() {
        return None;
    }
    cmake_supports_generator(&String::from_utf8_lossy(&cmake_help.stdout), &generator)
        .then_some(generator)
}

impl BuildContext {
    async fn setup_devkit(&self, maya_version: &str) -> Result<()> {
        // Explicit SDK roots are a contract; do not replace a mismatched root.
        for variable in ["MAYA_ROOT_DIR", "MAYA_LOCATION"] {
            if let Ok(root) = env::var(variable) {
                validate_maya_sdk_version(Path::new(&root), maya_version)?;
            }
        }
        let platform_name = platform_to_string(&self.current_platform);
        if let Some(platform_config) = self.config.platforms.get(&platform_name)
            && let Ok(existing_sdk) =
                self.resolve_maya_sdk_dir(&self.current_platform, platform_config, maya_version)
        {
            self.log_success(&format!(
                "Maya SDK/DevKit already exists: {}",
                existing_sdk.display()
            ));
            return Ok(());
        }

        self.log("Setting up Maya DevKit...");

        // Use official DevKit from config
        let devkit_config = self.devkit_config.as_ref().context(
            "Maya DevKit configuration not found. Please ensure maya-devkit-config.toml exists.",
        )?;

        let devkit_url = self.get_official_devkit_url(devkit_config, maya_version)?;

        self.log_verbose(&format!("Downloading from: {}", devkit_url));

        // Determine file type and download
        if devkit_url.ends_with(".zip") {
            self.download_and_extract_zip(&devkit_url).await?;
        } else if devkit_url.ends_with(".tgz") {
            self.download_and_extract_tgz(&devkit_url).await?;
        } else if devkit_url.ends_with(".dmg") {
            self.download_and_extract_dmg(&devkit_url).await?;
        } else {
            bail!("Unsupported DevKit archive format: {}", devkit_url);
        }

        self.log_success("Maya DevKit setup complete");
        let platform_config = self
            .config
            .platforms
            .get(&platform_name)
            .context("Missing platform configuration")?;
        self.resolve_maya_sdk_dir(&self.current_platform, platform_config, maya_version)?;
        Ok(())
    }

    fn get_official_devkit_url(
        &self,
        devkit_config: &DevKitConfig,
        maya_version: &str,
    ) -> Result<String> {
        let platform_name = platform_to_string(&self.current_platform);
        let override_name = format!(
            "MAYA_DEVKIT_URL_{}_{}",
            maya_version,
            platform_name.to_uppercase()
        );

        if let Ok(url) = env::var(&override_name)
            && !url.trim().is_empty()
        {
            return Ok(url);
        }

        if let Some(version_urls) = devkit_config.devkit.urls.get(maya_version) {
            if let Some(url) = version_urls.get(&platform_name) {
                Ok(url.clone())
            } else {
                bail!("No DevKit URL found for platform: {}", platform_name);
            }
        } else {
            bail!("No DevKit URL found for Maya version: {}", maya_version);
        }
    }

    fn resolve_maya_sdk_dir(
        &self,
        platform: &Platform,
        platform_config: &PlatformConfig,
        maya_version: &str,
    ) -> Result<PathBuf> {
        let platform_name = platform_to_string(platform);
        let mut candidates = Vec::new();

        if let Ok(maya_root_dir) = env::var("MAYA_ROOT_DIR") {
            candidates.push(PathBuf::from(maya_root_dir));
        }

        if let Ok(maya_location) = env::var("MAYA_LOCATION") {
            candidates.push(PathBuf::from(maya_location));
        }

        candidates.extend(default_maya_install_candidates(platform, maya_version));
        candidates.push(self.devkit_dir.join(&platform_config.devkit_platform));
        candidates.push(self.devkit_dir.clone());

        if let Some(devkit_config) = &self.devkit_config
            && let Some(structure_dir) = devkit_config.devkit.structure.get(&platform_name)
        {
            candidates.push(self.devkit_dir.join(structure_dir));
        }

        let checked: Vec<String> = candidates
            .iter()
            .map(|candidate| candidate.display().to_string())
            .collect();

        for candidate in candidates {
            if is_maya_sdk_dir(&candidate) {
                match validate_maya_sdk_version(&candidate, maya_version) {
                    Ok(()) => return Ok(candidate),
                    Err(error) => self.log_verbose(&error.to_string()),
                }
            }
        }

        bail!(
            "Maya {} SDK/DevKit with matching MAYA_API_VERSION not found for {}. Checked: {}",
            maya_version,
            platform_name,
            checked.join(", ")
        );
    }

    async fn download_and_extract_zip(&self, url: &str) -> Result<()> {
        let devkit_zip = self.project_root.join("maya-devkit.zip");

        // Download
        let response = reqwest::get(url)
            .await
            .context("Failed to download Maya DevKit")?;

        let bytes = response
            .bytes()
            .await
            .context("Failed to read DevKit download")?;

        async_fs::write(&devkit_zip, bytes)
            .await
            .context("Failed to write DevKit zip file")?;

        // Extract
        self.log_verbose("Extracting DevKit...");
        let file = std::fs::File::open(&devkit_zip).context("Failed to open DevKit zip")?;

        let mut archive = zip::ZipArchive::new(file).context("Failed to read zip archive")?;

        archive
            .extract(&self.project_root)
            .context("Failed to extract DevKit")?;

        // Find and rename extracted directory
        self.find_and_rename_devkit_dir()?;

        // Cleanup
        if devkit_zip.exists() {
            std::fs::remove_file(&devkit_zip).context("Failed to remove DevKit zip")?;
        }

        Ok(())
    }

    async fn download_and_extract_tgz(&self, url: &str) -> Result<()> {
        let devkit_tgz = self.project_root.join("maya-devkit.tgz");

        // Download
        let response = reqwest::get(url)
            .await
            .context("Failed to download Maya DevKit")?;

        let bytes = response
            .bytes()
            .await
            .context("Failed to read DevKit download")?;

        async_fs::write(&devkit_tgz, bytes)
            .await
            .context("Failed to write DevKit tgz file")?;

        // Extract
        self.log_verbose("Extracting DevKit...");
        let file = std::fs::File::open(&devkit_tgz).context("Failed to open DevKit tgz")?;

        let tar = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(tar);
        archive
            .unpack(&self.project_root)
            .context("Failed to extract DevKit")?;

        // Find and rename extracted directory
        self.find_and_rename_devkit_dir()?;

        // Cleanup
        if devkit_tgz.exists() {
            std::fs::remove_file(&devkit_tgz).context("Failed to remove DevKit tgz")?;
        }

        Ok(())
    }

    async fn download_and_extract_dmg(&self, url: &str) -> Result<()> {
        if !cfg!(target_os = "macos") {
            bail!("DMG DevKit archives can only be extracted on macOS runners");
        }

        let devkit_dmg = self.project_root.join("maya-devkit.dmg");
        let extract_dir = self.project_root.join("maya-devkit-dmg-extract");
        if extract_dir.exists() {
            std::fs::remove_dir_all(&extract_dir)
                .context("Failed to remove existing DMG extraction directory")?;
        }

        let response = reqwest::get(url)
            .await
            .context("Failed to download Maya DevKit")?;
        let bytes = response
            .bytes()
            .await
            .context("Failed to read DevKit download")?;
        async_fs::write(&devkit_dmg, bytes)
            .await
            .context("Failed to write DevKit dmg file")?;

        let attach = Command::new("hdiutil")
            .args([
                "attach",
                devkit_dmg.to_str().unwrap(),
                "-nobrowse",
                "-readonly",
            ])
            .output()
            .context("Failed to attach DevKit dmg")?;
        if !attach.status.success() {
            bail!("hdiutil attach failed: {}", command_output_summary(&attach));
        }

        let attach_stdout = String::from_utf8_lossy(&attach.stdout);
        let mount_point = attach_stdout
            .lines()
            .filter_map(|line| line.split('\t').next_back())
            .map(str::trim)
            .find(|part| part.starts_with("/Volumes/"))
            .map(ToOwned::to_owned)
            .context("Failed to determine DMG mount point")?;

        let copy = Command::new("ditto")
            .arg(&mount_point)
            .arg(&extract_dir)
            .output()
            .context("Failed to copy DevKit dmg contents")?;

        let detach = Command::new("hdiutil")
            .args(["detach", &mount_point])
            .output()
            .context("Failed to detach DevKit dmg")?;
        if !detach.status.success() {
            self.log_warning(&format!(
                "Failed to detach DevKit dmg: {}",
                command_output_summary(&detach)
            ));
        }

        if !copy.status.success() {
            bail!("ditto failed: {}", command_output_summary(&copy));
        }

        self.find_and_rename_devkit_dir()?;

        if devkit_dmg.exists() {
            std::fs::remove_file(&devkit_dmg).context("Failed to remove DevKit dmg")?;
        }
        if extract_dir.exists() {
            std::fs::remove_dir_all(&extract_dir)
                .context("Failed to remove DMG extraction directory")?;
        }

        Ok(())
    }

    fn find_and_rename_devkit_dir(&self) -> Result<()> {
        // Look for directories that might be the extracted DevKit
        let possible_names = ["Maya-devkit-master", "devkitBase", "devkit"];

        for name in &possible_names {
            let extracted_dir = self.project_root.join(name);
            if extracted_dir.exists() && extracted_dir.is_dir() {
                std::fs::rename(&extracted_dir, &self.devkit_dir)
                    .context("Failed to rename DevKit directory")?;
                self.log_verbose(&format!("Renamed {} to maya-devkit", name));
                return Ok(());
            }
        }

        let extract_dir = self.project_root.join("maya-devkit-dmg-extract");
        if extract_dir.exists()
            && let Some(sdk_dir) = find_sdk_dir_recursive(&extract_dir)
        {
            std::fs::rename(&sdk_dir, &self.devkit_dir)
                .context("Failed to move extracted DevKit directory")?;
            self.log_verbose(&format!("Moved {} to maya-devkit", sdk_dir.display()));
            return Ok(());
        }

        // If no standard directory found, look for any directory containing "devkit" or "Maya"
        for entry in std::fs::read_dir(&self.project_root)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap().to_string_lossy().to_lowercase();
                if name.contains("devkit") || name.contains("maya") {
                    std::fs::rename(&path, &self.devkit_dir)
                        .context("Failed to rename DevKit directory")?;
                    self.log_verbose(&format!("Renamed {} to maya-devkit", path.display()));
                    return Ok(());
                }
            }
        }

        bail!("Could not find extracted DevKit directory");
    }

    fn install_rust_targets(&self, platforms: &[Platform]) -> Result<()> {
        self.log("Installing Rust targets...");

        let mut targets = Vec::new();
        for platform in platforms {
            let platform_name = platform_to_string(platform);
            if let Some(config) = self.config.platforms.get(&platform_name) {
                targets.push(&config.rust_target);
            }
        }

        // Deduplicate
        targets.sort();
        targets.dedup();

        for target in targets {
            self.log_verbose(&format!("Installing target: {}", target));

            let output = Command::new("rustup")
                .args(["target", "add", target])
                .output()
                .context("Failed to run rustup")?;

            if output.status.success() {
                self.log_success(&format!("Installed: {}", target));
            } else {
                self.log_warning(&format!("Target {} may already be installed", target));
            }
        }

        Ok(())
    }

    fn build_rust_library(&self, platform: &Platform) -> Result<()> {
        let platform_name = platform_to_string(platform);
        self.log(&format!("Building Rust library for {}...", platform_name));

        let config = self
            .config
            .platforms
            .get(&platform_name)
            .context("Platform not found in config")?;

        let mut cmd = self.build_command("cargo");
        if *platform == Platform::Windows {
            for variable in [
                "RUSTFLAGS",
                "CARGO_ENCODED_RUSTFLAGS",
                "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS",
            ] {
                if env::var(variable)
                    .unwrap_or_default()
                    .contains("+crt-static")
                {
                    bail!(
                        "{} enables a static CRT; the Maya plugin requires the DLL CRT",
                        variable
                    );
                }
            }
        }
        cmd.args(["build", "--release", "--target", &config.rust_target]);
        self.log_verbose(&format!(
            "Running: cargo build --release --target {}",
            config.rust_target
        ));

        if self.verbose {
            cmd.arg("--verbose");
        }

        let output = cmd.output().context("Failed to run cargo build")?;

        if !output.status.success() {
            bail!("Rust build failed: {}", command_output_summary(&output));
        }

        // Generate C bindings
        self.log_verbose("Generating C bindings...");
        self.generate_c_bindings()?;

        self.log_success(&format!("Rust library built for {}", platform_name));
        Ok(())
    }

    fn generate_c_bindings(&self) -> Result<()> {
        let bindings_dir = self.project_root.join("build").join("include");
        std::fs::create_dir_all(&bindings_dir).context("Failed to create bindings directory")?;

        let output_file = bindings_dir.join("umbrella_maya_plugin.h");

        let output = Command::new("cbindgen")
            .args([
                "--config",
                "cbindgen.toml",
                "--crate",
                "umbrella_maya_plugin",
                "--output",
                output_file.to_str().unwrap(),
            ])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                self.log_verbose("C bindings generated successfully");
                Ok(())
            }
            Ok(output) => {
                bail!("cbindgen failed: {}", command_output_summary(&output));
            }
            Err(_) => {
                self.log_warning("cbindgen not found, installing...");

                let install_output = Command::new("cargo")
                    .args(["install", "cbindgen"])
                    .output()
                    .context("Failed to install cbindgen")?;

                if !install_output.status.success() {
                    bail!("Failed to install cbindgen");
                }

                // Retry generating bindings
                let retry_output = Command::new("cbindgen")
                    .args([
                        "--config",
                        "cbindgen.toml",
                        "--crate",
                        "umbrella_maya_plugin",
                        "--output",
                        output_file.to_str().unwrap(),
                    ])
                    .output()
                    .context("Failed to run cbindgen after installation")?;

                if !retry_output.status.success() {
                    bail!(
                        "cbindgen failed after installation: {}",
                        command_output_summary(&retry_output)
                    );
                }

                self.log_success("C bindings generated successfully");
                Ok(())
            }
        }
    }

    fn build_maya_plugin(&self, platform: &Platform, maya_version: &str) -> Result<()> {
        let platform_name = platform_to_string(platform);
        self.log(&format!(
            "Building Maya plugin for {} Maya {}...",
            platform_name, maya_version
        ));

        let config = self
            .config
            .platforms
            .get(&platform_name)
            .context("Platform not found in config")?;

        let maya_sdk_dir = self.resolve_maya_sdk_dir(platform, config, maya_version)?;

        // Create build directory
        let build_dir = self
            .project_root
            .join(format!("build_{}_{}", platform_name, maya_version));
        if build_dir.exists() {
            std::fs::remove_dir_all(&build_dir)
                .context("Failed to remove existing build directory")?;
        }
        std::fs::create_dir_all(&build_dir).context("Failed to create build directory")?;

        // Configure CMake
        let mut cmake_args = vec![
            "..".to_string(),
            format!("-DCMAKE_BUILD_TYPE=Release"),
            format!("-DMAYA_VERSION={}", maya_version),
            format!("-DMAYA_ROOT_DIR={}", maya_sdk_dir.display()),
            format!("-DRUST_TARGET={}", config.rust_target),
            format!("-DBUILD_TESTS=OFF"),
            format!(
                "-DUMBRELLA_OUTPUT_DIR={}",
                build_dir.join("maya-plugin-output").display()
            ),
        ];

        // Platform-specific generator
        if let Some(generator) = self.resolve_cmake_generator(config) {
            cmake_args.extend(["-G".to_string(), generator]);
        }
        if let Some(toolchain) = &self.toolchain {
            cmake_args.push(format!("-DCMAKE_CXX_COMPILER={}", toolchain.tools["cl"]));
            cmake_args.push(format!("-DCMAKE_C_COMPILER={}", toolchain.tools["cl"]));
        }

        self.log_verbose(&format!("Running: cmake {}", cmake_args.join(" ")));

        let cmake_output = self
            .build_command("cmake")
            .args(&cmake_args)
            .current_dir(&build_dir)
            .output()
            .context("Failed to run cmake configure")?;

        if !cmake_output.status.success() {
            bail!(
                "CMake configuration failed: {}",
                command_output_summary(&cmake_output)
            );
        }

        // Build
        self.log_verbose("Running: cmake --build . --config Release");

        let build_output = self
            .build_command("cmake")
            .args(["--build", ".", "--config", "Release"])
            .current_dir(&build_dir)
            .output()
            .context("Failed to run cmake build")?;

        if !build_output.status.success() {
            bail!(
                "CMake build failed: {}",
                command_output_summary(&build_output)
            );
        }

        self.log_success(&format!(
            "Maya plugin built for {} Maya {}",
            platform_name, maya_version
        ));
        Ok(())
    }

    fn package_artifacts(&self, platform: &Platform, maya_version: &str) -> Result<()> {
        let platform_name = platform_to_string(platform);
        self.log(&format!(
            "Packaging artifacts for {} Maya {}...",
            platform_name, maya_version
        ));

        let config = self
            .config
            .platforms
            .get(&platform_name)
            .context("Platform not found in config")?;

        // Create output directory
        let output_dir = self
            .dist_dir
            .join(format!("maya{}-{}", maya_version, platform_name));
        if output_dir.exists()
            && let Err(err) = std::fs::remove_dir_all(&output_dir)
        {
            bail!(
                "Failed to replace {}: {}. The existing plugin is probably loaded by Maya. Close Maya or unload umbrella_maya, then rebuild. Do not load stale files from this directory; use the installable module package under dist/modules after a clean build.",
                output_dir.display(),
                err
            );
        }
        std::fs::create_dir_all(&output_dir).context("Failed to create output directory")?;

        let build_dir = self
            .project_root
            .join(format!("build_{}_{}", platform_name, maya_version));
        let cmake_output_dir = self.dist_dir.join(format!("maya{}", maya_version));
        let plugin_search_dirs = [build_dir.as_path(), cmake_output_dir.as_path()];

        let expected_plugin_name = format!("umbrella_maya{}", config.plugin_ext);
        let mut plugin_found = false;
        for search_dir in plugin_search_dirs {
            if !search_dir.exists() {
                continue;
            }

            for entry in walkdir::WalkDir::new(search_dir) {
                let entry = entry.context("Failed to walk plugin search directory")?;
                let path = entry.path();

                if path.is_file()
                    && let Some(filename) = path.file_name()
                    && filename.to_string_lossy() == expected_plugin_name
                {
                    let dest = output_dir.join(filename);
                    std::fs::copy(path, &dest).context("Failed to copy plugin file")?;
                    self.log_verbose(&format!(
                        "Copied: {}",
                        dest.file_name().unwrap().to_string_lossy()
                    ));
                    plugin_found = true;
                }
            }
        }

        if !plugin_found {
            bail!(
                "No Maya plugin file named {} found for Maya {} {}",
                expected_plugin_name,
                maya_version,
                platform_name
            );
        }

        let target_dir = self
            .project_root
            .join("target")
            .join(&config.rust_target)
            .join("release");

        let mut lib_found = false;
        if target_dir.exists() {
            for entry in
                std::fs::read_dir(&target_dir).context("Failed to read target directory")?
            {
                let entry = entry.context("Failed to read directory entry")?;
                let path = entry.path();

                if path.is_file() {
                    let filename = path.file_name().unwrap().to_string_lossy();
                    if filename.contains("umbrella_maya_plugin")
                        && filename.ends_with(&config.lib_ext)
                    {
                        let dest = output_dir.join(path.file_name().unwrap());
                        std::fs::copy(&path, &dest).context("Failed to copy Rust library")?;
                        self.log_verbose(&format!(
                            "Copied: {}",
                            dest.file_name().unwrap().to_string_lossy()
                        ));
                        lib_found = true;
                    }
                }
            }
        }

        if !lib_found {
            bail!(
                "No Rust runtime library found with extension {} for Maya {} {}",
                config.lib_ext,
                maya_version,
                platform_name
            );
        }

        // Create version information
        let version_file = output_dir.join("VERSION.txt");
        let version_content = format!(
            "Maya Version: {}\nPlatform: {}\nBuild Date: {}\nRust Target: {}\n",
            maya_version,
            platform_name,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            config.rust_target
        );

        std::fs::write(&version_file, version_content).context("Failed to write version file")?;

        self.package_maya_module(&output_dir, platform, maya_version)?;

        self.log_success(&format!("Artifacts packaged in: {}", output_dir.display()));
        Ok(())
    }

    fn package_maya_module(
        &self,
        artifacts_dir: &Path,
        platform: &Platform,
        maya_version: &str,
    ) -> Result<PathBuf> {
        let platform_name = platform_to_string(platform);
        let module_parent = self.dist_dir.join("modules");
        let module_package_dir = module_parent.join(format!(
            "UmbrellaMayaPlugin-{}-maya{}-{}",
            env!("CARGO_PKG_VERSION"),
            maya_version,
            platform_name
        ));
        let module_root = module_package_dir.join("UmbrellaMayaPlugin");
        let plugin_dir = module_root.join("plug-ins");

        if module_package_dir.exists() {
            std::fs::remove_dir_all(&module_package_dir)
                .context("Failed to remove existing Maya module package")?;
        }

        std::fs::create_dir_all(&plugin_dir)
            .context("Failed to create Maya module plug-ins directory")?;

        for entry in
            std::fs::read_dir(artifacts_dir).context("Failed to read artifacts directory")?
        {
            let entry = entry.context("Failed to read artifact entry")?;
            let path = entry.path();
            if path.is_file() {
                let dest = plugin_dir.join(path.file_name().unwrap());
                std::fs::copy(&path, &dest)
                    .context("Failed to copy artifact into module package")?;
            }
        }

        let module_file = module_package_dir.join("UmbrellaMayaPlugin.mod");
        std::fs::write(
            &module_file,
            maya_module_file_content(&platform_name, maya_version),
        )
        .context("Failed to write Maya module file")?;

        std::fs::write(
            module_package_dir.join("README_INSTALL.txt"),
            format!(
                "Umbrella Maya Plugin\n\nInstall:\n1. Copy UmbrellaMayaPlugin.mod and the UmbrellaMayaPlugin directory into your Maya modules directory.\n2. Default Windows user modules directory: %USERPROFILE%\\Documents\\maya\\modules\n3. Start Maya {} and load umbrella_maya from Plug-in Manager.\n\nBuilt package: {}\n",
                maya_version,
                module_package_dir.display()
            ),
        )
        .context("Failed to write module install notes")?;

        self.log_success(&format!(
            "Installable Maya module package: {}",
            module_package_dir.display()
        ));

        Ok(module_package_dir)
    }
}

fn find_sdk_dir_recursive(root: &Path) -> Option<PathBuf> {
    for entry in walkdir::WalkDir::new(root).max_depth(5) {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_dir() && is_maya_sdk_dir(path) {
            return Some(path.to_path_buf());
        }
    }
    None
}

fn platform_to_string(platform: &Platform) -> String {
    match platform {
        Platform::Windows => "windows".to_string(),
        Platform::Linux => "linux".to_string(),
        Platform::MacOS => "macos".to_string(),
    }
}

fn default_maya_install_candidates(platform: &Platform, maya_version: &str) -> Vec<PathBuf> {
    match platform {
        Platform::Windows => vec![PathBuf::from(format!(
            "C:/Program Files/Autodesk/Maya{}",
            maya_version
        ))],
        Platform::Linux => vec![
            PathBuf::from(format!("/usr/autodesk/maya{}", maya_version)),
            PathBuf::from(format!("/opt/autodesk/maya{}", maya_version)),
        ],
        Platform::MacOS => vec![
            PathBuf::from(format!("/Applications/Autodesk/Maya{}", maya_version)),
            PathBuf::from(format!("/Applications/Autodesk/maya{}", maya_version)),
        ],
    }
}

fn is_maya_sdk_dir(path: &Path) -> bool {
    path.join("include").join("maya").join("MFn.h").exists()
        || path
            .join("Maya.app")
            .join("Contents")
            .join("include")
            .join("maya")
            .join("MFn.h")
            .exists()
}

fn is_full_version(version: &str, components: usize) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == components
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

fn maya_api_year(header: &str) -> Result<u32> {
    let version = header
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 3 && fields[0] == "#define" && fields[1] == "MAYA_API_VERSION" {
                fields[2].parse::<u32>().ok()
            } else {
                None
            }
        })
        .next()
        .context("MTypes.h does not define a numeric MAYA_API_VERSION")?;
    // Modern Maya encodes the year in the first four of eight digits.
    let year = version / 10_000;
    if !(2018..=2026).contains(&year) {
        bail!("Unsupported MAYA_API_VERSION {}", version);
    }
    Ok(year)
}

fn validate_maya_sdk_version(root: &Path, requested: &str) -> Result<()> {
    let normal = root.join("include/maya/MTypes.h");
    let header = if normal.exists() {
        normal
    } else {
        root.join("Maya.app/Contents/include/maya/MTypes.h")
    };
    let content = std::fs::read_to_string(&header)
        .with_context(|| format!("Cannot read {}", header.display()))?;
    let actual = maya_api_year(&content)?;
    if actual.to_string() != requested {
        bail!(
            "SDK {} targets Maya {}, requested Maya {}",
            root.display(),
            actual,
            requested
        );
    }
    Ok(())
}

fn command_output_summary(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => format!("process exited with status {}", output.status),
        (false, true) => format!("\nstdout:\n{}", stdout),
        (true, false) => format!("\nstderr:\n{}", stderr),
        (false, false) => format!("\nstdout:\n{}\nstderr:\n{}", stdout, stderr),
    }
}

fn maya_module_file_content(platform_name: &str, maya_version: &str) -> String {
    let maya_platform = match platform_name {
        "windows" => "win64",
        "linux" => "linux",
        "macos" => "mac",
        _ => platform_name,
    };

    format!(
        "+ MAYAVERSION:{maya_version} PLATFORM:{maya_platform} UmbrellaMayaPlugin {} ./UmbrellaMayaPlugin\nMAYA_PLUG_IN_PATH +:= plug-ins\nPATH +:= plug-ins\nLD_LIBRARY_PATH +:= plug-ins\nDYLD_LIBRARY_PATH +:= plug-ins\n",
        env!("CARGO_PKG_VERSION")
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = MayaBuildArgs::parse();

    let mut ctx = BuildContext::new(args.verbose)?;

    ctx.log("Starting Umbrella Maya Plugin build...");

    // Clean build directories
    if args.clean {
        ctx.log("Cleaning build directories...");

        let patterns = ["build", "build_*", "dist"];
        for pattern in &patterns {
            for entry in glob::glob(&format!("{}/{}", ctx.project_root.display(), pattern))
                .context("Failed to glob pattern")?
            {
                let path = entry.context("Failed to read glob entry")?;
                if path.exists() {
                    if path.is_dir() {
                        std::fs::remove_dir_all(&path).context("Failed to remove directory")?;
                    } else {
                        std::fs::remove_file(&path).context("Failed to remove file")?;
                    }
                    ctx.log_verbose(&format!("Removed: {}", path.display()));
                }
            }
        }

        ctx.log_success("Build directories cleaned");
        return Ok(());
    }

    ctx.configure_toolchain(&args)?;

    // Determine target platforms
    let platforms = if args.current_only {
        vec![ctx.current_platform.clone()]
    } else if args.all_platforms {
        vec![Platform::Windows, Platform::Linux, Platform::MacOS]
    } else if let Some(platform) = args.platform {
        vec![platform]
    } else {
        vec![ctx.current_platform.clone()]
    };

    // Determine Maya versions
    let maya_versions = if args.all_versions {
        ctx.config.maya_versions.clone()
    } else if let Some(version) = args.maya_version {
        vec![version]
    } else {
        vec!["2024".to_string()]
    };

    ctx.log(&format!("Target platforms: {:?}", platforms));
    ctx.log(&format!("Target Maya versions: {:?}", maya_versions));

    // Setup DevKit (use the first Maya version for DevKit download)
    if !args.skip_cpp {
        let first_maya_version = maya_versions
            .first()
            .context("No Maya versions specified")?;
        ctx.setup_devkit(first_maya_version).await?;
    }

    // Install Rust targets
    if !args.skip_rust {
        ctx.install_rust_targets(&platforms)?;
    }

    // Build each platform and version combination
    let mut success_count = 0;
    let total_count = platforms.len() * maya_versions.len();

    for platform in &platforms {
        // Build Rust library
        if !args.skip_rust
            && let Err(e) = ctx.build_rust_library(platform)
        {
            ctx.log_error(&format!(
                "Failed to build Rust library for {:?}: {}",
                platform, e
            ));
            continue;
        }

        for maya_version in &maya_versions {
            ctx.log(&format!("\n{}", "=".repeat(60)));
            ctx.log(&format!("Building: {:?} Maya {}", platform, maya_version));
            ctx.log(&"=".repeat(60));

            let mut build_success = true;

            // Build C++ plugin
            if !args.skip_cpp
                && let Err(e) = ctx.build_maya_plugin(platform, maya_version)
            {
                ctx.log_error(&format!("Failed to build Maya plugin: {}", e));
                build_success = false;
            }

            // Package artifacts
            if build_success && let Err(e) = ctx.package_artifacts(platform, maya_version) {
                ctx.log_error(&format!("Failed to package artifacts: {}", e));
                build_success = false;
            }

            if build_success {
                success_count += 1;
                ctx.log_success(&format!("{:?} Maya {} completed", platform, maya_version));
            } else {
                ctx.log_error(&format!("{:?} Maya {} failed", platform, maya_version));
            }
        }
    }

    // Summary
    ctx.log(&format!("\n{}", "=".repeat(60)));
    ctx.log("Build Summary");
    ctx.log(&"=".repeat(60));
    ctx.log(&format!(
        "Successful builds: {}/{}",
        success_count, total_count
    ));
    ctx.log(&format!("Output directory: {}", ctx.dist_dir.display()));

    if success_count > 0 {
        ctx.log("\nBuilt packages:");
        if ctx.dist_dir.exists() {
            for entry in
                std::fs::read_dir(&ctx.dist_dir).context("Failed to read dist directory")?
            {
                let entry = entry.context("Failed to read directory entry")?;
                if entry.path().is_dir() {
                    let file_count = std::fs::read_dir(entry.path())
                        .map(|entries| entries.count())
                        .unwrap_or(0);
                    ctx.log(&format!(
                        "  {} ({} files)",
                        entry.file_name().to_string_lossy(),
                        file_count
                    ));
                }
            }
        }
    }

    if success_count == total_count {
        ctx.log_success("\nAll builds completed successfully!");
        Ok(())
    } else {
        ctx.log_error("\nSome builds failed!");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_year_accepts_updates_and_rejects_unverifiable_headers() {
        assert_eq!(
            maya_api_year("#define MAYA_API_VERSION 20240205").unwrap(),
            2024
        );
        assert_eq!(
            maya_api_year("\t#define\tMAYA_API_VERSION\t20250000 // SDK").unwrap(),
            2025
        );
        assert!(maya_api_year("// #define MAYA_API_VERSION 20240000").is_err());
        assert!(maya_api_year("#define MAYA_API_VERSION unknown").is_err());
        assert!(maya_api_year("#define MAYA_API_VERSION 850").is_err());
    }

    #[test]
    fn full_versions_cannot_silently_select_the_latest_patch() {
        assert!(is_full_version("14.44.35207", 3));
        assert!(is_full_version("10.0.26100.0", 4));
        assert!(!is_full_version("14.44", 3));
        assert!(!is_full_version("latest", 3));
    }

    #[test]
    fn explicit_sdk_year_is_checked_before_compilation() {
        let directory =
            env::temp_dir().join(format!("umbrella-sdk-contract-{}", std::process::id()));
        let headers = directory.join("include/maya");
        std::fs::create_dir_all(&headers).unwrap();
        std::fs::write(
            headers.join("MTypes.h"),
            "#define MAYA_API_VERSION 20240205",
        )
        .unwrap();
        assert!(validate_maya_sdk_version(&directory, "2024").is_ok());
        assert!(
            validate_maya_sdk_version(&directory, "2025")
                .unwrap_err()
                .to_string()
                .contains("requested Maya 2025")
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn visual_studio_generators_follow_the_installed_major_version() {
        assert_eq!(
            visual_studio_generator(18).as_deref(),
            Some("Visual Studio 18 2026")
        );
        assert_eq!(
            visual_studio_generator(17).as_deref(),
            Some("Visual Studio 17 2022")
        );
        assert_eq!(visual_studio_generator(14), None);
        // A future release is not guessed; CMake picks its own default instead.
        assert_eq!(visual_studio_generator(19), None);
    }

    #[test]
    fn newest_installed_visual_studio_wins() {
        let output = "17.14.37111.16\r\n18.1.11309.65\r\n15.9.37506.8\r\n";
        assert_eq!(
            newest_visual_studio_generator(output).as_deref(),
            Some("Visual Studio 18 2026")
        );
        assert_eq!(
            newest_visual_studio_generator("17.14.37111.16\n17.4.33213.308\n").as_deref(),
            Some("Visual Studio 17 2022")
        );
        assert_eq!(newest_visual_studio_generator(""), None);
        assert_eq!(newest_visual_studio_generator("14.0.25420.1"), None);
    }

    #[test]
    fn generator_support_is_read_from_cmake_help() {
        let help = "The following generators are available on this platform:\n".to_owned()
            + "  Visual Studio 18 2026        = Generates Visual Studio 2026 project files.\n"
            + "* Visual Studio 17 2022        = Generates Visual Studio 2022 project files.\n"
            + "  Unix Makefiles               = Generates standard UNIX makefiles.\n";
        assert!(cmake_supports_generator(&help, "Visual Studio 18 2026"));
        assert!(cmake_supports_generator(&help, "Visual Studio 17 2022"));
        assert!(cmake_supports_generator(&help, "Unix Makefiles"));
        assert!(!cmake_supports_generator(&help, "Visual Studio 16 2019"));
        // A prefix must not match a longer generator name.
        assert!(!cmake_supports_generator(&help, "Unix Make"));
    }

    #[test]
    fn portable_toolchain_requires_version_selectors() {
        assert!(
            MayaBuildArgs::try_parse_from(["cargo-maya-build", "--msvc-kit", "msvc-kit"]).is_err()
        );
        assert!(
            MayaBuildArgs::try_parse_from(["cargo-maya-build", "--msvc-version", "14.44.35207"])
                .is_err()
        );
    }
}
