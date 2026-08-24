import { ActionRuntime, type ActionDisposable, type ActionScope } from '@orchbee/actions';
import {
	createOrchbeeIntl,
	projectedTerminalMessages,
	getOrchbeeTextDirection,
	resolveOrchbeeLocale,
} from '@orchbee/i18n';
import {
	DEFAULT_TERMINAL_INTERACTION_CONFIGURATION_SOURCE,
	ProjectedTerminal,
	type ProjectedTerminalActionContextRequest,
	type ProjectedTerminalThemePreparationResult,
	type ProjectedTerminalThemeReadinessResult,
	type ProjectedPasteReview,
	type ProjectedPasteResult,
	type ProjectedTerminalMetadata,
	type ProjectedTerminalOptions,
	type ProjectedTerminalPresentationSnapshot,
	type TerminalInputConfiguration,
	type TerminalInteractionConfiguration,
	type TerminalInteractionConfigurationSource,
	type TerminalPresentationConfiguration,
	type TerminalPresentationConfigurationUpdateResult,
} from '@orchbee/terminal';
import {
	type ProjectedAttachmentBootstrap,
	ProjectedTerminalEventSchema,
	type ProjectedTerminalFailure,
} from '@orchbee/workbench-client-protocol';
import type {
	WorkbenchTerminalActionContextProvider,
	WorkbenchTerminalBellEffect,
} from '@orchbee/workbench';

import type { TerminalCompletionReminderObserver } from '../../terminal-feedback-settings';
import { isTerminalThemePresentationSource } from '../../terminal-settings';
import {
	DEFAULT_TERMINAL_INPUT_CONFIGURATION_SOURCE,
	type TerminalInputConfigurationSource,
} from '../../terminal-input-configuration';
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
	type ProjectedTerminalActionResultEvent,
	type ProjectedTerminalShortcutBindingSource,
	type ProjectedTerminalShortcutPlatform,
} from '../projected-terminal-actions';
import type { ProjectedTerminalPlatformEffects } from '../projected-terminal-platform-effects';
import type { ProjectedTerminalTransport } from '../terminal-projection';
import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	type TerminalPresentationConfigurationSource,
} from '../../terminal-presentation';
import { createTerminalThemePresentationParticipant } from './createTerminalThemePresentationParticipant';

type ProjectedTerminalDriver = Pick<
	ProjectedTerminal,
	| 'applyFrame'
	| 'applyInputCapability'
	| 'applyRows'
	| 'canControlTerminal'
	| 'captureActionContext'
	| 'clear'
	| 'clearSearch'
	| 'copySelectionAsync'
	| 'dispose'
	| 'findNext'
	| 'findPrevious'
	| 'linkOpenRequestIsCurrent'
	| 'focus'
	| 'freeze'
	| 'getBufferRowsSnapshot'
	| 'notifyExited'
	| 'notifyFailed'
	| 'notifyBell'
	| 'notifyClipboardWriteRequest'
	| 'notifyBrokerFailure'
	| 'notifyHistoryChanged'
	| 'notifyLinkOpened'
	| 'notifySelectionChanged'
	| 'notifySelectionCopied'
	| 'onError'
	| 'onBell'
	| 'onClipboardWriteRequest'
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
	| 'paste'
	| 'reset'
	| 'scrollToBottom'
	| 'selectAll'
	| 'selection'
	| 'setOptionAsAltMode'
	| 'setCopyOnSelect'
	| 'setResizeOwnership'
	| 'updatePresentationConfiguration'
	| 'updateSearch'
> &
	Partial<Pick<ProjectedTerminal, 'capturePresentationSnapshot'>>;

export interface ProjectedTerminalWorkbenchHost {
	readonly admitBellEffect: (event: WorkbenchTerminalBellEffect) => boolean;
	readonly bindContextProvider: (
		provider: WorkbenchTerminalActionContextProvider,
	) => ActionDisposable;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}

interface ProjectedTerminalThemeDriver {
	abortThemePresentation(operationId: string): void;
	canCommitThemePresentation(operationId: string, revision: bigint): boolean;
	commitThemePresentation(operationId: string, revision: bigint): boolean;
	prepareThemePresentation(
		operationId: string,
		overlays: Parameters<ProjectedTerminal['prepareThemePresentation']>[1],
	): ProjectedTerminalThemePreparationResult;
	waitForThemePresentation(
		operationId: string,
		revision: bigint,
	): Promise<ProjectedTerminalThemeReadinessResult>;
}

function isProjectedTerminalThemeDriver(
	terminal: ProjectedTerminalDriver,
): terminal is ProjectedTerminalDriver & ProjectedTerminalThemeDriver {
	return (
		typeof Reflect.get(terminal, 'abortThemePresentation') === 'function' &&
		typeof Reflect.get(terminal, 'canCommitThemePresentation') === 'function' &&
		typeof Reflect.get(terminal, 'commitThemePresentation') === 'function' &&
		typeof Reflect.get(terminal, 'prepareThemePresentation') === 'function' &&
		typeof Reflect.get(terminal, 'waitForThemePresentation') === 'function'
	);
}

