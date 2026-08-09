import { defineMessages } from 'react-intl';

import { enUSApplicationRoutingMessages } from '../locales';

export const applicationRoutingMessages = defineMessages({
	unavailableDescription: {
		defaultMessage: enUSApplicationRoutingMessages['application.routing.unavailable.description'],
		id: 'application.routing.unavailable.description',
	},
	unavailableTitle: {
		defaultMessage: enUSApplicationRoutingMessages['application.routing.unavailable.title'],
		id: 'application.routing.unavailable.title',
	},
});
