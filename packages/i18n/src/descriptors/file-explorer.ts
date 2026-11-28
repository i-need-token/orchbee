import { defineMessages } from 'react-intl';

import { enUSFileExplorerMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSFileExplorerMessages);

/** The File Bar's Explorer: the tree of the active Workspace's current directory. */
export const fileExplorerMessages = defineMessages({
	collapse: message(
		'fileExplorer.actions.collapse',
		'Action and Explorer header button that collapses every expanded folder in the Explorer tree, as VS Code names it.',
	),
	noDirectory: message(
		'fileExplorer.empty.noDirectory',
		'Shown in place of the Explorer tree when the active Workspace has no directory to browse; worded as the Workbench says it in place of terminals.',
	),
	refresh: message(
		'fileExplorer.actions.refresh',
		'Action and Explorer header button that lists every loaded folder in the Explorer tree again, as VS Code names it.',
	),
	section: message(
		'fileExplorer.section.label',
		'Accessible name of the Explorer header, which shows the name of the folder the tree browses; {name} is that name.',
	),
	symbolicLink: message(
		'fileExplorer.decorations.symbolicLink',
		'Accessible name and tooltip of the mark beside an Explorer row whose entry is a symbolic link.',
	),
	tree: message(
		'fileExplorer.tree.label',
		'Accessible name of the Explorer tree of files and folders, as VS Code names it.',
	),
	unknownType: message(
		'fileExplorer.decorations.unknownType',
		'Accessible name and tooltip of the mark beside an Explorer row whose entry is neither a file nor a folder, such as a link to nothing.',
	),
	unresolved: message(
		'fileExplorer.root.unresolved',
		'Accessible name and tooltip of the error mark on the Explorer’s only row when the folder it browses cannot be listed, as VS Code words it; {reason} is one of the unresolved reasons, in lowercase.',
	),
	unresolvedDisconnected: message(
		'fileExplorer.root.unresolvedReason.disconnected',
		'Reason in the unresolved tooltip when the link to the machine that holds the folder dropped.',
	),
	unresolvedFailed: message(
		'fileExplorer.root.unresolvedReason.failed',
		'Reason in the unresolved tooltip when listing the folder failed for any other reason.',
	),
	unresolvedNotDirectory: message(
		'fileExplorer.root.unresolvedReason.notDirectory',
		'Reason in the unresolved tooltip when the path names a file rather than a folder.',
	),
	unresolvedNotFound: message(
		'fileExplorer.root.unresolvedReason.notFound',
		'Reason in the unresolved tooltip when the folder was deleted or moved away.',
	),
	unresolvedPermissionDenied: message(
		'fileExplorer.root.unresolvedReason.permissionDenied',
		'Reason in the unresolved tooltip when the operating system refused to list the folder.',
	),
	unresolvedTimeout: message(
		'fileExplorer.root.unresolvedReason.timeout',
		'Reason in the unresolved tooltip when listing the folder did not finish in time, such as on a network drive that stopped answering.',
	),
	unresolvedUnavailable: message(
		'fileExplorer.root.unresolvedReason.unavailable',
		'Reason in the unresolved tooltip when the file service on the machine that holds the folder is not running.',
	),
	unresolvedUnsupportedLocation: message(
		'fileExplorer.root.unresolvedReason.unsupportedLocation',
		'Reason in the unresolved tooltip when the folder is a kind of location the Explorer cannot browse, such as a Windows network share path.',
	),
});
