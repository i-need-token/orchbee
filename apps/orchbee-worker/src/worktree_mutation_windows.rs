//! Windows Git worktree effects; the job slot owns this function's whole lifetime.
use crate::git_repository_windows::canonical_path;
use orchbee_capability_node_protocol::{
    AbsoluteCwd, GitReference, WorktreeCheckout, WorktreeCheckoutSelector, WorktreeMutation,
    WorktreeMutationCompletion as Completion, WorktreeMutationEffect,
    WorktreeMutationError as Error, WorktreeMutationFailure, WorktreeMutationIntent,
    WorktreeMutationStage as Stage,
};
use std::{path::Path, sync::atomic::AtomicBool, time::Instant};

mod command;
mod listing;

struct Context<'a> {
    cancel: &'a AtomicBool,
    deadline: Instant,
    trust: bool,
    changed: bool,
    stage: Stage,
}

enum Abort {
    Known(WorktreeMutationFailure),
    PhysicalUnknown,
}

struct Repository {
    root: AbsoluteCwd,
    common: AbsoluteCwd,
    entries: Vec<listing::Entry>,
}

pub(crate) fn run(
    intent: WorktreeMutationIntent,
    cancel: &AtomicBool,
    deadline: Instant,
) -> Result<Completion, ()> {
    let mut context = Context {
        cancel,
        deadline,
        trust: intent.trust_repository(),
        changed: false,
        stage: Stage::Probe,
    };
    match execute(&mut context, &intent) {
        Ok(value) => Ok(value),
        Err(Abort::Known(failure)) => Ok(Completion::Failed(failure)),
        Err(Abort::PhysicalUnknown) => Err(()),
    }
}

fn execute(
    context: &mut Context<'_>,
    intent: &WorktreeMutationIntent,
) -> Result<Completion, Abort> {
    context.check()?;
    let source = canonical_path(Path::new(intent.source().as_str()))
        .map_err(|_| context.failure(Error::InvalidRepository, vec![]))?;
    let repository = context.repository(source.as_str())?;
    match intent.mutation() {
        WorktreeMutation::InspectSource => {
            let top = context.required(
                source.as_str(),
                &["rev-parse", "--show-toplevel"],
                Stage::Probe,
                false,
            )?;
            let top = std::str::from_utf8(&top)
                .map_err(|_| context.failure(Error::InvalidRepository, vec![]))?;
            let path = canonical_path(Path::new(top.trim_end_matches(['\r', '\n'])))
                .map_err(|_| context.failure(Error::InvalidRepository, vec![]))?;
            let entry =
                context.select(&repository, &WorktreeCheckoutSelector::Path(path), false)?;
            context
                .observe(&repository, entry)
                .map(Completion::Checkout)
        }
        WorktreeMutation::Create { path, branch, base } => {
            let branch_ref = format!("refs/heads/{}", branch.as_str());
            let checked = context.git(
                source.as_str(),
                &["check-ref-format", &branch_ref],
                Stage::CheckBranch,
                false,
            )?;
            if !checked.status.success() {
                return Err(context.failure(Error::InvalidReference, checked.stderr));
            }
            let exists = context.git(
                source.as_str(),
                &["show-ref", "--verify", "--quiet", &branch_ref],
                Stage::CheckBranch,
                false,
            )?;
            let args = match exists.status.code() {
                Some(0) => vec![
                    "worktree",
                    "add",
                    "--quiet",
                    "--",
                    path.as_str(),
                    branch.as_str(),
                ],
                Some(1) => vec![
                    "worktree",
                    "add",
                    "--quiet",
                    "-b",
                    branch.as_str(),
                    "--",
                    path.as_str(),
                    base.as_str(),
                ],
                _ => {
                    return Err(
                        context.failure(Error::GitFailed(exists.status.code()), exists.stderr)
                    );
                }
            };
            context.required(source.as_str(), &args, Stage::Mutate, true)?;
            let observed = context.repository(source.as_str())?;
            if observed.common != repository.common {
                return Err(context.failure(Error::RepositoryChanged, vec![]));
            }
            let entry = context.select(
                &observed,
                &WorktreeCheckoutSelector::Path(path.clone()),
                true,
            )?;
            let checkout = context.observe(&observed, entry)?;
            if checkout.branch() != Some(branch) {
                return Err(context.failure(Error::RepositoryChanged, vec![]));
            }
            Ok(Completion::Checkout(checkout))
        }
        WorktreeMutation::Open(selector) => {
            let entry = context.select(&repository, selector, false)?;
            context
                .observe(&repository, entry)
                .map(Completion::Checkout)
        }
        WorktreeMutation::InspectRemove { path } => {
            let entry = context.select(
                &repository,
                &WorktreeCheckoutSelector::Path(path.clone()),
                true,
            )?;
            context
                .observe(&repository, entry)
                .map(Completion::Checkout)
        }
        WorktreeMutation::Remove {
            path,
            expected_common_directory,
            force,
        } => {
            let expected = canonical_path(Path::new(expected_common_directory.as_str()))
                .map_err(|_| context.failure(Error::RepositoryChanged, vec![]))?;
            if expected != repository.common {
                return Err(context.failure(Error::RepositoryChanged, vec![]));
            }
            let entry = match context.select(
                &repository,
                &WorktreeCheckoutSelector::Path(path.clone()),
                true,
            ) {
                Ok(entry) => entry,
                Err(Abort::Known(failure)) if failure.error == Error::CheckoutNotFound => {
                    return context
                        .retire_unregistered(&repository, path)
                        .unwrap_or(Err(Abort::Known(failure)));
                }
                Err(abort) => return Err(abort),
            };
            let checkout = context.observe(&repository, entry)?;
            if checkout.dirty() && !force {
                return Err(context.failure(Error::Dirty, vec![]));
            }
            let mut args = vec!["worktree", "remove"];
            if *force {
                args.push("--force");
            }
            args.extend(["--", checkout.path().as_str()]);
            context.required(source.as_str(), &args, Stage::Mutate, true)?;
            let observed = context.repository(source.as_str())?;
            if observed.common != repository.common
                || registered(&observed, checkout.path())
                || std::fs::symlink_metadata(checkout.path().as_str()).is_ok()
            {
                return Err(context.failure(Error::RepositoryChanged, vec![]));
            }
            // Permission errors are not absence proofs.
            if !std::fs::symlink_metadata(checkout.path().as_str())
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            {
                return Err(context.failure(Error::Io, vec![]));
            }
            Ok(Completion::Removed {
                path: checkout.path().clone(),
            })
        }
    }
}

