use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Parser;
use umbrella_maya_plugin::antivirus::cleaner::CleanOptions;
use umbrella_maya_plugin::antivirus::detector::PatternDetector;
use umbrella_maya_plugin::antivirus::scanner::{FileSystemScanner, ScanOptions};

#[derive(Parser)]
#[command(name = "umbrella-maya")]
#[command(about = "Scan and clean Maya virus signatures from CLI, Maya standalone, or CI")]
struct Args {
    /// Path to a Maya scene, script file, or directory.
    #[arg(long)]
    path: PathBuf,

    /// Maya version used for standalone scene cleaning, for example 2024.
    #[arg(long)]
    maya_version: Option<String>,

    /// Clean infected files. With --maya-version, this launches mayapy and the MLL.
    #[arg(long)]
    clean: bool,

    /// Restrict scan to .ma/.mb files, matching maya_umbrella_scanner.
    #[arg(long)]
    scene_only: bool,

    /// Emit JSON instead of human-readable output.
    #[arg(long)]
    json: bool,

    /// Write infected file paths to this file. Defaults to temp/maya-umbrella/infected_file.txt.
    #[arg(long)]
    infected_file: Option<PathBuf>,

    /// Backup root for offline cleaning.
    #[arg(long)]
    backup_root: Option<PathBuf>,

    /// Remove both file and scriptJob signatures during offline cleaning.
    #[arg(long)]
    aggressive: bool,

    /// Explicit umbrella_maya plugin path for Maya standalone cleaning.
    #[arg(long)]
    plugin: Option<PathBuf>,

    /// Verified production Maya/plugin/dependency profile required for native cleanup.
    #[arg(long)]
    environment_profile: Option<PathBuf>,

    /// Parent directory for unique copy-only native cleanup results. Originals are never saved.
    #[arg(long)]
    output_dir: Option<PathBuf>,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    if !args.path.exists() {
        bail!("Path does not exist: {}", args.path.display());
    }

    let scanner = FileSystemScanner::new();
    let detector = PatternDetector::new();
    let mut scan_options = if args.scene_only || args.maya_version.is_some() {
        ScanOptions::maya_scene_files()
    } else {
        ScanOptions::default()
    };

    // A requested CLI scan must not silently skip large production scenes and
    // then report them clean. Read failures propagate instead of being ignored.
    scan_options.max_file_size = None;
    let report =
        scanner.scan_with_detector(&args.path.to_string_lossy(), &scan_options, &detector)?;
    if report.files_scanned == 0 {
        bail!("No supported files were scanned; no clean result can be established");
    }
    let infected_file = write_infected_file(args.infected_file.as_deref(), &report)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if report.is_clean() {
        println!("No infected files found.");
    } else {
        println!(
            "Found {} signature hits in {} file(s).",
            report.threats_found,
            report.infected_files.len()
        );
        println!("Export infected files to: {}", infected_file.display());
    }

    if report.is_clean() {
        return Ok(());
    }

    if args.clean
        && let Some(maya_version) = args.maya_version.as_deref()
    {
        let profile = args.environment_profile.as_deref().context(
            "Native cleanup requires --environment-profile matching production Maya, plugins and dependencies; originals were not modified"
        )?;
        let output = args
            .output_dir
            .as_deref()
            .context("Native cleanup requires --output-dir for separate reviewable copies")?;
        run_maya_standalone_clean(
            maya_version,
            &report,
            args.plugin.as_deref(),
            profile,
            output,
        )?;
        return Ok(());
    }

    if args.clean {
        let options = CleanOptions {
            backup_directory: args
                .backup_root
                .as_ref()
                .map(|path| path.to_string_lossy().to_string()),
            aggressive: args.aggressive,
            ..Default::default()
        };

        let cleaner = umbrella_maya_plugin::antivirus::cleaner::BackupCleaner::new();
        let summary = cleaner.clean_path(&args.path.to_string_lossy(), &options)?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&summary)?);
        } else {
            println!(
                "Cleaned {} file(s), deleted {}, failed {}, removed {} signature hit(s).",
                summary.files_cleaned,
                summary.files_deleted,
                summary.files_failed,
                summary.threats_removed
            );
        }
        if summary.files_failed > 0 {
            bail!(
                "Cleanup failed for {} file(s); inspect the report. Binary scenes require verified Maya standalone cleaning",
                summary.files_failed
            );
        }
    }

    Ok(())
}

fn write_infected_file(
    explicit_path: Option<&Path>,
    report: &umbrella_maya_plugin::antivirus::scanner::ScanReport,
) -> Result<PathBuf> {
    let infected_file = match explicit_path {
        Some(path) => path.to_path_buf(),
        None => tempfile::Builder::new()
            .prefix("maya-umbrella-scan-")
            .tempdir()?
            .keep()
            .join("infected_file.txt"),
    };

    if let Some(parent) = infected_file.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut content = String::new();
    for detection in &report.infected_files {
        content.push_str(&detection.file_path);
        content.push('\n');
    }
    fs::write(&infected_file, content)?;
    Ok(infected_file)
}

