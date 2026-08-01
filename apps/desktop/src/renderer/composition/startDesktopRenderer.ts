import {
	createOrchbeeIntl,
	desktopTerminalMessages,
	getOrchbeeTextDirection,
	resolveOrchbeeLocale,
} from '@orchbee/i18n';
import { ProjectedTerminal, type ProjectedTerminalOptions } from '@orchbee/terminal';

import { DesktopRendererEventSchema, type ProjectedAttachmentBootstrap } from '../../shared';

type DesktopProjectedTerminal = Pick<
	ProjectedTerminal,
	| 'applyFrame'
	| 'dispose'
	| 'focus'
	| 'notifyExited'
	| 'onError'
	| 'onExit'
	| 'onFrameTelemetry'
	| 'onMetadataChange'
	| 'onRendererHealth'
	| 'open'
>;

export interface DesktopRendererDependencies {
	readonly createTerminal: (options: ProjectedTerminalOptions) => DesktopProjectedTerminal;
}

export interface DesktopRendererSession {
	dispose(): void;
}

const defaultDependencies: DesktopRendererDependencies = {
	createTerminal: (options) => new ProjectedTerminal(options),
};

export async function startDesktopRenderer(
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dir' | 'lang'>,
	preferredLocales: readonly string[],
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
		labels: {
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
	const subscriptions = [
		terminal.onError(reportFailure),
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
			rootElement.dataset['desktopTerminalRenderedRevision'] = event.revision;
			rootElement.dataset['desktopTerminalRenderer'] = event.backend;
			updateFinalPresentation();
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
	];
	const handleMessage = (messageEvent: MessageEvent<unknown>): void => {
		const event = DesktopRendererEventSchema.safeParse(messageEvent.data);
		if (!event.success) {
			reportFailure();
			return;
		}
		switch (event.data.kind) {
			case 'failure':
				reportFailure();
				return;
			case 'frame': {
				const result = terminal.applyFrame(event.data.frame);
				if (result === 'committed' || result === 'duplicate') {
					committedFrames += 1;
					rootElement.dataset['desktopTerminalCommittedFrames'] = String(committedFrames);
				}
				return;
			}
			case 'terminal-exited':
				terminal.notifyExited(event.data.event);
				return;
		}
	};
	const handleMessageError = (): void => reportFailure();
	port.addEventListener('message', handleMessage);
	port.addEventListener('messageerror', handleMessageError);
	port.start();

	try {
		await terminal.open(rootElement);
		rootElement.dataset['desktopTerminalState'] = 'open';
		terminal.focus();
	} catch (error) {
		reportFailure();
		throw error;
	}

	return {
		dispose: () => {
			port.removeEventListener('message', handleMessage);
			port.removeEventListener('messageerror', handleMessageError);
			for (const subscription of subscriptions) subscription.dispose();
			terminal.dispose();
			port.close();
			status.remove();
		},
	};
}
