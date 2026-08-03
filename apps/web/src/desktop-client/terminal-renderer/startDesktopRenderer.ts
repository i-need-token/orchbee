import {
	createOrchbeeIntl,
	desktopTerminalMessages,
	getOrchbeeTextDirection,
	resolveOrchbeeLocale,
} from '@orchbee/i18n';
import { ProjectedTerminal, type ProjectedTerminalOptions } from '@orchbee/terminal';

import {
	DesktopRendererEventSchema,
	type ProjectedAttachmentBootstrap,
} from '@orchbee/desktop-client-protocol';
import {
	DesktopTerminalFindController,
	mountDesktopTerminalFindSurface,
	type DesktopTerminalFindSurfaceMount,
	type MountDesktopTerminalFindSurfaceOptions,
} from '../terminal-find-surface';
import {
	createDesktopTerminalActionComposition,
	DESKTOP_TERMINAL_ACTION_IDS,
	installDesktopTerminalShortcuts,
	type DesktopShortcutPlatform,
} from '../terminal-actions';

type DesktopProjectedTerminal = Pick<
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

export interface DesktopRendererDependencies {
	readonly createTerminal: (options: ProjectedTerminalOptions) => DesktopProjectedTerminal;
	readonly installShortcuts: typeof installDesktopTerminalShortcuts;
	readonly mountFindSurface: (
		options: MountDesktopTerminalFindSurfaceOptions,
	) => DesktopTerminalFindSurfaceMount;
}

export interface DesktopRendererSession {
	dispose(): void;
}

const defaultDependencies: DesktopRendererDependencies = {
	createTerminal: (options) => new ProjectedTerminal(options),
	installShortcuts: installDesktopTerminalShortcuts,
	mountFindSurface: mountDesktopTerminalFindSurface,
};

