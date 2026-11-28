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
	containsEmphasizedItems: message(
		'fileExplorer.decorations.containsEmphasizedItems',
		'Accessible name and tooltip of the dot at the end of an Explorer folder row when decorations below it, such as changed files, color the folder, as VS Code words it.',
	),
	copyPath: message(
		'fileExplorer.actions.copyPath',
		'Explorer context menu command that copies the absolute paths of the selected files and folders to the clipboard, one per line, as VS Code names it.',
	),
	copyRelativePath: message(
		'fileExplorer.actions.copyRelativePath',
		'Explorer context menu command that copies the paths of the selected files and folders below the folder the Explorer shows to the clipboard, one per line, as VS Code names it.',
	),
	folderUnreadable: message(
		'fileExplorer.folder.unreadable',
		'Error notification when a folder in the Explorer tree could not be listed, so it shows empty, as VS Code reports it; {name} is the folder name and {reason} one of the read failure reasons, in lowercase.',
	),
	noDirectory: message(
		'fileExplorer.empty.noDirectory',
		'Shown in place of the Explorer tree when the active Workspace has no directory to browse; worded as the Workbench says it in place of terminals.',
	),
	openToSide: message(
		'fileExplorer.actions.openToSide',
		'Explorer context menu command that opens the selected files in the file overlay as tabs that stay open and moves the keyboard there, as VS Code names the command that opens them beside the current editor.',
	),
	progress: message(
		'fileExplorer.progress.label',
		'Accessible name of the thin progress bar at the top of the Explorer while folders are slow to list.',
	),
	readFailureDisconnected: message(
		'fileExplorer.readFailure.disconnected',
		'Why a folder could not be listed, quoted in lowercase by the unresolved root’s tooltip and by the notification for a folder that could not be read: the link to the machine that holds the folder dropped.',
	),
	readFailureFailed: message(
		'fileExplorer.readFailure.failed',
		'Why a folder could not be listed, quoted in lowercase by the unresolved root’s tooltip and by the notification for a folder that could not be read: listing the folder failed for any other reason.',
	),
	readFailureNotDirectory: message(
		'fileExplorer.readFailure.notDirectory',
		'Why a folder could not be listed, quoted in lowercase by the unresolved root’s tooltip and by the notification for a folder that could not be read: the path names a file rather than a folder.',
	),
	readFailureNotFound: message(
		'fileExplorer.readFailure.notFound',
		'Why a folder could not be listed, quoted in lowercase by the unresolved root’s tooltip and by the notification for a folder that could not be read: the folder was deleted or moved away.',
	),
	readFailurePermissionDenied: message(
		'fileExplorer.readFailure.permissionDenied',
		'Why a folder could not be listed, quoted in lowercase by the unresolved root’s tooltip and by the notification for a folder that could not be read: the operating system refused to list the folder.',
	),
	readFailureTimeout: message(
		'fileExplorer.readFailure.timeout',
		'Why a folder could not be listed, quoted in lowercase by the unresolved root’s tooltip and by the notification for a folder that could not be read: listing the folder did not finish in time, such as on a network drive that stopped answering.',
	),
	readFailureUnavailable: message(
		'fileExplorer.readFailure.unavailable',
		'Why a folder could not be listed, quoted in lowercase by the unresolved root’s tooltip and by the notification for a folder that could not be read: the file service on the machine that holds the folder is not running.',
	),
	readFailureUnsupportedLocation: message(
		'fileExplorer.readFailure.unsupportedLocation',
		'Why a folder could not be listed, quoted in lowercase by the unresolved root’s tooltip and by the notification for a folder that could not be read: the folder is a kind of location the Explorer cannot browse, such as a Windows network share path.',
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
		'Accessible name and tooltip of the error mark on the Explorer’s only row when the folder it browses cannot be listed, as VS Code words it; {reason} is one of the read failure reasons, in lowercase.',
	),
});
