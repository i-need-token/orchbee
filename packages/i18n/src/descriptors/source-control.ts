import { defineMessages } from 'react-intl';

import { enUSSourceControlMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSSourceControlMessages);

/** The File Bar's Source Control view: VS Code's Git experience over the Workspace's repositories. */
export const sourceControlMessages = defineMessages({
	actionCategory: message(
		'sourceControl.actions.category',
		'Category of Source Control’s Git Actions in Action search, as VS Code’s Git commands are grouped.',
	),
	badge: message(
		'sourceControl.badge',
		'Hover text of the count on the Source Control toggle: every change of every shown repository.',
	),
	buttonCommit: message(
		'sourceControl.button.commit',
		'Title of the action button that commits, and of the Commit Action.',
	),
	buttonCommitAmend: message(
		'sourceControl.button.commitAmend',
		"Title of the action button's command that amends the last commit.",
	),
	buttonCommitAndPush: message(
		'sourceControl.button.commitAndPush',
		"Title of the action button's command that commits, then pushes.",
	),
	buttonCommitAndSync: message(
		'sourceControl.button.commitAndSync',
		"Title of the action button's command that commits, then syncs.",
	),
	buttonContinue: message(
		'sourceControl.button.continue',
		'Title of the action button that continues a rebase or merge.',
	),
	buttonPublish: message(
		'sourceControl.button.publish',
		'Title of the action button that publishes the current branch.',
	),
	buttonSync: message(
		'sourceControl.button.sync',
		'Title of the action button that pulls and pushes; the commit counts follow it.',
	),
	buttonTooltipCommit: message(
		'sourceControl.button.tooltip.commit',
		'Tooltip of the Commit button without a branch.',
	),
	buttonTooltipCommitAndPush: message(
		'sourceControl.button.tooltip.commitAndPush',
		'Tooltip of the Commit & Push button.',
	),
	buttonTooltipCommitAndSync: message(
		'sourceControl.button.tooltip.commitAndSync',
		'Tooltip of the Commit & Sync button.',
	),
	buttonTooltipCommitOnBranch: message(
		'sourceControl.button.tooltip.commitOnBranch',
		'Tooltip of the Commit button; branch is the current branch.',
	),
	buttonTooltipCommittingAndPushing: message(
		'sourceControl.button.tooltip.committingAndPushing',
		'Tooltip of the Commit & Push button while it runs.',
	),
	buttonTooltipCommittingAndSyncing: message(
		'sourceControl.button.tooltip.committingAndSyncing',
		'Tooltip of the Commit & Sync button while it runs.',
	),
	buttonTooltipContinueMerge: message(
		'sourceControl.button.tooltip.continueMerge',
		'Tooltip of the Continue button during a merge.',
	),
	buttonTooltipContinueRebase: message(
		'sourceControl.button.tooltip.continueRebase',
		'Tooltip of the Continue button during a rebase.',
	),
	buttonTooltipContinuingMerge: message(
		'sourceControl.button.tooltip.continuingMerge',
		'Tooltip of the Continue button while a merge continues.',
	),
	buttonTooltipContinuingRebase: message(
		'sourceControl.button.tooltip.continuingRebase',
		'Tooltip of the Continue button while a rebase continues.',
	),
	buttonTooltipPublish: message(
		'sourceControl.button.tooltip.publish',
		'Tooltip of the Publish Branch button; branch is the current branch.',
	),
	buttonTooltipPublishing: message(
		'sourceControl.button.tooltip.publishing',
		'Tooltip of the Publish Branch button while it runs.',
	),
	changes: message(
		'sourceControl.changes.label',
		'Title of the section that lists the repositories’ changes, as VS Code’s Changes view.',
	),
	checkoutCommitting: message(
		'sourceControl.checkout.committing',
		'Hover text of a repository’s branch while a commit runs. {head} is the branch with its marks.',
	),
	checkoutRebasing: message(
		'sourceControl.checkout.rebasing',
		'A repository’s branch while a rebase stops in it. {head} is the branch with its marks.',
	),
	checkoutSyncing: message(
		'sourceControl.checkout.syncing',
		'Hover text of a repository’s branch while a sync runs. {head} is the branch with its marks.',
	),
	closeOtherRepositories: message(
		'sourceControl.actions.closeOtherRepositories',
		'Action in a repository’s context menu that closes every other shown repository, as VS Code’s Close Other Repositories.',
	),
	closeRepository: message(
		'sourceControl.actions.closeRepository',
		'Action in a repository’s context menu that stops showing it until it is reopened, as VS Code’s Close Repository.',
	),
	closedRepositoriesPlaceholder: message(
		'sourceControl.closedRepositories.placeholder',
		'Placeholder of the quick pick that lists the repositories the person closed.',
	),
	closedRepositoriesTitle: message(
		'sourceControl.closedRepositories.title',
		'Title of the quick pick that reopens repositories the person closed.',
	),
	collapseAll: message(
		'sourceControl.actions.collapseAll',
		'Group menu Action that collapses every folder of the group in tree view.',
	),
	commitMessagePlaceholder: message(
		'sourceControl.commit.messagePlaceholder',
		'Placeholder of the prompt for a commit message; branch is the current branch.',
	),
	commitMessagePlaceholderNoBranch: message(
		'sourceControl.commit.messagePlaceholderNoBranch',
		'Placeholder of the prompt for a commit message without a branch.',
	),
	commitMessagePrompt: message(
		'sourceControl.commit.messagePrompt',
		'Prompt asking for a commit message when the input is empty.',
	),
	commitNoChanges: message(
		'sourceControl.commit.noChanges',
		'Notice when a commit finds nothing to commit.',
	),
	deletePermanently: message(
		'sourceControl.actions.deletePermanently',
		'Action and confirmation button that delete untracked files the trash could not take, as VS Code’s Delete Permanently.',
	),
	deleteUntrackedConfirm: message(
		'sourceControl.discard.untracked.confirm',
		'Button that moves the untracked files the question names to the trash. {os} is the operating system of the machine holding them.',
	),
	deleteUntrackedDescription: message(
		'sourceControl.discard.untracked.description',
		'Explanation under the question before moving untracked files to the trash. {os} is the operating system of the machine holding them; {count} is how many files.',
	),
	deleteUntrackedTitle: message(
		'sourceControl.discard.untracked.title',
		'Question before moving untracked files to the trash. {count} is how many files; {name} is the file’s name when there is one.',
	),
	discard: message(
		'sourceControl.actions.discard',
		'Action and row button that discard the chosen files’ changes, as VS Code’s Discard Changes.',
	),
	discardAll: message(
		'sourceControl.actions.discardAll',
		'Action and button on the Changes group that discard every change in it, as VS Code’s Discard All Changes.',
	),
	discardMixedDescription: message(
		'sourceControl.discard.mixed.description',
		'Explanation under the question before discarding untracked and tracked files together. {os} is the machine’s operating system; {untracked} and {tracked} count each kind; {name} is the tracked file’s name when there is one.',
	),
	discardTrackedConfirm: message(
		'sourceControl.discard.tracked.confirm',
		'Button that discards the changes the question names. {count} is how many files.',
	),
	discardTrackedDescription: message(
		'sourceControl.discard.tracked.description',
		'Explanation under the question before discarding tracked files’ changes. {count} is how many files.',
	),
	discardTrackedTitle: message(
		'sourceControl.discard.tracked.title',
		'Question before discarding tracked files’ changes. {count} is how many files; {name} is the file’s name when there is one.',
	),
	errorAuthenticationFailed: message(
		'sourceControl.error.authenticationFailed',
		'Error when the remote refuses the credentials; remote is the address Git names.',
	),
	errorAuthenticationFailedPlain: message(
		'sourceControl.error.authenticationFailedPlain',
		'Error when the remote refuses the credentials and Git names no address.',
	),
	errorConflict: message(
		'sourceControl.error.conflict',
		'Warning when a pull or sync leaves merge conflicts.',
	),
	errorDirtyWorkTree: message(
		'sourceControl.error.dirtyWorkTree',
		'Error when local changes keep Git from updating the working tree.',
	),
	errorDisconnected: message(
		'sourceControl.error.disconnected',
		'Git failed because the link to the machine that holds the repository dropped.',
	),
	errorEmptyCommitMessage: message(
		'sourceControl.error.emptyCommitMessage',
		'Notice when Git cancels a commit for its empty message.',
	),
	errorGeneric: message('sourceControl.error.generic', 'Git failed without saying why.'),
	errorGit: message(
		'sourceControl.error.git',
		'Git failed. {hint} is the first line of Git’s own error output, shown as Git wrote it.',
	),
	errorNoUserConfigured: message(
		'sourceControl.error.noUserConfigured',
		'Error when a commit finds no Git identity.',
	),
	errorPermissionDenied: message(
		'sourceControl.error.permissionDenied',
		'Git failed because the person may not change these files.',
	),
	errorPushRejected: message(
		'sourceControl.error.pushRejected',
		'Error when the remote rejects a push.',
	),
	errorTimeout: message(
		'sourceControl.error.timeout',
		'Git failed because the machine took too long to answer.',
	),
	errorUnavailable: message(
		'sourceControl.error.unavailable',
		'Git failed because the machine’s file service is not running.',
	),
	fetch: message('sourceControl.actions.fetch', 'Title of the Fetch Action.'),
	groupIndex: message('sourceControl.group.index', 'Group of changes added to the Git index.'),
	groupMerge: message('sourceControl.group.merge', 'Group of conflicted paths during a merge.'),
	groupWorkingTree: message(
		'sourceControl.group.workingTree',
		'Group of changes in the working tree, untracked files included.',
	),
	inputPlaceholder: message(
		'sourceControl.input.placeholder',
		"Placeholder of a repository's commit input, as VS Code words it; key is the Accept Input shortcut, branch the current branch or commit.",
	),
	inputPlaceholderNoBranch: message(
		'sourceControl.input.placeholderNoBranch',
		'Placeholder of a commit input in a repository without a HEAD; key is the Accept Input shortcut.',
	),
	inputTooManyChanges: message(
		'sourceControl.input.tooManyChanges',
		"Warning below a commit input while the repository's status stopped at its bound.",
	),
	inputValidationClose: message(
		'sourceControl.input.validationClose',
		"Button that hides the commit input's warning.",
	),
	moreActions: message(
		'sourceControl.actions.more',
		'Button on the Changes title that opens the menu of Source Control’s other Actions.',
	),
	parentRepositoriesPlaceholder: message(
		'sourceControl.parentRepositories.placeholder',
		'Placeholder of the quick pick that lists the repositories found in parent folders.',
	),
	parentRepositoriesTitle: message(
		'sourceControl.parentRepositories.title',
		'Title of the quick pick that opens repositories found in parent folders.',
	),
	permanentConfirm: message(
		'sourceControl.discard.permanent.confirm',
		'Button that deletes permanently the untracked files the trash could not take. {count} is how many files.',
	),
	permanentDescription: message(
		'sourceControl.discard.permanent.description',
		'Explanation under the question to delete permanently the untracked files the trash could not take.',
	),
	permanentTitle: message(
		'sourceControl.discard.permanent.title',
		'Question when the trash could not take untracked files being discarded. {os} is the operating system of the machine holding them.',
	),
	progress: message(
		'sourceControl.progress.label',
		'Accessible name of the line at the top of Source Control that runs while Git works.',
	),
	publish: message('sourceControl.actions.publish', 'Title of the Publish Branch Action.'),
	pull: message('sourceControl.actions.pull', 'Title of the Pull Action.'),
	push: message('sourceControl.actions.push', 'Title of the Push Action.'),
	remoteConfirmSync: message(
		'sourceControl.remote.confirmSync',
		'Question before a sync; remote and branch name the upstream.',
	),
	remoteFetchAll: message(
		'sourceControl.remote.fetchAll',
		'Fetch picker item that fetches every remote.',
	),
	remoteFetchNoRemotes: message(
		'sourceControl.remote.fetchNoRemotes',
		'Warning when a fetch finds no remote.',
	),
	remoteFetchPick: message(
		'sourceControl.remote.fetchPick',
		'Placeholder of the remote picker a fetch shows.',
	),
	remoteOk: message('sourceControl.remote.ok', 'Answer that goes ahead with a sync or publish.'),
	remoteOkDontAskAgain: message(
		'sourceControl.remote.okDontAskAgain',
		'Answer that publishes and stops asking for the run.',
	),
	remoteOkDontShowAgain: message(
		'sourceControl.remote.okDontShowAgain',
		'Answer that syncs and stops asking before syncs.',
	),
	remotePublishNoRemotes: message(
		'sourceControl.remote.publishNoRemotes',
		'Warning when a publish finds no remote.',
	),
	remotePublishPick: message(
		'sourceControl.remote.publishPick',
		'Placeholder of the remote picker a publish shows; branch is the current branch.',
	),
	remotePublishQuestion: message(
		'sourceControl.remote.publishQuestion',
		'Question when a push finds no upstream; branch is the current branch.',
	),
	remotePullNoRemotes: message(
		'sourceControl.remote.pullNoRemotes',
		'Warning when a pull finds no remote.',
	),
	remotePushDetached: message(
		'sourceControl.remote.pushDetached',
		'Warning when a push finds no branch checked out.',
	),
	remotePushNoRemotes: message(
		'sourceControl.remote.pushNoRemotes',
		'Warning when a push finds no remote.',
	),
	repositories: message(
		'sourceControl.repositories.label',
		'Title of the section that lists the Workspace’s Git repositories with their branches.',
	),
	repositoryLabel: message(
		'sourceControl.repository.label',
		'Accessible name of a repository row. {name} is the repository folder’s name; Git is the provider.',
	),
	repositoryPickerAll: message(
		'sourceControl.repositoryPicker.all',
		'Quick pick item that chooses every repository listed above it.',
	),
	repositoryPickerPlaceholder: message(
		'sourceControl.repositoryPicker.placeholder',
		'Placeholder of the picker that asks which repository a command runs in.',
	),
	repositoryTitle: message(
		'sourceControl.repository.title',
		'Hover text of a repository row. {path} is the repository’s whole path.',
	),
	restoreConfirm: message(
		'sourceControl.discard.restore.confirm',
		'Button that restores the deleted files the question names. {count} is how many files.',
	),
	restoreDescription: message(
		'sourceControl.discard.restore.description',
		'Explanation under the question before restoring deleted files. {count} is how many files.',
	),
	restoreTitle: message(
		'sourceControl.discard.restore.title',
		'Question before restoring deleted tracked files from the index. {count} is how many files; {name} is the file’s name when there is one.',
	),
	section: message(
		'sourceControl.section.label',
		'Accessible name of a section header. {name} is the section title.',
	),
	smartCommitAlways: message(
		'sourceControl.smartCommit.always',
		'Answer that stages every change now and turns smart commit on.',
	),
	smartCommitNever: message(
		'sourceControl.smartCommit.never',
		'Answer that commits nothing and stops suggesting smart commit.',
	),
	smartCommitPlaceholder: message(
		'sourceControl.smartCommit.placeholder',
		'The question a commit asks when nothing is staged.',
	),
	smartCommitTitle: message(
		'sourceControl.smartCommit.title',
		'Title of the question a commit asks when nothing is staged.',
	),
	smartCommitYes: message(
		'sourceControl.smartCommit.yes',
		'Answer that stages every change for this commit.',
	),
	sortByName: message(
		'sourceControl.actions.sortByName',
		'View & Sort menu item that lists changes by file name.',
	),
	sortByPath: message(
		'sourceControl.actions.sortByPath',
		'View & Sort menu item that lists changes by path.',
	),
	sortByStatus: message(
		'sourceControl.actions.sortByStatus',
		'View & Sort menu item that lists changes by status.',
	),
	stage: message(
		'sourceControl.actions.stage',
		'Action and row button that stage the chosen files’ changes, as VS Code’s Stage Changes.',
	),
	stageAll: message(
		'sourceControl.actions.stageAll',
		'Action and button on the Changes group that stage every change in it, as VS Code’s Stage All Changes.',
	),
	statusAddedByThem: message('sourceControl.status.addedByThem', 'Hover text of a Git status.'),
	statusAddedByUs: message('sourceControl.status.addedByUs', 'Hover text of a Git status.'),
	statusBothAdded: message('sourceControl.status.bothAdded', 'Hover text of a Git status.'),
	statusBothDeleted: message('sourceControl.status.bothDeleted', 'Hover text of a Git status.'),
	statusBothModified: message('sourceControl.status.bothModified', 'Hover text of a Git status.'),
	statusDeleted: message('sourceControl.status.deleted', 'Hover text of a Git status.'),
	statusDeletedByThem: message('sourceControl.status.deletedByThem', 'Hover text of a Git status.'),
	statusDeletedByUs: message('sourceControl.status.deletedByUs', 'Hover text of a Git status.'),
	statusIgnored: message('sourceControl.status.ignored', 'Hover text of a Git status.'),
	statusIndexAdded: message('sourceControl.status.indexAdded', 'Hover text of a Git status.'),
	statusIndexCopied: message('sourceControl.status.indexCopied', 'Hover text of a Git status.'),
	statusIndexDeleted: message('sourceControl.status.indexDeleted', 'Hover text of a Git status.'),
	statusIndexModified: message('sourceControl.status.indexModified', 'Hover text of a Git status.'),
	statusIndexRenamed: message('sourceControl.status.indexRenamed', 'Hover text of a Git status.'),
	statusIntentToAdd: message('sourceControl.status.intentToAdd', 'Hover text of a Git status.'),
	statusIntentToRename: message(
		'sourceControl.status.intentToRename',
		'Hover text of a Git status.',
	),
	statusModified: message('sourceControl.status.modified', 'Hover text of a Git status.'),
	statusTypeChanged: message('sourceControl.status.typeChanged', 'Hover text of a Git status.'),
	statusUntracked: message('sourceControl.status.untracked', 'Hover text of a Git status.'),
	sync: message('sourceControl.actions.sync', 'Title of the Sync Action.'),
	syncBoth: message(
		'sourceControl.sync.both',
		'Text of the sync control: commits to pull, then commits to push.',
	),
	syncCommitting: message(
		'sourceControl.sync.committing',
		'Hover text of the sync control while a commit runs, which disables it.',
	),
	syncIncoming: message(
		'sourceControl.sync.incoming',
		'Text of the sync control when the remote cannot be pushed to: commits to pull.',
	),
	syncPublish: message(
		'sourceControl.sync.publish',
		'Hover text of the control that pushes a branch without an upstream and sets one.',
	),
	syncPull: message(
		'sourceControl.sync.pull',
		'Hover text of the sync control. {upstream} is the upstream branch, such as origin/main.',
	),
	syncPullAndPush: message(
		'sourceControl.sync.pullAndPush',
		'Hover text of the sync control. {upstream} is the upstream branch, such as origin/main.',
	),
	syncPush: message(
		'sourceControl.sync.push',
		'Hover text of the sync control. {upstream} is the upstream branch, such as origin/main.',
	),
	syncSynchronize: message(
		'sourceControl.sync.synchronize',
		'Hover text of the sync control when the branch is even with its upstream.',
	),
	syncSyncing: message(
		'sourceControl.sync.syncing',
		'Hover text of the sync control while a fetch, pull, push or sync runs.',
	),
	tree: message('sourceControl.tree.label', 'Accessible name of the Changes tree.'),
	unsafeRepositoriesPlaceholder: message(
		'sourceControl.unsafeRepositories.placeholder',
		'Placeholder of the quick pick that lists repositories Git refuses as unsafe.',
	),
	unsafeRepositoriesTitle: message(
		'sourceControl.unsafeRepositories.title',
		'Title of the quick pick that marks unsafe repositories as safe and opens them.',
	),
	unstage: message(
		'sourceControl.actions.unstage',
		'Action and row button that unstage the chosen files’ changes, as VS Code’s Unstage Changes.',
	),
	unstageAll: message(
		'sourceControl.actions.unstageAll',
		'Action and button on the Staged Changes group that unstage every change in it, as VS Code’s Unstage All Changes.',
	),
	viewAndSort: message(
		'sourceControl.actions.viewAndSort',
		'Title menu that switches the list and tree views and the sort order.',
	),
	viewAsList: message(
		'sourceControl.actions.viewAsList',
		'View & Sort menu item that lists changes flat, with folder paths.',
	),
	viewAsTree: message(
		'sourceControl.actions.viewAsTree',
		'View & Sort menu item that nests changes under their folders.',
	),
	welcomeClosedRepositories: message(
		'sourceControl.welcome.closedRepositories',
		'Shown in Source Control when only repositories the person closed were found.',
	),
	welcomeInitialize: message(
		'sourceControl.welcome.initialize',
		'Button that runs git init in the open folder.',
	),
	welcomeManageUnsafe: message(
		'sourceControl.welcome.manageUnsafe',
		'Button that lets the person mark unsafe repositories as safe in their global Git configuration.',
	),
	welcomeMissingGitDownloadLinux: message(
		'sourceControl.welcome.missingGit.downloadLinux',
		'Button that opens Git’s download page for Linux.',
	),
	welcomeMissingGitDownloadMac: message(
		'sourceControl.welcome.missingGit.downloadMac',
		'Button that opens Git’s download page for macOS.',
	),
	welcomeMissingGitDownloadWindows: message(
		'sourceControl.welcome.missingGit.downloadWindows',
		'Button that opens Git’s download page for Windows.',
	),
	welcomeMissingGitLinux: message(
		'sourceControl.welcome.missingGit.linux',
		'Shown above the Linux download button when the machine has no usable Git.',
	),
	welcomeMissingGitRemote: message(
		'sourceControl.welcome.missingGit.remote',
		'Shown when another machine holding the files has no usable Git.',
	),
	welcomeNoDirectory: message(
		'sourceControl.welcome.noDirectory',
		'Shown in Source Control when the Workspace has no current directory.',
	),
	welcomeNoRepository: message(
		'sourceControl.welcome.noRepository',
		'Shown in Source Control when the open folder holds no Git repository.',
	),
	welcomeOpenRepository: message(
		'sourceControl.welcome.openRepository',
		'Button that opens the Git repositories found in parent folders.',
	),
	welcomeParentRepositories: message(
		'sourceControl.welcome.parentRepositories',
		'Shown in Source Control when repositories above the open folder wait for the person’s consent.',
	),
	welcomeReopenClosed: message(
		'sourceControl.welcome.reopenClosed',
		'Button that reopens repositories the person closed.',
	),
	welcomeScanning: message(
		'sourceControl.welcome.scanning',
		'Shown in Source Control until the first search for repositories answers.',
	),
	welcomeUnsafeRepositories: message(
		'sourceControl.welcome.unsafeRepositories',
		'Shown in Source Control when Git refuses repositories owned by another user.',
	),
});
