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
	controlFailure: {
		defaultMessage: 'The terminal control action could not be confirmed.',
		id: 'terminal.projected.failure.control',
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
	outputCopyAction: {
		defaultMessage: 'Copy command output',
		id: 'terminal.projected.action.outputCopy',
	},
	outputCopyCompleted: {
		defaultMessage: 'Command output copied.',
		id: 'terminal.projected.output.copyCompleted',
	},
	outputCopyFailed: {
		defaultMessage: 'The command output could not be copied.',
		id: 'terminal.projected.output.copyFailed',
	},
	outputShowLastAction: {
		defaultMessage: 'Show last command output',
		id: 'terminal.projected.action.outputShowLast',
	},
	outputViewerAlternateScreen: {
		defaultMessage: 'Command output is unavailable on the alternate screen.',
		id: 'terminal.projected.output.alternateScreen',
	},
	outputViewerClose: {
		defaultMessage: 'Close command output',
		id: 'terminal.projected.output.close',
	},
	outputViewerContent: {
		defaultMessage: 'Last command output',
		id: 'terminal.projected.output.content',
	},
	outputViewerNoMatches: {
		defaultMessage: 'No matches in this command output.',
		id: 'terminal.projected.output.noMatches',
	},
	outputViewerNoOutput: {
		defaultMessage: 'The last command produced no output.',
		id: 'terminal.projected.output.noOutput',
	},
	outputViewerNoShellIntegration: {
		defaultMessage: 'No shell command markers are available for this terminal.',
		id: 'terminal.projected.output.noShellIntegration',
	},
	outputViewerSearch: {
		defaultMessage: 'Find in command output',
		id: 'terminal.projected.output.search',
	},
	outputViewerSourceLimit: {
		defaultMessage: 'The command output is larger than the viewer limit.',
		id: 'terminal.projected.output.sourceLimit',
	},
	outputViewerStale: {
		defaultMessage: 'Terminal output changed before the command output could be prepared.',
		id: 'terminal.projected.output.stale',
	},
	outputViewerTitle: {
		defaultMessage: 'Last command output',
		id: 'terminal.projected.output.title',
	},
	outputViewerTruncated: {
		defaultMessage: 'Earlier lines were evicted from retained history.',
		id: 'terminal.projected.output.truncated',
	},
	outputViewerUnavailable: {
		defaultMessage: 'Command output is temporarily unavailable.',
		id: 'terminal.projected.output.unavailable',
	},
	scrollbackCopyAction: {
		defaultMessage: 'Copy scrollback',
		id: 'terminal.projected.action.scrollbackCopy',
	},
	scrollbackCopyCompleted: {
		defaultMessage: 'Scrollback copied.',
		id: 'terminal.projected.scrollback.copyCompleted',
	},
	scrollbackCopyFailed: {
		defaultMessage: 'The scrollback could not be copied.',
		id: 'terminal.projected.scrollback.copyFailed',
	},
	scrollbackShowAction: {
		defaultMessage: 'Show scrollback',
		id: 'terminal.projected.action.scrollbackShow',
	},
	scrollbackViewerAlternateScreen: {
		defaultMessage: 'Scrollback is unavailable on the alternate screen.',
		id: 'terminal.projected.scrollback.alternateScreen',
	},
	scrollbackViewerClose: {
		defaultMessage: 'Close scrollback',
		id: 'terminal.projected.scrollback.close',
	},
	scrollbackViewerContent: {
		defaultMessage: 'Retained terminal scrollback',
		id: 'terminal.projected.scrollback.content',
	},
	scrollbackViewerNoMatches: {
		defaultMessage: 'No matches in this scrollback.',
		id: 'terminal.projected.scrollback.noMatches',
	},
	scrollbackViewerNoScrollback: {
		defaultMessage: 'No terminal scrollback is currently retained.',
		id: 'terminal.projected.scrollback.noScrollback',
	},
	scrollbackViewerSearch: {
		defaultMessage: 'Find in scrollback',
		id: 'terminal.projected.scrollback.search',
	},
	scrollbackViewerSourceLimit: {
		defaultMessage: 'The retained scrollback is larger than the viewer limit.',
		id: 'terminal.projected.scrollback.sourceLimit',
	},
	scrollbackViewerStale: {
		defaultMessage: 'Terminal output changed before the scrollback could be prepared.',
		id: 'terminal.projected.scrollback.stale',
	},
	scrollbackViewerTitle: {
		defaultMessage: 'Scrollback',
		id: 'terminal.projected.scrollback.title',
	},
	scrollbackViewerUnavailable: {
		defaultMessage: 'Scrollback is temporarily unavailable.',
		id: 'terminal.projected.scrollback.unavailable',
	},
	clearAction: {
		defaultMessage: 'Clear terminal',
		id: 'terminal.projected.action.clear',
	},
	clearCompleted: {
		defaultMessage: 'Terminal output cleared.',
		id: 'terminal.projected.clear.completed',
	},
	clipboardWriteFailed: {
		defaultMessage: 'The terminal text could not be written to the clipboard.',
		id: 'terminal.projected.clipboard.writeFailed',
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
	promptNextAction: {
		defaultMessage: 'Scroll to next shell prompt',
		id: 'terminal.projected.action.promptNext',
	},
	promptPreviousAction: {
		defaultMessage: 'Scroll to previous shell prompt',
		id: 'terminal.projected.action.promptPrevious',
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
	hintsActive: {
		defaultMessage:
			'{count, plural, one {One terminal hint is active.} other {# terminal hints are active.}} Type a label or press Escape to cancel.',
		id: 'terminal.projected.hints.active',
	},
	hintsCompleted: {
		defaultMessage: 'Terminal hint selected.',
		id: 'terminal.projected.hints.completed',
	},
	hintsFailed: {
		defaultMessage: 'The selected terminal hint could not be used.',
		id: 'terminal.projected.hints.failed',
	},
	hintsLoading: {
		defaultMessage: 'Finding terminal hints…',
		id: 'terminal.projected.hints.loading',
	},
	hintsNoMatches: {
		defaultMessage: 'No supported targets are visible in the terminal.',
		id: 'terminal.projected.hints.noMatches',
	},
	hintsOpenAction: {
		defaultMessage: 'Show terminal hints',
		id: 'terminal.projected.action.hintsOpen',
	},
	hintsStale: {
		defaultMessage: 'Terminal output changed before the hint could be selected.',
		id: 'terminal.projected.hints.stale',
	},
	hintsUnavailable: {
		defaultMessage: 'Terminal hints are unavailable for the visible output.',
		id: 'terminal.projected.hints.unavailable',
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
	inputFailure: {
		defaultMessage: 'The terminal input request could not be confirmed.',
		id: 'terminal.projected.failure.input',
	},
	input: {
		defaultMessage: 'Terminal input',
		id: 'terminal.projected.input',
	},
	progressLabel: {
		defaultMessage: 'Terminal task progress',
		id: 'terminal.projected.progress.label',
	},
	commandCompletedTitle: {
		defaultMessage: 'Terminal command completed',
		id: 'terminal.projected.commandCompleted.title',
	},
	commandCompletedBody: {
		defaultMessage:
			'Command completed in {duration, number} seconds with exit code {exitCode, number}.',
		id: 'terminal.projected.commandCompleted.body',
	},
	fileDropFailureInvalid: {
		defaultMessage: 'The dropped paths could not be prepared safely.',
		id: 'terminal.projected.fileDrop.failure.invalid',
	},
	fileDropFailureInputTooLarge: {
		defaultMessage: 'The generated terminal input exceeds the {maximum, number}-character limit.',
		id: 'terminal.projected.fileDrop.failure.inputTooLarge',
	},
	fileDropFailureTerminalUnavailable: {
		defaultMessage: 'The active terminal is not ready to receive dropped paths.',
		id: 'terminal.projected.fileDrop.failure.terminalUnavailable',
	},
	fileDropFailureTooMany: {
		defaultMessage:
			'{count, plural, one {One file was} other {# files were}} dropped. Select no more than {maximum, number}.',
		id: 'terminal.projected.fileDrop.failure.tooMany',
	},
	fileDropFailureUnsupported: {
		defaultMessage: 'File drop is unavailable in this client.',
		id: 'terminal.projected.fileDrop.failure.unsupported',
	},
	fileDropReviewConfirm: {
		defaultMessage: 'Paste paths',
		id: 'terminal.projected.fileDrop.review.confirm',
	},
	fileDropReviewDescription: {
		defaultMessage: 'Review the exact shell input generated from the dropped local paths.',
		id: 'terminal.projected.fileDrop.review.description',
	},
	fileDropReviewInput: {
		defaultMessage: 'Generated terminal input',
		id: 'terminal.projected.fileDrop.review.input',
	},
	fileDropReviewNewline: {
		defaultMessage:
			'At least one local path contains a line break. Verify the exact input carefully.',
		id: 'terminal.projected.fileDrop.review.newline',
	},
	fileDropReviewRemote: {
		defaultMessage:
			'These are paths on this device. They may not exist in SSH, containers, or other remote shells.',
		id: 'terminal.projected.fileDrop.review.remote',
	},
	fileDropReviewTitle: {
		defaultMessage: 'Review dropped paths',
		id: 'terminal.projected.fileDrop.review.title',
	},
	linkBlocked: {
		defaultMessage: 'This terminal link is blocked by the desktop link policy.',
		id: 'terminal.projected.link.blocked',
	},
	linkFailed: {
		defaultMessage: 'The terminal link could not be opened.',
		id: 'terminal.projected.link.failed',
	},
	linkStale: {
		defaultMessage: 'The terminal link changed before it could be opened.',
		id: 'terminal.projected.link.stale',
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
	openLinkCompleted: {
		defaultMessage: 'Terminal link opened.',
		id: 'terminal.projected.link.completed',
	},
	pasteAction: {
		defaultMessage: 'Paste',
		id: 'terminal.projected.action.paste',
	},
	pasteCompleted: {
		defaultMessage: 'Clipboard text pasted into the terminal.',
		id: 'terminal.projected.paste.completed',
	},
	pasteFailed: {
		defaultMessage: 'Clipboard text could not be pasted into the terminal.',
		id: 'terminal.projected.paste.failed',
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
	requestFailure: {
		defaultMessage: 'The terminal action could not be completed.',
		id: 'terminal.projected.failure.request',
	},
	resizeFailure: {
		defaultMessage: 'The terminal size could not be synchronized.',
		id: 'terminal.projected.failure.resize',
	},
	resetAction: {
		defaultMessage: 'Reset terminal',
		id: 'terminal.projected.action.reset',
	},
	resetCompleted: {
		defaultMessage: 'Terminal reset.',
		id: 'terminal.projected.reset.completed',
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
	selectAllCompleted: {
		defaultMessage: 'Selected all retained terminal output.',
		id: 'terminal.projected.selectAll.completed',
	},
	scrollLineDownAction: {
		defaultMessage: 'Scroll down one line',
		id: 'terminal.projected.action.scrollLineDown',
	},
	scrollLineUpAction: {
		defaultMessage: 'Scroll up one line',
		id: 'terminal.projected.action.scrollLineUp',
	},
	scrollPageDownAction: {
		defaultMessage: 'Scroll down one page',
		id: 'terminal.projected.action.scrollPageDown',
	},
	scrollPageUpAction: {
		defaultMessage: 'Scroll up one page',
		id: 'terminal.projected.action.scrollPageUp',
	},
	scrollToBottomAction: {
		defaultMessage: 'Scroll to bottom',
		id: 'terminal.projected.action.scrollToBottom',
	},
	scrollToTopAction: {
		defaultMessage: 'Scroll to top',
		id: 'terminal.projected.action.scrollToTop',
	},
	scrollToBottomCompleted: {
		defaultMessage: 'Scrolled to the latest terminal output.',
		id: 'terminal.projected.scrollToBottom.completed',
	},
	scrollToBottomUnchanged: {
		defaultMessage: 'Already showing the latest terminal output.',
		id: 'terminal.projected.scrollToBottom.unchanged',
	},
	selectionFailure: {
		defaultMessage: 'The terminal selection could not be updated.',
		id: 'terminal.projected.failure.selection',
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
	retryableFailure: {
		defaultMessage: '{consequence} Try the action again.',
		id: 'terminal.projected.failure.retryable',
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
