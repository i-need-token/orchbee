import { ActionRuntime } from '@orchbee/actions';
import {
	createOrchbeeIntl,
	getOrchbeeTextDirection,
	resolveOrchbeeLocale,
	workbenchTabMessages,
} from '@orchbee/i18n';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	WorkbenchProjectWorkspaceRuntime,
	type WorkbenchTerminalPresentationComposition,
	type WorkbenchShortcutPlatform,
} from '@orchbee/workbench';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import type { DesktopClientSessionBootstrap } from '@orchbee/desktop-client-protocol';
import { DesktopClientSessionClient } from '../client-session';
import {
	DesktopApplicationLifetimeClient,
	DesktopApplicationLifetimeCoordinator,
} from '../desktop-application-lifetime-dialog';
import { DesktopWorkbench } from '../desktop-workbench';
import { DesktopTerminalProjectionClient } from '../terminal-projection-control';
import { DesktopWorkbenchDocumentCoordinator } from '../workbench-document-control';

export interface DesktopWorkbenchRendererSession {
	dispose(): void;
	readonly ready: Promise<
		{ readonly kind: 'ready' } | { readonly kind: 'unavailable'; readonly reason: string }
	>;
}

export function startDesktopWorkbenchRenderer(
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	shortcutPlatform: WorkbenchShortcutPlatform,
	storage: Pick<Storage, 'getItem' | 'setItem'>,
	bootstrap: DesktopClientSessionBootstrap,
	lifetimePort: MessagePort,
	clientSessionPort: MessagePort,
	projectionPort: MessagePort,
): DesktopWorkbenchRendererSession {
	if (!(rootElement instanceof HTMLElement)) {
		throw new Error('The Orchbee Desktop application root element is missing.');
	}
	const applicationRoot = rootElement;
	const locale = resolveOrchbeeLocale(preferredLocales);
	const intl = createOrchbeeIntl(locale);
	documentElement.lang = locale;
	documentElement.dir = getOrchbeeTextDirection(locale);
	const clientSession = new DesktopClientSessionClient(clientSessionPort, bootstrap);
	const projectionClient = new DesktopTerminalProjectionClient(projectionPort);
	let actionRuntime: ActionRuntime | undefined;
	let applicationLifetimeCoordinator: DesktopApplicationLifetimeCoordinator | undefined;
	let disposed = false;
	let lifetimeClient: DesktopApplicationLifetimeClient | undefined;
	let reactRoot: Root | undefined;
	let workbenchDocumentCoordinator: DesktopWorkbenchDocumentCoordinator | undefined;
	const terminalPresentation: WorkbenchTerminalPresentationComposition = {
		createOperationId: () =>
			terminalPresentationOperationId(`desktop:operation:${crypto.randomUUID()}`),
		createPresentationId: () =>
			terminalPresentationId(`desktop:presentation:${crypto.randomUUID()}`),
		createRemovalId: () => terminalPresentationRemovalId(`desktop:removal:${crypto.randomUUID()}`),
		dispatchCommands: () => undefined,
		initialPresentationId: bootstrap.initialPresentationId,
		kind: 'managed',
	};

	const dispose = (): void => {
		if (disposed) return;
		disposed = true;
		applicationLifetimeCoordinator?.dispose();
		applicationLifetimeCoordinator = undefined;
		reactRoot?.unmount();
		reactRoot = undefined;
		actionRuntime?.dispose();
		actionRuntime = undefined;
		workbenchDocumentCoordinator?.dispose();
		workbenchDocumentCoordinator = undefined;
		clientSession.dispose();
		projectionClient.dispose();
		if (lifetimeClient === undefined) lifetimePort.close();
		else lifetimeClient.dispose();
	};

	const ready = initialize().catch(() => {
		dispose();
		return { kind: 'unavailable', reason: 'runtime-unavailable' } as const;
	});
	return { dispose, ready };

	async function initialize(): Promise<
		{ readonly kind: 'ready' } | { readonly kind: 'unavailable'; readonly reason: string }
	> {
		const runtimeOptions = {
			standaloneCwd: bootstrap.defaultCwd,
			standaloneTerminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle, {
				number: 1,
			}),
			terminalPresentation,
		};
		const runtime = new WorkbenchProjectWorkspaceRuntime({
			...runtimeOptions,
			initialDocument:
				bootstrap.documentState.kind === 'available'
					? bootstrap.documentState.document
					: bootstrap.initializationDocument,
		});
		workbenchDocumentCoordinator = new DesktopWorkbenchDocumentCoordinator(runtime, clientSession);
		const documentStart = await workbenchDocumentCoordinator.start();
		if (documentStart.kind === 'unavailable') {
			dispose();
			return documentStart;
		}
		if (disposed) return { kind: 'unavailable', reason: 'control-unavailable' };
		lifetimeClient = new DesktopApplicationLifetimeClient(lifetimePort);
		applicationLifetimeCoordinator = new DesktopApplicationLifetimeCoordinator(
			runtime,
			workbenchDocumentCoordinator,
			lifetimeClient,
		);
		actionRuntime = new ActionRuntime();
		reactRoot = createRoot(applicationRoot);
		reactRoot.render(
			createElement(DesktopWorkbench, {
				actionRuntime,
				applicationLifetimeCoordinator,
				layoutStorage: storage,
				locale,
				projectWorkspaceRuntime: runtime,
				shortcutPlatform,
				terminalSurfaceCapability: {
					documentElement,
					preferredLocales,
					projectionClient,
					shortcutPlatform,
					startStoppedTerminalSession: (presentationId) => {
						runtime.startStoppedTerminalSession(presentationId);
					},
				},
			}),
		);
		return { kind: 'ready' };
	}
}
