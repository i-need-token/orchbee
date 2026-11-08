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
	unavailableRecoveryRequired: message(
		'session.connection.unavailable.recoveryRequired',
		'The <command> tag holds a command, which is not translated.',
	),
	unavailableSessionTooOld: message(
		'session.connection.unavailable.sessionTooOld',
		`The running Session is too old for this Desktop; without a version it predates version checks, and only the Orchbee that started it can stop it. Matches the TUI and CLI refusal. The <command> tag holds a command, which is not translated. ${REFUSED_VERSIONS}`,
	),
	unavailableTitle: message('session.connection.unavailable.title'),
	unavailableTryAgain: message('session.connection.unavailable.tryAgain'),
});
