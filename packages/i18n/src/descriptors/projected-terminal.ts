import { defineMessages } from 'react-intl';

export const projectedTerminalMessages = defineMessages({
	actionCategory: {
		defaultMessage: 'Terminal',
		id: 'terminal.projected.action.category',
	},
	actionUnavailable: {
		defaultMessage: 'This terminal action is unavailable.',
		id: 'terminal.projected.action.unavailable',
	},
	accessibilityDeleted: {
		defaultMessage: 'Deleted: {text}',
		id: 'terminal.projected.accessibility.deleted',
	},
	accessibilityRow: {
		defaultMessage: 'Terminal row {row}: {text}',
		id: 'terminal.projected.accessibility.row',
	},
	accessibilityScreenCursor: {
		defaultMessage: 'Terminal screen, cursor at row {row}, column {column}',
		id: 'terminal.projected.accessibility.screenCursor',
	},
	accessibilitySelection: {
		defaultMessage: 'Selected text: {text}',
		id: 'terminal.projected.accessibility.selection',
	},
	closeFind: {
		defaultMessage: 'Close find',
		id: 'terminal.projected.find.close',
	},
	connectionFailure: {
		defaultMessage: 'The terminal connection failed.',
		id: 'terminal.projected.connectionFailure',
	},
	connectionRecovering: {
		defaultMessage: 'Restoring the terminal connection…',
		id: 'terminal.projected.connectionRecovering',
	},
	connectionRecoveryFailed: {
		defaultMessage: 'The terminal connection could not be restored.',
		id: 'terminal.projected.connectionRecoveryFailed',
	},
	runtimeFailure: {
		defaultMessage: 'The terminal session stopped after a runtime failure.',
		id: 'terminal.projected.runtimeFailure',
	},
	copyAction: {
		defaultMessage: 'Copy selection',
		id: 'terminal.projected.action.copy',
	},
	copyCompleted: {
		defaultMessage: 'Copied {byteLength, number} bytes.',
		id: 'terminal.projected.copy.completed',
	},
	copyFailed: {
		defaultMessage: 'The terminal selection could not be copied.',
		id: 'terminal.projected.copy.failed',
	},
	findIdle: {
		defaultMessage: 'Search retained terminal output.',
		id: 'terminal.projected.find.idle',
	},
	findInvalidRegex: {
		defaultMessage: 'Enter a valid regular expression.',
		id: 'terminal.projected.find.invalidRegex',
	},
	findLabel: {
		defaultMessage: 'Find in terminal',
		id: 'terminal.projected.find.label',
	},
	findNextAction: {
		defaultMessage: 'Find next match',
		id: 'terminal.projected.action.findNext',
	},
	findNoResults: {
		defaultMessage: 'No matches in retained terminal output.',
		id: 'terminal.projected.find.noResults',
	},
	findOpenAction: {
		defaultMessage: 'Find in terminal',
		id: 'terminal.projected.action.findOpen',
	},
	findPlaceholder: {
		defaultMessage: 'Find',
		id: 'terminal.projected.find.placeholder',
	},
	findPreviousAction: {
		defaultMessage: 'Find previous match',
		id: 'terminal.projected.action.findPrevious',
	},
	findResult: {
		defaultMessage: '{current, number} of {total, number} matches',
		id: 'terminal.projected.find.result',
	},
	findResultLimit: {
		defaultMessage: 'Showing the first {total, number} matches.',
		id: 'terminal.projected.find.resultLimit',
	},
	findSearching: {
		defaultMessage: 'Searching retained terminal output…',
		id: 'terminal.projected.find.searching',
	},
	findTimeout: {
		defaultMessage: 'Search stopped after reaching the time limit.',
		id: 'terminal.projected.find.timeout',
	},
	findUnavailable: {
		defaultMessage: 'Retained terminal output could not be searched.',
		id: 'terminal.projected.find.unavailable',
	},
	historyEvicted: {
		defaultMessage: 'Earlier terminal output was evicted from retained history.',
		id: 'terminal.projected.history.evicted',
	},
	historyRange: {
		defaultMessage: 'Showing terminal rows {start} through {end} of {total}.',
		id: 'terminal.projected.history.range',
	},
	historyUnavailable: {
		defaultMessage: 'Retained terminal history is unavailable for this connection.',
		id: 'terminal.projected.history.unavailable',
	},
	input: {
		defaultMessage: 'Terminal input',
		id: 'terminal.projected.input',
	},
	linkBlocked: {
		defaultMessage: 'This terminal link is blocked by the desktop link policy.',
		id: 'terminal.projected.link.blocked',
	},
	linkFailed: {
		defaultMessage: 'The terminal link could not be opened.',
		id: 'terminal.projected.link.failed',
	},
	matchCase: {
		defaultMessage: 'Match case',
		id: 'terminal.projected.find.matchCase',
	},
	nextMatch: {
		defaultMessage: 'Next match',
		id: 'terminal.projected.find.next',
	},
	openLinkAction: {
		defaultMessage: 'Open terminal link',
		id: 'terminal.projected.action.openLink',
	},
	pasteReviewCancel: {
		defaultMessage: 'Cancel',
		id: 'terminal.projected.pasteReview.cancel',
	},
	pasteReviewConfirm: {
		defaultMessage: 'Paste',
		id: 'terminal.projected.pasteReview.confirm',
	},
	pasteReviewControls: {
		defaultMessage:
			'{count, plural, one {# control character will} other {# control characters will}} be replaced with spaces.',
		id: 'terminal.projected.pasteReview.controls',
	},
	pasteReviewDescription: {
		defaultMessage: 'Review this text before sending it to the terminal.',
		id: 'terminal.projected.pasteReview.description',
	},
	pasteReviewMultiline: {
		defaultMessage: 'This paste contains multiple lines and may run multiple commands.',
		id: 'terminal.projected.pasteReview.multiline',
	},
	pasteReviewOriginal: {
		defaultMessage: 'Original paste text',
		id: 'terminal.projected.pasteReview.original',
	},
	pasteReviewTerminator: {
		defaultMessage: 'This paste contains a bracketed-paste terminator.',
		id: 'terminal.projected.pasteReview.terminator',
	},
	pasteReviewTitle: {
		defaultMessage: 'Review terminal paste',
		id: 'terminal.projected.pasteReview.title',
	},
	pasteReviewTruncated: {
		defaultMessage: 'Preview truncated.',
		id: 'terminal.projected.pasteReview.truncated',
	},
	previousMatch: {
		defaultMessage: 'Previous match',
		id: 'terminal.projected.find.previous',
	},
	resetAction: {
		defaultMessage: 'Reset terminal',
		id: 'terminal.projected.action.reset',
	},
	retryConnection: {
		defaultMessage: 'Retry',
		id: 'terminal.projected.connectionRetry',
	},
	screen: {
		defaultMessage: 'Terminal screen',
		id: 'terminal.projected.screen',
	},
	selectAllAction: {
		defaultMessage: 'Select all retained terminal output',
		id: 'terminal.projected.action.selectAll',
	},
	selectionEvicted: {
		defaultMessage: 'The terminal selection was evicted from retained history.',
		id: 'terminal.projected.selection.evicted',
	},
	sessionStopped: {
		defaultMessage: 'Terminal session stopped',
		id: 'terminal.projected.session.stopped',
	},
	sessionStartFailed: {
		defaultMessage: 'The terminal session could not be started.',
		id: 'terminal.projected.session.startFailed',
	},
	startNewSession: {
		defaultMessage: 'Start New Session',
		id: 'terminal.projected.session.startNew',
	},
	useRegex: {
		defaultMessage: 'Use regular expression',
		id: 'terminal.projected.find.useRegex',
	},
	wholeWord: {
		defaultMessage: 'Match whole word',
		id: 'terminal.projected.find.wholeWord',
	},
});
