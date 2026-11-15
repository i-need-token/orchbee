import { defineMessages } from 'react-intl';

import { enUSWebsiteDesktopMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSWebsiteDesktopMessages);

export const websiteDesktopMessages = defineMessages({
	metaTitle: message(
		'website.desktop.metaTitle',
		'Document title of the desktop application page.',
	),
	metaDescription: message(
		'website.desktop.metaDescription',
		'Search and link-preview description of the desktop application page.',
	),
	lead: message('website.desktop.lead', 'Paragraph beside the page headline.'),
});
