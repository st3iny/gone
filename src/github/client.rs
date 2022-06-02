use anyhow::{anyhow, Context, Result};
use std::fmt::Display;

use reqwest::{
    header::{HeaderMap, ACCEPT, AUTHORIZATION, USER_AGENT},
    Client, ClientBuilder,
};

use super::PackageVersion;

pub enum PackageOwner {
    User(String),
    Organizaion(String),
}

impl PackageOwner {
    pub fn parse(user: Option<String>, org: Option<String>) -> Self {
        if let Some(user) = user {
            return Self::User(user);
        }

        if let Some(org) = org {
            return Self::Organizaion(org);
        }

        panic!("Both user and org are None");
    }

    fn base_url(&self) -> String {
        match self {
            Self::User(user) => format!("users/{user}"),
            Self::Organizaion(org) => format!("orgs/{org}"),
        }
    }
}

impl Display for PackageOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User(user) => f.write_str(user),
            Self::Organizaion(org) => f.write_str(org),
        }
    }
}

pub struct GithubClient {
    client: Client,
}

impl GithubClient {
    pub fn new(token: impl AsRef<str>) -> Result<Self> {
        let user_agent = format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        log::debug!("{}: {}", USER_AGENT.as_str(), user_agent);

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, "application/vnd.github.v3+json".try_into()?);
        headers.insert(
            AUTHORIZATION,
            format!("Bearer {}", token.as_ref()).try_into()?,
        );
        headers.insert(USER_AGENT, user_agent.try_into()?);

        let client = ClientBuilder::new().default_headers(headers).build()?;
        Ok(Self { client })
    }

    pub async fn get_package_version(
        &self,
        owner: &PackageOwner,
        package_name: impl Display,
        page: Option<u32>,
    ) -> Result<Vec<PackageVersion>> {
        let response = self
            .client
            .get(format!(
                "https://api.github.com/{base}/packages/container/{package_name}/versions?page={page}",
                base = owner.base_url(),
                page = page.unwrap_or(1),
            ))
            .send()
            .await
            .context("Failed to send request")?;

        if response.status().as_u16() == 404 {
            return Err(anyhow!("Package {}/{} does not exist", owner, package_name));
        } else if !response.status().is_success() {
            return Err(anyhow!("Server returned status {}", response.status()));
        }

        let versions = response
            .json()
            .await
            .context("Failed to parse reply as json")?;

        Ok(versions)
    }

    pub async fn delete_package_version(
        &self,
        owner: &PackageOwner,
        package_name: impl Display,
        version_id: impl Display,
    ) -> Result<()> {
        // The endpoint always returns 204 even if the version id is invalid.
        self.client
            .delete(format!(
                "https://api.github.com/{base}/packages/container/{package_name}/versions/{version_id}",
                base = owner.base_url(),
            ))
            .send()
            .await
            .context("Failed to send request")?;
        Ok(())
    }
}
