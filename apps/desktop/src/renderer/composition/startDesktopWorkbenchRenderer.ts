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
	type TerminalPresentationCommand,
	type WorkbenchTerminalPresentationComposition,
	type WorkbenchShortcutPlatform,
} from '@orchbee/workbench';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import type { TerminalDocumentControlBootstrap } from '../../shared';
import {
	DesktopApplicationLifetimeClient,
	DesktopApplicationLifetimeCoordinator,
} from '../desktop-application-lifetime-dialog';
import { DesktopWorkbench } from '../desktop-workbench';
import { DesktopTerminalDocumentClient } from '../terminal-document-control';
import { DesktopTerminalProjectionClient } from '../terminal-projection-control';
import {
	DesktopWorkbenchDocumentClient,
	DesktopWorkbenchDocumentCoordinator,
} from '../workbench-document-control';

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
	bootstrap: TerminalDocumentControlBootstrap,
	lifetimePort: MessagePort,
	documentPort: MessagePort,
	projectionPort: MessagePort,
	workbenchDocumentPort: MessagePort,
): DesktopWorkbenchRendererSession {
	if (!(rootElement instanceof HTMLElement)) {
		throw new Error('The Orchbee Desktop application root element is missing.');
	}
	const applicationRoot = rootElement;
	const locale = resolveOrchbeeLocale(preferredLocales);
	const intl = createOrchbeeIntl(locale);
	documentElement.lang = locale;
	documentElement.dir = getOrchbeeTextDirection(locale);
	const documentClient = new DesktopTerminalDocumentClient(documentPort);
	const projectionClient = new DesktopTerminalProjectionClient(projectionPort);
	const workbenchDocumentClient = new DesktopWorkbenchDocumentClient(workbenchDocumentPort);
	let actionRuntime: ActionRuntime | undefined;
	let applicationLifetimeCoordinator: DesktopApplicationLifetimeCoordinator | undefined;
	let disposed = false;
	let lifetimeClient: DesktopApplicationLifetimeClient | undefined;
	let reactRoot: Root | undefined;
	let workbenchDocumentCoordinator: DesktopWorkbenchDocumentCoordinator | undefined;
	let unsubscribeDocument: (() => void) | undefined;
	const pendingCommands: TerminalPresentationCommand[] = [];
	const terminalPresentation: WorkbenchTerminalPresentationComposition = {
		createOperationId: () =>
			terminalPresentationOperationId(`desktop:operation:${crypto.randomUUID()}`),
		createPresentationId: () =>
			terminalPresentationId(`desktop:presentation:${crypto.randomUUID()}`),
		createRemovalId: () => terminalPresentationRemovalId(`desktop:removal:${crypto.randomUUID()}`),
		dispatchCommands: (commands) => {
			const terminalCommands: TerminalPresentationCommand[] = [];
			for (const command of commands) {
				if (command.type !== 'tab-close-ready') terminalCommands.push(command);
			}
			if (workbenchDocumentCoordinator === undefined) {
				pendingCommands.push(...terminalCommands);
				return;
			}
			workbenchDocumentCoordinator.dispatchCommands(terminalCommands);
		},
		initialPresentationId: bootstrap.initialPresentationId,
		kind: 'managed',
	};

	const dispose = (): void => {
		if (disposed) return;
		disposed = true;
		unsubscribeDocument?.();
		unsubscribeDocument = undefined;
		applicationLifetimeCoordinator?.dispose();
		applicationLifetimeCoordinator = undefined;
		reactRoot?.unmount();
		reactRoot = undefined;
		actionRuntime?.dispose();
		actionRuntime = undefined;
		workbenchDocumentCoordinator?.dispose();
		workbenchDocumentCoordinator = undefined;
		projectionClient.dispose();
		documentClient.dispose();
		workbenchDocumentClient.dispose();
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
		const loaded = await workbenchDocumentClient.load();
		if (loaded.kind === 'unavailable') {
			dispose();
			return loaded;
		}
		if (disposed) return { kind: 'unavailable', reason: 'control-unavailable' };
		const recoveredPresentations = bootstrap.presentations.map((presentation, index) => ({
			cwd: presentation.launchCwd ?? bootstrap.defaultCwd,
			presentationId: presentation.presentationId,
			title: intl.formatMessage(workbenchTabMessages.terminalTitle, { number: index + 1 }),
		}));
		const runtimeOptions = {
			standaloneCwd: bootstrap.defaultCwd,
			standaloneTerminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle, {
				number: 1,
			}),
			terminalPresentation,
		};
		let runtime: WorkbenchProjectWorkspaceRuntime;
		if (loaded.kind === 'available') {
			runtime = new WorkbenchProjectWorkspaceRuntime({
				...runtimeOptions,
				initialDocument: loaded.document,
			});
		} else if (recoveredPresentations.length === 0) {
			runtime = new WorkbenchProjectWorkspaceRuntime(runtimeOptions);
		} else {
			runtime = new WorkbenchProjectWorkspaceRuntime({
				...runtimeOptions,
				standalonePresentations: recoveredPresentations,
			});
		}
		unsubscribeDocument = documentClient.subscribe((event) => {
			runtime.applyTerminalPresentationEvent(event);
		});
		workbenchDocumentCoordinator = new DesktopWorkbenchDocumentCoordinator(
			runtime,
			workbenchDocumentClient,
			documentClient,
		);
		const documentStart = await workbenchDocumentCoordinator.start(
			loaded,
			pendingCommands.splice(0),
		);
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
		if (loaded.kind === 'available') runtime.reconcileDurableTerminalPresentations();
		return { kind: 'ready' };
	}
}
