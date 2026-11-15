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
	stageCaption: message(
		'website.terminal.stage.caption',
		'Caption under the drawn Session; <b> marks the word that labels it a demo.',
	),
	panelsKicker: message(
		'website.terminal.panels.kicker',
		'Lowercase label above the section where the visitor splits Panels.',
	),
	panelsTitle: message(
		'website.terminal.panels.title',
		'Heading of the section where the visitor splits Panels.',
	),
	panelsLead: message(
		'website.terminal.panels.lead',
		'How to use the window of Panels beside it; <code> marks a key.',
	),
	highlightsTitle: message('website.terminal.highlights.title', 'Heading of the highlights.'),
	highlightPrefixTitle: message(
		'website.terminal.highlight.prefix.title',
		'Title of the highlight "One prefix, every action".',
	),
	highlightPrefixBody: message(
		'website.terminal.highlight.prefix.body',
		'Explanation of the highlight "One prefix, every action"; <code> marks a key, a command or a file name.',
	),
	highlightMouseTitle: message(
		'website.terminal.highlight.mouse.title',
		'Title of the highlight "The mouse works".',
	),
	highlightMouseBody: message(
		'website.terminal.highlight.mouse.body',
		'Explanation of the highlight "The mouse works"; <code> marks a key, a command or a file name.',
	),
	highlightCopyModeTitle: message(
		'website.terminal.highlight.copyMode.title',
		'Title of the highlight "Copy mode".',
	),
	highlightCopyModeBody: message(
		'website.terminal.highlight.copyMode.body',
		'Explanation of the highlight "Copy mode"; <code> marks a key, a command or a file name.',
	),
	highlightFindTitle: message(
		'website.terminal.highlight.find.title',
		'Title of the highlight "Find in a Panel".',
	),
	highlightFindBody: message(
		'website.terminal.highlight.find.body',
		'Explanation of the highlight "Find in a Panel"; <code> marks a key, a command or a file name.',
	),
	highlightAgentsTitle: message(
		'website.terminal.highlight.agents.title',
		'Title of the highlight "Agents from the keyboard".',
	),
	highlightAgentsBody: message(
		'website.terminal.highlight.agents.body',
		'Explanation of the highlight "Agents from the keyboard"; <code> marks a key, a command or a file name.',
	),
	highlightWorktreesTitle: message(
		'website.terminal.highlight.worktrees.title',
		'Title of the highlight "Git worktrees as Workspaces".',
	),
	highlightWorktreesBody: message(
		'website.terminal.highlight.worktrees.body',
		'Explanation of the highlight "Git worktrees as Workspaces"; <code> marks a key, a command or a file name.',
	),
	highlightSettingsTitle: message(
		'website.terminal.highlight.settings.title',
		'Title of the highlight "Settings inside the terminal".',
	),
	highlightSettingsBody: message(
		'website.terminal.highlight.settings.body',
		'Explanation of the highlight "Settings inside the terminal"; <code> marks a key, a command or a file name.',
	),
	highlightLeaveTitle: message(
		'website.terminal.highlight.leave.title',
		'Title of the highlight "Leave, and come back".',
	),
	highlightLeaveBody: message(
		'website.terminal.highlight.leave.body',
		'Explanation of the highlight "Leave, and come back"; <code> marks a key, a command or a file name.',
	),
	highlightSizeTitle: message(
		'website.terminal.highlight.size.title',
		'Title of the highlight "Any window size".',
	),
	highlightSizeBody: message(
		'website.terminal.highlight.size.body',
		'Explanation of the highlight "Any window size"; <code> marks a key, a command or a file name.',
	),
	keysTitle: message('website.terminal.keys.title', 'Heading of the key reference.'),
	keysLead: message(
		'website.terminal.keys.lead',
		'Paragraph above the key tables; <code> marks the prefix key.',
	),
	keysPanels: message(
		'website.terminal.keys.panels',
		'Lowercase heading of a group in the key reference.',
	),
	keysPanelsSplitRight: message(
		'website.terminal.keys.panels.splitRight',
		'An action in the key reference, beside its key.',
	),
	keysPanelsSplitDown: message(
		'website.terminal.keys.panels.splitDown',
		'An action in the key reference, beside its key.',
	),
	keysPanelsSplitLeft: message(
		'website.terminal.keys.panels.splitLeft',
		'An action in the key reference, beside its key.',
	),
	keysPanelsSplitUp: message(
		'website.terminal.keys.panels.splitUp',
		'An action in the key reference, beside its key.',
	),
	keysPanelsFocus: message(
		'website.terminal.keys.panels.focus',
		'An action in the key reference, beside its key.',
	),
	keysPanelsResize: message(
		'website.terminal.keys.panels.resize',
		'An action in the key reference, beside its key.',
	),
	keysPanelsMaximize: message(
		'website.terminal.keys.panels.maximize',
		'An action in the key reference, beside its key.',
	),
	keysPanelsMove: message(
		'website.terminal.keys.panels.move',
		'An action in the key reference, beside its key.',
	),
	keysPanelsLast: message(
		'website.terminal.keys.panels.last',
		'An action in the key reference, beside its key.',
	),
	keysPanelsClose: message(
		'website.terminal.keys.panels.close',
		'An action in the key reference, beside its key.',
	),
	keysTabs: message(
		'website.terminal.keys.tabs',
		'Lowercase heading of a group in the key reference.',
	),
	keysTabsNewTab: message(
		'website.terminal.keys.tabs.newTab',
		'An action in the key reference, beside its key.',
	),
	keysTabsNextTab: message(
		'website.terminal.keys.tabs.nextTab',
		'An action in the key reference, beside its key.',
	),
	keysTabsTabNumber: message(
		'website.terminal.keys.tabs.tabNumber',
		'An action in the key reference, beside its key.',
	),
	keysTabsMoveTab: message(
		'website.terminal.keys.tabs.moveTab',
		'An action in the key reference, beside its key.',
	),
	keysTabsRenameTab: message(
		'website.terminal.keys.tabs.renameTab',
		'An action in the key reference, beside its key.',
	),
	keysTabsCloseTab: message(
		'website.terminal.keys.tabs.closeTab',
		'An action in the key reference, beside its key.',
	),
	keysTabsNewWorkspace: message(
		'website.terminal.keys.tabs.newWorkspace',
		'An action in the key reference, beside its key.',
	),
	keysTabsRenameWorkspace: message(
		'website.terminal.keys.tabs.renameWorkspace',
		'An action in the key reference, beside its key.',
	),
	keysTabsCloseWorkspace: message(
		'website.terminal.keys.tabs.closeWorkspace',
		'An action in the key reference, beside its key.',
	),
	keysTabsNavigator: message(
		'website.terminal.keys.tabs.navigator',
		'An action in the key reference, beside its key.',
	),
	keysAgents: message(
		'website.terminal.keys.agents',
		'Lowercase heading of a group in the key reference.',
	),
	keysAgentsStartAgent: message(
		'website.terminal.keys.agents.startAgent',
		'An action in the key reference, beside its key.',
	),
	keysAgentsNextAgent: message(
		'website.terminal.keys.agents.nextAgent',
		'An action in the key reference, beside its key.',
	),
	keysAgentsNotification: message(
		'website.terminal.keys.agents.notification',
		'An action in the key reference, beside its key.',
	),
	keysAgentsCreateWorktree: message(
		'website.terminal.keys.agents.createWorktree',
		'An action in the key reference, beside its key.',
	),
	keysAgentsOpenWorktree: message(
		'website.terminal.keys.agents.openWorktree',
		'An action in the key reference, beside its key.',
	),
	keysInterface: message(
		'website.terminal.keys.interface',
		'Lowercase heading of a group in the key reference.',
	),
	keysInterfaceActionMenu: message(
		'website.terminal.keys.interface.actionMenu',
		'An action in the key reference, beside its key.',
	),
	keysInterfaceSettings: message(
		'website.terminal.keys.interface.settings',
		'An action in the key reference, beside its key.',
	),
	keysInterfaceSidebar: message(
		'website.terminal.keys.interface.sidebar',
		'An action in the key reference, beside its key.',
	),
	keysInterfaceFocusSidebar: message(
		'website.terminal.keys.interface.focusSidebar',
		'An action in the key reference, beside its key.',
	),
	keysInterfaceFind: message(
		'website.terminal.keys.interface.find',
		'An action in the key reference, beside its key.',
	),
	keysInterfaceCopyMode: message(
		'website.terminal.keys.interface.copyMode',
		'An action in the key reference, beside its key.',
	),
	keysInterfaceHistory: message(
		'website.terminal.keys.interface.history',
		'An action in the key reference, beside its key.',
	),
	keysInterfaceMouse: message(
		'website.terminal.keys.interface.mouse',
		'An action in the key reference, beside its key.',
	),
	keysInterfaceSendPrefix: message(
		'website.terminal.keys.interface.sendPrefix',
		'An action in the key reference, beside its key.',
	),
	keysInterfaceLeave: message(
		'website.terminal.keys.interface.leave',
		'An action in the key reference, beside its key.',
	),
});
