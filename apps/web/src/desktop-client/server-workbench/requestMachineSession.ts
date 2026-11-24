import {
	DESKTOP_MACHINE_SESSION_WINDOW_REQUEST,
	DesktopMachineSessionWindowMessageSchema,
	type DesktopMachineSessionRefusal,
} from '@orchbee/desktop-client-protocol';

/** How long the page waits for Main's answer, past Main's own 30 seconds for the relay. */
const ANSWER_MILLISECONDS = 40_000;

export type MachineSessionAnswer =
	| {
			readonly capabilities: readonly string[];
			readonly kind: 'admitted';
			readonly port: MessagePort;
	  }
	| { readonly kind: 'refused'; readonly reason: DesktopMachineSessionRefusal };

let lastRequest = 0;

/**
 * Asks Main, through the preload, for a connection to `machine`'s Session: the port of its session
 * relay once the Session admitted it, or why there is none.
 */
export function requestMachineSession(
	targetWindow: Window,
	machine: string,
): Promise<MachineSessionAnswer> {
	lastRequest += 1;
	const request = lastRequest;
	return new Promise((resolve) => {
		const finish = (answer: MachineSessionAnswer): void => {
			clearTimeout(timer);
			targetWindow.removeEventListener('message', handleMessage);
			resolve(answer);
		};
		const handleMessage = (event: MessageEvent<unknown>): void => {
			if (event.source !== targetWindow) return;
			const message = DesktopMachineSessionWindowMessageSchema.safeParse(event.data);
			if (!message.success || message.data.request !== request) return;
			const [port] = event.ports;
			if (message.data.outcome === 'admitted' && port !== undefined) {
				finish({
					capabilities: message.data.bootstrap.negotiatedCapabilities,
					kind: 'admitted',
					port,
				});
				return;
			}
			for (const unused of event.ports) unused.close();
			finish({
				kind: 'refused',
				reason: message.data.outcome === 'refused' ? message.data.reason : 'protocol',
			});
		};
		const timer = setTimeout(
			() => finish({ kind: 'refused', reason: 'timeout' }),
			ANSWER_MILLISECONDS,
		);
		targetWindow.addEventListener('message', handleMessage);
		targetWindow.postMessage(
			{ machine, request, type: DESKTOP_MACHINE_SESSION_WINDOW_REQUEST },
			targetWindow.location.origin,
		);
	});
}
