//! One serialized offline install/update/uninstall workflow with atomic catalog commitment. A
//! source is fetched into the store's pending source, then moved into its plugin's folder under
//! its version's name before it builds, so every name it lives under is known before creation.
use super::{
    Catalog, Error, FILE, Generation, MAX_REVISION, PluginRegistry, PluginRegistryEntry,
    REGISTRY_LIMIT, REGISTRY_RECORD, decode, encode, load, naming, platform, storage,
};
use crate::{
    CommandFailure, CommandOutput, PluginGithubSource, PluginManagedSource, PrivateCache,
    plugin_git_reference,
};
use serde::Serialize;
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

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
    /// An update target has no recorded managed source, such as a locally linked plugin.
    SourceUnrecorded,
    /// The updated source declares a different plugin ID than the one being updated.
    IdentityChanged,
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
/// The managed provenance an update replaces: its plugin ID and the commit it was built from.
struct Recorded {
    id: String,
    commit: String,
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
        let result = owner.install(source, reference, id, None, executor);
        owner.finish(result, executor)
    }
    /// Re-installs a managed plugin from its recorded source and requested reference. Enabled
    /// state and user directories are retained; a fetched commit equal to the recorded one
    /// commits nothing. Consent belongs to the caller, as for `install`.
    pub fn update(
        &self,
        target: &str,
        id: &str,
        executor: &mut impl PluginProvisionExecutor,
    ) -> PluginProvisionReport {
        let target = match super::super::plugin_identifier(target) {
            Ok(target) => target,
            Err(error) => return failed(error.into()),
        };
        if !super::super::source::installation_id(id) {
            return failed(Error::InvalidPath.into());
        }
        if let Err(reason) = executor.checkpoint() {
            return failed(PluginProvisionFailure::Command(reason));
        }
        let mut owner = match Provision::open(self) {
            Ok(owner) => owner,
            Err(error) => return failed(error),
        };
        let result = owner.update(&target, id, executor);
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
        let store = PrivateCache::acquire(registry.root(), REGISTRY_RECORD).map_err(storage)?;
        let catalog = decode(
            store
                .read(FILE, REGISTRY_LIMIT)
                .map_err(storage)?
                .as_deref(),
            &registry.paths,
        )?;
        Ok(Self {
            registry,
            store,
            catalog,
            report: PluginProvisionReport::new(),
        })
    }
    fn persist(&mut self) -> Result<(), PluginProvisionFailure> {
        let bytes = encode(&self.catalog, &self.registry.paths)?;
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
    /// Records `generation` for removal unless a recorded key already names its folder.
    fn queue(&mut self, generation: &Generation) -> Result<(), PluginProvisionFailure> {
        let recorded = self
            .catalog
            .cleanup
            .iter()
            .filter_map(|key| Generation::parse(key))
            .any(|queued| queued.same_folder(generation));
        if !recorded {
            if self.catalog.cleanup.len() == 64 {
                return Err(Error::LimitExceeded.into());
            }
            self.catalog.cleanup.push(generation.key());
        }
        Ok(())
    }
    fn dequeue(&mut self, generation: &Generation) {
        self.catalog.cleanup.retain(|key| {
            Generation::parse(key).is_none_or(|queued| !queued.same_folder(generation))
        });
    }
    /// The source folder a registered managed entry lives in, which decoding already checked.
    fn generation(
        &self,
        entry: &PluginRegistryEntry,
    ) -> Result<Generation, PluginProvisionFailure> {
        let folder = self
            .catalog
            .folders
            .get(&entry.manifest.id)
            .ok_or(Error::InvalidRegistry)?;
        let canonical = self.registry.root().canonicalize().map_err(storage)?;
        let sources = super::PluginStorePaths::new(canonical).sources(folder);
        match Path::new(&entry.plugin_root)
            .strip_prefix(&sources)
            .ok()
            .and_then(|rest| rest.components().next())
        {
            Some(Component::Normal(version)) => Ok(Generation::Source {
                folder: folder.clone(),
                version: version.to_str().ok_or(Error::InvalidRegistry)?.to_owned(),
            }),
            _ => Err(Error::InvalidRegistry.into()),
        }
    }
    /// The first name the plugin's `version` can take among its source folders: neither present
    /// nor recorded for removal.
    fn free_version(&self, folder: &str, version: &str) -> Result<String, PluginProvisionFailure> {
        let base = naming::spelling(version, "version");
        for name in naming::candidates(&base) {
            let candidate = Generation::Source {
                folder: folder.to_owned(),
                version: name.clone(),
            };
            let recorded = self
                .catalog
                .cleanup
                .iter()
                .filter_map(|key| Generation::parse(key))
                .any(|queued| queued.same_folder(&candidate));
            if recorded {
                continue;
            }
            match fs::symlink_metadata(candidate.path(&self.registry.paths)) {
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(name),
                Err(error) => return Err(storage(error).into()),
            }
        }
        Err(Error::LimitExceeded.into())
    }
    fn update(
        &mut self,
        target: &str,
        id: &str,
        executor: &mut impl PluginProvisionExecutor,
    ) -> Result<(), PluginProvisionFailure> {
        let source = self
            .catalog
            .plugins
            .iter()
            .find(|p| p.manifest.id == target)
            .ok_or(Error::PluginNotFound)?
            .source
            .clone()
            .ok_or(PluginProvisionFailure::SourceUnrecorded)?;
        let recorded = Recorded {
            id: target.into(),
            commit: source.resolved_commit,
        };
        self.install(
            source.github,
            source.requested_ref,
            id,
            Some(&recorded),
            executor,
        )
    }
    fn install(
        &mut self,
        source: PluginGithubSource,
        reference: Option<String>,
        id: &str,
        update: Option<&Recorded>,
        executor: &mut impl PluginProvisionExecutor,
    ) -> Result<(), PluginProvisionFailure> {
        self.report.stage = PluginProvisionStage::Cleanup;
        self.cleanup(executor)?;
        self.report.stage = PluginProvisionStage::Prepare;
        executor
            .checkpoint()
            .map_err(PluginProvisionFailure::Command)?;
        let pending = self.registry.paths.pending_source();
        // Never adopt an existing directory: recorded cleanup has removed every one it knew.
        if fs::symlink_metadata(&pending).is_ok() {
            return Err(Error::InvalidPath.into());
        }
        self.queue(&Generation::Pending)?;
        self.persist()?;
        fs::create_dir(&pending).map_err(storage)?;
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
            self.git(&pending, args, executor)?;
        }
        let resolved = self.git(&pending, vec!["rev-parse".into(), "HEAD".into()], executor)?;
        let resolved_commit = String::from_utf8(resolved.stdout)
            .map_err(|_| Error::InvalidManifest)?
            .trim()
            .to_owned();
        if let Some(update) = update
            && update.commit == resolved_commit
        {
            // Already current: the pending source stays queued, so finishing removes it.
            self.report.plugin = self
                .catalog
                .plugins
                .iter()
                .find(|p| p.manifest.id == update.id)
                .cloned();
            return Ok(());
        }
        self.report.stage = PluginProvisionStage::Inspect;
        let within = |checkout: &Path| {
            source
                .subdir
                .as_ref()
                .map_or_else(|| checkout.to_owned(), |subdir| checkout.join(subdir))
        };
        let fetched = load(&within(&pending), true)?;
        if update.is_some_and(|update| update.id != fetched.manifest.id) {
            return Err(PluginProvisionFailure::IdentityChanged);
        }
        let canonical_pending = pending.canonicalize().map_err(storage)?;
        if !Path::new(&fetched.plugin_root).starts_with(&canonical_pending) {
            return Err(Error::InvalidPath.into());
        }
        let existing = self
            .catalog
            .plugins
            .iter()
            .position(|p| p.manifest.id == fetched.manifest.id);
        if existing.is_some_and(|i| self.catalog.plugins[i].source.is_none()) {
            return Err(PluginProvisionFailure::LinkedConflict);
        }
        if existing.is_none() && self.catalog.plugins.len() == 32 {
            return Err(Error::LimitExceeded.into());
        }
        let provenance = PluginManagedSource {
            github: source.clone(),
            requested_ref: reference,
            resolved_commit,
            installation_id: id.into(),
        };
        provenance.validate()?;
        // Build outputs may record their own location, so the source moves to its final folder,
        // recorded for removal first, before any build command runs.
        let (folder, _) = self
            .registry
            .assign_folder(&mut self.catalog, &fetched.manifest.id)?;
        let version = self.free_version(&folder, &fetched.manifest.version)?;
        let generation = Generation::Source {
            folder: folder.clone(),
            version,
        };
        self.queue(&generation)?;
        self.persist()?;
        let _pins =
            platform::pin_private(&self.registry.paths.sources(&folder)).map_err(storage)?;
        let checkout = generation.path(&self.registry.paths);
        self.place(&pending, &checkout, executor)?;
        self.dequeue(&Generation::Pending);
        self.persist()?;
        let root = within(&checkout);
        let before = load(&root, true)?;
        if before.manifest != fetched.manifest {
            return Err(PluginProvisionFailure::ManifestChanged);
        }
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
            let old = self.generation(&self.catalog.plugins[index])?;
            self.queue(&old)?;
            self.catalog.plugins[index] = entry.clone();
        } else {
            self.catalog.plugins.push(entry.clone());
        }
        self.catalog
            .plugins
            .sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
        self.dequeue(&generation);
        if let Err(error) = self.bump_revision().and_then(|()| self.persist()) {
            self.catalog = saved;
            return Err(error);
        }
        self.report.plugin = Some(entry);
        self.report.changed = true;
        self.report.output = None;
        Ok(())
    }
    /// Moves the fetched source to its final folder. A virus scanner or indexer that briefly holds
    /// a file just written refuses the move on Windows, so a refusal is retried within a short
    /// bound before the install fails.
    fn place(
        &self,
        pending: &Path,
        checkout: &Path,
        executor: &mut impl PluginProvisionExecutor,
    ) -> Result<(), PluginProvisionFailure> {
        let mut attempts = 0;
        loop {
            match fs::rename(pending, checkout) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied && attempts < 20 => {
                    attempts += 1;
                    executor
                        .checkpoint()
                        .map_err(PluginProvisionFailure::Command)?;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(error) => return Err(storage(error).into()),
            }
        }
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
        if self.catalog.plugins[index].source.is_some() {
            let generation = self.generation(&self.catalog.plugins[index])?;
            self.queue(&generation)?;
        }
        let entry = self.catalog.plugins.remove(index);
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
            .filter_map(|key| Generation::parse(key))
            .map(|generation| {
                generation
                    .path(&self.registry.paths)
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
