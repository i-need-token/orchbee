import {
	createOrchbeeIntl,
	projectedTerminalMessages,
	getOrchbeeTextDirection,
	resolveOrchbeeLocale,
} from '@orchbee/i18n';
import {
	ProjectedTerminal,
	type ProjectedPasteReview,
	type ProjectedTerminalMetadata,
	type ProjectedTerminalOptions,
	type TerminalPresentationConfiguration,
	type TerminalPresentationConfigurationUpdateResult,
} from '@orchbee/terminal';
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
import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	type TerminalPresentationConfigurationSource,
} from './terminal-presentation-configuration-source';

type ProjectedTerminalDriver = Pick<
	ProjectedTerminal,
	| 'applyFrame'
	| 'applyRows'
	| 'canControlTerminal'
	| 'clearSearch'
	| 'copySelectionAsync'
	| 'dispose'
	| 'findNext'
	| 'findPrevious'
	| 'focus'
	| 'getBufferRowsSnapshot'
	| 'notifyExited'
	| 'notifyBell'
	| 'notifyBrokerFailure'
	| 'notifyHistoryChanged'
	| 'notifyLinkOpened'
	| 'notifySelectionChanged'
	| 'notifySelectionCopied'
	| 'onError'
	| 'onBell'
	| 'onExit'
	| 'onFrameTelemetry'
	| 'onHistoryChange'
	| 'onLinkOpenRequest'
	| 'onMetadataChange'
	| 'onRendererHealth'
	| 'onSearchResults'
	| 'onSelectionChange'
	| 'onSelectionCopyResult'
	| 'onSelectionCopied'
	| 'openLinkAsync'
	| 'open'
	| 'reset'
	| 'selectAll'
	| 'selection'
	| 'setResizeOwnership'
	| 'updatePresentationConfiguration'
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
	updatePresentationConfiguration(
		configuration: TerminalPresentationConfiguration,
	): Promise<TerminalPresentationConfigurationUpdateResult>;
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
	presentationConfigurationSource: TerminalPresentationConfigurationSource = DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	reviewPaste?: (review: ProjectedPasteReview, signal: AbortSignal) => boolean | Promise<boolean>,
	observeMetadata?: (metadata: ProjectedTerminalMetadata) => void,
): Promise<ProjectedTerminalRendererSession> {
	if (!(rootElement instanceof HTMLElement)) {
		throw new Error('The Orchbee projected terminal root element is missing.');
	}
	const locale = resolveOrchbeeLocale(preferredLocales);
	const intl = createOrchbeeIntl(locale);
	documentElement.lang = locale;
	documentElement.dir = getOrchbeeTextDirection(locale);
	let initialPresentationConfiguration: TerminalPresentationConfiguration;
	try {
		initialPresentationConfiguration = presentationConfigurationSource.current();
	} catch (error) {
		runCleanup([() => transport.close()]);
		throw error;
	}
	const status = (() => {
		let element: HTMLElement | undefined;
		try {
			element = rootElement.ownerDocument.createElement('div');
			element.dataset['projectedTerminalStatus'] = 'true';
			element.hidden = true;
			element.setAttribute('role', 'alert');
			rootElement.appendChild(element);
			return element;
		} catch (error) {
			runCleanup([() => element?.remove(), () => transport.close()]);
			throw error;
		}
	})();
	rootElement.dataset['projectedTerminalState'] = 'opening';
	rootElement.dataset['projectedTerminalAttachmentId'] = bootstrap.attachmentId;
	rootElement.dataset['projectedTerminalCreationId'] = bootstrap.creationId;
	rootElement.dataset['projectedTerminalHostEpoch'] = bootstrap.hostEpoch;
	rootElement.dataset['projectedTerminalId'] = bootstrap.terminalId;
	let committedFrames = 0;
	let committedPresentationEpoch = 0;
	let disposed = false;
	let finalRevision: bigint | undefined;
	let lastRenderedRevision: bigint | undefined;
	let renderedFrames = 0;
	let presentationRequestEpoch = 0;
	let statusKind: 'connection-failure' | 'copy' | 'runtime-failure' | undefined;
	let terminalOpened = false;
	let terminalOutcomeKind: 'exited' | 'failed' | undefined;
	let pendingPresentationConfiguration: TerminalPresentationConfiguration | undefined;
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

	const clearStatus = (): void => {
		status.textContent = null;
		status.hidden = true;
		statusKind = undefined;
	};
	const reportProjectionReady = (): void => {
		if (!terminalOpened || terminalOutcomeKind !== undefined) return;
		rootElement.dataset['projectedTerminalState'] = 'open';
		if (statusKind === 'connection-failure') clearStatus();
	};
	const reportFailure = (): void => {
		if (disposed || terminalOutcomeKind !== undefined) return;
		rootElement.dataset['projectedTerminalState'] = 'failed';
		statusKind = 'connection-failure';
		status.setAttribute('role', 'alert');
		status.textContent = intl.formatMessage(projectedTerminalMessages.connectionFailure);
		status.hidden = false;
	};
	let terminal: ProjectedTerminalDriver;
	try {
		terminal = dependencies.createTerminal({
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
			renderer: initialPresentationConfiguration,
			...(reviewPaste === undefined ? {} : { reviewPaste }),
			terminalId: bootstrap.terminalId,
		});
	} catch (error) {
		runCleanup([() => status.remove(), () => transport.close()]);
		throw error;
	}
	const updatePresentationConfiguration = async (
		configuration: TerminalPresentationConfiguration,
	): Promise<TerminalPresentationConfigurationUpdateResult> => {
		if (disposed) return { kind: 'cancelled', reason: 'disposed' };
		const epoch = ++presentationRequestEpoch;
		rootElement.dataset['projectedTerminalPresentationRequestEpoch'] = String(epoch);
		rootElement.dataset['projectedTerminalPresentationState'] = 'preparing';
		let result: TerminalPresentationConfigurationUpdateResult;
		try {
			result = await terminal.updatePresentationConfiguration(configuration);
		} catch (error) {
			if (!disposed && epoch === presentationRequestEpoch) {
				rootElement.dataset['projectedTerminalPresentationState'] = 'failed';
				rootElement.dataset['projectedTerminalPresentationFailure'] = 'update-rejected';
			}
			throw error;
		}
		if (disposed || epoch !== presentationRequestEpoch) return result;
		rootElement.dataset['projectedTerminalPresentationState'] = result.kind;
		if (result.kind === 'completed') {
			committedPresentationEpoch = epoch;
			rootElement.dataset['projectedTerminalPresentationCommittedEpoch'] = String(epoch);
			rootElement.dataset['projectedTerminalPresentationFontFamily'] =
				result.configuration.fontFamily;
			delete rootElement.dataset['projectedTerminalPresentationFailure'];
		} else if (result.kind === 'failed') {
			rootElement.dataset['projectedTerminalPresentationFailure'] = result.code;
		}
		return result;
	};
	const observePresentationConfigurationUpdate = (
		configuration: TerminalPresentationConfiguration,
	): void => {
		void updatePresentationConfiguration(configuration).catch(() => undefined);
	};
	let unsubscribePresentationConfiguration: () => void;
	try {
		const unsubscribe = presentationConfigurationSource.subscribe((configuration) => {
			if (!terminalOpened) {
				pendingPresentationConfiguration = configuration;
				return;
			}
			observePresentationConfigurationUpdate(configuration);
		});
		if (typeof unsubscribe !== 'function') {
			throw new TypeError('The terminal presentation configuration source did not unsubscribe.');
		}
		unsubscribePresentationConfiguration = unsubscribe;
	} catch (error) {
		runCleanup([() => terminal.dispose(), () => status.remove(), () => transport.close()]);
		throw error;
	}
	const reportRuntimeFailure = (
		event: Extract<
			NonNullable<ProjectedAttachmentBootstrap['terminalOutcome']>,
			{ readonly kind: 'failed' }
		>['event'],
	): void => {
		if (terminalOutcomeKind === 'exited') return;
		terminalOutcomeKind = 'failed';
		rootElement.dataset['projectedTerminalState'] = 'failed';
		rootElement.dataset['projectedTerminalFailureKind'] = event.failure.kind;
		rootElement.dataset['projectedTerminalFailureStage'] = event.failure.stage;
		rootElement.dataset['projectedTerminalFinalRevision'] = event.finalRevision;
		finalRevision = BigInt(event.finalRevision);
		const message = intl.formatMessage(projectedTerminalMessages.runtimeFailure);
		statusKind = 'runtime-failure';
		status.setAttribute('role', 'alert');
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
	const { actions, findController, subscriptions } = (() => {
		let pendingActions: ReturnType<typeof createProjectedTerminalActionComposition> | undefined;
		let pendingFindController: TerminalFindController | undefined;
		const pendingSubscriptions: Array<ReturnType<ProjectedTerminalDriver['onBell']>> = [];
		try {
			pendingFindController = new TerminalFindController(
				terminal,
				terminal.getBufferRowsSnapshot().totalRows,
			);
			pendingActions = createProjectedTerminalActionComposition(
				terminal,
				pendingFindController,
				platformEffects,
			);
			const activeActions = pendingActions;
			const activeFindController = pendingFindController;
			pendingSubscriptions.push(terminal.onBell((event) => platformEffects.ringBell?.(event)));
			pendingSubscriptions.push(
				terminal.onError((event) => {
					if (
						event.operation === 'frame' ||
						event.operation === 'open' ||
						event.operation === 'render'
					) {
						reportFailure();
					}
				}),
			);
			pendingSubscriptions.push(
				terminal.onExit((event) => {
					if (terminalOutcomeKind === 'failed') return;
					terminalOutcomeKind = 'exited';
					rootElement.dataset['projectedTerminalState'] = 'exited';
					rootElement.dataset['projectedTerminalFinalRevision'] = event.finalRevision;
					finalRevision = BigInt(event.finalRevision);
					if (statusKind === 'connection-failure') clearStatus();
					updateFinalPresentation();
				}),
			);
			pendingSubscriptions.push(
				terminal.onFrameTelemetry((event) => {
					renderedFrames += 1;
					lastRenderedRevision = BigInt(event.revision);
					rootElement.dataset['projectedTerminalRenderedFrames'] = String(renderedFrames);
					rootElement.dataset['projectedTerminalRenderedAtMilliseconds'] = String(
						performance.now(),
					);
					rootElement.dataset['projectedTerminalRenderedGeneration'] = event.generation;
					rootElement.dataset['projectedTerminalRenderedRevision'] = event.revision;
					rootElement.dataset['projectedTerminalPresentationDrawnEpoch'] = String(
						committedPresentationEpoch,
					);
					rootElement.dataset['projectedTerminalRenderer'] = event.backend;
					updateFinalPresentation();
				}),
			);
			pendingSubscriptions.push(
				terminal.onHistoryChange((event) => activeFindController.applyHistoryEvent(event)),
			);
			pendingSubscriptions.push(
				terminal.onLinkOpenRequest((event) => {
					void activeActions.execute(
						PROJECTED_TERMINAL_ACTION_IDS.openLink,
						{ target: event.target },
						activeActions.terminalScope,
						'pointer',
					).completion;
				}),
			);
			pendingSubscriptions.push(
				terminal.onMetadataChange((metadata) => {
					observeMetadata?.(metadata);
					rootElement.style.backgroundColor = terminalRgbColor(
						metadata.defaultColors.backgroundRgb,
					);
					rootElement.dataset['projectedTerminalCellPixelHeight'] = String(
						metadata.cellPixelHeight,
					);
					rootElement.dataset['projectedTerminalCellPixelWidth'] = String(metadata.cellPixelWidth);
					rootElement.dataset['projectedTerminalColumns'] = String(metadata.columns);
					rootElement.dataset['projectedTerminalRows'] = String(metadata.rows);
				}),
			);
			pendingSubscriptions.push(
				terminal.onRendererHealth((event) => {
					rootElement.dataset['projectedTerminalRenderer'] = event.backend;
					rootElement.dataset['projectedTerminalRendererHealth'] = event.state;
				}),
			);
			pendingSubscriptions.push(
				terminal.onSearchResults((event) => {
					activeFindController.applySearchEvent(event);
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
			);
			pendingSubscriptions.push(
				terminal.onSelectionChange((event) => {
					activeFindController.applySelectionEvent(event);
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
			);
			pendingSubscriptions.push(
				terminal.onSelectionCopyResult((event) => {
					statusKind = 'copy';
					rootElement.dataset['projectedTerminalCopySource'] = event.source;
					rootElement.dataset['projectedTerminalCopyState'] = event.result.kind;
					if (event.result.kind === 'completed') {
						delete rootElement.dataset['projectedTerminalCopyFailure'];
						status.setAttribute('role', 'status');
						status.textContent = intl.formatMessage(projectedTerminalMessages.copyCompleted, {
							byteLength: event.result.byteLength ?? 0,
						});
					} else {
						rootElement.dataset['projectedTerminalCopyFailure'] = event.result.code;
						status.setAttribute('role', 'alert');
						status.textContent = intl.formatMessage(projectedTerminalMessages.copyFailed);
					}
					status.hidden = false;
				}),
			);
			pendingSubscriptions.push(
				terminal.onSelectionCopied((event) => {
					rootElement.dataset['projectedTerminalCopiedBytes'] = String(event.byteLength);
					rootElement.dataset['projectedTerminalCopiedAtMilliseconds'] = String(performance.now());
				}),
			);
			return {
				actions: activeActions,
				findController: activeFindController,
				subscriptions: pendingSubscriptions,
			};
		} catch (error) {
			runCleanup([
				unsubscribePresentationConfiguration,
				...pendingSubscriptions.map((subscription) => () => subscription.dispose()),
				() => pendingActions?.dispose(),
				() => pendingFindController?.dispose(),
				() => terminal.dispose(),
				() => status.remove(),
				() => transport.close(),
			]);
			throw error;
		}
	})();
	const handleMessage = (value: unknown): void => {
		const event = ProjectedTerminalEventSchema.safeParse(value);
		if (!event.success) {
			reportFailure();
			return;
		}
		switch (event.data.kind) {
			case 'bell':
				terminal.notifyBell(event.data.event);
				return;
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
					reportProjectionReady();
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
			case 'resize-lease-changed':
				if (
					event.data.event.attachmentId !== bootstrap.attachmentId ||
					event.data.event.terminalId !== bootstrap.terminalId
				) {
					reportFailure();
					return;
				}
				terminal.setResizeOwnership(event.data.event.resizeLease);
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
	let unsubscribeTransport: () => void;
	try {
		const unsubscribe = transport.subscribe(handleMessage, () => {
			reportFailure();
			resolveTransportUnavailable?.();
			resolveTransportUnavailable = undefined;
		});
		if (typeof unsubscribe !== 'function') {
			throw new TypeError('The projected terminal transport did not unsubscribe.');
		}
		unsubscribeTransport = unsubscribe;
	} catch (error) {
		runCleanup([
			unsubscribePresentationConfiguration,
			...subscriptions.map((subscription) => () => subscription.dispose()),
			() => actions.dispose(),
			() => findController.dispose(),
			() => terminal.dispose(),
			() => status.remove(),
			() => transport.close(),
		]);
		throw error;
	}

	let findRoot: HTMLElement | undefined;
	let findSurface: TerminalFindSurfaceMount | undefined;
	let disposeShortcuts: (() => void) | undefined;
	try {
		findRoot = rootElement.ownerDocument.createElement('div');
		findRoot.dataset['projectedTerminalFindSurface'] = 'true';
		await terminal.open(rootElement);
		terminalOpened = true;
		reportProjectionReady();
		rootElement.dataset['projectedTerminalPresentationCommittedEpoch'] = '0';
		rootElement.dataset['projectedTerminalPresentationDrawnEpoch'] = '0';
		rootElement.dataset['projectedTerminalPresentationFontFamily'] =
			initialPresentationConfiguration.fontFamily;
		rootElement.dataset['projectedTerminalPresentationState'] = 'completed';
		if (pendingPresentationConfiguration !== undefined) {
			const pending = pendingPresentationConfiguration;
			pendingPresentationConfiguration = undefined;
			observePresentationConfigurationUpdate(pending);
		}
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
	} catch (error) {
		reportFailure();
		runCleanup([
			unsubscribePresentationConfiguration,
			() => disposeShortcuts?.(),
			() => findSurface?.dispose(),
			() => findRoot?.remove(),
			unsubscribeTransport,
			...subscriptions.map((subscription) => () => subscription.dispose()),
			() => actions.dispose(),
			() => findController.dispose(),
			() => terminal.dispose(),
			() => transport.close(),
			() => status.remove(),
		]);
		throw error;
	}

	return {
		dispose: () => {
			if (disposed) return;
			disposed = true;
			runCleanup([
				unsubscribePresentationConfiguration,
				() => disposeShortcuts?.(),
				() => findSurface?.dispose(),
				unsubscribeTransport,
				...subscriptions.map((subscription) => () => subscription.dispose()),
				() => actions.dispose(),
				() => findController.dispose(),
				() => terminal.dispose(),
				() => transport.close(),
				() => status.remove(),
				() => findRoot?.remove(),
			]);
		},
		transportUnavailable,
		updatePresentationConfiguration,
	};
}

function runCleanup(cleanups: readonly (() => void)[]): void {
	for (const cleanup of cleanups) {
		try {
			cleanup();
		} catch {
			// Cleanup must continue so later transport and DOM owners are still released.
		}
	}
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
