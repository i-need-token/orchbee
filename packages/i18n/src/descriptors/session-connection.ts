import { defineMessages } from 'react-intl';

import { enUSSessionConnectionMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSSessionConnectionMessages);

/** The variables of a refusal that names the refused versions. */
const REFUSED_VERSIONS =
	'desktopVersion is this Desktop’s product version; hasSessionVersion is true when the running Session stated its product version, sessionVersion.';

export const sessionConnectionMessages = defineMessages({
	disconnected: message('session.connection.disconnected'),
	reconnect: message('session.connection.reconnect'),
	reconnectDescription: message('session.connection.reconnectDescription'),
	unavailableDescription: message('session.connection.unavailable.description'),
	unavailableDesktopTooOld: message(
		'session.connection.unavailable.desktopTooOld',
		`The running Session refused this Desktop as too old. Matches the TUI and CLI refusal. ${REFUSED_VERSIONS}`,
	),
	unavailableDevelopmentBuildMismatch: message(
		'session.connection.unavailable.developmentBuildMismatch',
		'A development build whose protocol differs from the running Session’s was refused. Matches the TUI and CLI refusal. The <command> tag holds a command, which is not translated.',
	),
	unavailableOtherEnvironment: message(
		'session.connection.unavailable.otherEnvironment',
		'The running Session belongs to another environment, such as release beside a development build. Matches the TUI and CLI refusal.',
	),
	unavailableRecoveryBlocked: message(
		'session.connection.unavailable.recoveryBlocked',
		'A process an earlier start left running keeps the Session from recovering. Matches the TUI and CLI refusal word for word. role is server, worker or terminal-host, whose names Server, Worker and Terminal Host stay in English; program is its program file name and processId its process ID, neither translated. system is macos or windows and picks the tool the system ends a process with: Activity Monitor on macOS, Task Manager on Windows.',
	),
	unavailableRecoveryRequired: message(
		'session.connection.unavailable.recoveryRequired',
		'The <command> tag holds a command, which is not translated.',
	),
	unavailableServerNotStarted: message(
		'session.connection.unavailable.serverNotStarted',
		'The system refused to create the Session’s Server process. Matches the TUI and CLI refusal word for word. cause is missing when the program file was not found, denied when access to it was denied, code when the system stated an error number, code, and other otherwise.',
	),
	unavailableSessionTooOld: message(
		'session.connection.unavailable.sessionTooOld',
		`The running Session is too old for this Desktop; without a version it predates version checks, and only the Orchbee that started it can stop it. Where restartable is true (Windows), a restart replaces its Server and keeps every terminal; elsewhere it must be stopped. Matches the TUI and CLI refusal. The <command> tag holds a command, which is not translated. ${REFUSED_VERSIONS}`,
	),
	unavailableTitle: message('session.connection.unavailable.title'),
	unavailableTryAgain: message('session.connection.unavailable.tryAgain'),
	workbenchFailedDescription: message('session.connection.workbenchFailed.description'),
	workbenchFailedReload: message('session.connection.workbenchFailed.reload'),
	workbenchFailedTitle: message('session.connection.workbenchFailed.title'),
});

const PATH_REFUSED =
	'A data folder, socket folder or a file in one failed Orchbee’s ownership or permission check. Matches the TUI and CLI refusal word for word. path is the refused item, which is not translated; system, where present, is windows on Windows, whose remedy uses the folder’s Security settings, and other elsewhere. The <command> tag holds a command, which is not translated.';

/**
 * A refused folder or file, keyed by the rule it breaks as `server start` names it, for the page and
 * for Main's startup dialog alike.
 */
export const sessionPathRefusalMessages = defineMessages({
	'access-control-list': message(
		'session.connection.unavailable.pathRefused.accessControlList',
		PATH_REFUSED,
	),
	'locks-unsupported': message(
		'session.connection.unavailable.pathRefused.locksUnsupported',
		PATH_REFUSED,
	),
	'other-owner': message('session.connection.unavailable.pathRefused.otherOwner', PATH_REFUSED),
	'others-can-change': message(
		'session.connection.unavailable.pathRefused.othersCanChange',
		PATH_REFUSED,
	),
	'others-can-read': message(
		'session.connection.unavailable.pathRefused.othersCanRead',
		PATH_REFUSED,
	),
	'permissions-not-kept': message(
		'session.connection.unavailable.pathRefused.permissionsNotKept',
		PATH_REFUSED,
	),
	'symbolic-link': message('session.connection.unavailable.pathRefused.symbolicLink', PATH_REFUSED),
	'unsafe-socket-folder': message(
		'session.connection.unavailable.pathRefused.unsafeSocketFolder',
		PATH_REFUSED,
	),
});
