use std::env;

use crate::github::GithubClient;
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use github::{PackageOwner, PackageVersion};
use once_cell::sync::OnceCell;

mod github;

static DRY_RUN: OnceCell<bool> = OnceCell::new();

/// Delete all untagged versions of GitHub container packages.
#[derive(Parser)]
#[clap(version)]
struct Args {
    /// User owning the packages (conflicts with --org)
    #[clap(long, conflicts_with = "org")]
    user: Option<String>,

    /// Organization owning the packages (conflicts with --user)
    #[clap(long, conflicts_with = "user")]
    org: Option<String>,

    /// Path to a file containing a GitHub token.
    /// You can also pass a token verbatim via the GITHUB_TOKEN env variable.
    #[clap(long)]
    token: Option<String>,

    /// Don't persist but only print changes
    #[clap(long, short = 'n')]
    dry_run: bool,

    /// Make logging more verbose.
    /// You can also specify the log level via the RUST_LOG env variable.
    #[clap(long, short)]
    verbose: bool,

    /// Packages to clean
    #[clap(required = true)]
    package_names: Vec<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    DRY_RUN.set(args.dry_run).unwrap();

    if env::var("RUST_LOG").is_err() {
        let level = match args.verbose {
            true => "debug",
            false => "info",
        };
        env::set_var("RUST_LOG", format!("{}={}", env!("CARGO_PKG_NAME"), level));
    }
    env_logger::init();

    log::info!(
        "Starting {} {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
    );
    log::debug!("With arguments {:?}", env::args().collect::<Vec<_>>());

    if let Err(error) = run(args).await {
        log::error!("{:?}", error);
    }
}

async fn run(args: Args) -> Result<()> {
    if args.user.is_none() && args.org.is_none() {
        return Err(anyhow!("Neither --user nor --org was provided"));
    }

    let token = match args.token {
        Some(path) => tokio::fs::read_to_string(&path)
            .await
            .context(format!("Failed to read the github token from {}", path))?
            .trim()
            .to_string(),
        None => env::var("GITHUB_TOKEN")
            .context("No github token provided via --token or GITHUB_TOKEN")?,
    };
    let client = GithubClient::new(token).context("Failed to create github client")?;

    let owner = PackageOwner::parse(args.user, args.org);

    for package_name in args.package_names {
        clean_package(&client, &owner, &package_name)
            .await
            .context(format!(
                "Failed to clean package {}/{}",
                owner, package_name,
            ))?;
    }

    Ok(())
}

async fn clean_package(
    client: &GithubClient,
    owner: &PackageOwner,
    package_name: &str,
) -> Result<()> {
    log::info!("Cleaning package {}/{}", owner, package_name);

    let mut page = 1;
    loop {
        let versions = client
            .get_package_version(owner, package_name, Some(page))
            .await
            .context("Failed to get package versions from github")?;

        if versions.is_empty() {
            break;
        }

        clean_package_versions(client, owner, package_name, versions)
            .await
            .context("Failed to clean package versions")?;

        page += 1;
    }

    Ok(())
}

async fn clean_package_versions(
    client: &GithubClient,
    owner: &PackageOwner,
    package_name: &str,
    versions: Vec<PackageVersion>,
) -> Result<()> {
    for version in versions {
        if !version.metadata.container.tags.is_empty() {
            continue;
        }

        let dry_run = match DRY_RUN.get() {
            Some(&true) => "(DRY RUN)",
            _ => "",
        };
        log::info!(
            "Deleting {}/{}:{} {}",
            owner,
            package_name,
            version.name,
            dry_run,
        );

        if DRY_RUN.get() == Some(&true) {
            continue;
        }

        if let Err(error) = client
            .delete_package_version(owner, package_name, &version.id)
            .await
        {
            log::warn!("{:?}\n", error);
        }
    }

    Ok(())
}
