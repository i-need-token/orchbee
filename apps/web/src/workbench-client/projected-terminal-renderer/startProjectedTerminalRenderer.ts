import {
	createOrchbeeIntl,
	projectedTerminalMessages,
	getOrchbeeTextDirection,
	resolveOrchbeeLocale,
} from '@orchbee/i18n';
import { ProjectedTerminal, type ProjectedTerminalOptions } from '@orchbee/terminal';
import {
	type ProjectedAttachmentBootstrap,
	ProjectedTerminalEventSchema,
} from '@orchbee/workbench-client-protocol';

import {
	TerminalFindController,
	mountTerminalFindSurface,
	type TerminalFindSurfaceMount,
	type MountTerminalFindSurfaceOptions,
} from '../terminal-find-surface';
import {
	createProjectedTerminalActionComposition,
	PROJECTED_TERMINAL_ACTION_IDS,
	installProjectedTerminalShortcuts,
	type ProjectedTerminalShortcutPlatform,
} from '../projected-terminal-actions';
import type { ProjectedTerminalPlatformEffects } from '../projected-terminal-platform-effects';
import type { ProjectedTerminalTransport } from '../terminal-projection';

type ProjectedTerminalDriver = Pick<
	ProjectedTerminal,
	| 'applyFrame'
	| 'applyRows'
	| 'clearSearch'
	| 'copySelectionAsync'
	| 'dispose'
	| 'findNext'
	| 'findPrevious'
	| 'focus'
	| 'getBufferRowsSnapshot'
	| 'notifyExited'
	| 'notifyBrokerFailure'
	| 'notifyHistoryChanged'
	| 'notifyLinkOpened'
	| 'notifySelectionChanged'
	| 'notifySelectionCopied'
	| 'onError'
	| 'onExit'
	| 'onFrameTelemetry'
	| 'onHistoryChange'
	| 'onLinkOpenRequest'
	| 'onMetadataChange'
	| 'onRendererHealth'
	| 'onSearchResults'
	| 'onSelectionChange'
	| 'onSelectionCopied'
	| 'openLinkAsync'
	| 'open'
	| 'selectAll'
	| 'selection'
	| 'updateSearch'
>;

export interface ProjectedTerminalRendererDependencies {
	readonly createTerminal: (options: ProjectedTerminalOptions) => ProjectedTerminalDriver;
	readonly installShortcuts: typeof installProjectedTerminalShortcuts;
	readonly mountFindSurface: (options: MountTerminalFindSurfaceOptions) => TerminalFindSurfaceMount;
}

export interface ProjectedTerminalRendererSession {
	dispose(): void;
	readonly transportUnavailable: Promise<void>;
}

const defaultDependencies: ProjectedTerminalRendererDependencies = {
	createTerminal: (options) => new ProjectedTerminal(options),
	installShortcuts: installProjectedTerminalShortcuts,
	mountFindSurface: mountTerminalFindSurface,
};

