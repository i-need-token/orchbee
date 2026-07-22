import { defineMessages } from 'react-intl';

import { enUSWorkbenchMessages } from '../locales';

export const workbenchMessages = defineMessages({
	hideSidebar: {
		id: 'workbench.sidebar.actions.hide',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.actions.hide'],
		description: 'Action that hides the Workbench sidebar.',
	},
	newTerminal: {
		id: 'workbench.sidebar.items.newTerminal',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.newTerminal'],
		description: 'Label for the new-terminal entry in the Workbench sidebar.',
	},
	project: {
		id: 'workbench.sidebar.sections.project',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.sections.project'],
		description: 'Heading for the project section in the Workbench sidebar.',
	},
	search: {
		id: 'workbench.sidebar.items.search',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.search'],
		description: 'Label for the search entry in the Workbench sidebar.',
	},
	settings: {
		id: 'workbench.sidebar.items.settings',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.settings'],
		description: 'Label for the settings entry in the Workbench sidebar footer.',
	},
	showSidebar: {
		id: 'workbench.sidebar.actions.show',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.actions.show'],
		description: 'Action that shows the Workbench sidebar.',
	},
	sidebarLabel: {
		id: 'workbench.sidebar.label',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.label'],
		description: 'Accessible label for the Workbench navigation sidebar.',
	},
});
