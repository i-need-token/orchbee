import { defineMessages } from 'react-intl';

import { enUSSessionConnectionMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSSessionConnectionMessages);

export const sessionConnectionMessages = defineMessages({
	disconnected: message('session.connection.disconnected'),
	reconnect: message('session.connection.reconnect'),
	reconnectDescription: message('session.connection.reconnectDescription'),
	unavailableDescription: message('session.connection.unavailable.description'),
	unavailableRecoveryRequired: message('session.connection.unavailable.recoveryRequired'),
	unavailableTitle: message('session.connection.unavailable.title'),
	unavailableTryAgain: message('session.connection.unavailable.tryAgain'),
});