export async function startProjectedTerminalRenderer(
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	shortcutPlatform: ProjectedTerminalShortcutPlatform,
	bootstrap: ProjectedAttachmentBootstrap,
	transport: ProjectedTerminalTransport,
	dependencies: ProjectedTerminalRendererDependencies = defaultDependencies,
	platformEffects: ProjectedTerminalPlatformEffects = {},
): Promise<ProjectedTerminalRendererSession> {
	if (!(rootElement instanceof HTMLElement)) {
		throw new Error('The Orchbee projected terminal root element is missing.');
	}
	const locale = resolveOrchbeeLocale(preferredLocales);
	const intl = createOrchbeeIntl(locale);
	documentElement.lang = locale;
	documentElement.dir = getOrchbeeTextDirection(locale);
	const status = rootElement.ownerDocument.createElement('div');
	status.dataset['projectedTerminalStatus'] = 'true';
	status.hidden = true;
	status.setAttribute('role', 'alert');
	rootElement.appendChild(status);
	rootElement.dataset['projectedTerminalState'] = 'opening';
	rootElement.dataset['projectedTerminalAttachmentId'] = bootstrap.attachmentId;
	rootElement.dataset['projectedTerminalCreationId'] = bootstrap.creationId;
	rootElement.dataset['projectedTerminalHostEpoch'] = bootstrap.hostEpoch;
	rootElement.dataset['projectedTerminalId'] = bootstrap.terminalId;
	let committedFrames = 0;
	let finalRevision: bigint | undefined;
	let lastRenderedRevision: bigint | undefined;
	let renderedFrames = 0;
	let resolveTransportUnavailable: (() => void) | undefined;
	const transportUnavailable = new Promise<void>((resolve) => {
		resolveTransportUnavailable = resolve;
	});
	const updateFinalPresentation = (): void => {
		if (
			finalRevision !== undefined &&
			lastRenderedRevision !== undefined &&
			lastRenderedRevision >= finalRevision
		) {
			rootElement.dataset['projectedTerminalFinalFramePresented'] = 'true';
		}
	};

	const reportFailure = (): void => {
		rootElement.dataset['projectedTerminalState'] = 'failed';
		status.textContent = intl.formatMessage(projectedTerminalMessages.connectionFailure);
		status.hidden = false;
	};
	const terminal = dependencies.createTerminal({
		attachmentId: bootstrap.attachmentId,
		canResize: bootstrap.canResize,
		creationId: bootstrap.creationId,
		hostEpoch: bootstrap.hostEpoch,
		inputEnabled: bootstrap.canInput !== false,
		labels: {
			deleted: (text) =>
				intl.formatMessage(projectedTerminalMessages.accessibilityDeleted, { text }),
			historyRange: (start, end, total) =>
				intl.formatMessage(projectedTerminalMessages.historyRange, {
					end: end.toString(),
					start: start.toString(),
					total: total.toString(),
				}),
			input: intl.formatMessage(projectedTerminalMessages.input),
			row: (row, text) =>
				intl.formatMessage(projectedTerminalMessages.accessibilityRow, {
					row: row.toString(),
					text,
				}),
			screen: intl.formatMessage(projectedTerminalMessages.screen),
			screenCursor: (row, column) =>
				intl.formatMessage(projectedTerminalMessages.accessibilityScreenCursor, {
					column,
					row: row.toString(),
				}),
			selectedText: (text) =>
				intl.formatMessage(projectedTerminalMessages.accessibilitySelection, { text }),
		},
		port: {
			send: (command) => transport.send(command),
		},
		projectionBufferTrust: transport.projectionBufferTrust,
		renderer: {
			fontFamily: '"Menlo", monospace',
			fontSize: 15,
			paddingBalance: true,
		},
		terminalId: bootstrap.terminalId,
	});
	const reportRuntimeFailure = (
		event: Extract<
			NonNullable<ProjectedAttachmentBootstrap['terminalOutcome']>,
			{ readonly kind: 'failed' }
		>['event'],
	): void => {
		rootElement.dataset['projectedTerminalState'] = 'failed';
		rootElement.dataset['projectedTerminalFailureKind'] = event.failure.kind;
		rootElement.dataset['projectedTerminalFailureStage'] = event.failure.stage;
		rootElement.dataset['projectedTerminalFinalRevision'] = event.finalRevision;
		finalRevision = BigInt(event.finalRevision);
		const message = intl.formatMessage(projectedTerminalMessages.runtimeFailure);
		status.textContent = message;
		status.hidden = false;
		terminal.notifyBrokerFailure({
			code: `terminal-${event.failure.kind}`,
			message,
			operation: 'connection',
			retryable: false,
		});
		updateFinalPresentation();
	};
	const findController = new TerminalFindController(
		terminal,
		terminal.getBufferRowsSnapshot().totalRows,
	);
	const actions = createProjectedTerminalActionComposition(
		terminal,
		findController,
		platformEffects,
	);
	const subscriptions = [
		terminal.onError((event) => {
			if (
				event.operation === 'frame' ||
				event.operation === 'open' ||
				event.operation === 'render'
			) {
				reportFailure();
			}
		}),
		terminal.onExit((event) => {
			rootElement.dataset['projectedTerminalState'] = 'exited';
			rootElement.dataset['projectedTerminalFinalRevision'] = event.finalRevision;
			finalRevision = BigInt(event.finalRevision);
			updateFinalPresentation();
		}),
		terminal.onFrameTelemetry((event) => {
			renderedFrames += 1;
			lastRenderedRevision = BigInt(event.revision);
			rootElement.dataset['projectedTerminalRenderedFrames'] = String(renderedFrames);
			rootElement.dataset['projectedTerminalRenderedAtMilliseconds'] = String(performance.now());
			rootElement.dataset['projectedTerminalRenderedRevision'] = event.revision;
			rootElement.dataset['projectedTerminalRenderer'] = event.backend;
			updateFinalPresentation();
		}),
		terminal.onHistoryChange((event) => findController.applyHistoryEvent(event)),
		terminal.onLinkOpenRequest((event) => {
			void actions.execute(
				PROJECTED_TERMINAL_ACTION_IDS.openLink,
				{ target: event.target },
				actions.terminalScope,
				'pointer',
			).completion;
		}),
		terminal.onMetadataChange((metadata) => {
			rootElement.style.backgroundColor = terminalRgbColor(metadata.defaultColors.backgroundRgb);
			rootElement.dataset['projectedTerminalCellPixelHeight'] = String(metadata.cellPixelHeight);
			rootElement.dataset['projectedTerminalCellPixelWidth'] = String(metadata.cellPixelWidth);
			rootElement.dataset['projectedTerminalColumns'] = String(metadata.columns);
			rootElement.dataset['projectedTerminalRows'] = String(metadata.rows);
		}),
		terminal.onRendererHealth((event) => {
			rootElement.dataset['projectedTerminalRenderer'] = event.backend;
			rootElement.dataset['projectedTerminalRendererHealth'] = event.state;
		}),
		terminal.onSearchResults((event) => {
			findController.applySearchEvent(event);
			if (event.state === 'error' && event.regexError !== null) {
				rootElement.dataset['projectedTerminalSearchError'] = event.regexError;
			} else {
				delete rootElement.dataset['projectedTerminalSearchError'];
			}
			if (event.state === 'complete') {
				rootElement.dataset['projectedTerminalSearchCompletedAtMilliseconds'] = String(
					performance.now(),
				);
			}
		}),
		terminal.onSelectionChange((event) => {
			findController.applySelectionEvent(event);
			rootElement.dataset['projectedTerminalSelectionState'] = event.state;
			if (event.state === 'active') {
				rootElement.dataset['projectedTerminalSelectionStart'] =
					`${event.start.row}:${event.start.column}`;
				rootElement.dataset['projectedTerminalSelectionEnd'] =
					`${event.end.row}:${event.end.column}`;
			} else {
				delete rootElement.dataset['projectedTerminalSelectionStart'];
				delete rootElement.dataset['projectedTerminalSelectionEnd'];
			}
		}),
		terminal.onSelectionCopied((event) => {
			rootElement.dataset['projectedTerminalCopiedBytes'] = String(event.byteLength);
			rootElement.dataset['projectedTerminalCopiedAtMilliseconds'] = String(performance.now());
		}),
	];
	const handleMessage = (value: unknown): void => {
		const event = ProjectedTerminalEventSchema.safeParse(value);
		if (!event.success) {
			reportFailure();
			return;
		}
		switch (event.data.kind) {
			case 'failure':
				terminal.notifyBrokerFailure(event.data.failure);
				if (
					event.data.failure.operation === 'history' &&
					(event.data.failure.code === 'unsupported-message' ||
						event.data.failure.code === 'protocol-minor-too-old' ||
						event.data.failure.code === 'unsupported-version')
				) {
					findController.setNotice({ kind: 'history-unavailable' });
				}
				if (
					!event.data.failure.retryable &&
					(event.data.failure.operation === undefined ||
						event.data.failure.operation === 'connection' ||
						event.data.failure.operation === 'protocol')
				) {
					reportFailure();
				}
				return;
			case 'frame': {
				const result = terminal.applyFrame(event.data.frame);
				if (result === 'committed' || result === 'duplicate') {
					committedFrames += 1;
					rootElement.dataset['projectedTerminalCommittedFrames'] = String(committedFrames);
				}
				return;
			}
			case 'history-changed':
				terminal.notifyHistoryChanged(event.data.event);
				return;
			case 'link-opened':
				terminal.notifyLinkOpened(event.data.requestSequence);
				return;
			case 'rows':
				terminal.applyRows(event.data.rows);
				return;
			case 'selection-changed':
				terminal.notifySelectionChanged(event.data.event);
				return;
			case 'selection-copy-content':
				void applyClientSelectionCopy(
					terminal,
					event.data.content,
					event.data.requestSequence,
					platformEffects,
				);
				return;
			case 'selection-copied':
				terminal.notifySelectionCopied(event.data.byteLength, event.data.requestSequence);
				return;
			case 'terminal-exited':
				terminal.notifyExited(event.data.event);
				return;
			case 'terminal-failed':
				reportRuntimeFailure(event.data.event);
				return;
		}
	};
	const unsubscribeTransport = transport.subscribe(handleMessage, () => {
		reportFailure();
		resolveTransportUnavailable?.();
		resolveTransportUnavailable = undefined;
	});

	const findRoot = rootElement.ownerDocument.createElement('div');
	findRoot.dataset['projectedTerminalFindSurface'] = 'true';
	let findSurface: TerminalFindSurfaceMount | undefined;
	let disposeShortcuts: (() => void) | undefined;
	try {
		await terminal.open(rootElement);
		rootElement.dataset['projectedTerminalState'] = 'open';
		if (bootstrap.terminalOutcome?.kind === 'exited') {
			terminal.notifyExited(bootstrap.terminalOutcome.event);
		} else if (bootstrap.terminalOutcome?.kind === 'failed') {
			reportRuntimeFailure(bootstrap.terminalOutcome.event);
		}
		rootElement.appendChild(findRoot);
		findSurface = dependencies.mountFindSurface({
			controller: findController,
			locale,
			onNext: () => {
				void actions.execute(
					PROJECTED_TERMINAL_ACTION_IDS.findNext,
					undefined,
					actions.findScope,
					'button',
				).completion;
			},
			onPrevious: () => {
				void actions.execute(
					PROJECTED_TERMINAL_ACTION_IDS.findPrevious,
					undefined,
					actions.findScope,
					'button',
				).completion;
			},
			restoreFocus: () => terminal.focus(),
			root: findRoot,
		});
		disposeShortcuts = dependencies.installShortcuts({
			document: rootElement.ownerDocument,
			findRoot,
			findScope: actions.findScope,
			invoke: (actionId, input, scope) => {
				void actions.execute(actionId, input, scope, 'keyboard').completion;
			},
			platform: shortcutPlatform,
			terminalRoot: rootElement,
			terminalScope: actions.terminalScope,
		});
		terminal.focus();
	} catch (error) {
		reportFailure();
		disposeShortcuts?.();
		findSurface?.dispose();
		findRoot.remove();
		unsubscribeTransport();
		for (const subscription of subscriptions) subscription.dispose();
		actions.dispose();
		findController.dispose();
		terminal.dispose();
		transport.close();
		throw error;
	}

	return {
		dispose: () => {
			disposeShortcuts?.();
			findSurface?.dispose();
			unsubscribeTransport();
			for (const subscription of subscriptions) subscription.dispose();
			actions.dispose();
			findController.dispose();
			terminal.dispose();
			transport.close();
			status.remove();
			findRoot.remove();
		},
		transportUnavailable,
	};
}