fn run_maya_standalone_clean(
    maya_version: &str,
    report: &umbrella_maya_plugin::antivirus::scanner::ScanReport,
    plugin: Option<&Path>,
    profile: &Path,
    output_root: &Path,
) -> Result<()> {
    let mayapy = find_mayapy(maya_version).with_context(|| {
        format!(
            "Could not find mayapy for Maya {}; originals were not modified",
            maya_version
        )
    })?;
    let plugin_path = match plugin {
        Some(path) => path.to_path_buf(),
        None => find_adjacent_plugin(maya_version).with_context(|| {
            format!("Could not find umbrella_maya next to the CLI for Maya {maya_version}. Pass --plugin.")
        })?,
    }.canonicalize()?;
    let profile = profile
        .canonicalize()
        .context("Production environment profile is missing")?;
    fs::create_dir_all(output_root)?;
    let output = tempfile::Builder::new()
        .prefix("umbrella-native-")
        .tempdir_in(output_root)?
        .keep()
        .canonicalize()?;
    // Per-invocation files prevent one concurrent run from replacing another's
    // script or list of scenes. JSON preserves paths with whitespace/newlines.
    let invocation = tempfile::Builder::new()
        .prefix("umbrella-runner-")
        .tempdir()?;
    let script = invocation.path().join("runner.py");
    let job_path = invocation.path().join("job.json");
    fs::write(
        &script,
        include_str!("../../scripts/maya_standalone_clean.py"),
    )?;
    let files = report
        .infected_files
        .iter()
        .map(|detection| Path::new(&detection.file_path).canonicalize())
        .collect::<std::io::Result<Vec<_>>>()?;
    let job = serde_json::json!({
        "maya_version": maya_version,
        "plugin": plugin_path,
        "profile": profile,
        "output_dir": output,
        "files": files,
        "cli": std::env::current_exe()?,
    });
    fs::write(&job_path, serde_json::to_vec(&job)?)?;
    let status = Command::new(&mayapy)
        .arg(&script)
        .arg(&job_path)
        .status()
        .with_context(|| {
            format!(
                "Failed to launch {}; originals were not modified",
                mayapy.display()
            )
        })?;
    if !status.success() || !output.join("verified-results.json").is_file() {
        bail!(
            "Maya standalone cleanup was not verified ({}). Originals were not modified; unaccepted intermediates: {}",
            status,
            output.display()
        );
    }
    Ok(())
}

fn find_mayapy(maya_version: &str) -> Option<PathBuf> {
    if let Ok(maya_location) = std::env::var("MAYA_LOCATION") {
        let candidate = Path::new(&maya_location).join("bin").join(mayapy_name());
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let candidates = cfg_select! {
        windows => vec![PathBuf::from(format!(
            "C:/Program Files/Autodesk/Maya{}/bin/mayapy.exe",
            maya_version
        ))],
        target_os = "macos" => vec![
            PathBuf::from(format!(
                "/Applications/Autodesk/Maya{}/Maya.app/Contents/bin/mayapy",
                maya_version
            )),
            PathBuf::from(format!(
                "/Applications/Autodesk/maya{}/Maya.app/Contents/bin/mayapy",
                maya_version
            )),
        ],
        _ => vec![
            PathBuf::from(format!("/usr/autodesk/maya{}/bin/mayapy", maya_version)),
            PathBuf::from(format!("/opt/autodesk/maya{}/bin/mayapy", maya_version)),
        ],
    };

    candidates.into_iter().find(|path| path.exists())
}

fn mayapy_name() -> &'static str {
    cfg_select! {
        windows => "mayapy.exe",
        _ => "mayapy",
    }
}

fn find_adjacent_plugin(_maya_version: &str) -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let extension = cfg_select! {
        windows => "mll",
        target_os = "macos" => "bundle",
        _ => "so",
    };
    let candidate = exe_dir.join(format!("umbrella_maya.{}", extension));
    candidate.exists().then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_scan_manifests_use_distinct_paths() {
        let report = umbrella_maya_plugin::antivirus::scanner::ScanReport {
            root: "test".into(),
            files_scanned: 0,
            threats_found: 0,
            infected_files: vec![],
            duration_ms: 0,
        };
        let one = write_infected_file(None, &report).unwrap();
        let two = write_infected_file(None, &report).unwrap();
        assert_ne!(one, two);
        fs::remove_dir_all(one.parent().unwrap()).unwrap();
        fs::remove_dir_all(two.parent().unwrap()).unwrap();
    }
}
