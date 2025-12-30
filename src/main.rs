use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Parser)]
#[command(name = "alts")]
#[command(about = "A version control software", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new repository
    Init {
        /// The directory name to track
        dir_name: String,
    },
    /// Create a checkpoint (alias: ck)
    #[command(alias = "ck")]
    Checkpoint {
        /// Optional checkpoint name
        name: Option<String>,
    },
    /// List all checkpoints (alias: ls)
    #[command(alias = "ls")]
    List,
    /// Remove unfound checkpoints from index
    Prune,
    /// Show repository metadata
    Info,
}

const ALTS_DIR: &str = ".alts";
const CONFIG_FILE: &str = "alts.toml";

#[derive(Serialize, Deserialize)]
struct Checkpoint {
    timestamp: String,
}

#[derive(Serialize, Deserialize)]
struct Config {
    target_dir: String,
    #[serde(default)]
    checkpoints: BTreeMap<String, Checkpoint>,
}

fn init(dir_name: &str) -> Result<()> {
    // Normalize the path and check if it exists under current directory
    let current_dir = std::env::current_dir()?;
    let current_dir_normalized = current_dir.canonicalize()?;
    let target_path = current_dir.join(dir_name);

    if !target_path.exists() {
        return Err(anyhow::anyhow!(
            "Directory '{}' does not exist under current directory",
            dir_name
        ));
    }

    if !target_path.is_dir() {
        return Err(anyhow::anyhow!("'{}' is not a directory", dir_name));
    }

    // Get canonical paths to ensure we're comparing the same paths
    let target_path_normalized = target_path.canonicalize()?;

    // Check if the target directory is under the current directory
    match target_path_normalized.strip_prefix(&current_dir_normalized) {
        Ok(relative_path) => {
            // Ensure the path doesn't contain ".." (parent directory references)
            if relative_path
                .components()
                .any(|c| c == std::path::Component::ParentDir)
            {
                return Err(anyhow::anyhow!(
                    "Directory '{}' is not under current directory",
                    dir_name
                ));
            }
        }
        Err(_) => {
            return Err(anyhow::anyhow!(
                "Directory '{}' is not under current directory",
                dir_name
            ));
        }
    }

    // Check if repository is already initialized
    let alts_dir = current_dir.join(ALTS_DIR);
    if alts_dir.exists() {
        return Err(anyhow::anyhow!(
            "Repository is already initialized. Please manually remove the '{}' directory first.",
            ALTS_DIR
        ));
    }

    // Create .alts directory
    fs::create_dir_all(&alts_dir).context("Failed to create .alts directory")?;

    // Write config file using toml serialization
    let config_path = alts_dir.join(CONFIG_FILE);
    let config = Config {
        // Use the canonicalized relative path without trailing slashes
        target_dir: target_path_normalized
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid directory name"))?
            .to_string(),
        checkpoints: BTreeMap::new(),
    };
    let config_content = toml::to_string_pretty(&config).context("Failed to serialize config")?;
    fs::write(&config_path, config_content).context("Failed to write config file")?;

    info!(
        "Initialized alts repository tracking '{}'",
        config.target_dir
    );
    Ok(())
}

fn load_config() -> Result<Config> {
    let current_dir = std::env::current_dir()?;
    let config_path = current_dir.join(ALTS_DIR).join(CONFIG_FILE);

    if !config_path.exists() {
        return Err(anyhow::anyhow!(
            "Not initialized. Run 'alts init <dir_name>' first"
        ));
    }

    let content = fs::read_to_string(&config_path)?;
    let config: Config = toml::from_str(&content).context("Failed to parse config file")?;

    Ok(config)
}

fn save_config(config: &Config) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let config_path = current_dir.join(ALTS_DIR).join(CONFIG_FILE);
    let config_content = toml::to_string_pretty(&config).context("Failed to serialize config")?;
    fs::write(&config_path, config_content).context("Failed to write config file")?;
    Ok(())
}

fn checkpoint(name: Option<String>) -> Result<()> {
    // Load config
    let mut config = load_config()?;
    let target_dir = config.target_dir.clone();

    let current_dir = std::env::current_dir()?;
    let target_path = current_dir.join(&target_dir);

    // Check if target exists and is not empty
    if !target_path.exists() {
        return Err(anyhow::anyhow!(
            "Target directory '{}' does not exist",
            target_dir
        ));
    }

    let is_empty = fs::read_dir(&target_path)
        .context("Failed to read target directory")?
        .next()
        .is_none();

    if is_empty {
        return Err(anyhow::anyhow!(
            "Target directory '{}' is empty",
            target_dir
        ));
    }

    let alts_dir = current_dir.join(ALTS_DIR);
    let checkpoint_name = match name {
        Some(n) => {
            // Normalize the checkpoint name
            let normalized_name = Path::new(&n);
            let checkpoint_name = normalized_name
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(&n)
                .to_string();

            // Check if name is already used
            let checkpoint_path = alts_dir.join(&checkpoint_name);
            if checkpoint_path.exists() {
                return Err(anyhow::anyhow!(
                    "Checkpoint name '{}' already exists",
                    checkpoint_name
                ));
            }
            checkpoint_name
        }
        None => {
            // Generate name with timestamp
            let now: DateTime<Utc> = Utc::now();
            let timestamp = now.format("%Y_%m_%d_%H_%M_%S").to_string();

            // Handle file extensions correctly - insert timestamp before extension
            let target_path = Path::new(&target_dir);
            let file_stem = target_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&target_dir);
            let extension = target_path
                .extension()
                .and_then(|s| s.to_str())
                .map(|ext| format!(".{}", ext))
                .unwrap_or_default();

            format!("{}_{}{}", file_stem, timestamp, extension)
        }
    };

    let checkpoint_path = alts_dir.join(&checkpoint_name);

    info!("Creating checkpoint '{}'...", checkpoint_name);
    copy_dir_recursive(&target_path, &checkpoint_path)?;

    // Add checkpoint to index
    let now: DateTime<Utc> = Utc::now();
    let timestamp = now.to_rfc3339();
    config.checkpoints.insert(
        checkpoint_name.clone(),
        Checkpoint {
            timestamp: timestamp.clone(),
        },
    );
    save_config(&config)?;

    info!("Checkpoint '{}' created successfully", checkpoint_name);

    Ok(())
}

