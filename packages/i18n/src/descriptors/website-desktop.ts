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
	stageCaption: message(
		'website.desktop.stage.caption',
		'Caption under the drawn desktop application; <b> marks the word that labels it a demo.',
	),
	panelsKicker: message(
		'website.desktop.panels.kicker',
		'Lowercase label above the section where the visitor arranges Panels.',
	),
	panelsTitle: message(
		'website.desktop.panels.title',
		'Heading of the section where the visitor arranges Panels.',
	),
	panelsLead: message(
		'website.desktop.panels.lead',
		'What the window of Panels beside it lets the visitor do, and what the application adds.',
	),
	highlightsTitle: message('website.desktop.highlights.title', 'Heading of the highlights.'),
	highlightTabsTitle: message(
		'website.desktop.highlight.tabs.title',
		'Title of the highlight "Tabs you can arrange".',
	),
	highlightTabsBody: message(
		'website.desktop.highlight.tabs.body',
		'Explanation of the highlight "Tabs you can arrange".',
	),
	highlightSidebarTitle: message(
		'website.desktop.highlight.sidebar.title',
		'Title of the highlight "A sidebar for every project".',
	),
	highlightSidebarBody: message(
		'website.desktop.highlight.sidebar.body',
		'Explanation of the highlight "A sidebar for every project".',
	),
	highlightSearchTitle: message(
		'website.desktop.highlight.search.title',
		'Title of the highlight "Search for anything".',
	),
	highlightSearchBody: message(
		'website.desktop.highlight.search.body',
		'Explanation of the highlight "Search for anything"; <code> marks a key or a text pattern.',
	),
	highlightFindTitle: message(
		'website.desktop.highlight.find.title',
		'Title of the highlight "Find in the terminal".',
	),
	highlightFindBody: message(
		'website.desktop.highlight.find.body',
		'Explanation of the highlight "Find in the terminal".',
	),
	highlightQuickSelectTitle: message(
		'website.desktop.highlight.quickSelect.title',
		'Title of the highlight "Quick Select".',
	),
	highlightQuickSelectBody: message(
		'website.desktop.highlight.quickSelect.body',
		'Explanation of the highlight "Quick Select"; <code> marks a key or a text pattern.',
	),
	highlightAgentsTitle: message(
		'website.desktop.highlight.agents.title',
		'Title of the highlight "Start an agent where you want it".',
	),
	highlightAgentsBody: message(
		'website.desktop.highlight.agents.body',
		'Explanation of the highlight "Start an agent where you want it".',
	),
	highlightSettingsTitle: message(
		'website.desktop.highlight.settings.title',
		'Title of the highlight "Settings on one page".',
	),
	highlightSettingsBody: message(
		'website.desktop.highlight.settings.body',
		'Explanation of the highlight "Settings on one page".',
	),
	highlightAppearanceTitle: message(
		'website.desktop.highlight.appearance.title',
		'Title of the highlight "Light and dark, chosen separately".',
	),
	highlightAppearanceBody: message(
		'website.desktop.highlight.appearance.body',
		'Explanation of the highlight "Light and dark, chosen separately".',
	),
	highlightInputTitle: message(
		'website.desktop.highlight.input.title',
		'Title of the highlight "Text input as you expect it".',
	),
	highlightInputBody: message(
		'website.desktop.highlight.input.body',
		'Explanation of the highlight "Text input as you expect it".',
	),
	backgroundKicker: message(
		'website.desktop.background.kicker',
		'Lowercase label above the section on running in the background.',
	),
	backgroundTitle: message(
		'website.desktop.background.title',
		'Heading of the section on running in the background.',
	),
	backgroundBody: message(
		'website.desktop.background.body',
		'What closing the window does while terminals run.',
	),
	backgroundQuitAll: message(
		'website.desktop.background.quitAll',
		'Second paragraph of the section on running in the background.',
	),
	trayFigureLabel: message(
		'website.desktop.tray.label',
		'Accessible label of the drawn Windows notification and notification-area menu.',
	),
	trayWhenClosed: message(
		'website.desktop.tray.whenClosed',
		'Label above the drawn notification shown after the window closes.',
	),
	trayWhenMenu: message(
		'website.desktop.tray.whenMenu',
		'Label above the drawn notification-area menu.',
	),
	renderingKicker: message(
		'website.desktop.rendering.kicker',
		'Lowercase label above the section on rendering.',
	),
	renderingTitle: message(
		'website.desktop.rendering.title',
		'Heading of the section on rendering.',
	),
	renderingBody: message(
		'website.desktop.rendering.body',
		'How the desktop application renders its terminals.',
	),
	factCanvas: message(
		'website.desktop.fact.canvas',
		'Rendering fact: the renderer used where WebGPU is unavailable.',
	),
	factColor: message('website.desktop.fact.color', "Rendering fact: the terminal's color depth."),
	factKitty: message(
		'website.desktop.fact.kitty',
		'Rendering fact: the image protocol the terminal supports.',
	),
	factHistory: message(
		'website.desktop.fact.history',
		'Rendering fact: how much output history each terminal keeps.',
	),
	ligatures: message(
		'website.desktop.ligatures',
		'Label before the switch that turns ligatures on and off in the sample.',
	),
	ligaturesOn: message(
		'website.desktop.ligatures.on',
		'Button of the ligature switch that turns ligatures on.',
	),
	ligaturesOff: message(
		'website.desktop.ligatures.off',
		'Button of the ligature switch that turns ligatures off.',
	),
	keysTitle: message('website.desktop.keys.title', 'Heading of the shortcut tables.'),
	keysLead: message(
		'website.desktop.keys.lead',
		'Line under the shortcut heading; <code> marks a key.',
	),
	keysTerminals: message(
		'website.desktop.keys.terminals',
		'Title of the shortcut group "Terminals and Tabs".',
	),
	keysTerminalsNewTerminal: message(
		'website.desktop.keys.terminals.newTerminal',
		'Action "New terminal" in the shortcut group "Terminals and Tabs".',
	),
	keysTerminalsNewWorkspace: message(
		'website.desktop.keys.terminals.newWorkspace',
		'Action "New Workspace" in the shortcut group "Terminals and Tabs".',
	),
	keysTerminalsStartAgent: message(
		'website.desktop.keys.terminals.startAgent',
		'Action "Start an agent" in the shortcut group "Terminals and Tabs".',
	),
	keysTerminalsGoToTab: message(
		'website.desktop.keys.terminals.goToTab',
		'Action "Go to a Tab" in the shortcut group "Terminals and Tabs".',
	),
	keysPanels: message('website.desktop.keys.panels', 'Title of the shortcut group "Panels".'),
	keysPanelsSplitRight: message(
		'website.desktop.keys.panels.splitRight',
		'Action "Split right" in the shortcut group "Panels".',
	),
	keysPanelsSplitDown: message(
		'website.desktop.keys.panels.splitDown',
		'Action "Split down" in the shortcut group "Panels".',
	),
	keysPanelsMaximize: message(
		'website.desktop.keys.panels.maximize',
		'Action "Maximize / Restore" in the shortcut group "Panels".',
	),
	keysPanelsEqualize: message(
		'website.desktop.keys.panels.equalize',
		'Action "Equalize Panels" in the shortcut group "Panels".',
	),
	keysPanelsFocus: message(
		'website.desktop.keys.panels.focus',
		'Action "Move focus" in the shortcut group "Panels".',
	),
	keysPanelsResize: message(
		'website.desktop.keys.panels.resize',
		'Action "Resize" in the shortcut group "Panels".',
	),
	keysSearch: message('website.desktop.keys.search', 'Title of the shortcut group "Search".'),
	keysSearchActions: message(
		'website.desktop.keys.search.actions',
		'Action "Search actions" in the shortcut group "Search".',
	),
	keysSearchEverything: message(
		'website.desktop.keys.search.everything',
		'Action "Search everything" in the shortcut group "Search".',
	),
	keysSearchFind: message(
		'website.desktop.keys.search.find',
		'Action "Find in the terminal" in the shortcut group "Search".',
	),
	keysSearchQuickSelect: message(
		'website.desktop.keys.search.quickSelect',
		'Action "Quick Select" in the shortcut group "Search".',
	),
	keysWindow: message('website.desktop.keys.window', 'Title of the shortcut group "Window".'),
	keysWindowSettings: message(
		'website.desktop.keys.window.settings',
		'Action "Settings" in the shortcut group "Window".',
	),
	keysWindowSidebar: message(
		'website.desktop.keys.window.sidebar',
		'Action "Show or hide the sidebar" in the shortcut group "Window".',
	),
});