impl Context<'_> {
    fn repository(&mut self, source: &str) -> Result<Repository, Abort> {
        let stage = if self.changed {
            Stage::Verify
        } else {
            Stage::Probe
        };
        let common = self.required(
            source,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            stage,
            false,
        )?;
        let common = std::str::from_utf8(&common)
            .map_err(|_| self.failure(Error::InvalidRepository, vec![]))?;
        let common = canonical_path(Path::new(common.trim_end_matches(['\r', '\n'])))
            .map_err(|_| self.failure(Error::InvalidRepository, vec![]))?;
        let listing = self.required(
            source,
            &["worktree", "list", "--porcelain", "-z"],
            stage,
            false,
        )?;
        let entries =
            listing::parse(&listing).map_err(|_| self.failure(Error::InvalidRepository, vec![]))?;
        let first = entries
            .first()
            .ok_or_else(|| self.failure(Error::InvalidRepository, vec![]))?;
        let root = canonical_path(Path::new(first.path.as_str()))
            .map_err(|_| self.failure(Error::InvalidRepository, vec![]))?;
        Ok(Repository {
            root,
            common,
            entries,
        })
    }

    fn select<'a>(
        &mut self,
        repository: &'a Repository,
        selector: &WorktreeCheckoutSelector,
        linked_only: bool,
    ) -> Result<&'a listing::Entry, Abort> {
        self.stage = if self.changed {
            Stage::Verify
        } else {
            Stage::Select
        };
        let target = match selector {
            WorktreeCheckoutSelector::Path(path) => Some(
                canonical_path(Path::new(path.as_str()))
                    .map_err(|_| self.failure(Error::CheckoutNotFound, vec![]))?,
            ),
            WorktreeCheckoutSelector::Branch(_) => None,
        };
        let mut matching =
            repository
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| match selector {
                    WorktreeCheckoutSelector::Path(_) => {
                        canonical_path(Path::new(entry.path.as_str())).ok().as_ref()
                            == target.as_ref()
                    }
                    WorktreeCheckoutSelector::Branch(branch) => {
                        entry.branch.as_ref() == Some(branch)
                    }
                });
        let Some((index, entry)) = matching.next() else {
            return Err(self.failure(Error::CheckoutNotFound, vec![]));
        };
        if matching.next().is_some() {
            return Err(self.failure(Error::AmbiguousCheckout, vec![]));
        }
        if (linked_only && index == 0) || entry.bare {
            return Err(self.failure(Error::MainCheckout, vec![]));
        }
        Ok(entry)
    }

    fn observe(
        &mut self,
        repository: &Repository,
        entry: &listing::Entry,
    ) -> Result<WorktreeCheckout, Abort> {
        let path = canonical_path(Path::new(entry.path.as_str()))
            .map_err(|_| self.failure(Error::CheckoutNotFound, vec![]))?;
        let common = self.required(
            path.as_str(),
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            Stage::Verify,
            false,
        )?;
        let common = std::str::from_utf8(&common)
            .map_err(|_| self.failure(Error::RepositoryChanged, vec![]))?;
        if canonical_path(Path::new(common.trim_end_matches(['\r', '\n'])))
            .ok()
            .as_ref()
            != Some(&repository.common)
        {
            return Err(self.failure(Error::RepositoryChanged, vec![]));
        }
        let status = self.required(
            path.as_str(),
            &[
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--ignore-submodules=none",
            ],
            Stage::DirtyCheck,
            false,
        )?;
        WorktreeCheckout::new(
            repository.root.clone(),
            repository.common.clone(),
            path,
            entry.branch.clone(),
            !status.is_empty(),
        )
        .map_err(|_| self.failure(Error::TooLarge, vec![]))
    }

    /// Settles a checkout Git no longer lists. Git deletes the registration and contents even
    /// when the directory itself cannot be deleted, so only an absent path or an empty ordinary
    /// directory is accepted as that residue; `None` keeps the original failure.
    fn retire_unregistered(
        &mut self,
        repository: &Repository,
        path: &AbsoluteCwd,
    ) -> Option<Result<Completion, Abort>> {
        let target = Path::new(path.as_str());
        if !crate::git_repository_windows::is_drive_path(target) || registered(repository, path) {
            return None;
        }
        if let Err(abort) = self.check() {
            return Some(Err(abort));
        }
        self.stage = Stage::Mutate;
        match std::fs::symlink_metadata(target) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            // A link or reparse point is never a directory here, so it is never followed.
            Ok(metadata) if metadata.file_type().is_dir() => {
                match std::fs::read_dir(target).map(|mut entries| entries.next().is_none()) {
                    Ok(true) => {}
                    Ok(false) => return None,
                    Err(_) => return Some(Err(self.failure(Error::Io, vec![]))),
                }
                // Non-recursive: a directory that gained content is refused by the filesystem.
                match std::fs::remove_dir(target) {
                    Ok(()) => self.changed = true,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => {
                        return None;
                    }
                    Err(_) => return Some(Err(self.failure(Error::Io, vec![]))),
                }
            }
            Ok(_) => return None,
            Err(_) => return Some(Err(self.failure(Error::Io, vec![]))),
        }
        // Permission errors and pending deletion are not absence proofs.
        Some(
            if std::fs::symlink_metadata(target)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            {
                Ok(Completion::Removed { path: path.clone() })
            } else {
                Err(self.failure(Error::Io, vec![]))
            },
        )
    }
}

/// Git lists checkout paths as recorded; compare them without touching the filesystem.
fn registered(repository: &Repository, path: &AbsoluteCwd) -> bool {
    let spelling = |value: &str| {
        value
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_ascii_lowercase()
    };
    let path = spelling(path.as_str());
    repository
        .entries
        .iter()
        .any(|entry| spelling(entry.path.as_str()) == path)
}

fn failed(stage: Stage, changed: bool, error: Error, diagnostics: Vec<u8>) -> Abort {
    Abort::Known(
        WorktreeMutationFailure::new(
            stage,
            if changed {
                WorktreeMutationEffect::MayHaveChanged
            } else {
                WorktreeMutationEffect::None
            },
            error,
            diagnostics,
        )
        .expect("bounded Git diagnostics"),
    )
}

#[cfg(test)]
mod tests;
