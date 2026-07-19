import { defineMessages } from 'react-intl';

import { enUSWorkbenchMessages } from '../locales';

export const workbenchMessages = defineMessages({
	addTab: {
		id: 'workbench.tabs.actions.add',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.add'],
		description: 'Action that creates a new Workbench tab.',
	},
	closeTab: {
		id: 'workbench.tabs.actions.close',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.close'],
		description: 'Action that closes a named Workbench tab.',
	},
	hideSidebar: {
		id: 'workbench.sidebar.actions.hide',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.actions.hide'],
		description: 'Action that hides the Workbench sidebar.',
	},
	showSidebar: {
		id: 'workbench.sidebar.actions.show',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.actions.show'],
		description: 'Action that shows the Workbench sidebar.',
	},
	tabsLabel: {
		id: 'workbench.tabs.label',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.label'],
		description: 'Accessible label for the Workbench tab list.',
	},
	untitledTab: {
		id: 'workbench.tabs.untitled',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.untitled'],
		description: 'Default label for a newly created Workbench tab.',
	},
});
