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
import { DesktopWorkbench } from '../desktop-workbench';
import { DesktopTerminalDocumentClient } from '../terminal-document-control';
import { DesktopTerminalProjectionClient } from '../terminal-projection-control';

export interface DesktopWorkbenchRendererSession {
	dispose(): void;
}

export function startDesktopWorkbenchRenderer(
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	shortcutPlatform: WorkbenchShortcutPlatform,
	storage: Pick<Storage, 'getItem' | 'setItem'>,
	bootstrap: TerminalDocumentControlBootstrap,
	documentPort: MessagePort,
	projectionPort: MessagePort,
): DesktopWorkbenchRendererSession {
	if (!(rootElement instanceof HTMLElement)) {
		throw new Error('The Orchbee Desktop application root element is missing.');
	}
	const locale = resolveOrchbeeLocale(preferredLocales);
	const intl = createOrchbeeIntl(locale);
	documentElement.lang = locale;
	documentElement.dir = getOrchbeeTextDirection(locale);
	const documentClient = new DesktopTerminalDocumentClient(documentPort);
	const projectionClient = new DesktopTerminalProjectionClient(projectionPort);
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
			documentClient.dispatch(terminalCommands);
		},
		initialPresentationId: bootstrap.initialPresentationId,
		kind: 'managed',
	};
	const recoveredPresentations = bootstrap.presentations.map((presentation, index) => ({
		cwd: presentation.launchCwd ?? bootstrap.defaultCwd,
		presentationId: presentation.presentationId,
		title: intl.formatMessage(workbenchTabMessages.terminalTitle, { number: index + 1 }),
	}));
	const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: bootstrap.defaultCwd,
		...(recoveredPresentations.length === 0
			? {}
			: { standalonePresentations: recoveredPresentations }),
		standaloneTerminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle, { number: 1 }),
		terminalPresentation,
	});
	const unsubscribeDocument = documentClient.subscribe((event) => {
		projectWorkspaceRuntime.applyTerminalPresentationEvent(event);
	});
	const actionRuntime = new ActionRuntime();
	const reactRoot: Root = createRoot(rootElement);
	reactRoot.render(
		createElement(DesktopWorkbench, {
			actionRuntime,
			layoutStorage: storage,
			locale,
			projectWorkspaceRuntime,
			shortcutPlatform,
			terminalSurfaceCapability: {
				documentElement,
				preferredLocales,
				projectionClient,
				shortcutPlatform,
			},
		}),
	);
	return {
		dispose: () => {
			unsubscribeDocument();
			reactRoot.unmount();
			actionRuntime.dispose();
			projectionClient.dispose();
			documentClient.dispose();
		},
	};
}
