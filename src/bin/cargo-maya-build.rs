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
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Result, Context, bail};
use clap::{Parser, ValueEnum};
use colored::*;
use serde::{Deserialize, Serialize};
use tokio::fs as async_fs;

#[derive(Parser)]
#[command(about = "🛡️ Umbrella Maya Plugin Cross-platform Build Tool")]
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
    cmake_generator: String,
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
        })
    }

    fn log(&self, message: &str) {
        println!("{}", message);
    }

    fn log_verbose(&self, message: &str) {
        if self.verbose {
            println!("{} {}", "🔧".blue(), message.dimmed());
        }
    }

    fn log_success(&self, message: &str) {
        println!("{} {}", "✅".green(), message.green());
    }

    fn log_error(&self, message: &str) {
        eprintln!("{} {}", "❌".red(), message.red());
    }

    fn log_warning(&self, message: &str) {
        println!("{} {}", "⚠️".yellow(), message.yellow());
    }
}

fn detect_platform() -> Result<Platform> {
    match env::consts::OS {
        "windows" => Ok(Platform::Windows),
        "macos" => Ok(Platform::MacOS),
        "linux" => Ok(Platform::Linux),
        os => bail!("Unsupported platform: {}", os),
    }
}

fn load_devkit_config(project_root: &PathBuf) -> Option<DevKitConfig> {
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

    platforms.insert("windows".to_string(), PlatformConfig {
        rust_target: "x86_64-pc-windows-msvc".to_string(),
        plugin_ext: ".mll".to_string(),
        lib_ext: ".dll".to_string(),
        devkit_platform: "win".to_string(),
        cmake_generator: "Visual Studio 17 2022".to_string(),
    });

    platforms.insert("linux".to_string(), PlatformConfig {
        rust_target: "x86_64-unknown-linux-gnu".to_string(),
        plugin_ext: ".so".to_string(),
        lib_ext: ".so".to_string(),
        devkit_platform: "linux".to_string(),
        cmake_generator: "Unix Makefiles".to_string(),
    });

    platforms.insert("macos".to_string(), PlatformConfig {
        rust_target: "x86_64-apple-darwin".to_string(),
        plugin_ext: ".bundle".to_string(),
        lib_ext: ".dylib".to_string(),
        devkit_platform: "osx".to_string(),
        cmake_generator: "Unix Makefiles".to_string(),
    });

    BuildConfig {
        maya_versions: vec![
            "2018".to_string(),
            "2020".to_string(),
            "2022".to_string(),
            "2023".to_string(),
            "2024".to_string(),
        ],
        platforms,
    }
}

impl BuildContext {
    async fn setup_devkit(&self, maya_version: &str) -> Result<()> {
        if self.devkit_dir.exists() {
            self.log_success("Maya DevKit already exists");
            return Ok(());
        }

        self.log("📦 Setting up Maya DevKit...");

        // Use official DevKit from config
        let devkit_config = self.devkit_config.as_ref()
            .context("Maya DevKit configuration not found. Please ensure maya-devkit-config.toml exists.")?;

        let devkit_url = self.get_official_devkit_url(devkit_config, maya_version)?;

        self.log_verbose(&format!("Downloading from: {}", devkit_url));

        // Determine file type and download
        if devkit_url.ends_with(".zip") {
            self.download_and_extract_zip(&devkit_url).await?;
        } else if devkit_url.ends_with(".tgz") {
            self.download_and_extract_tgz(&devkit_url).await?;
        } else if devkit_url.ends_with(".dmg") {
            bail!("DMG extraction not supported in this build tool. Please extract manually.");
        } else {
            bail!("Unsupported DevKit archive format: {}", devkit_url);
        }

        self.log_success("Maya DevKit setup complete");
        Ok(())
    }

