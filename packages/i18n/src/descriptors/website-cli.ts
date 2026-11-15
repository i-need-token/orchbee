import { defineMessages } from 'react-intl';

import { enUSWebsiteCliMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSWebsiteCliMessages);

export const websiteCliMessages = defineMessages({
	metaTitle: message('website.cli.metaTitle', 'Document title of the command line page.'),
	metaDescription: message(
		'website.cli.metaDescription',
		'Search and link-preview description of the command line page.',
	),
	lead: message(
		'website.cli.lead',
		'Paragraph beside the page headline; <code> marks the command name.',
	),
});