function terminalRgbColor(value: number): string {
	return `#${value.toString(16).padStart(6, '0')}`;
}

async function applyClientSelectionCopy(
	terminal: Pick<ProjectedTerminalDriver, 'notifyBrokerFailure' | 'notifySelectionCopied'>,
	content: ArrayBuffer,
	requestSequence: Parameters<ProjectedTerminalDriver['notifySelectionCopied']>[1],
	platformEffects: ProjectedTerminalPlatformEffects,
): Promise<void> {
	const writeClipboardText = platformEffects.writeClipboardText;
	if (writeClipboardText === undefined) {
		terminal.notifyBrokerFailure({
			code: 'client-clipboard-unavailable',
			message: 'The client clipboard effect is unavailable.',
			operation: 'copy',
			requestSequence,
			retryable: false,
		});
		return;
	}
	let text: string;
	try {
		text = new TextDecoder('utf-8', { fatal: true }).decode(content);
	} catch {
		terminal.notifyBrokerFailure({
			code: 'invalid-copy-utf8',
			message: 'The exported selection is not valid UTF-8.',
			operation: 'copy',
			requestSequence,
			retryable: false,
		});
		return;
	}
	let result: Awaited<
		ReturnType<NonNullable<ProjectedTerminalPlatformEffects['writeClipboardText']>>
	>;
	try {
		result = await writeClipboardText(text);
	} catch {
		result = { code: 'client-clipboard-failed', kind: 'failed', retryable: false };
	}
	if (result.kind === 'completed') {
		terminal.notifySelectionCopied(content.byteLength, requestSequence);
		return;
	}
	terminal.notifyBrokerFailure({
		code: result.kind === 'failed' ? result.code : 'client-copy-cancelled',
		message: 'The client clipboard effect did not complete.',
		operation: 'copy',
		requestSequence,
		retryable: result.kind === 'failed' && result.retryable,
	});
}