    fn get_official_devkit_url(&self, devkit_config: &DevKitConfig, maya_version: &str) -> Result<String> {
        let platform_name = platform_to_string(&self.current_platform);

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

    async fn download_and_extract_zip(&self, url: &str) -> Result<()> {
        let devkit_zip = self.project_root.join("maya-devkit.zip");

        // Download
        let response = reqwest::get(url).await
            .context("Failed to download Maya DevKit")?;

        let bytes = response.bytes().await
            .context("Failed to read DevKit download")?;

        async_fs::write(&devkit_zip, bytes).await
            .context("Failed to write DevKit zip file")?;

        // Extract
        self.log_verbose("Extracting DevKit...");
        let file = std::fs::File::open(&devkit_zip)
            .context("Failed to open DevKit zip")?;

        let mut archive = zip::ZipArchive::new(file)
            .context("Failed to read zip archive")?;

        archive.extract(&self.project_root)
            .context("Failed to extract DevKit")?;

        // Find and rename extracted directory
        self.find_and_rename_devkit_dir()?;

        // Cleanup
        if devkit_zip.exists() {
            std::fs::remove_file(&devkit_zip)
                .context("Failed to remove DevKit zip")?;
        }

        Ok(())
    }

    async fn download_and_extract_tgz(&self, url: &str) -> Result<()> {
        let devkit_tgz = self.project_root.join("maya-devkit.tgz");

        // Download
        let response = reqwest::get(url).await
            .context("Failed to download Maya DevKit")?;

        let bytes = response.bytes().await
            .context("Failed to read DevKit download")?;

        async_fs::write(&devkit_tgz, bytes).await
            .context("Failed to write DevKit tgz file")?;

        // Extract
        self.log_verbose("Extracting DevKit...");
        let file = std::fs::File::open(&devkit_tgz)
            .context("Failed to open DevKit tgz")?;

        let tar = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(&self.project_root)
            .context("Failed to extract DevKit")?;

        // Find and rename extracted directory
        self.find_and_rename_devkit_dir()?;

        // Cleanup
        if devkit_tgz.exists() {
            std::fs::remove_file(&devkit_tgz)
                .context("Failed to remove DevKit tgz")?;
        }

        Ok(())
    }

    fn find_and_rename_devkit_dir(&self) -> Result<()> {
        // Look for directories that might be the extracted DevKit
        let possible_names = [
            "Maya-devkit-master",
            "devkitBase",
            "devkit",
        ];

        for name in &possible_names {
            let extracted_dir = self.project_root.join(name);
            if extracted_dir.exists() && extracted_dir.is_dir() {
                std::fs::rename(&extracted_dir, &self.devkit_dir)
                    .context("Failed to rename DevKit directory")?;
                self.log_verbose(&format!("Renamed {} to maya-devkit", name));
                return Ok(());
            }
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
        self.log("🦀 Installing Rust targets...");

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
                .args(&["target", "add", target])
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
        self.log(&format!("🦀 Building Rust library for {}...", platform_name));

        let config = self.config.platforms.get(&platform_name)
            .context("Platform not found in config")?;

        // Build Rust library
        let mut cmd = Command::new("cargo");

        // Only use target if it's different from current platform
        if *platform != self.current_platform {
            cmd.args(&["build", "--release", "--target", &config.rust_target]);
            self.log_verbose(&format!("Running: cargo build --release --target {}", config.rust_target));
        } else {
            cmd.args(&["build", "--release"]);
            self.log_verbose("Running: cargo build --release");
        }

        if self.verbose {
            cmd.arg("--verbose");
        }

        let output = cmd.output()
            .context("Failed to run cargo build")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Rust build failed: {}", stderr);
        }

        // Generate C bindings
        self.log_verbose("Generating C bindings...");
        self.generate_c_bindings()?;

        self.log_success(&format!("Rust library built for {}", platform_name));
        Ok(())
    }

    fn generate_c_bindings(&self) -> Result<()> {
        let bindings_dir = self.project_root.join("build").join("include");
        std::fs::create_dir_all(&bindings_dir)
            .context("Failed to create bindings directory")?;

        let output_file = bindings_dir.join("umbrella_maya_plugin.h");

        let output = Command::new("cbindgen")
            .args(&[
                "--config", "cbindgen.toml",
                "--crate", "umbrella_maya_plugin",
                "--output", output_file.to_str().unwrap()
            ])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                self.log_verbose("C bindings generated successfully");
                Ok(())
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                bail!("cbindgen failed: {}", stderr);
            }
            Err(_) => {
                self.log_warning("cbindgen not found, installing...");

                let install_output = Command::new("cargo")
                    .args(&["install", "cbindgen"])
                    .output()
                    .context("Failed to install cbindgen")?;

                if !install_output.status.success() {
                    bail!("Failed to install cbindgen");
                }

                // Retry generating bindings
                let retry_output = Command::new("cbindgen")
                    .args(&[
                        "--config", "cbindgen.toml",
                        "--crate", "umbrella_maya_plugin",
                        "--output", output_file.to_str().unwrap()
                    ])
                    .output()
                    .context("Failed to run cbindgen after installation")?;

                if !retry_output.status.success() {
                    let stderr = String::from_utf8_lossy(&retry_output.stderr);
                    bail!("cbindgen failed after installation: {}", stderr);
                }

                self.log_success("C bindings generated successfully");
                Ok(())
            }
        }
    }

    fn build_maya_plugin(&self, platform: &Platform, maya_version: &str) -> Result<()> {
        let platform_name = platform_to_string(platform);
        self.log(&format!("🏗️ Building Maya plugin for {} Maya {}...", platform_name, maya_version));

        let config = self.config.platforms.get(&platform_name)
            .context("Platform not found in config")?;

        // Check DevKit path
        let devkit_platform_dir = self.devkit_dir.join(&config.devkit_platform);
        if !devkit_platform_dir.exists() {
            bail!("Maya DevKit not found for {}: {}", platform_name, devkit_platform_dir.display());
        }

        // Create build directory
        let build_dir = self.project_root.join(format!("build_{}_{}", platform_name, maya_version));
        if build_dir.exists() {
            std::fs::remove_dir_all(&build_dir)
                .context("Failed to remove existing build directory")?;
        }
        std::fs::create_dir_all(&build_dir)
            .context("Failed to create build directory")?;

        // Configure CMake
        let mut cmake_args = vec![
            "..".to_string(),
            format!("-DCMAKE_BUILD_TYPE=Release"),
            format!("-DMAYA_VERSION={}", maya_version),
            format!("-DMAYA_ROOT_DIR={}", devkit_platform_dir.display()),
            format!("-DRUST_TARGET={}", config.rust_target),
            format!("-DBUILD_TESTS=OFF"),
        ];

        // Platform-specific generator
        cmake_args.extend(["-G".to_string(), config.cmake_generator.clone()]);

        self.log_verbose(&format!("Running: cmake {}", cmake_args.join(" ")));

        let cmake_output = Command::new("cmake")
            .args(&cmake_args)
            .current_dir(&build_dir)
            .output()
            .context("Failed to run cmake configure")?;

        if !cmake_output.status.success() {
            let stderr = String::from_utf8_lossy(&cmake_output.stderr);
            bail!("CMake configuration failed: {}", stderr);
        }

        // Build
        self.log_verbose("Running: cmake --build . --config Release");

        let build_output = Command::new("cmake")
            .args(&["--build", ".", "--config", "Release"])
            .current_dir(&build_dir)
            .output()
            .context("Failed to run cmake build")?;

        if !build_output.status.success() {
            let stderr = String::from_utf8_lossy(&build_output.stderr);
            bail!("CMake build failed: {}", stderr);
        }

        self.log_success(&format!("Maya plugin built for {} Maya {}", platform_name, maya_version));
        Ok(())
    }

    fn package_artifacts(&self, platform: &Platform, maya_version: &str) -> Result<()> {
        let platform_name = platform_to_string(platform);
        self.log(&format!("📦 Packaging artifacts for {} Maya {}...", platform_name, maya_version));

        let config = self.config.platforms.get(&platform_name)
            .context("Platform not found in config")?;

        // Create output directory
        let output_dir = self.dist_dir.join(format!("maya{}-{}", maya_version, platform_name));
        if output_dir.exists() {
            std::fs::remove_dir_all(&output_dir)
                .context("Failed to remove existing output directory")?;
        }
        std::fs::create_dir_all(&output_dir)
            .context("Failed to create output directory")?;

        // Find and copy plugin files
        let build_dir = self.project_root.join(format!("build_{}_{}", platform_name, maya_version));

        let mut plugin_found = false;
        for entry in walkdir::WalkDir::new(&build_dir) {
            let entry = entry.context("Failed to walk build directory")?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext.to_string_lossy() == config.plugin_ext.trim_start_matches('.') {
                        let dest = output_dir.join(path.file_name().unwrap());
                        std::fs::copy(path, &dest)
                            .context("Failed to copy plugin file")?;
                        self.log_verbose(&format!("Copied: {}", dest.file_name().unwrap().to_string_lossy()));
                        plugin_found = true;
                    }
                }
            }
        }

        if !plugin_found {
            self.log_warning(&format!("No plugin file found with extension {}", config.plugin_ext));
        }

        // Find and copy Rust library
        let target_dir = if *platform == self.current_platform {
            self.project_root.join("target").join("release")
        } else {
            self.project_root.join("target").join(&config.rust_target).join("release")
        };

        let mut lib_found = false;
        if target_dir.exists() {
            for entry in std::fs::read_dir(&target_dir).context("Failed to read target directory")? {
                let entry = entry.context("Failed to read directory entry")?;
                let path = entry.path();

                if path.is_file() {
                    let filename = path.file_name().unwrap().to_string_lossy();
                    if filename.contains("umbrella_maya_plugin") && filename.ends_with(&config.lib_ext) {
                        let dest = output_dir.join(path.file_name().unwrap());
                        std::fs::copy(&path, &dest)
                            .context("Failed to copy Rust library")?;
                        self.log_verbose(&format!("Copied: {}", dest.file_name().unwrap().to_string_lossy()));
                        lib_found = true;
                    }
                }
            }
        }

        if !lib_found {
            self.log_warning(&format!("No Rust library found with extension {}", config.lib_ext));
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

        std::fs::write(&version_file, version_content)
            .context("Failed to write version file")?;

        self.log_success(&format!("Artifacts packaged in: {}", output_dir.display()));
        Ok(())
    }
}

