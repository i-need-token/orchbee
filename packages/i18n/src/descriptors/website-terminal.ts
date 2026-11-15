import { defineMessages } from 'react-intl';

import { enUSWebsiteTerminalMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSWebsiteTerminalMessages);

export const websiteTerminalMessages = defineMessages({
	metaTitle: message(
		'website.terminal.metaTitle',
		'Document title of the terminal interface page.',
	),
	metaDescription: message(
		'website.terminal.metaDescription',
		'Search and link-preview description of the terminal interface page.',
	),
	lead: message('website.terminal.lead', 'Paragraph beside the page headline.'),
	note: message('website.terminal.note', 'Line under the npm command; <code> marks the command.'),
});