function isProjectedTerminalSnapshotDriver(
	terminal: ProjectedTerminalDriver,
): terminal is ProjectedTerminalDriver & Pick<ProjectedTerminal, 'capturePresentationSnapshot'> {
	return typeof Reflect.get(terminal, 'capturePresentationSnapshot') === 'function';
}

export interface ProjectedTerminalRendererDependencies {
	readonly createTerminal: (options: ProjectedTerminalOptions) => ProjectedTerminalDriver;
	readonly installShortcuts: typeof installProjectedTerminalShortcuts;
	readonly mountFindSurface: (options: MountTerminalFindSurfaceOptions) => TerminalFindSurfaceMount;
}

export interface ProjectedTerminalRendererSession {
	capturePresentationSnapshot?(): ProjectedTerminalPresentationSnapshot | null;
	dispose(): void;
	readonly displayReady?: Promise<void> | undefined;
	focus(): void;
	isTransportAvailable(): boolean;
	pasteFileDrop(text: string): Promise<ProjectedPasteResult>;
	readonly presentationReady: Promise<void>;
	readonly transportUnavailable: Promise<void>;
	/** Retire transport and interaction while retaining the already painted frame DOM. */
	freeze(): void;
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
	shortcutBindingSource?: ProjectedTerminalShortcutBindingSource,
	completionReminderObserver?: TerminalCompletionReminderObserver,
	inputConfigurationSource: TerminalInputConfigurationSource = DEFAULT_TERMINAL_INPUT_CONFIGURATION_SOURCE,
	workbenchHost?: ProjectedTerminalWorkbenchHost,
	interactionConfigurationSource: TerminalInteractionConfigurationSource = DEFAULT_TERMINAL_INTERACTION_CONFIGURATION_SOURCE,
	presentationSnapshot?: ProjectedTerminalPresentationSnapshot,
): Promise<ProjectedTerminalRendererSession> {
	if (!(rootElement instanceof HTMLElement)) {
		throw new Error('The Orchbee projected terminal root element is missing.');
	}
	const locale = resolveOrchbeeLocale(preferredLocales);
	const intl = createOrchbeeIntl(locale);
	documentElement.lang = locale;
	documentElement.dir = getOrchbeeTextDirection(locale);
	const terminalThemeSource = isTerminalThemePresentationSource(presentationConfigurationSource)
		? presentationConfigurationSource
		: undefined;
	const initialRootVisibility = rootElement.style.visibility;
	let initialPresentationConfiguration: TerminalPresentationConfiguration;
	let initialInputConfiguration: TerminalInputConfiguration;
	let initialInteractionConfiguration: TerminalInteractionConfiguration;
	try {
		initialPresentationConfiguration = presentationConfigurationSource.current();
		initialInputConfiguration = inputConfigurationSource.current();
		initialInteractionConfiguration = interactionConfigurationSource.current();
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
	const progress = (() => {
		let element: HTMLElement | undefined;
		try {
			element = rootElement.ownerDocument.createElement('div');
			element.dataset['projectedTerminalProgress'] = 'true';
			element.hidden = true;
			element.setAttribute(
				'aria-label',
				intl.formatMessage(projectedTerminalMessages.progressLabel),
			);
			element.setAttribute('role', 'progressbar');
			const indicator = rootElement.ownerDocument.createElement('span');
			indicator.setAttribute('aria-hidden', 'true');
			element.appendChild(indicator);
			rootElement.appendChild(element);
			return element;
		} catch (error) {
			runCleanup([() => element?.remove(), () => status.remove(), () => transport.close()]);
			throw error;
		}
	})();
	rootElement.dataset['projectedTerminalState'] = 'opening';
	rootElement.dataset['projectedTerminalAttachmentId'] = bootstrap.attachmentId;
	rootElement.dataset['projectedTerminalCreationId'] = bootstrap.creationId;
	rootElement.dataset['projectedTerminalHostEpoch'] = bootstrap.hostEpoch;
	rootElement.dataset['projectedTerminalId'] = bootstrap.terminalId;
	rootElement.dataset['projectedTerminalInputCapability'] = bootstrap.canInput
		? 'read-write'
		: 'read-only';
	rootElement.dataset['projectedTerminalCapabilityRevision'] = bootstrap.capabilityRevision;
	let committedFrames = 0;
	let committedPresentationEpoch = 0;
	let committedPresentationConfiguration = initialPresentationConfiguration;
	let disposed = false;
	let frozen = false;
	let finalRevision: bigint | undefined;
	let lastObservedCommandCompletionSequence: bigint | undefined;
	let lastMetadata: ProjectedTerminalMetadata | undefined;
	let lastRenderedRevision: bigint | undefined;
	let renderedFrames = 0;
	let presentationRequestEpoch = 0;
	let statusKind:
		| 'action'
		| 'command-completion'
		| 'connection-failure'
		| 'copy'
		| 'feature-failure'
		| 'runtime-failure'
		| undefined;
	let terminalOpened = false;
	let bellTimer: ReturnType<typeof setTimeout> | undefined;
	let bellVisualPhase: 'a' | 'b' | undefined;
	let actionStatusTimer: ReturnType<typeof setTimeout> | undefined;
	let completionStatusTimer: ReturnType<typeof setTimeout> | undefined;
	let clipboardWriteController: AbortController | undefined;
	let terminalOutcomeKind: 'exited' | 'failed' | undefined;
	let pendingPresentationConfiguration: TerminalPresentationConfiguration | undefined;
	let transportAvailable = true;
	let activationPresentationDrawn = false;
	let displayPresentationDrawn = false;
	let presentationVisible = terminalThemeSource === undefined;
	let resolveDisplayReady: (() => void) | undefined;
	let resolveTransportUnavailable: (() => void) | undefined;
	let resolvePresentationReady: (() => void) | undefined;
	const displayReady = new Promise<void>((resolve) => {
		resolveDisplayReady = resolve;
	});
	const presentationReady = new Promise<void>((resolve) => {
		resolvePresentationReady = resolve;
	});
	const transportUnavailable = new Promise<void>((resolve) => {
		resolveTransportUnavailable = resolve;
	});
	const resolveReadiness = (): void => {
		if (!presentationVisible) return;
		if (displayPresentationDrawn) {
			resolveDisplayReady?.();
			resolveDisplayReady = undefined;
		}
		if (activationPresentationDrawn) {
			resolvePresentationReady?.();
			resolvePresentationReady = undefined;
		}
	};
	const reportTransportUnavailable = (): void => {
		if (!transportAvailable) return;
		transportAvailable = false;
		resolveTransportUnavailable?.();
		resolveTransportUnavailable = undefined;
	};
	const updateFinalPresentation = (): void => {
		if (
			finalRevision !== undefined &&
			lastRenderedRevision !== undefined &&
			lastRenderedRevision >= finalRevision
		) {
			rootElement.dataset['projectedTerminalFinalFramePresented'] = 'true';
		}
	};
	const syncRootBackground = (): void => {
		if (lastMetadata === undefined) return;
		const background = terminalRgbColor(lastMetadata.defaultColors.backgroundRgb);
		rootElement.style.backgroundColor = terminalBackgroundColor(
			background,
			committedPresentationConfiguration.backgroundOpacity,
		);
		rootElement.dataset['projectedTerminalBackgroundOpacity'] = String(
			committedPresentationConfiguration.backgroundOpacity,
		);
	};
	const clearStatus = (): void => {
		status.textContent = null;
		status.hidden = true;
		statusKind = undefined;
	};
	const clearVisualBell = (): void => {
		if (bellTimer !== undefined) clearTimeout(bellTimer);
		bellTimer = undefined;
		delete rootElement.dataset['projectedTerminalBellActive'];
	};
	const showVisualBell = (sequence: string): void => {
		bellVisualPhase = bellVisualPhase === 'a' ? 'b' : 'a';
		rootElement.dataset['projectedTerminalBellActive'] = 'true';
		rootElement.dataset['projectedTerminalBellPhase'] = bellVisualPhase;
		rootElement.dataset['projectedTerminalBellSequence'] = sequence;
		if (bellTimer !== undefined) clearTimeout(bellTimer);
		bellTimer = setTimeout(clearVisualBell, 140);
	};
	const clearCompletionStatusTimer = (): void => {
		if (completionStatusTimer !== undefined) clearTimeout(completionStatusTimer);
		completionStatusTimer = undefined;
	};
	const clearActionStatusTimer = (): void => {
		if (actionStatusTimer !== undefined) clearTimeout(actionStatusTimer);
		actionStatusTimer = undefined;
	};
	const reportActionResult = (event: ProjectedTerminalActionResultEvent): void => {
		rootElement.dataset['projectedTerminalActionId'] = event.actionId;
		rootElement.dataset['projectedTerminalActionResult'] = event.result.kind;
		if (event.result.kind === 'blocked') {
			rootElement.dataset['projectedTerminalActionCode'] = event.result.code;
		} else if (event.result.kind === 'failed') {
			rootElement.dataset['projectedTerminalActionCode'] = event.result.failure.code;
		} else {
			delete rootElement.dataset['projectedTerminalActionCode'];
		}
		const presentation = projectedTerminalActionResultPresentation(event);
		if (presentation === null) return;
		clearActionStatusTimer();
		statusKind = 'action';
		status.setAttribute('role', presentation.role);
		status.textContent = intl.formatMessage(presentation.message);
		status.hidden = false;
		actionStatusTimer = setTimeout(() => {
			actionStatusTimer = undefined;
			if (statusKind === 'action') clearStatus();
		}, 5_000);
	};
	const observeCommandCompletion = (metadata: ProjectedTerminalMetadata): void => {
		const completion = metadata.commandCompletion;
		if (
			completion === null ||
			completion === undefined ||
			completionReminderObserver === undefined
		) {
			return;
		}
		if (completion.sequence === lastObservedCommandCompletionSequence) return;
		lastObservedCommandCompletionSequence = completion.sequence;
		rootElement.dataset['projectedTerminalCommandCompletionObservedSequence'] =
			completion.sequence.toString();
		const duration =
			Math.min(Number(completion.durationMilliseconds), Number.MAX_SAFE_INTEGER) / 1000;
		const message = intl.formatMessage(projectedTerminalMessages.commandCompletedBody, {
			duration,
			exitCode: completion.exitCode,
		});
		const ownerDocument = rootElement.ownerDocument;
		const result = completionReminderObserver.observeCompletion({
			completion,
			focused: ownerDocument.hasFocus() && rootElement.contains(ownerDocument.activeElement),
			notification: {
				body: message,
				tag: `orchbee-terminal:${bootstrap.hostEpoch}:${bootstrap.terminalId}`,
				title: intl.formatMessage(projectedTerminalMessages.commandCompletedTitle),
			},
			terminalIdentity: `${bootstrap.hostEpoch}:${bootstrap.terminalId}`,
		});
		rootElement.dataset['projectedTerminalCommandReminderDecision'] =
			result.kind === 'remind' ? `remind-${result.notification}` : `ignored-${result.reason}`;
		if (result.kind !== 'remind') return;
		clearCompletionStatusTimer();
		statusKind = 'command-completion';
		status.setAttribute('role', 'status');
		status.textContent = message;
		status.hidden = false;
		rootElement.dataset['projectedTerminalCommandCompletionSequence'] =
			completion.sequence.toString();
		rootElement.dataset['projectedTerminalCommandNotification'] = result.notification;
		completionStatusTimer = setTimeout(() => {
			completionStatusTimer = undefined;
			if (statusKind === 'command-completion') clearStatus();
		}, 8_000);
	};
	const updateProgress = (metadata: ProjectedTerminalMetadata): void => {
		const terminalProgress = metadata.progress;
		progress.dataset['state'] = terminalProgress.state;
		progress.hidden = terminalProgress.state === 'remove';
		if (terminalProgress.value === null) {
			progress.removeAttribute('aria-valuemax');
			progress.removeAttribute('aria-valuemin');
			progress.removeAttribute('aria-valuenow');
			progress.style.removeProperty('--ob-comp-projected-terminal-progress-value');
			return;
		}
		progress.setAttribute('aria-valuemax', '100');
		progress.setAttribute('aria-valuemin', '0');
		progress.setAttribute('aria-valuenow', String(terminalProgress.value));
		progress.style.setProperty(
			'--ob-comp-projected-terminal-progress-value',
			`${terminalProgress.value}%`,
		);
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
	const reportFeatureFailure = (failure: ProjectedTerminalFailure): void => {
		rootElement.dataset['projectedTerminalFailureCode'] = failure.code;
		rootElement.dataset['projectedTerminalFailureRetryable'] = String(failure.retryable);
		if (failure.correlationId === undefined) {
			delete rootElement.dataset['projectedTerminalFailureCorrelationId'];
		} else {
			rootElement.dataset['projectedTerminalFailureCorrelationId'] = failure.correlationId;
		}
		if (failure.operation === undefined) {
			delete rootElement.dataset['projectedTerminalFailureOperation'];
		} else {
			rootElement.dataset['projectedTerminalFailureOperation'] = failure.operation;
		}
		if (failure.requestSequence === undefined) {
			delete rootElement.dataset['projectedTerminalFailureRequestSequence'];
		} else {
			rootElement.dataset['projectedTerminalFailureRequestSequence'] = failure.requestSequence;
		}
		if (
			terminalOutcomeKind !== undefined &&
			(failure.operation === undefined ||
				failure.operation === 'connection' ||
				failure.operation === 'protocol')
		) {
			return;
		}
		if (
			failure.operation === 'history' ||
			failure.operation === 'input' ||
			failure.operation === 'resize'
		) {
			return;
		}
		const consequence = intl.formatMessage(projectedTerminalFailureMessage(failure));
		const message = failure.retryable
			? intl.formatMessage(projectedTerminalMessages.retryableFailure, { consequence })
			: consequence;
		if (
			!failure.retryable &&
			(failure.operation === undefined ||
				failure.operation === 'connection' ||
				failure.operation === 'protocol')
		) {
			rootElement.dataset['projectedTerminalState'] = 'failed';
			statusKind = 'connection-failure';
		} else {
			statusKind = 'feature-failure';
		}
		status.setAttribute('role', 'alert');
		status.textContent = message;
		status.hidden = false;
	};
	let terminal: ProjectedTerminalDriver;
	try {
		terminal = dependencies.createTerminal({
			attachmentId: bootstrap.attachmentId,
			canResize: bootstrap.canResize,
			capabilityRevision: bootstrap.capabilityRevision,
			creationId: bootstrap.creationId,
			hostEpoch: bootstrap.hostEpoch,
			inputConfiguration: initialInputConfiguration,
			copyOnSelect: initialInteractionConfiguration.copyOnSelect,
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
			...(presentationSnapshot === undefined ? {} : { presentationSnapshot }),
			projectionBufferTrust: transport.projectionBufferTrust,
			renderer: initialPresentationConfiguration,
			...(reviewPaste === undefined ? {} : { reviewPaste }),
			terminalId: bootstrap.terminalId,
		});
	} catch (error) {
		runCleanup([() => progress.remove(), () => status.remove(), () => transport.close()]);
		throw error;
	}
	if (terminalThemeSource !== undefined && !isProjectedTerminalThemeDriver(terminal)) {
		runCleanup([() => terminal.dispose(), () => transport.close()]);
		throw new TypeError('The projected terminal does not support runtime theme transactions.');
	}
	const themeParticipant =
		terminalThemeSource === undefined || !isProjectedTerminalThemeDriver(terminal)
			? undefined
			: createTerminalThemePresentationParticipant({
					bootstrap,
					onPresented: () => {
						rootElement.style.visibility = initialRootVisibility;
						rootElement.dataset['projectedTerminalThemeState'] = 'presented';
						presentationVisible = true;
						resolveReadiness();
					},
					terminal,
					transport,
				});
	if (themeParticipant !== undefined) rootElement.style.visibility = 'hidden';
	const disposeThemeParticipant = (): void => {
		themeParticipant?.dispose();
		rootElement.style.visibility = initialRootVisibility;
	};
	const updatePresentationConfiguration = async (
		configuration: TerminalPresentationConfiguration,
	): Promise<TerminalPresentationConfigurationUpdateResult> => {
		if (disposed || frozen) return { kind: 'cancelled', reason: 'disposed' };
		const epoch = ++presentationRequestEpoch;
		rootElement.dataset['projectedTerminalPresentationRequestEpoch'] = String(epoch);
		rootElement.dataset['projectedTerminalPresentationState'] = 'preparing';
		let result: TerminalPresentationConfigurationUpdateResult;
		try {
			result = await terminal.updatePresentationConfiguration(configuration);
		} catch (error) {
			if (!disposed && !frozen && epoch === presentationRequestEpoch) {
				rootElement.dataset['projectedTerminalPresentationState'] = 'failed';
				rootElement.dataset['projectedTerminalPresentationFailure'] = 'update-rejected';
			}
			throw error;
		}
		if (disposed || frozen || epoch !== presentationRequestEpoch) return result;
		rootElement.dataset['projectedTerminalPresentationState'] = result.kind;
		if (result.kind === 'completed') {
			committedPresentationEpoch = epoch;
			committedPresentationConfiguration = result.configuration;
			syncRootBackground();
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
	let unsubscribeInputConfiguration = (): void => undefined;
	let unsubscribeInteractionConfiguration = (): void => undefined;
	let unsubscribePresentationConfiguration = (): void => undefined;
	let unregisterThemeParticipant = (): void => undefined;
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
		const unsubscribeInput = inputConfigurationSource.subscribe((configuration) => {
			terminal.setOptionAsAltMode(configuration.optionAsAlt);
		});
		if (typeof unsubscribeInput !== 'function') {
			throw new TypeError('The terminal input configuration source did not unsubscribe.');
		}
		unsubscribeInputConfiguration = unsubscribeInput;
		const unsubscribeInteraction = interactionConfigurationSource.subscribe((configuration) => {
			terminal.setCopyOnSelect(configuration.copyOnSelect);
		});
		if (typeof unsubscribeInteraction !== 'function') {
			throw new TypeError('The terminal interaction configuration source did not unsubscribe.');
		}
		unsubscribeInteractionConfiguration = unsubscribeInteraction;
	} catch (error) {
		runCleanup([
			unsubscribeInputConfiguration,
			unsubscribeInteractionConfiguration,
			unsubscribePresentationConfiguration,
			disposeThemeParticipant,
			() => terminal.dispose(),
			() => progress.remove(),
			() => status.remove(),
			() => transport.close(),
		]);
		throw error;
	}
	const unsubscribeConfigurationSources = (): void => {
		unsubscribeInputConfiguration();
		unsubscribeInteractionConfiguration();
		unsubscribePresentationConfiguration();
		unregisterThemeParticipant();
		unregisterThemeParticipant = () => undefined;
	};
	const reportRuntimeFailure = (
		event: Extract<
			NonNullable<ProjectedAttachmentBootstrap['terminalOutcome']>,
			{ readonly kind: 'failed' }
		>['event'],
	): void => {
		if (terminalOutcomeKind === 'exited') return;
		terminalOutcomeKind = 'failed';
		terminal.notifyFailed(event);
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
	const { actions, disposeActions, findController, subscriptions } = (() => {
		const ownedActionRuntime = workbenchHost === undefined ? new ActionRuntime() : undefined;
		const actionRuntime = workbenchHost?.runtime ?? ownedActionRuntime;
		if (actionRuntime === undefined) throw new Error('The terminal action runtime is unavailable.');
		const terminalActionScope =
			workbenchHost?.scope ??
			actionRuntime.createScope(`projected-terminal:${bootstrap.attachmentId}`);
		let pendingActions: ReturnType<typeof createProjectedTerminalActionComposition> | undefined;
		let pendingContextRegistration: ActionDisposable | undefined;
		let pendingFindController: TerminalFindController | undefined;
		const pendingSubscriptions: Array<ReturnType<ProjectedTerminalDriver['onBell']>> = [];
		const disposePendingActions = (): void => {
			pendingContextRegistration?.dispose();
			pendingActions?.dispose();
			ownedActionRuntime?.dispose();
		};
		try {
			pendingFindController = new TerminalFindController(
				terminal,
				terminal.getBufferRowsSnapshot().totalRows,
			);
			pendingActions = createProjectedTerminalActionComposition(terminal, pendingFindController, {
				platformEffects,
				reportResult: reportActionResult,
				runtime: actionRuntime,
				terminalScope: terminalActionScope,
			});
			const activeActions = pendingActions;
			const activeFindController = pendingFindController;
			pendingContextRegistration = workbenchHost?.bindContextProvider({
				capture: (request) => {
					const terminalRequest: ProjectedTerminalActionContextRequest = {
						clientX: request.clientX,
						clientY: request.clientY,
						shiftKey: request.shiftKey,
					};
					const capture = terminal.captureActionContext(terminalRequest);
					if (capture.kind === 'terminal-owned') return capture;
					return {
						actionInputs:
							capture.link === null
								? {}
								: { [PROJECTED_TERMINAL_ACTION_IDS.openLink]: capture.link },
						focus: { terminal: true },
						kind: 'product',
						target: { id: bootstrap.attachmentId, type: 'terminal-attachment' },
					};
				},
				executePrepared: (invocation) => activeActions.executePrepared(invocation),
			});
			pendingSubscriptions.push(
				terminal.onBell((event) => {
					const effect: WorkbenchTerminalBellEffect = {
						sequence: event.sequence,
						terminalId: bootstrap.terminalId,
					};
					if (workbenchHost !== undefined && !workbenchHost.admitBellEffect(effect)) return;
					showVisualBell(event.sequence);
					try {
						platformEffects.playBellSound?.(effect);
					} catch {
						// Optional sound failure does not suppress the visual effect or notification.
					}
					try {
						platformEffects.showBellNotification?.(effect);
					} catch {
						// Optional notification failure does not suppress the accepted visual effect.
					}
				}),
			);
			pendingSubscriptions.push(
				terminal.onClipboardWriteRequest((event) => {
					if (clipboardWriteController !== undefined) return;
					const sendDecision = (approved: boolean): void => {
						if (disposed || frozen) return;
						try {
							transport.send({
								kind: 'clipboard-write-decision',
								payload: { ...event, approved },
							});
						} catch {
							reportFailure();
						}
					};
					const writeClipboardText = platformEffects.writeClipboardText;
					if (writeClipboardText === undefined) {
						// Desktop Main owns this effect; the exact positive decision delegates one write.
						sendDecision(true);
						return;
					}
					const controller = new AbortController();
					clipboardWriteController = controller;
					void (async () => {
						let result;
						try {
							result = await writeClipboardText(event.text);
						} catch {
							result = {
								code: 'client-clipboard-failed',
								kind: 'failed',
								retryable: false,
							} as const;
						}
						if (controller.signal.aborted || disposed || frozen) return;
						sendDecision(result.kind === 'completed');
						if (result.kind === 'failed') {
							terminal.notifyBrokerFailure({
								code: result.code,
								message: 'The terminal clipboard write did not complete.',
								operation: 'clipboard',
								retryable: result.retryable,
							});
						}
					})().finally(() => {
						if (clipboardWriteController === controller) clipboardWriteController = undefined;
					});
				}),
			);
			pendingSubscriptions.push(
				terminal.onError((event) => {
					if ('code' in event) {
						reportFeatureFailure(event);
						return;
					}
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
					displayPresentationDrawn = true;
					activationPresentationDrawn = true;
					resolveReadiness();
				}),
			);
			pendingSubscriptions.push(
				terminal.onHistoryChange((event) => activeFindController.applyHistoryEvent(event)),
			);
			pendingSubscriptions.push(
				terminal.onLinkOpenRequest((event) => {
					void activeActions.execute(
						PROJECTED_TERMINAL_ACTION_IDS.openLink,
						event,
						activeActions.terminalScope,
						'pointer',
					).completion;
				}),
			);
			pendingSubscriptions.push(
				terminal.onMetadataChange((metadata) => {
					observeMetadata?.(metadata);
					lastMetadata = metadata;
					syncRootBackground();
					rootElement.dataset['projectedTerminalCellPixelHeight'] = String(
						metadata.cellPixelHeight,
					);
					rootElement.dataset['projectedTerminalCellPixelWidth'] = String(metadata.cellPixelWidth);
					rootElement.dataset['projectedTerminalColumns'] = String(metadata.columns);
					rootElement.dataset['projectedTerminalCursorBlinking'] = String(metadata.cursor.blinking);
					rootElement.dataset['projectedTerminalCursorShape'] = metadata.cursor.shape;
					rootElement.dataset['projectedTerminalRows'] = String(metadata.rows);
					rootElement.dataset['projectedTerminalThemeRevision'] = metadata.themeRevision.toString();
					updateProgress(metadata);
					observeCommandCompletion(metadata);
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
				disposeActions: disposePendingActions,
				findController: activeFindController,
				subscriptions: pendingSubscriptions,
			};
		} catch (error) {
			runCleanup([
				unsubscribeConfigurationSources,
				disposeThemeParticipant,
				...pendingSubscriptions.map((subscription) => () => subscription.dispose()),
				disposePendingActions,
				() => pendingFindController?.dispose(),
				() => terminal.dispose(),
				() => progress.remove(),
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
		if (themeParticipant?.handleEvent(event.data) === true) return;
		switch (event.data.kind) {
			case 'attachment-input-capability-changed': {
				const { capabilities } = event.data;
				if (
					capabilities.attachmentId !== bootstrap.attachmentId ||
					capabilities.terminalId !== bootstrap.terminalId
				) {
					reportFailure();
					return;
				}
				const result = terminal.applyInputCapability({
					attachmentId: capabilities.attachmentId,
					capabilityRevision: capabilities.capabilityRevision,
					input: capabilities.capabilities.input,
					terminalId: capabilities.terminalId,
				});
				if (result.kind === 'stale') return;
				if (result.kind === 'rejected') {
					rootElement.dataset['projectedTerminalInputCapability'] = 'read-only';
					rootElement.dataset['projectedTerminalCapabilityFailure'] = result.reason;
					reportFailure();
					return;
				}
				rootElement.dataset['projectedTerminalInputCapability'] = capabilities.capabilities.input
					? 'read-write'
					: 'read-only';
				rootElement.dataset['projectedTerminalCapabilityRevision'] =
					capabilities.capabilityRevision;
				delete rootElement.dataset['projectedTerminalCapabilityFailure'];
				return;
			}
			case 'bell':
				terminal.notifyBell(event.data.event);
				return;
			case 'clipboard-write-request':
				terminal.notifyClipboardWriteRequest(event.data.event);
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
				if (attachmentFailureRequiresRecovery(event.data.failure)) {
					reportTransportUnavailable();
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
			case 'terminal-theme-failed':
			case 'terminal-theme-reconciled':
			case 'terminal-theme-result':
				return;
		}
	};
	let unsubscribeTransport: () => void;
	try {
		const unsubscribe = transport.subscribe(handleMessage, () => {
			reportFailure();
			reportTransportUnavailable();
		});
		if (typeof unsubscribe !== 'function') {
			throw new TypeError('The projected terminal transport did not unsubscribe.');
		}
		unsubscribeTransport = unsubscribe;
	} catch (error) {
		runCleanup([
			unsubscribeConfigurationSources,
			disposeThemeParticipant,
			...subscriptions.map((subscription) => () => subscription.dispose()),
			disposeActions,
			() => findController.dispose(),
			() => terminal.dispose(),
			() => progress.remove(),
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
		if (!transportAvailable) {
			throw new Error('Projected terminal transport became unavailable during renderer startup.');
		}
		const openOutcome = await Promise.race([
			terminal.open(rootElement).then(() => 'opened' as const),
			transportUnavailable.then(() => 'unavailable' as const),
		]);
		if (openOutcome !== 'opened' || !transportAvailable) {
			throw new Error('Projected terminal transport became unavailable during renderer startup.');
		}
		terminalOpened = true;
		if (
			presentationSnapshot !== undefined &&
			isProjectedTerminalSnapshotDriver(terminal) &&
			terminal.capturePresentationSnapshot() === presentationSnapshot
		) {
			displayPresentationDrawn = true;
			rootElement.dataset['projectedTerminalCachedPresentation'] = 'true';
			resolveReadiness();
		}
		reportProjectionReady();
		if (terminalThemeSource !== undefined && themeParticipant !== undefined) {
			unregisterThemeParticipant = terminalThemeSource.registerThemeParticipant(themeParticipant);
			rootElement.dataset['projectedTerminalThemeState'] = 'preparing';
		}
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
			invoke: (actionId, input, scope) => actions.invokeShortcut(actionId, input, scope),
			platform: shortcutPlatform,
			shortcutBindingSource,
			terminalRoot: rootElement,
			terminalScope: actions.terminalScope,
		});
	} catch (error) {
		reportFailure();
		runCleanup([
			unsubscribeConfigurationSources,
			disposeThemeParticipant,
			() => disposeShortcuts?.(),
			() => findSurface?.dispose(),
			() => findRoot?.remove(),
			unsubscribeTransport,
			...subscriptions.map((subscription) => () => subscription.dispose()),
			disposeActions,
			() => findController.dispose(),
			() => terminal.dispose(),
			() => transport.close(),
			() => progress.remove(),
			() => status.remove(),
		]);
		throw error;
	}

	return {
		capturePresentationSnapshot: () =>
			isProjectedTerminalSnapshotDriver(terminal) ? terminal.capturePresentationSnapshot() : null,
		dispose: () => {
			if (disposed) return;
			disposed = true;
			reportTransportUnavailable();
			clipboardWriteController?.abort();
			clipboardWriteController = undefined;
			clearVisualBell();
			clearActionStatusTimer();
			clearCompletionStatusTimer();
			if (frozen) {
				runCleanup([
					disposeThemeParticipant,
					() => terminal.dispose(),
					() => status.remove(),
					() => progress.remove(),
					() => findRoot?.remove(),
				]);
				return;
			}
			runCleanup([
				unsubscribeConfigurationSources,
				disposeThemeParticipant,
				() => disposeShortcuts?.(),
				() => findSurface?.dispose(),
				unsubscribeTransport,
				...subscriptions.map((subscription) => () => subscription.dispose()),
				disposeActions,
				() => findController.dispose(),
				() => terminal.dispose(),
				() => transport.close(),
				() => status.remove(),
				() => progress.remove(),
				() => findRoot?.remove(),
			]);
		},
		displayReady,
		focus: () => {
			if (disposed || frozen) return;
			terminal.focus();
		},
		freeze: () => {
			if (disposed || frozen) return;
			frozen = true;
			reportTransportUnavailable();
			clipboardWriteController?.abort();
			clipboardWriteController = undefined;
			clearVisualBell();
			clearActionStatusTimer();
			clearCompletionStatusTimer();
			runCleanup([
				unsubscribeConfigurationSources,
				disposeThemeParticipant,
				() => disposeShortcuts?.(),
				() => findSurface?.dispose(),
				unsubscribeTransport,
				...subscriptions.map((subscription) => () => subscription.dispose()),
				disposeActions,
				() => findController.dispose(),
				() => terminal.freeze(),
				() => transport.close(),
				() => status.remove(),
				() => progress.remove(),
				() => findRoot?.remove(),
			]);
		},
		isTransportAvailable: () => transportAvailable,
		pasteFileDrop: async (text) => await terminal.paste(text, 'file-drop'),
		presentationReady,
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

function terminalBackgroundColor(color: string, opacity: number): string {
	if (opacity >= 1) return color;
	const red = Number.parseInt(color.slice(1, 3), 16);
	const green = Number.parseInt(color.slice(3, 5), 16);
	const blue = Number.parseInt(color.slice(5, 7), 16);
	return `rgba(${red}, ${green}, ${blue}, ${opacity})`;
}

function attachmentFailureRequiresRecovery(failure: ProjectedTerminalFailure): boolean {
	return (
		failure.retryable &&
		(failure.code === 'renderer-backpressure' ||
			failure.code === 'host-transport-backpressure' ||
			failure.code === 'host-connection-unavailable')
	);
}

function projectedTerminalFailureMessage(failure: ProjectedTerminalFailure) {
	switch (failure.operation) {
		case 'clipboard':
			return projectedTerminalMessages.clipboardWriteFailed;
		case 'connection':
		case 'protocol':
			return projectedTerminalMessages.connectionFailure;
		case 'control':
			return projectedTerminalMessages.controlFailure;
		case 'copy':
			return projectedTerminalMessages.copyFailed;
		case 'history':
			return projectedTerminalMessages.historyUnavailable;
		case 'input':
			return projectedTerminalMessages.inputFailure;
		case 'link':
			return failure.code === 'link-policy-blocked'
				? projectedTerminalMessages.linkBlocked
				: projectedTerminalMessages.linkFailed;
		case 'resize':
			return projectedTerminalMessages.resizeFailure;
		case 'selection':
			return projectedTerminalMessages.selectionFailure;
		case undefined:
			return projectedTerminalMessages.requestFailure;
	}
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

type ProjectedTerminalActionResultPresentation = {
	readonly message: (typeof projectedTerminalMessages)[keyof typeof projectedTerminalMessages];
	readonly role: 'alert' | 'status';
};

function projectedTerminalActionResultPresentation(
	event: ProjectedTerminalActionResultEvent,
): ProjectedTerminalActionResultPresentation | null {
	if (event.result.kind === 'blocked' && event.result.code === 'action-unavailable') {
		return { message: projectedTerminalMessages.actionUnavailable, role: 'alert' };
	}
	if (
		event.actionId === PROJECTED_TERMINAL_ACTION_IDS.copySelection ||
		event.actionId === PROJECTED_TERMINAL_ACTION_IDS.findOpen ||
		event.actionId === PROJECTED_TERMINAL_ACTION_IDS.findNext ||
		event.actionId === PROJECTED_TERMINAL_ACTION_IDS.findPrevious ||
		event.result.kind === 'cancelled'
	) {
		return null;
	}
	if (event.result.kind === 'completed') {
		switch (event.actionId) {
			case PROJECTED_TERMINAL_ACTION_IDS.clear:
				return { message: projectedTerminalMessages.clearCompleted, role: 'status' };
			case PROJECTED_TERMINAL_ACTION_IDS.openLink:
				return { message: projectedTerminalMessages.openLinkCompleted, role: 'status' };
			case PROJECTED_TERMINAL_ACTION_IDS.paste:
				return { message: projectedTerminalMessages.pasteCompleted, role: 'status' };
			case PROJECTED_TERMINAL_ACTION_IDS.reset:
				return { message: projectedTerminalMessages.resetCompleted, role: 'status' };
			case PROJECTED_TERMINAL_ACTION_IDS.scrollToBottom:
				return { message: projectedTerminalMessages.scrollToBottomCompleted, role: 'status' };
			case PROJECTED_TERMINAL_ACTION_IDS.selectAll:
				return { message: projectedTerminalMessages.selectAllCompleted, role: 'status' };
		}
	}
	if (
		event.result.kind === 'unchanged' &&
		event.actionId === PROJECTED_TERMINAL_ACTION_IDS.scrollToBottom
	) {
		return { message: projectedTerminalMessages.scrollToBottomUnchanged, role: 'status' };
	}
	if (event.actionId === PROJECTED_TERMINAL_ACTION_IDS.openLink) {
		let message = projectedTerminalMessages.linkFailed;
		if (event.result.kind === 'blocked') {
			message =
				event.result.code === 'stale-terminal-link-context'
					? projectedTerminalMessages.linkStale
					: projectedTerminalMessages.linkBlocked;
		}
		return {
			message,
			role: 'alert',
		};
	}
	if (event.actionId === PROJECTED_TERMINAL_ACTION_IDS.paste) {
		return { message: projectedTerminalMessages.pasteFailed, role: 'alert' };
	}
	return event.result.kind === 'unchanged'
		? null
		: { message: projectedTerminalMessages.requestFailure, role: 'alert' };
}
