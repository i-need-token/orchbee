/**
 * One projected terminal on a Server connection, with no Workbench around it. It proves the Client
 * path in a real window until the Workbench moves onto this connection and replaces this page.
 */
import { DesktopServerConnectionWindowMessageSchema } from '@orchbee/desktop-client-protocol';
import type { WorkbenchShortcutPlatform } from '@orchbee/workbench';

import {
	type ServerTerminalTarget,
	ServerConnection,
	createServerTerminal,
	openServerTerminalProjection,
	startProjectedTerminalRenderer,
} from '../../workbench-client';
import { MessagePortServerCarrier } from './MessagePortServerCarrier';

const INITIAL_GEOMETRY = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 120, rows: 30 };

/** The Workbench owns the real take-control command; this key lets the page exercise the path. */
function isTakeControlKey(event: KeyboardEvent): boolean {
	return event.ctrlKey && event.altKey && event.code === 'KeyT';
}

function receiveConnectionPort(targetWindow: Window): Promise<MessagePort> {
	return new Promise((resolve, reject) => {
		const handleMessage = (event: MessageEvent<unknown>): void => {
			const port = event.ports[0];
			if (event.source !== targetWindow || port === undefined) return;
			targetWindow.removeEventListener('message', handleMessage);
			if (DesktopServerConnectionWindowMessageSchema.safeParse(event.data).success) resolve(port);
			else reject(new Error('The Server connection announcement is invalid.'));
		};
		targetWindow.addEventListener('message', handleMessage);
	});
}

/** `new` starts a terminal; otherwise the value names an existing `session.worker.terminal`. */
async function resolveTarget(
	connection: ServerConnection,
	requested: string,
): Promise<ServerTerminalTarget> {
	if (requested === 'new') {
		return {
			kind: 'created',
			terminalCreateRequestId: await createServerTerminal(connection, INITIAL_GEOMETRY),
		};
	}
	const [sessionId, workerId, terminalId] = requested.split('.');
	if (sessionId === undefined || workerId === undefined || terminalId === undefined) {
		throw new Error('The requested terminal must be session.worker.terminal.');
	}
	return { kind: 'binding', sessionId, terminalId, workerId };
}

export async function startServerTerminalPage(
	targetWindow: Window,
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	shortcutPlatform: WorkbenchShortcutPlatform,
	requestedTerminal: string,
): Promise<void> {
	const connection = new ServerConnection(
		new MessagePortServerCarrier(await receiveConnectionPort(targetWindow)),
	);
	targetWindow.addEventListener('unload', () => connection.close(), { once: true });
	const projection = await openServerTerminalProjection(
		connection,
		await resolveTarget(connection, requestedTerminal),
		INITIAL_GEOMETRY,
		// These seed the Host's default colors; the Workbench supplies the real theme.
		{
			colorScheme: 'dark',
			defaultBackgroundRgb: 0x10_10_10,
			defaultCursorRgb: null,
			defaultForegroundRgb: 0xe0_e0_e0,
			palette: Array.from({ length: 256 }, () => 0),
		},
	);
	targetWindow.addEventListener(
		'keydown',
		(event) => {
			if (!(event instanceof KeyboardEvent) || !isTakeControlKey(event)) return;
			event.preventDefault();
			event.stopPropagation();
			void projection.takeControl();
		},
		{ capture: true },
	);
	const session = await startProjectedTerminalRenderer(
		rootElement,
		documentElement,
		preferredLocales,
		shortcutPlatform,
		projection.bootstrap,
		projection.transport,
	);
	session.focus();
}