fn platform_to_string(platform: &Platform) -> String {
    match platform {
        Platform::Windows => "windows".to_string(),
        Platform::Linux => "linux".to_string(),
        Platform::MacOS => "macos".to_string(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = MayaBuildArgs::parse();

    let ctx = BuildContext::new(args.verbose)?;

    ctx.log("🚀 Starting Umbrella Maya Plugin build...");

    // Clean build directories
    if args.clean {
        ctx.log("🧹 Cleaning build directories...");

        let patterns = ["build_*", "dist"];
        for pattern in &patterns {
            for entry in glob::glob(&format!("{}/{}", ctx.project_root.display(), pattern))
                .context("Failed to glob pattern")? {
                let path = entry.context("Failed to read glob entry")?;
                if path.exists() {
                    if path.is_dir() {
                        std::fs::remove_dir_all(&path)
                            .context("Failed to remove directory")?;
                    } else {
                        std::fs::remove_file(&path)
                            .context("Failed to remove file")?;
                    }
                    ctx.log_verbose(&format!("Removed: {}", path.display()));
                }
            }
        }

        ctx.log_success("Build directories cleaned");
        return Ok(());
    }

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

    ctx.log(&format!("🎯 Target platforms: {:?}", platforms));
    ctx.log(&format!("🎯 Target Maya versions: {:?}", maya_versions));

    // Setup DevKit (use the first Maya version for DevKit download)
    if !args.skip_cpp {
        let first_maya_version = maya_versions.first()
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
        if !args.skip_rust {
            if let Err(e) = ctx.build_rust_library(platform) {
                ctx.log_error(&format!("Failed to build Rust library for {:?}: {}", platform, e));
                continue;
            }
        }

        for maya_version in &maya_versions {
            ctx.log(&format!("\n{}", "=".repeat(60)));
            ctx.log(&format!("Building: {:?} Maya {}", platform, maya_version));
            ctx.log(&format!("{}", "=".repeat(60)));

            let mut build_success = true;

            // Build C++ plugin
            if !args.skip_cpp {
                if let Err(e) = ctx.build_maya_plugin(platform, maya_version) {
                    ctx.log_error(&format!("Failed to build Maya plugin: {}", e));
                    build_success = false;
                }
            }

            // Package artifacts
            if build_success {
                if let Err(e) = ctx.package_artifacts(platform, maya_version) {
                    ctx.log_error(&format!("Failed to package artifacts: {}", e));
                    build_success = false;
                }
            }

            if build_success {
                success_count += 1;
                ctx.log_success(&format!("✅ {:?} Maya {} completed", platform, maya_version));
            } else {
                ctx.log_error(&format!("❌ {:?} Maya {} failed", platform, maya_version));
            }
        }
    }

    // Summary
    ctx.log(&format!("\n{}", "=".repeat(60)));
    ctx.log("🎉 Build Summary");
    ctx.log(&format!("{}", "=".repeat(60)));
    ctx.log(&format!("✅ Successful builds: {}/{}", success_count, total_count));
    ctx.log(&format!("📁 Output directory: {}", ctx.dist_dir.display()));

    if success_count > 0 {
        ctx.log("\n📦 Built packages:");
        if ctx.dist_dir.exists() {
            for entry in std::fs::read_dir(&ctx.dist_dir).context("Failed to read dist directory")? {
                let entry = entry.context("Failed to read directory entry")?;
                if entry.path().is_dir() {
                    let file_count = std::fs::read_dir(entry.path())
                        .map(|entries| entries.count())
                        .unwrap_or(0);
                    ctx.log(&format!("  📂 {} ({} files)", entry.file_name().to_string_lossy(), file_count));
                }
            }
        }
    }

    if success_count == total_count {
        ctx.log_success("\n🎉 All builds completed successfully!");
        Ok(())
    } else {
        ctx.log_error("\n❌ Some builds failed!");
        std::process::exit(1);
    }
}
