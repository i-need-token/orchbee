export const enUSSourceControlMessages = {
	'sourceControl.actions.category': 'Git',
	'sourceControl.actions.closeOtherRepositories': 'Close Other Repositories',
	'sourceControl.actions.closeRepository': 'Close Repository',
	'sourceControl.actions.collapseAll': 'Collapse All',
	'sourceControl.actions.deletePermanently': 'Delete Permanently',
	'sourceControl.actions.discard': 'Discard Changes',
	'sourceControl.actions.discardAll': 'Discard All Changes',
	'sourceControl.actions.more': 'More Actions…',
	'sourceControl.actions.sortByName': 'Sort Changes by Name',
	'sourceControl.actions.sortByPath': 'Sort Changes by Path',
	'sourceControl.actions.sortByStatus': 'Sort Changes by Status',
	'sourceControl.actions.stage': 'Stage Changes',
	'sourceControl.actions.stageAll': 'Stage All Changes',
	'sourceControl.actions.unstage': 'Unstage Changes',
	'sourceControl.actions.unstageAll': 'Unstage All Changes',
	'sourceControl.actions.viewAndSort': 'View & Sort',
	'sourceControl.actions.viewAsList': 'View as List',
	'sourceControl.actions.viewAsTree': 'View as Tree',
	'sourceControl.badge': '{count, plural, one {# pending change} other {# pending changes}}',
	'sourceControl.changes.label': 'Changes',
	'sourceControl.checkout.committing': '{head}, Committing Changes…',
	'sourceControl.checkout.rebasing': '{head} (Rebasing)',
	'sourceControl.checkout.syncing': '{head}, Synchronizing Changes…',
	'sourceControl.closedRepositories.placeholder': 'Pick a repository to reopen',
	'sourceControl.closedRepositories.title': 'Reopen Closed Repositories',
	'sourceControl.discard.mixed.description':
		'{os, select, windows {{untracked, plural, =1 {You can restore this file from the Recycle Bin.} other {You can restore these files from the Recycle Bin.}}} other {{untracked, plural, =1 {You can restore this file from the Trash.} other {You can restore these files from the Trash.}}}} {tracked, plural, =1 {Are you sure you want to discard changes in “{name}”?} other {Are you sure you want to discard ALL changes in # files?}} This is IRREVERSIBLE! Your current working set will be FOREVER LOST if you proceed.',
	'sourceControl.discard.permanent.confirm':
		'{count, plural, =1 {Delete File} other {Delete All # Files}}',
	'sourceControl.discard.permanent.description': 'This action is irreversible!',
	'sourceControl.discard.permanent.title':
		'{os, select, windows {Failed to delete using the Recycle Bin. Do you want to permanently delete instead?} other {Failed to delete using the Trash. Do you want to permanently delete instead?}}',
	'sourceControl.discard.restore.confirm':
		'{count, plural, =1 {Restore File} other {Restore All # Files}}',
	'sourceControl.discard.restore.description':
		'{count, plural, =1 {The file comes back with its last staged or committed content.} other {The files come back with their last staged or committed content.}}',
	'sourceControl.discard.restore.title':
		'{count, plural, =1 {Are you sure you want to restore “{name}”?} other {Are you sure you want to restore ALL # files?}}',
	'sourceControl.discard.tracked.confirm':
		'{count, plural, =1 {Discard File} other {Discard All # Files}}',
	'sourceControl.discard.tracked.description':
		'{count, plural, =1 {This action is irreversible!} other {This is IRREVERSIBLE! Your current working set will be FOREVER LOST if you proceed.}}',
	'sourceControl.discard.tracked.title':
		'{count, plural, =1 {Are you sure you want to discard changes in “{name}”?} other {Are you sure you want to discard ALL changes in # files?}}',
	'sourceControl.discard.untracked.confirm':
		'{os, select, windows {Move to Recycle Bin} other {Move to Trash}}',
	'sourceControl.discard.untracked.description':
		'{os, select, windows {{count, plural, =1 {You can restore this file from the Recycle Bin.} other {You can restore these files from the Recycle Bin.}}} other {{count, plural, =1 {You can restore this file from the Trash.} other {You can restore these files from the Trash.}}}}',
	'sourceControl.discard.untracked.title':
		'{count, plural, =1 {Are you sure you want to DELETE the following untracked file: “{name}”?} other {Are you sure you want to DELETE the # untracked files?}}',
	'sourceControl.error.disconnected': 'The machine is disconnected.',
	'sourceControl.error.generic': 'Git error',
	'sourceControl.error.git': 'Git: {hint}',
	'sourceControl.error.permissionDenied': 'Permission denied.',
	'sourceControl.error.timeout': 'Git took too long to respond.',
	'sourceControl.error.unavailable': 'Git on this machine is unavailable.',
	'sourceControl.group.index': 'Staged Changes',
	'sourceControl.group.merge': 'Merge Changes',
	'sourceControl.group.workingTree': 'Changes',
	'sourceControl.parentRepositories.placeholder': 'Pick a repository to open',
	'sourceControl.parentRepositories.title': 'Open Repositories In Parent Folders',
	'sourceControl.progress.label': 'Running Git',
	'sourceControl.repositories.label': 'Repositories',
	'sourceControl.repository.label': '{name} Git',
	'sourceControl.repositoryPicker.all': 'All Repositories',
	'sourceControl.repository.title': 'Git: {path}',
	'sourceControl.section.label': '{name} Section',
	'sourceControl.status.addedByThem': 'Conflict: Added By Them',
	'sourceControl.status.addedByUs': 'Conflict: Added By Us',
	'sourceControl.status.bothAdded': 'Conflict: Both Added',
	'sourceControl.status.bothDeleted': 'Conflict: Both Deleted',
	'sourceControl.status.bothModified': 'Conflict: Both Modified',
	'sourceControl.status.deleted': 'Deleted',
	'sourceControl.status.deletedByThem': 'Conflict: Deleted By Them',
	'sourceControl.status.deletedByUs': 'Conflict: Deleted By Us',
	'sourceControl.status.ignored': 'Ignored',
	'sourceControl.status.indexAdded': 'Index Added',
	'sourceControl.status.indexCopied': 'Index Copied',
	'sourceControl.status.indexDeleted': 'Index Deleted',
	'sourceControl.status.indexModified': 'Index Modified',
	'sourceControl.status.indexRenamed': 'Index Renamed',
	'sourceControl.status.intentToAdd': 'Intent to Add',
	'sourceControl.status.intentToRename': 'Intent to Rename',
	'sourceControl.status.modified': 'Modified',
	'sourceControl.status.typeChanged': 'Type Changed',
	'sourceControl.status.untracked': 'Untracked',
	'sourceControl.sync.both': '{behind, number}↓ {ahead, number}↑',
	'sourceControl.sync.committing': 'Committing Changes…',
	'sourceControl.sync.incoming': '{behind, number}↓',
	'sourceControl.sync.publish': 'Publish Branch',
	'sourceControl.sync.pull':
		'{count, plural, one {Pull # commit from {upstream}} other {Pull # commits from {upstream}}}',
	'sourceControl.sync.pullAndPush':
		'Pull {behind, number} and push {ahead, plural, one {# commit} other {# commits}} between {upstream}',
	'sourceControl.sync.push':
		'{count, plural, one {Push # commit to {upstream}} other {Push # commits to {upstream}}}',
	'sourceControl.sync.synchronize': 'Synchronize Changes',
	'sourceControl.sync.syncing': 'Synchronizing Changes…',
	'sourceControl.tree.label': 'Source Control Management',
	'sourceControl.unsafeRepositories.placeholder': 'Pick a repository to mark as safe and open',
	'sourceControl.unsafeRepositories.title': 'Manage Unsafe Repositories',
	'sourceControl.welcome.closedRepositories':
		'{count, plural, one {A Git repository was found that was previously closed.} other {Git repositories were found that were previously closed.}}',
	'sourceControl.welcome.initialize': 'Initialize Repository',
	'sourceControl.welcome.manageUnsafe': 'Manage Unsafe Repositories',
	'sourceControl.welcome.missingGit.downloadLinux': 'Download Git for Linux',
	'sourceControl.welcome.missingGit.downloadMac': 'Download Git for macOS',
	'sourceControl.welcome.missingGit.downloadWindows': 'Download Git for Windows',
	'sourceControl.welcome.missingGit.linux': 'Source control depends on Git being installed.',
	'sourceControl.welcome.missingGit.remote':
		'Install Git, a popular source control system, to track code changes and collaborate with others.',
	'sourceControl.welcome.noDirectory': 'No directory configured.',
	'sourceControl.welcome.noRepository':
		'The folder currently open doesn’t have a Git repository. You can initialize a repository which will enable source control features powered by Git.',
	'sourceControl.welcome.openRepository': 'Open Repository',
	'sourceControl.welcome.parentRepositories':
		'{count, plural, one {A Git repository was found in the parent folders of the open folder or files.} other {Git repositories were found in the parent folders of the open folder or files.}}',
	'sourceControl.welcome.reopenClosed':
		'{count, plural, one {Reopen Closed Repository} other {Reopen Closed Repositories}}',
	'sourceControl.welcome.scanning': 'Scanning folder for Git repositories…',
	'sourceControl.welcome.unsafeRepositories':
		'{count, plural, one {The detected Git repository is potentially unsafe as the folder is owned by someone other than the current user.} other {The detected Git repositories are potentially unsafe as the folders are owned by someone other than the current user.}}',
} as const;

export type SourceControlMessageId = keyof typeof enUSSourceControlMessages;
