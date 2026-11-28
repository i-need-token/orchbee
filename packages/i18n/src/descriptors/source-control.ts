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
	errorDisconnected: message(
		'sourceControl.error.disconnected',
		'Git failed because the link to the machine that holds the repository dropped.',
	),
	errorGeneric: message('sourceControl.error.generic', 'Git failed without saying why.'),
	errorGit: message(
		'sourceControl.error.git',
		'Git failed. {hint} is the first line of Git’s own error output, shown as Git wrote it.',
	),
	errorPermissionDenied: message(
		'sourceControl.error.permissionDenied',
		'Git failed because the person may not change these files.',
	),
	errorTimeout: message(
		'sourceControl.error.timeout',
		'Git failed because the machine took too long to answer.',
	),
	errorUnavailable: message(
		'sourceControl.error.unavailable',
		'Git failed because the machine’s file service is not running.',
	),
	groupIndex: message('sourceControl.group.index', 'Group of changes added to the Git index.'),
	groupMerge: message('sourceControl.group.merge', 'Group of conflicted paths during a merge.'),
	groupWorkingTree: message(
		'sourceControl.group.workingTree',
		'Group of changes in the working tree, untracked files included.',
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
	progress: message(
		'sourceControl.progress.label',
		'Accessible name of the line at the top of Source Control that runs while Git works.',
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
	repositoryTitle: message(
		'sourceControl.repository.title',
		'Hover text of a repository row. {path} is the repository’s whole path.',
	),
	section: message(
		'sourceControl.section.label',
		'Accessible name of a section header. {name} is the section title.',
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