export async function startDesktopRenderer(
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	shortcutPlatform: DesktopShortcutPlatform,
	bootstrap: ProjectedAttachmentBootstrap,
	port: MessagePort,
	dependencies: DesktopRendererDependencies = defaultDependencies,
): Promise<DesktopRendererSession> {
	if (!(rootElement instanceof HTMLElement)) {
		throw new Error('The Orchbee Desktop terminal root element is missing.');
	}
	const locale = resolveOrchbeeLocale(preferredLocales);
	const intl = createOrchbeeIntl(locale);
	documentElement.lang = locale;
	documentElement.dir = getOrchbeeTextDirection(locale);
	const status = rootElement.ownerDocument.createElement('div');
	status.hidden = true;
	status.setAttribute('role', 'alert');
	rootElement.appendChild(status);
	rootElement.dataset['desktopTerminalState'] = 'opening';
	rootElement.dataset['desktopTerminalAttachmentId'] = bootstrap.attachmentId;
	rootElement.dataset['desktopTerminalCreationId'] = bootstrap.creationId;
	rootElement.dataset['desktopTerminalHostEpoch'] = bootstrap.hostEpoch;
	rootElement.dataset['desktopTerminalId'] = bootstrap.terminalId;
	let committedFrames = 0;
	let finalRevision: bigint | undefined;
	let lastRenderedRevision: bigint | undefined;
	let renderedFrames = 0;
	const updateFinalPresentation = (): void => {
		if (
			finalRevision !== undefined &&
			lastRenderedRevision !== undefined &&
			lastRenderedRevision >= finalRevision
		) {
			rootElement.dataset['desktopTerminalFinalFramePresented'] = 'true';
		}
	};

	const reportFailure = (): void => {
		rootElement.dataset['desktopTerminalState'] = 'failed';
		status.textContent = intl.formatMessage(desktopTerminalMessages.connectionFailure);
		status.hidden = false;
	};
	const terminal = dependencies.createTerminal({
		attachmentId: bootstrap.attachmentId,
		canResize: bootstrap.canResize,
		creationId: bootstrap.creationId,
		hostEpoch: bootstrap.hostEpoch,
		inputEnabled: bootstrap.canInput !== false,
		labels: {
			historyRange: (start, end, total) =>
				intl.formatMessage(desktopTerminalMessages.historyRange, {
					end: end.toString(),
					start: start.toString(),
					total: total.toString(),
				}),
			input: intl.formatMessage(desktopTerminalMessages.input),
			screen: intl.formatMessage(desktopTerminalMessages.screen),
		},
		port: {
			send: (command) => port.postMessage(command),
		},
		renderer: {
			fontFamily: '"Menlo", monospace',
			fontSize: 15,
		},
		terminalId: bootstrap.terminalId,
	});
	const findController = new DesktopTerminalFindController(
		terminal,
		terminal.getBufferRowsSnapshot().totalRows,
	);
	const actions = createDesktopTerminalActionComposition(terminal, findController);
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
			rootElement.dataset['desktopTerminalState'] = 'exited';
			rootElement.dataset['desktopTerminalFinalRevision'] = event.finalRevision;
			finalRevision = BigInt(event.finalRevision);
			updateFinalPresentation();
		}),
		terminal.onFrameTelemetry((event) => {
			renderedFrames += 1;
			lastRenderedRevision = BigInt(event.revision);
			rootElement.dataset['desktopTerminalRenderedFrames'] = String(renderedFrames);
			rootElement.dataset['desktopTerminalRenderedAtMilliseconds'] = String(performance.now());
			rootElement.dataset['desktopTerminalRenderedRevision'] = event.revision;
			rootElement.dataset['desktopTerminalRenderer'] = event.backend;
			updateFinalPresentation();
		}),
		terminal.onHistoryChange((event) => findController.applyHistoryEvent(event)),
		terminal.onLinkOpenRequest((event) => {
			void actions.execute(
				DESKTOP_TERMINAL_ACTION_IDS.openLink,
				{ target: event.target },
				actions.terminalScope,
				'pointer',
			).completion;
		}),
		terminal.onMetadataChange((metadata) => {
			rootElement.dataset['desktopTerminalCellPixelHeight'] = String(metadata.cellPixelHeight);
			rootElement.dataset['desktopTerminalCellPixelWidth'] = String(metadata.cellPixelWidth);
			rootElement.dataset['desktopTerminalColumns'] = String(metadata.columns);
			rootElement.dataset['desktopTerminalRows'] = String(metadata.rows);
		}),
		terminal.onRendererHealth((event) => {
			rootElement.dataset['desktopTerminalRenderer'] = event.backend;
			rootElement.dataset['desktopTerminalRendererHealth'] = event.state;
		}),
		terminal.onSearchResults((event) => {
			findController.applySearchEvent(event);
			if (event.state === 'error' && event.regexError !== null) {
				rootElement.dataset['desktopTerminalSearchError'] = event.regexError;
			} else {
				delete rootElement.dataset['desktopTerminalSearchError'];
			}
			if (event.state === 'complete') {
				rootElement.dataset['desktopTerminalSearchCompletedAtMilliseconds'] = String(
					performance.now(),
				);
			}
		}),
		terminal.onSelectionChange((event) => {
			findController.applySelectionEvent(event);
			rootElement.dataset['desktopTerminalSelectionState'] = event.state;
			if (event.state === 'active') {
				rootElement.dataset['desktopTerminalSelectionStart'] =
					`${event.start.row}:${event.start.column}`;
				rootElement.dataset['desktopTerminalSelectionEnd'] = `${event.end.row}:${event.end.column}`;
			} else {
				delete rootElement.dataset['desktopTerminalSelectionStart'];
				delete rootElement.dataset['desktopTerminalSelectionEnd'];
			}
		}),
		terminal.onSelectionCopied((event) => {
			rootElement.dataset['desktopTerminalCopiedBytes'] = String(event.byteLength);
			rootElement.dataset['desktopTerminalCopiedAtMilliseconds'] = String(performance.now());
		}),
	];
	const handleMessage = (messageEvent: MessageEvent<unknown>): void => {
		const event = DesktopRendererEventSchema.safeParse(messageEvent.data);
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
					rootElement.dataset['desktopTerminalCommittedFrames'] = String(committedFrames);
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
			case 'selection-copied':
				terminal.notifySelectionCopied(event.data.byteLength, event.data.requestSequence);
				return;
			case 'terminal-exited':
				terminal.notifyExited(event.data.event);
				return;
		}
	};
	const handleMessageError = (): void => reportFailure();
	port.addEventListener('message', handleMessage);
	port.addEventListener('messageerror', handleMessageError);
	port.start();

	const findRoot = rootElement.ownerDocument.createElement('div');
	findRoot.dataset['desktopTerminalFindSurface'] = 'true';
	let findSurface: DesktopTerminalFindSurfaceMount | undefined;
	let disposeShortcuts: (() => void) | undefined;
	try {
		await terminal.open(rootElement);
		if (bootstrap.terminalExit !== null && bootstrap.terminalExit !== undefined) {
			terminal.notifyExited(bootstrap.terminalExit);
		}
		rootElement.appendChild(findRoot);
		findSurface = dependencies.mountFindSurface({
			controller: findController,
			locale,
			onNext: () => {
				void actions.execute(
					DESKTOP_TERMINAL_ACTION_IDS.findNext,
					undefined,
					actions.findScope,
					'button',
				).completion;
			},
			onPrevious: () => {
				void actions.execute(
					DESKTOP_TERMINAL_ACTION_IDS.findPrevious,
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
		rootElement.dataset['desktopTerminalState'] = 'open';
		terminal.focus();
	} catch (error) {
		reportFailure();
		disposeShortcuts?.();
		findSurface?.dispose();
		findRoot.remove();
		port.removeEventListener('message', handleMessage);
		port.removeEventListener('messageerror', handleMessageError);
		for (const subscription of subscriptions) subscription.dispose();
		actions.dispose();
		findController.dispose();
		terminal.dispose();
		port.close();
		throw error;
	}

	return {
		dispose: () => {
			disposeShortcuts?.();
			findSurface?.dispose();
			port.removeEventListener('message', handleMessage);
			port.removeEventListener('messageerror', handleMessageError);
			for (const subscription of subscriptions) subscription.dispose();
			actions.dispose();
			findController.dispose();
			terminal.dispose();
			port.close();
			status.remove();
			findRoot.remove();
		},
	};
}
