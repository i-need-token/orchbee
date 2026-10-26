//! One serialized offline install/uninstall workflow with atomic catalog commitment.
use super::{
    Catalog, Error, FILE, MAX_REVISION, PluginRegistry, PluginRegistryEntry, REGISTRY_LIMIT,
    decode, load, storage,
};
use crate::{
    CommandFailure, CommandOutput, PluginGithubSource, PluginManagedSource, PrivateCache,
    plugin_git_reference,
};
use serde::Serialize;
use std::path::{Path, PathBuf};

mod cleanup;
#[cfg(all(test, windows))]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginProvisionStage {
    Prepare,
    Checkout,
    Inspect,
    Build,
    Commit,
    Cleanup,
    Done,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "reason", rename_all = "kebab-case")]
pub enum PluginProvisionFailure {
    Registry(Error),
    Command(CommandFailure),
    ManifestChanged,
    LinkedConflict,
    CleanupIncomplete,
    PhysicalUnknown,
}
impl From<Error> for PluginProvisionFailure {
    fn from(value: Error) -> Self {
        Self::Registry(value)
    }
}

pub struct PluginProvisionCommand {
    pub stage: PluginProvisionStage,
    pub directory: PathBuf,
    pub argv: Vec<String>,
}
/// The invoking application owns process capture, deadline and interruption.
pub trait PluginProvisionExecutor {
    fn checkpoint(&mut self) -> Result<(), CommandFailure>;
    fn run(&mut self, command: &PluginProvisionCommand) -> CommandOutput;
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginProvisionOutput {
    pub command: Vec<String>,
    pub exit_code: Option<i32>,
    pub failure: Option<CommandFailure>,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginProvisionReport {
    pub stage: PluginProvisionStage,
    pub plugin: Option<PluginRegistryEntry>,
    pub changed: bool,
    pub cleanup_pending: Vec<String>,
    pub failure: Option<PluginProvisionFailure>,
    pub output: Option<PluginProvisionOutput>,
}
impl PluginProvisionReport {
    fn new() -> Self {
        Self {
            stage: PluginProvisionStage::Prepare,
            plugin: None,
            changed: false,
            cleanup_pending: vec![],
            failure: None,
            output: None,
        }
    }
}
struct Provision<'a> {
    registry: &'a PluginRegistry,
    store: PrivateCache,
    catalog: Catalog,
    report: PluginProvisionReport,
}
impl PluginRegistry {
    /// Explicit remote-source installation. Grammar/consent belong to the caller.
    pub fn install(
        &self,
        source: PluginGithubSource,
        reference: Option<String>,
        id: &str,
        executor: &mut impl PluginProvisionExecutor,
    ) -> PluginProvisionReport {
        let valid = source.validate().and_then(|()| {
            if super::super::source::installation_id(id)
                && reference.as_deref().is_none_or(plugin_git_reference)
            {
                Ok(())
            } else {
                Err(Error::InvalidPath)
            }
        });
        if let Err(error) = valid {
            return failed(error.into());
        }
        if let Err(reason) = executor.checkpoint() {
            return failed(PluginProvisionFailure::Command(reason));
        }
        let mut owner = match Provision::open(self) {
            Ok(owner) => owner,
            Err(error) => return failed(error),
        };
        let result = owner.install(source, reference, id, executor);
        owner.finish(result, executor)
    }
    /// Unlinks a local source without deleting it; removes only explicitly managed source trees.
    pub fn uninstall(
        &self,
        target: &str,
        executor: &mut impl PluginProvisionExecutor,
    ) -> PluginProvisionReport {
        let source = PluginGithubSource::parse(target).ok();
        if source.is_none() && super::super::plugin_identifier(target).is_err() {
            return failed(Error::InvalidIdentifier.into());
        }
        if let Err(reason) = executor.checkpoint() {
            return failed(PluginProvisionFailure::Command(reason));
        }
        let mut owner = match Provision::open(self) {
            Ok(owner) => owner,
            Err(error) => return failed(error),
        };
        let result = owner.uninstall(target.trim(), source.as_ref(), executor);
        owner.finish(result, executor)
    }
}
fn failed(failure: PluginProvisionFailure) -> PluginProvisionReport {
    let mut report = PluginProvisionReport::new();
    report.failure = Some(failure);
    report
}
impl<'a> Provision<'a> {
    fn open(registry: &'a PluginRegistry) -> Result<Self, PluginProvisionFailure> {
        let store = PrivateCache::acquire(&registry.root).map_err(storage)?;
        let catalog = decode(
            store
                .read(FILE, REGISTRY_LIMIT)
                .map_err(storage)?
                .as_deref(),
            &registry.root,
        )?;
        Ok(Self {
            registry,
            store,
            catalog,
            report: PluginProvisionReport::new(),
        })
    }
    fn persist(&mut self) -> Result<(), PluginProvisionFailure> {
        let bytes = serde_json::to_vec(&self.catalog).map_err(|_| Error::InvalidRegistry)?;
        if bytes.len() > REGISTRY_LIMIT {
            return Err(Error::LimitExceeded.into());
        }
        decode(Some(&bytes), &self.registry.root)?;
        self.store.replace(FILE, &bytes).map_err(storage)?;
        Ok(())
    }
    fn bump_revision(&mut self) -> Result<(), PluginProvisionFailure> {
        self.catalog.revision = self
            .catalog
            .revision
            .checked_add(1)
            .filter(|n| *n <= MAX_REVISION)
            .ok_or(Error::LimitExceeded)?;
        Ok(())
    }
    fn queue(&mut self, id: &str) -> Result<(), PluginProvisionFailure> {
        if !self.catalog.cleanup.iter().any(|s| s == id) {
            if self.catalog.cleanup.len() == 64 {
                return Err(Error::LimitExceeded.into());
            }
            self.catalog.cleanup.push(id.into());
        }
        Ok(())
    }
    fn install(
        &mut self,
        source: PluginGithubSource,
        reference: Option<String>,
        id: &str,
        executor: &mut impl PluginProvisionExecutor,
    ) -> Result<(), PluginProvisionFailure> {
        self.report.stage = PluginProvisionStage::Cleanup;
        self.cleanup(executor)?;
        self.report.stage = PluginProvisionStage::Prepare;
        executor
            .checkpoint()
            .map_err(PluginProvisionFailure::Command)?;
        let managed = self.registry.root.join("managed");
        let _managed = PrivateCache::acquire(&managed).map_err(storage)?;
        let container = managed.join(format!("i-{id}"));
        // Never adopt an existing directory, even if the caller reused a valid identity.
        if std::fs::symlink_metadata(&container).is_ok() {
            return Err(Error::InvalidPath.into());
        }
        self.queue(id)?;
        self.persist()?;
        std::fs::create_dir(&container).map_err(storage)?;
        let checkout = container.join("checkout");
        std::fs::create_dir(&checkout).map_err(storage)?;
        self.report.stage = PluginProvisionStage::Checkout;
        for args in [
            vec!["init".into()],
            vec![
                "remote".into(),
                "add".into(),
                "origin".into(),
                source.remote_url(),
            ],
            vec![
                "fetch".into(),
                "--depth=1".into(),
                "--".into(),
                "origin".into(),
                reference.clone().unwrap_or_else(|| "HEAD".into()),
            ],
            vec!["checkout".into(), "--detach".into(), "FETCH_HEAD".into()],
        ] {
            self.git(&checkout, args, executor)?;
        }
        let resolved = self.git(&checkout, vec!["rev-parse".into(), "HEAD".into()], executor)?;
        let resolved_commit = String::from_utf8(resolved.stdout)
            .map_err(|_| Error::InvalidManifest)?
            .trim()
            .to_owned();
        self.report.stage = PluginProvisionStage::Inspect;
        let root = source
            .subdir
            .as_ref()
            .map_or_else(|| checkout.clone(), |subdir| checkout.join(subdir));
        let before = load(&root, true)?;
        let canonical_checkout = checkout.canonicalize().map_err(storage)?;
        if !Path::new(&before.plugin_root).starts_with(&canonical_checkout) {
            return Err(Error::InvalidPath.into());
        }
        let existing = self
            .catalog
            .plugins
            .iter()
            .position(|p| p.manifest.id == before.manifest.id);
        if existing.is_some_and(|i| self.catalog.plugins[i].source.is_none()) {
            return Err(PluginProvisionFailure::LinkedConflict);
        }
        if existing.is_none() && self.catalog.plugins.len() == 32 {
            return Err(Error::LimitExceeded.into());
        }
        let provenance = PluginManagedSource {
            github: source,
            requested_ref: reference,
            resolved_commit,
            installation_id: id.into(),
        };
        provenance.validate()?;
        self.report.stage = PluginProvisionStage::Build;
        for step in &before.manifest.build {
            if before.manifest.supports_windows(&step.platforms) {
                self.command(
                    PluginProvisionCommand {
                        stage: PluginProvisionStage::Build,
                        directory: root.clone(),
                        argv: step.command.clone(),
                    },
                    executor,
                )?;
            }
        }
        let mut entry = load(&root, true)?;
        if entry != before {
            return Err(PluginProvisionFailure::ManifestChanged);
        }
        executor
            .checkpoint()
            .map_err(PluginProvisionFailure::Command)?;
        self.report.stage = PluginProvisionStage::Commit;
        entry.source = Some(provenance);
        let saved = self.catalog.clone();
        if let Some(index) = existing {
            entry.enabled = self.catalog.plugins[index].enabled;
            let old = self.catalog.plugins[index]
                .source
                .as_ref()
                .expect("managed replacement checked")
                .installation_id
                .clone();
            self.queue(&old)?;
            self.catalog.plugins[index] = entry.clone();
        } else {
            self.catalog.plugins.push(entry.clone());
        }
        self.catalog
            .plugins
            .sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
        self.catalog.cleanup.retain(|candidate| candidate != id);
        if let Err(error) = self.bump_revision().and_then(|()| self.persist()) {
            self.catalog = saved;
            return Err(error);
        }
        self.report.plugin = Some(entry);
        self.report.changed = true;
        self.report.output = None;
        Ok(())
    }
    fn git(
        &mut self,
        directory: &Path,
        args: Vec<String>,
        executor: &mut impl PluginProvisionExecutor,
    ) -> Result<CommandOutput, PluginProvisionFailure> {
        let mut argv = vec![
            "git.exe".into(),
            "-c".into(),
            "core.hooksPath=NUL".into(),
            "-c".into(),
            "core.symlinks=false".into(),
        ];
        argv.extend(args);
        self.command(
            PluginProvisionCommand {
                stage: PluginProvisionStage::Checkout,
                directory: directory.into(),
                argv,
            },
            executor,
        )
    }
    fn command(
        &mut self,
        command: PluginProvisionCommand,
        executor: &mut impl PluginProvisionExecutor,
    ) -> Result<CommandOutput, PluginProvisionFailure> {
        executor
            .checkpoint()
            .map_err(PluginProvisionFailure::Command)?;
        // Publish possible physical ownership before creation. A crash or a failed
        // publication after settlement must not make an unproved owner deletable.
        self.catalog
            .blocked_cleanup
            .clone_from(&self.catalog.cleanup);
        self.persist()?;
        let output = executor.run(&command);
        if output.failure != Some(CommandFailure::PhysicalUnknown) {
            self.catalog.blocked_cleanup.clear();
            if let Err(error) = self.persist() {
                self.catalog
                    .blocked_cleanup
                    .clone_from(&self.catalog.cleanup);
                return Err(error);
            }
        }
        if let Some(failure) = output.failure {
            self.report.output = Some(PluginProvisionOutput {
                command: command.argv,
                exit_code: output.exit_code,
                failure: output.failure,
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                stdout_truncated: output.stdout_truncated,
                stderr_truncated: output.stderr_truncated,
            });
            return Err(PluginProvisionFailure::Command(failure));
        }
        Ok(output)
    }
    fn uninstall(
        &mut self,
        target: &str,
        source: Option<&PluginGithubSource>,
        executor: &mut impl PluginProvisionExecutor,
    ) -> Result<(), PluginProvisionFailure> {
        self.report.stage = PluginProvisionStage::Cleanup;
        self.cleanup(executor)?;
        self.report.stage = PluginProvisionStage::Prepare;
        let index = self
            .catalog
            .plugins
            .iter()
            .position(|p| match source {
                Some(source) => p.source.as_ref().is_some_and(|s| s.github == *source),
                None => p.manifest.id == target,
            })
            .ok_or(Error::PluginNotFound)?;
        self.report.stage = PluginProvisionStage::Commit;
        let saved = self.catalog.clone();
        let entry = self.catalog.plugins.remove(index);
        if let Some(source) = &entry.source {
            self.queue(&source.installation_id)?;
        }
        if let Err(error) = self.bump_revision().and_then(|()| self.persist()) {
            self.catalog = saved;
            return Err(error);
        }
        self.report.plugin = Some(entry);
        self.report.changed = true;
        Ok(())
    }
    fn finish(
        mut self,
        result: Result<(), PluginProvisionFailure>,
        executor: &mut impl PluginProvisionExecutor,
    ) -> PluginProvisionReport {
        self.report.failure = result.err();
        let failed_stage = self.report.stage;
        self.report.stage = PluginProvisionStage::Cleanup;
        if failed_stage != PluginProvisionStage::Cleanup
            && let Err(error) = self.cleanup(executor)
            && self.report.failure.is_none()
        {
            self.report.failure = Some(error);
        }
        self.report.cleanup_pending = self
            .catalog
            .cleanup
            .iter()
            .map(|id| {
                self.registry
                    .root
                    .join("managed")
                    .join(format!("i-{id}"))
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        self.report.stage = if self.report.failure.is_none() {
            PluginProvisionStage::Done
        } else if result_stage_is_cleanup(failed_stage, self.report.changed) {
            PluginProvisionStage::Cleanup
        } else {
            failed_stage
        };
        self.report
    }
}
fn result_stage_is_cleanup(stage: PluginProvisionStage, committed: bool) -> bool {
    committed || stage == PluginProvisionStage::Cleanup
}