fn list() -> Result<()> {
    let config = load_config()?;
    let current_dir = std::env::current_dir()?;
    let alts_dir = current_dir.join(ALTS_DIR);

    if config.checkpoints.is_empty() {
        info!("No checkpoints found");
        return Ok(());
    }

    info!("Checkpoints:");
    for (name, checkpoint) in &config.checkpoints {
        let checkpoint_path = alts_dir.join(Path::new(name));
        let exists = checkpoint_path.exists();
        let status = if exists { "✓" } else { "✗" };
        println!("  {} {} - {}", status, name, checkpoint.timestamp);
    }

    Ok(())
}

fn info() -> Result<()> {
    let config = load_config()?;
    let current_dir = std::env::current_dir()?;
    let alts_dir = current_dir.join(ALTS_DIR);

    println!("Repository Information:");
    println!("=======================");
    println!("Target Directory: {}", config.target_dir);
    println!("Total Checkpoints: {}", config.checkpoints.len());

    if config.checkpoints.is_empty() {
        println!("\nNo checkpoints available.");
        return Ok(());
    }

    // Count valid checkpoints
    let mut valid_count = 0;
    let mut invalid_count = 0;

    for (name, _checkpoint) in &config.checkpoints {
        let checkpoint_path = alts_dir.join(Path::new(name));
        if checkpoint_path.exists() {
            valid_count += 1;
        } else {
            invalid_count += 1;
        }
    }

    println!("Valid Checkpoints: {}", valid_count);
    println!("Invalid Checkpoints: {}", invalid_count);

    println!("\nCheckpoint Details:");
    for (name, checkpoint) in &config.checkpoints {
        let checkpoint_path = alts_dir.join(Path::new(name));
        let exists = checkpoint_path.exists();
        let status = if exists { "Valid" } else { "Missing" };
        println!("  - Name: {}", name);
        println!("    Status: {}", status);
        println!("    Created: {}", checkpoint.timestamp);
    }

    Ok(())
}

fn prune() -> Result<()> {
    let mut config = load_config()?;
    let current_dir = std::env::current_dir()?;
    let alts_dir = current_dir.join(ALTS_DIR);

    if config.checkpoints.is_empty() {
        info!("No checkpoints to prune");
        return Ok(());
    }

    info!("Checking checkpoints...");
    let mut to_remove: Vec<String> = Vec::new();

    for name in config.checkpoints.keys() {
        let checkpoint_path = alts_dir.join(Path::new(name));
        if checkpoint_path.exists() {
            info!("  Found: {}", name);
        } else {
            info!("  Not found: {}", name);
            to_remove.push(name.clone());
        }
    }

    // Remove unfound checkpoints from index
    for name in &to_remove {
        config.checkpoints.remove(name);
    }

    if !to_remove.is_empty() {
        save_config(&config)?;
        info!(
            "Removed {} unfound checkpoint(s) from index",
            to_remove.len()
        );
    } else {
        info!("All checkpoints found, nothing to remove");
    }

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).context("Failed to create directory")?;

    for entry in fs::read_dir(src).context("Failed to read directory")? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            info!("Copying directory: {}", src_path.display());
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            info!("Copying file: {}", src_path.display());
            fs::copy(&src_path, &dst_path).context("Failed to copy file")?;
        }
    }

    Ok(())
}

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { dir_name } => {
            if let Err(e) = init(&dir_name) {
                error!("{}", e);
                std::process::exit(1);
            }
        }
        Commands::Checkpoint { name } => {
            if let Err(e) = checkpoint(name) {
                error!("{}", e);
                std::process::exit(1);
            }
        }
        Commands::List => {
            if let Err(e) = list() {
                error!("{}", e);
                std::process::exit(1);
            }
        }
        Commands::Prune => {
            if let Err(e) = prune() {
                error!("{}", e);
                std::process::exit(1);
            }
        }
        Commands::Info => {
            if let Err(e) = info() {
                error!("{}", e);
                std::process::exit(1);
            }
        }
    }
}
