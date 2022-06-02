use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageVersion {
    pub id: u64,
    pub name: String,
    pub metadata: PackageVersionMetadata,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageVersionMetadata {
    pub package_type: String,
    pub container: ContainerVersionMetadata,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContainerVersionMetadata {
    pub tags: Vec<String>,
}
