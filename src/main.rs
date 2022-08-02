#![deny(missing_docs)]

//! `terraform_profile` is a utility program designed to help with owning multiple terraform account
//!
//! The `~/.terraform.d/credentials.tfrc.json` file is unfortunately unique, and for governance issues,
//! you can't switch easily between teams with different terraform cloud accounts

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

/// Select a subcommand to interact with your terraform cloud profile.
///
/// Leave blank for the CLI
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Switch the current terraform cloud profile for another.
    Switch {
        #[clap(value_parser)]
        name: String,
    },
    /// Import your current unregistered terraform cloud profile
    Import {
        #[clap(value_parser)]
        name: String,
    },
    /// Check which terraform cloud profile is currently used
    Status,
    /// List all the different registered terraform cloud profiles
    List,
}

/// Fetch and initialize the root project directory
fn initialize_folder() -> Result<PathBuf> {
    let home_dir = home::home_dir().context("Impossible to get your home dir!")?;

    let project_dir = home_dir.join(format!(".{}", env!("CARGO_PKG_NAME")));

    if !project_dir.exists() {
        std::fs::create_dir(&project_dir)?;
    }
    Ok(project_dir)
}

/// Get all the files and register their profiles names
fn get_profiles<P: AsRef<Path>>(path: P) -> Result<HashMap<String, PathBuf>> {
    let mut entries = HashMap::new();

    for file in std::fs::read_dir(path)? {
        if let Ok(file) = file {
            let file = file;

            let file_name = file
                .file_name()
                .to_str()
                .context("Couldn't convert OsString to &str")?
                .split_once(".tfrc.json")
                .context("Couldn't split file name")?
                .0
                .to_string();

            entries.insert(file_name, file.path());
        }
    }
    Ok(entries)
}

/// Entrypoint of the CLI
fn main() -> Result<()> {
    let terraform_directory = home::home_dir()
        .context("Impossible to get your home dir!")?
        .join(".terraform.d");
    let project_directory = initialize_folder()?;

    let profiles = get_profiles(&project_directory)?;

    match Cli::try_parse() {
        Ok(args) => match args.command {
            Commands::Switch { name } => switch_profile(&terraform_directory, &profiles, name)?,
            Commands::Import { name } => {
                import_profile(name, &terraform_directory, &profiles, project_directory)?
            }
            Commands::Status => show_profile_status(terraform_directory, &profiles)?,
            Commands::List => show_profiles_list(&profiles),
        },
        Err(e) => match e.kind() {
            clap::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => e.exit(),
            _ => e.exit(),
        },
    }
    Ok(())
}

/// Switch an old credentials files with a new profile
fn switch_profile(
    terraform_directory: &PathBuf,
    profiles: &HashMap<String, PathBuf>,
    name: String,
) -> Result<(), anyhow::Error> {
    let credentials_files = terraform_directory.join("credentials.tfrc.json");
    let profile_path = if let Some(profile_path) = profiles.get(&name) {
        profile_path
    } else {
        eprintln!("Couldn't find the profile to switch with.");
        std::process::exit(1);
    };
    if credentials_files.exists() {
        if credentials_files.is_symlink() {
            std::fs::remove_file(&credentials_files)?;
            symlink_credentials(profile_path, credentials_files)?;
            println!("Switched credentials with the new profile");
        } else {
            eprintln!("A non-profile credentials already exists. This is a destructive operation, you should import or delete it first.");
            std::process::exit(1);
        }
    } else {
        symlink_credentials(profile_path, credentials_files)?;
        println!("Switched credentials with the new profile");
    }
    Ok(())
}

/// Symlink credentials with new profiles credentials depending on platform
fn symlink_credentials(
    profile_path: &PathBuf,
    credentials_files: PathBuf,
) -> Result<(), anyhow::Error> {
    #[cfg(target_family = "windows")]
    std::os::windows::fs::symlink_file(profile_path, credentials_files)?;
    #[cfg(target_family = "unix")]
    std::os::unix::fs::symlink(profile_path, credentials_files)?;
    Ok(())
}

/// Import a new profile into the registry
fn import_profile(
    name: String,
    terraform_directory: &PathBuf,
    profiles: &HashMap<String, PathBuf>,
    project_directory: PathBuf,
) -> Result<()> {
    let credentials_files = terraform_directory.join("credentials.tfrc.json");

    if credentials_files.is_symlink() {
        let link = credentials_files.read_link()?;
        if let Some(key) = get_profile_name_for_path(link, profiles) {
            eprintln!("The profile is already imported under `{key}`");
            std::process::exit(1)
        } else {
            eprintln!("The profile is an unknown symbolic link.");
            std::process::exit(1)
        }
    } else {
        let new_path = project_directory.join(format!("{name}.tfrc.json"));
        std::fs::rename(credentials_files, new_path)?;
        println!("The terraform cloud profile was safely registered");
    }
    Ok(())
}

/// Get profile name for path
fn get_profile_name_for_path<P: AsRef<Path>>(
    path: P,
    profiles: &HashMap<String, PathBuf>,
) -> Option<&String> {
    for (key, value) in profiles {
        if value == path.as_ref() {
            return Some(key);
        }
    }
    None
}

/// Show the current profile status
fn show_profile_status<P: AsRef<Path>>(
    path: P,
    profiles: &HashMap<String, PathBuf>,
) -> Result<(), anyhow::Error> {
    let credentials_files = path.as_ref().join("credentials.tfrc.json");
    if credentials_files.is_symlink() {
        let link = credentials_files.read_link()?;

        if let Some(key) = get_profile_name_for_path(link, profiles) {
            println!("{key}");
        } else {
            eprintln!("No profile is currently in use.");
            std::process::exit(1);
        }
    } else {
        eprintln!("No profile is currently in use.");
        std::process::exit(1);
    }
    Ok(())
}

/// Show the different profiles list
fn show_profiles_list(profiles: &HashMap<String, PathBuf>) {
    if profiles.is_empty() {
        eprintln!("No profiles is currently available");
        std::process::exit(1);
    } else {
        println!("Currently available profiles:");
        for profile in profiles.keys() {
            println!("\t{profile}");
        }
    }
}
