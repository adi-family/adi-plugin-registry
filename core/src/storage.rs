use anyhow::{Context, Result};
use lib_plugin_registry::{
    PackageEntry, PackageInfo, PlatformBuild, PluginEntry, PluginInfo, RegistryIndex,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// File-based registry storage.
pub struct RegistryStorage {
    root: PathBuf,
}

impl RegistryStorage {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Initialize storage directories.
    pub async fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.root).await?;
        fs::create_dir_all(self.root.join("packages")).await?;
        fs::create_dir_all(self.root.join("plugins")).await?;

        // Create empty index if not exists
        let index_path = self.root.join("index.json");
        if !index_path.exists() {
            let index = RegistryIndex::default();
            let json = serde_json::to_string_pretty(&index)?;
            fs::write(&index_path, json).await?;
        }

        Ok(())
    }

    /// Load the registry index.
    pub async fn load_index(&self) -> Result<RegistryIndex> {
        let path = self.root.join("index.json");
        let data = fs::read_to_string(&path)
            .await
            .context("Failed to read index.json")?;
        serde_json::from_str(&data).context("Failed to parse index.json")
    }

    /// Save the registry index.
    pub async fn save_index(&self, index: &RegistryIndex) -> Result<()> {
        let path = self.root.join("index.json");
        let json = serde_json::to_string_pretty(index)?;
        fs::write(&path, json).await?;
        Ok(())
    }

    // === Package Operations ===

    /// Get package directory path.
    fn package_dir(&self, id: &str) -> PathBuf {
        self.root.join("packages").join(id)
    }

    /// Get package version directory path.
    fn package_version_dir(&self, id: &str, version: &str) -> PathBuf {
        self.package_dir(id).join(version)
    }

    /// Get package info for a specific version.
    pub async fn get_package_info(&self, id: &str, version: &str) -> Result<PackageInfo> {
        let path = self.package_version_dir(id, version).join("info.json");
        let data = fs::read_to_string(&path).await?;
        serde_json::from_str(&data).context("Failed to parse package info")
    }

    /// Get latest package version.
    pub async fn get_package_latest(&self, id: &str) -> Result<PackageInfo> {
        let index = self.load_index().await?;
        let entry = index
            .packages
            .iter()
            .find(|p| p.id == id)
            .context("Package not found")?;
        self.get_package_info(id, &entry.latest_version).await
    }

    /// Get package artifact path.
    pub fn package_artifact_path(&self, id: &str, version: &str, platform: &str) -> PathBuf {
        self.package_version_dir(id, version)
            .join(format!("{}.tar.gz", platform))
    }

    /// Publish a package version.
    #[allow(clippy::too_many_arguments)]
    pub async fn publish_package(
        &self,
        id: &str,
        name: &str,
        description: &str,
        version: &str,
        platform: &str,
        data: &[u8],
        author: &str,
        tags: Vec<String>,
    ) -> Result<()> {
        let version_dir = self.package_version_dir(id, version);
        fs::create_dir_all(&version_dir).await?;

        // Calculate checksum
        let mut hasher = Sha256::new();
        hasher.update(data);
        let checksum = hex::encode(hasher.finalize());

        // Write artifact
        let artifact_path = version_dir.join(format!("{}.tar.gz", platform));
        let mut file = fs::File::create(&artifact_path).await?;
        file.write_all(data).await?;

        // Load or create package info
        let info_path = version_dir.join("info.json");
        let mut info = if info_path.exists() {
            let data = fs::read_to_string(&info_path).await?;
            serde_json::from_str::<PackageInfo>(&data)?
        } else {
            PackageInfo {
                id: id.to_string(),
                version: version.to_string(),
                platforms: Vec::new(),
                published_at: now_unix(),
                changelog: None,
            }
        };

        // Add platform build
        let build = PlatformBuild {
            platform: platform.to_string(),
            download_url: format!("/v1/packages/{}/{}/{}.tar.gz", id, version, platform),
            size_bytes: data.len() as u64,
            checksum,
            signature: None,
        };

        // Update or add platform
        if let Some(existing) = info.platforms.iter_mut().find(|p| p.platform == platform) {
            *existing = build;
        } else {
            info.platforms.push(build);
        }

        // Save info
        let json = serde_json::to_string_pretty(&info)?;
        fs::write(&info_path, json).await?;

        // Update index
        self.update_package_index(id, name, description, version, author, tags)
            .await?;

        Ok(())
    }

    /// Update package entry in index.
    async fn update_package_index(
        &self,
        id: &str,
        name: &str,
        description: &str,
        version: &str,
        author: &str,
        tags: Vec<String>,
    ) -> Result<()> {
        let mut index = self.load_index().await?;

        if let Some(entry) = index.packages.iter_mut().find(|p| p.id == id) {
            // Update existing
            if semver_greater(version, &entry.latest_version) {
                entry.latest_version = version.to_string();
            }
            entry.name = name.to_string();
            entry.description = description.to_string();
            entry.author = author.to_string();
            entry.tags = tags;
        } else {
            // Add new
            index.packages.push(PackageEntry {
                id: id.to_string(),
                name: name.to_string(),
                description: description.to_string(),
                plugin_count: 0,
                plugin_ids: Vec::new(),
                latest_version: version.to_string(),
                downloads: 0,
                author: author.to_string(),
                tags,
            });
        }

        index.updated_at = now_unix();
        self.save_index(&index).await
    }

    // === Plugin Operations ===

    /// Get plugin directory path.
    fn plugin_dir(&self, id: &str) -> PathBuf {
        self.root.join("plugins").join(id)
    }

    /// Get plugin version directory path.
    fn plugin_version_dir(&self, id: &str, version: &str) -> PathBuf {
        self.plugin_dir(id).join(version)
    }

    /// Get plugin info for a specific version.
    pub async fn get_plugin_info(&self, id: &str, version: &str) -> Result<PluginInfo> {
        let path = self.plugin_version_dir(id, version).join("info.json");
        let data = fs::read_to_string(&path).await?;
        serde_json::from_str(&data).context("Failed to parse plugin info")
    }

    /// Get latest plugin version.
    pub async fn get_plugin_latest(&self, id: &str) -> Result<PluginInfo> {
        let index = self.load_index().await?;
        let entry = index
            .plugins
            .iter()
            .find(|p| p.id == id)
            .context("Plugin not found")?;
        self.get_plugin_info(id, &entry.latest_version).await
    }

    /// Get plugin artifact path.
    pub fn plugin_artifact_path(&self, id: &str, version: &str, platform: &str) -> PathBuf {
        self.plugin_version_dir(id, version)
            .join(format!("{}.tar.gz", platform))
    }

    /// Publish a plugin version.
    #[allow(clippy::too_many_arguments)]
    pub async fn publish_plugin(
        &self,
        id: &str,
        name: &str,
        description: &str,
        plugin_type: &str,
        version: &str,
        platform: &str,
        data: &[u8],
        author: &str,
        tags: Vec<String>,
    ) -> Result<()> {
        let version_dir = self.plugin_version_dir(id, version);
        fs::create_dir_all(&version_dir).await?;

        // Calculate checksum
        let mut hasher = Sha256::new();
        hasher.update(data);
        let checksum = hex::encode(hasher.finalize());

        // Write artifact
        let artifact_path = version_dir.join(format!("{}.tar.gz", platform));
        let mut file = fs::File::create(&artifact_path).await?;
        file.write_all(data).await?;

        // Load or create plugin info
        let info_path = version_dir.join("info.json");
        let mut info = if info_path.exists() {
            let data = fs::read_to_string(&info_path).await?;
            serde_json::from_str::<PluginInfo>(&data)?
        } else {
            PluginInfo {
                id: id.to_string(),
                version: version.to_string(),
                platforms: Vec::new(),
                published_at: now_unix(),
            }
        };

        // Add platform build
        let build = PlatformBuild {
            platform: platform.to_string(),
            download_url: format!("/v1/plugins/{}/{}/{}.tar.gz", id, version, platform),
            size_bytes: data.len() as u64,
            checksum,
            signature: None,
        };

        // Update or add platform
        if let Some(existing) = info.platforms.iter_mut().find(|p| p.platform == platform) {
            *existing = build;
        } else {
            info.platforms.push(build);
        }

        // Save info
        let json = serde_json::to_string_pretty(&info)?;
        fs::write(&info_path, json).await?;

        // Update index
        self.update_plugin_index(id, name, description, plugin_type, version, author, tags)
            .await?;

        Ok(())
    }

    /// Update plugin entry in index.
    #[allow(clippy::too_many_arguments)]
    async fn update_plugin_index(
        &self,
        id: &str,
        name: &str,
        description: &str,
        plugin_type: &str,
        version: &str,
        author: &str,
        tags: Vec<String>,
    ) -> Result<()> {
        let mut index = self.load_index().await?;

        if let Some(entry) = index.plugins.iter_mut().find(|p| p.id == id) {
            // Update existing
            if semver_greater(version, &entry.latest_version) {
                entry.latest_version = version.to_string();
            }
            entry.name = name.to_string();
            entry.description = description.to_string();
            entry.plugin_type = plugin_type.to_string();
            entry.author = author.to_string();
            entry.tags = tags;
        } else {
            // Add new
            index.plugins.push(PluginEntry {
                id: id.to_string(),
                name: name.to_string(),
                description: description.to_string(),
                plugin_type: plugin_type.to_string(),
                package_id: None,
                latest_version: version.to_string(),
                downloads: 0,
                author: author.to_string(),
                tags,
            });
        }

        index.updated_at = now_unix();
        self.save_index(&index).await
    }

    /// Increment download counter.
    pub async fn increment_downloads(&self, kind: &str, id: &str) -> Result<()> {
        let mut index = self.load_index().await?;

        match kind {
            "packages" => {
                if let Some(entry) = index.packages.iter_mut().find(|p| p.id == id) {
                    entry.downloads += 1;
                }
            }
            "plugins" => {
                if let Some(entry) = index.plugins.iter_mut().find(|p| p.id == id) {
                    entry.downloads += 1;
                }
            }
            _ => {}
        }

        self.save_index(&index).await
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn semver_greater(a: &str, b: &str) -> bool {
    match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(va), Ok(vb)) => va > vb,
        _ => a > b,
    }
}
