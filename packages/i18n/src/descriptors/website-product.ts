import { defineMessages } from 'react-intl';

import { enUSWebsiteProductMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSWebsiteProductMessages);

export const websiteProductMessages = defineMessages({
	terminalInterface: message(
		'website.product.terminalInterface',
		'Accessible name of a drawn picture of the Orchbee terminal interface.',
	),
	commandLine: message(
		'website.product.commandLine',
		'Accessible name of a drawn picture of a plain terminal running orchbee commands.',
	),
	summaryAttention: message(
		'website.product.summary.attention',
		'Part of the status line of a drawn Session: how many agents need attention.',
	),
	summaryWorking: message(
		'website.product.summary.working',
		'Part of the status line of a drawn Session: how many agents are working.',
	),
	summaryDone: message(
		'website.product.summary.done',
		'Part of the status line of a drawn Session: how many agents are done.',
	),
	summaryReady: message(
		'website.product.summary.ready',
		'Part of the status line of a drawn Session: how many agents are ready.',
	),
});
