import { defineMessages } from 'react-intl';

export const desktopTerminalMessages = defineMessages({
	actionCategory: {
		defaultMessage: 'Terminal',
		id: 'desktop.terminal.action.category',
	},
	actionUnavailable: {
		defaultMessage: 'This terminal action is unavailable.',
		id: 'desktop.terminal.action.unavailable',
	},
	closeFind: {
		defaultMessage: 'Close find',
		id: 'desktop.terminal.find.close',
	},
	connectionFailure: {
		defaultMessage: 'The terminal connection failed.',
		id: 'desktop.terminal.connectionFailure',
	},
	copyAction: {
		defaultMessage: 'Copy selection',
		id: 'desktop.terminal.action.copy',
	},
	copyCompleted: {
		defaultMessage: 'Copied {byteLength, number} bytes.',
		id: 'desktop.terminal.copy.completed',
	},
	copyFailed: {
		defaultMessage: 'The terminal selection could not be copied.',
		id: 'desktop.terminal.copy.failed',
	},
	findIdle: {
		defaultMessage: 'Search retained terminal output.',
		id: 'desktop.terminal.find.idle',
	},
	findInvalidRegex: {
		defaultMessage: 'Enter a valid regular expression.',
		id: 'desktop.terminal.find.invalidRegex',
	},
	findLabel: {
		defaultMessage: 'Find in terminal',
		id: 'desktop.terminal.find.label',
	},
	findNextAction: {
		defaultMessage: 'Find next match',
		id: 'desktop.terminal.action.findNext',
	},
	findNoResults: {
		defaultMessage: 'No matches in retained terminal output.',
		id: 'desktop.terminal.find.noResults',
	},
	findOpenAction: {
		defaultMessage: 'Find in terminal',
		id: 'desktop.terminal.action.findOpen',
	},
	findPlaceholder: {
		defaultMessage: 'Find',
		id: 'desktop.terminal.find.placeholder',
	},
	findPreviousAction: {
		defaultMessage: 'Find previous match',
		id: 'desktop.terminal.action.findPrevious',
	},
	findResult: {
		defaultMessage: '{current, number} of {total, number} matches',
		id: 'desktop.terminal.find.result',
	},
	findResultLimit: {
		defaultMessage: 'Showing the first {total, number} matches.',
		id: 'desktop.terminal.find.resultLimit',
	},
	findSearching: {
		defaultMessage: 'Searching retained terminal output…',
		id: 'desktop.terminal.find.searching',
	},
	findTimeout: {
		defaultMessage: 'Search stopped after reaching the time limit.',
		id: 'desktop.terminal.find.timeout',
	},
	findUnavailable: {
		defaultMessage: 'Retained terminal output could not be searched.',
		id: 'desktop.terminal.find.unavailable',
	},
	historyEvicted: {
		defaultMessage: 'Earlier terminal output was evicted from retained history.',
		id: 'desktop.terminal.history.evicted',
	},
	historyRange: {
		defaultMessage: 'Showing terminal rows {start} through {end} of {total}.',
		id: 'desktop.terminal.history.range',
	},
	historyUnavailable: {
		defaultMessage: 'Retained terminal history is unavailable for this connection.',
		id: 'desktop.terminal.history.unavailable',
	},
	input: {
		defaultMessage: 'Terminal input',
		id: 'desktop.terminal.input',
	},
	linkBlocked: {
		defaultMessage: 'This terminal link is blocked by the desktop link policy.',
		id: 'desktop.terminal.link.blocked',
	},
	linkFailed: {
		defaultMessage: 'The terminal link could not be opened.',
		id: 'desktop.terminal.link.failed',
	},
	matchCase: {
		defaultMessage: 'Match case',
		id: 'desktop.terminal.find.matchCase',
	},
	nextMatch: {
		defaultMessage: 'Next match',
		id: 'desktop.terminal.find.next',
	},
	openLinkAction: {
		defaultMessage: 'Open terminal link',
		id: 'desktop.terminal.action.openLink',
	},
	previousMatch: {
		defaultMessage: 'Previous match',
		id: 'desktop.terminal.find.previous',
	},
	screen: {
		defaultMessage: 'Terminal screen',
		id: 'desktop.terminal.screen',
	},
	selectAllAction: {
		defaultMessage: 'Select all retained terminal output',
		id: 'desktop.terminal.action.selectAll',
	},
	selectionEvicted: {
		defaultMessage: 'The terminal selection was evicted from retained history.',
		id: 'desktop.terminal.selection.evicted',
	},
	useRegex: {
		defaultMessage: 'Use regular expression',
		id: 'desktop.terminal.find.useRegex',
	},
	wholeWord: {
		defaultMessage: 'Match whole word',
		id: 'desktop.terminal.find.wholeWord',
	},
});
