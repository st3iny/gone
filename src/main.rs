use std::env;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use github::{GithubClientImpl, PackageOwner, PackageVersion};

use crate::github::GithubClient;

mod github;

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
    let client = GithubClientImpl::new(token).context("Failed to create github client")?;

    let owner = PackageOwner::parse(args.user, args.org);

    for package_name in args.package_names {
        clean_package(&client, &owner, &package_name, args.dry_run)
            .await
            .context(format!(
                "Failed to clean package {}/{}",
                owner, package_name,
            ))?;
    }

    Ok(())
}

async fn clean_package(
    client: &impl GithubClient,
    owner: &PackageOwner,
    package_name: &str,
    dry_run: bool,
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

        clean_package_versions(client, owner, package_name, &versions, dry_run)
            .await
            .context("Failed to clean package versions")?;

        page += 1;
    }

    Ok(())
}

async fn clean_package_versions(
    client: &impl GithubClient,
    owner: &PackageOwner,
    package_name: &str,
    versions: &[PackageVersion],
    dry_run: bool,
) -> Result<()> {
    for version in versions {
        if !version.metadata.container.tags.is_empty() {
            continue;
        }

        let dry_run_suffix = match dry_run {
            true => " (DRY RUN)",
            false => "",
        };
        log::info!(
            "Deleting {}/{}:{}{}",
            owner,
            package_name,
            version.name,
            dry_run_suffix,
        );

        if dry_run {
            continue;
        }

        if let Err(error) = client
            .delete_package_version(owner, package_name, &version.id.to_string())
            .await
        {
            log::warn!("{:?}\n", error);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use mockall::predicate::*;

    use super::*;
    use crate::github::{ContainerVersionMetadata, MockGithubClient, PackageVersionMetadata};

    #[tokio::test]
    async fn test_clean_package_versions() {
        let mut client = MockGithubClient::new();

        let user = PackageOwner::User("user".to_string());
        let org = PackageOwner::Organizaion("org".to_string());

        // No versions
        clean_package_versions(&client, &user, "my-package", &[], true)
            .await
            .unwrap();
        clean_package_versions(&client, &user, "my-package", &[], false)
            .await
            .unwrap();

        // No untagged versions
        let versions = vec![PackageVersion {
            id: 1,
            name: "sha256:foobar1".to_string(),
            metadata: PackageVersionMetadata {
                package_type: "container".to_string(),
                container: ContainerVersionMetadata {
                    tags: vec!["some-tag".to_string()],
                },
            },
        }];
        clean_package_versions(&client, &user, "my-package", &versions, true)
            .await
            .unwrap();
        clean_package_versions(&client, &user, "my-package", &versions, false)
            .await
            .unwrap();

        // One untagged version
        let versions = vec![
            PackageVersion {
                id: 1,
                name: "sha256:foobar1".to_string(),
                metadata: PackageVersionMetadata {
                    package_type: "container".to_string(),
                    container: ContainerVersionMetadata {
                        tags: vec!["some-tag".to_string()],
                    },
                },
            },
            PackageVersion {
                id: 2,
                name: "sha256:foobar2".to_string(),
                metadata: PackageVersionMetadata {
                    package_type: "container".to_string(),
                    container: ContainerVersionMetadata { tags: vec![] },
                },
            },
        ];
        client
            .expect_delete_package_version()
            .with(eq(user.clone()), eq("my-package"), eq("2"))
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        clean_package_versions(&client, &user, "my-package", &versions, false)
            .await
            .unwrap();
        client.checkpoint();
        clean_package_versions(&client, &user, "my-package", &versions, true)
            .await
            .unwrap();

        // Multiple untagged version
        let versions = vec![
            PackageVersion {
                id: 1,
                name: "sha256:foobar1".to_string(),
                metadata: PackageVersionMetadata {
                    package_type: "container".to_string(),
                    container: ContainerVersionMetadata {
                        tags: vec!["some-tag".to_string()],
                    },
                },
            },
            PackageVersion {
                id: 2,
                name: "sha256:foobar2".to_string(),
                metadata: PackageVersionMetadata {
                    package_type: "container".to_string(),
                    container: ContainerVersionMetadata { tags: vec![] },
                },
            },
            PackageVersion {
                id: 3,
                name: "sha256:foobar3".to_string(),
                metadata: PackageVersionMetadata {
                    package_type: "container".to_string(),
                    container: ContainerVersionMetadata { tags: vec![] },
                },
            },
        ];
        client
            .expect_delete_package_version()
            .with(eq(org.clone()), eq("my-package"), eq("2"))
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        client
            .expect_delete_package_version()
            .with(eq(org.clone()), eq("my-package"), eq("3"))
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        clean_package_versions(&client, &org, "my-package", &versions, false)
            .await
            .unwrap();
        client.checkpoint();
        clean_package_versions(&client, &org, "my-package", &versions, true)
            .await
            .unwrap();
    }
}
