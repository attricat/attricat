//! Application service for turning a selected release archive into an installed
//! extension. Registry discovery supplies the archive and source identity; this
//! service owns bounded unpacking and S3 staging, while the repository owns the
//! installation transaction.

use std::sync::Arc;

use bytes::Bytes;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    extensions::{ExtensionPackage, ManifestError},
    repository::{CatalogRepository, ExtensionInstallation, RepositoryError},
    storage::{ObjectStore, ObjectStoreError, StoredObject},
};

#[derive(Debug, Error)]
pub enum ExtensionInstallError {
    #[error("invalid extension release: {0}")]
    Package(#[from] ManifestError),
    #[error("extension artifact storage failed: {0}")]
    Storage(#[from] ObjectStoreError),
    #[error("extension installation failed: {0}")]
    Repository(#[from] RepositoryError),
}

/// Coordinates object-store staging with the repository's installation
/// transaction. S3 cannot participate in the SQL transaction, so all uploaded
/// objects are deleted if either staging or persistence fails.
#[derive(Clone)]
pub struct ExtensionInstaller {
    repository: CatalogRepository,
    object_store: Arc<dyn ObjectStore>,
}

impl ExtensionInstaller {
    pub fn new(repository: CatalogRepository, object_store: Arc<dyn ObjectStore>) -> Self {
        Self {
            repository,
            object_store,
        }
    }

    pub async fn install(
        &self,
        source: &str,
        archive: &[u8],
    ) -> Result<ExtensionInstallation, ExtensionInstallError> {
        self.install_package(source, ExtensionPackage::from_tar_zst(archive)?)
            .await
    }

    /// Installs an already validated package. Callers on an async runtime
    /// should unpack archives off the executor (e.g. with `spawn_blocking`).
    pub async fn install_package(
        &self,
        source: &str,
        package: ExtensionPackage,
    ) -> Result<ExtensionInstallation, ExtensionInstallError> {
        self.install_package_as(source, package, Uuid::new_v4())
            .await
    }

    /// Installs a package under an installed-release ID the caller reserved,
    /// such as one a reviewed solution-pack plan already references.
    pub async fn install_package_as(
        &self,
        source: &str,
        package: ExtensionPackage,
        installed_release_id: Uuid,
    ) -> Result<ExtensionInstallation, ExtensionInstallError> {
        let keys = self.stage_artifacts(&package, installed_release_id).await?;
        match self
            .repository
            .install_extension(package.manifest(), source, installed_release_id)
            .await
        {
            Ok(installation) => Ok(installation),
            Err(error) => {
                self.cleanup(&keys).await;
                Err(error.into())
            }
        }
    }

    pub async fn upgrade(
        &self,
        source: &str,
        archive: &[u8],
    ) -> Result<ExtensionInstallation, ExtensionInstallError> {
        self.upgrade_package(source, ExtensionPackage::from_tar_zst(archive)?)
            .await
    }

    /// Upgrades to an already validated package; see [`Self::install_package`].
    pub async fn upgrade_package(
        &self,
        source: &str,
        package: ExtensionPackage,
    ) -> Result<ExtensionInstallation, ExtensionInstallError> {
        let installed_release_id = Uuid::new_v4();
        let keys = self.stage_artifacts(&package, installed_release_id).await?;
        match self
            .repository
            .upgrade_extension(package.manifest(), source, installed_release_id)
            .await
        {
            Ok(installation) => Ok(installation),
            Err(error) => {
                self.cleanup(&keys).await;
                Err(error.into())
            }
        }
    }

    async fn stage_artifacts(
        &self,
        package: &ExtensionPackage,
        installed_release_id: Uuid,
    ) -> Result<Vec<String>, ExtensionInstallError> {
        let mut keys = Vec::new();
        for artifact in &package.manifest().artifacts {
            let bytes = package
                .artifacts()
                .find_map(|(path, bytes)| (path == artifact.path).then_some(bytes))
                .expect("validated package retains every declared artifact");
            let key = installed_artifact_key(installed_release_id, &artifact.id);
            // Include the key before writing: a timed-out or otherwise
            // ambiguous S3 error may still have created the object.
            keys.push(key.clone());
            if let Err(error) = self
                .object_store
                .put(
                    &key,
                    StoredObject {
                        bytes: Bytes::copy_from_slice(bytes),
                        content_type: None,
                    },
                )
                .await
            {
                self.cleanup(&keys).await;
                return Err(error.into());
            }
        }
        Ok(keys)
    }

    async fn cleanup(&self, keys: &[String]) {
        for key in keys.iter().rev() {
            if let Err(error) = self.object_store.delete(key).await {
                tracing::error!(%error, key, "failed to clean up staged extension artifact");
            }
        }
    }
}

/// Immutable, opaque, format-versioned object key derived from the
/// installed-release identity and manifest artifact ID. The runtime can derive
/// it from the release record and manifest without a second artifact table.
/// New storage layouts must use a new prefix rather than reinterpret v1 bytes.
pub fn installed_artifact_key(installed_release_id: Uuid, artifact_id: &str) -> String {
    format!("extensions/v1/{installed_release_id}/{artifact_id}")
}

#[cfg(test)]
mod tests {
    use super::installed_artifact_key;
    use uuid::Uuid;

    #[test]
    fn installed_artifact_keys_pin_the_storage_format_version() {
        let key = installed_artifact_key(Uuid::nil(), "server");
        assert_eq!(
            key,
            "extensions/v1/00000000-0000-0000-0000-000000000000/server"
        );
    }
}
