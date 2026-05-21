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
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    if !args.path.exists() {
        bail!("Path does not exist: {}", args.path.display());
    }

    let scanner = FileSystemScanner::new();
    let detector = PatternDetector::new();
    let scan_options = if args.scene_only || args.maya_version.is_some() {
        ScanOptions::maya_scene_files()
    } else {
        ScanOptions::default()
    };

    let report =
        scanner.scan_with_detector(&args.path.to_string_lossy(), &scan_options, &detector)?;
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

    if let Some(maya_version) = args.maya_version.as_deref() {
        run_maya_standalone_clean(maya_version, &infected_file, args.plugin.as_deref())?;
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
    }

    Ok(())
}

fn write_infected_file(
    explicit_path: Option<&Path>,
    report: &umbrella_maya_plugin::antivirus::scanner::ScanReport,
) -> Result<PathBuf> {
    let infected_file = match explicit_path {
        Some(path) => path.to_path_buf(),
        None => std::env::temp_dir()
            .join("maya-umbrella")
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
    infected_file: &Path,
    plugin: Option<&Path>,
) -> Result<()> {
    let mayapy = find_mayapy(maya_version)
        .with_context(|| format!("Could not find mayapy for Maya {}", maya_version))?;
    let plugin_path = match plugin {
        Some(path) => path.to_path_buf(),
        None => find_adjacent_plugin(maya_version).with_context(|| {
            format!("Could not find umbrella_maya next to the CLI for Maya {maya_version}. Pass --plugin.")
        })?,
    };

    let script = write_maya_runner_script()?;
    let status = Command::new(&mayapy)
        .arg(&script)
        .arg(infected_file)
        .arg(&plugin_path)
        .status()
        .with_context(|| format!("Failed to launch {}", mayapy.display()))?;

    if !status.success() {
        bail!("Maya standalone cleaning failed with status {}", status);
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

fn write_maya_runner_script() -> Result<PathBuf> {
    let script = std::env::temp_dir()
        .join("maya-umbrella")
        .join("run_umbrella_standalone.py");
    if let Some(parent) = script.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(
        &script,
        r#"import sys
import maya.standalone
import maya.cmds as cmds

infected_file = sys.argv[1]
plugin_path = sys.argv[2]

maya.standalone.initialize()
try:
    cmds.loadPlugin(plugin_path, quiet=True)
    with open(infected_file, "r", encoding="utf-8") as stream:
        files = [line.strip() for line in stream if line.strip()]
    for scene in files:
        cmds.file(scene, open=True, force=True, ignoreVersion=True, executeScriptNodes=False, prompt=False)
        if hasattr(cmds, "umbrellaFixScene"):
            cmds.umbrellaFixScene()
        cmds.file(save=True, force=True)
        cmds.file(new=True, force=True)
finally:
    maya.standalone.uninitialize()
"#,
    )?;

    Ok(script)
}
