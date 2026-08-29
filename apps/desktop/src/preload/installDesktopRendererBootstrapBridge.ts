import {
	DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
	DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE,
	DESKTOP_RENDERER_PORT_ROLES,
	DesktopRendererBootstrapSchema,
	DesktopRendererBootstrapWindowMessageSchema,
} from '@orchbee/desktop-client-protocol';
import type { IpcRendererEvent } from 'electron';

export interface DesktopRendererBootstrapIpc {
	once(channel: string, listener: (event: IpcRendererEvent, value: unknown) => void): void;
}

export interface DesktopRendererBootstrapWindow {
	readonly location: Pick<Location, 'origin'>;
	postMessage(message: unknown, targetOrigin: string, transfer: Transferable[]): void;
}

export function installDesktopRendererBootstrapBridge(
	ipc: DesktopRendererBootstrapIpc,
	targetWindow: DesktopRendererBootstrapWindow,
	security: { readonly contextIsolated: boolean; readonly sandboxed: boolean },
): void {
	ipc.once(DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL, (event, value) => {
		const bootstrap = DesktopRendererBootstrapSchema.safeParse(value);
		if (!bootstrap.success || event.ports.length !== DESKTOP_RENDERER_PORT_ROLES.length) {
			closeTransferredPorts(event.ports);
			return;
		}
		const clientSessionPort = event.ports[0];
		const terminalProjectionPort = event.ports[1];
		const agentObservationPort = event.ports[2];
		const agentIntegrationManagementPort = event.ports[3];
		const applicationLifetimePort = event.ports[4];
		const browserAccessPort = event.ports[5];
		const projectFolderAcquisitionPort = event.ports[6];
		const projectWorktreePort = event.ports[7];
		const terminalLaunchProfilesPort = event.ports[8];
		const terminalDefaultSettingsPort = event.ports[9];
		const clipboardReadPort = event.ports[10];
		const windowLayoutPort = event.ports[11];
		const remoteTerminalCreationPort = event.ports[12];
		const sshProjectEnrollmentPort = event.ports[13];
		if (
			clientSessionPort === undefined ||
			terminalProjectionPort === undefined ||
			agentObservationPort === undefined ||
			agentIntegrationManagementPort === undefined ||
			applicationLifetimePort === undefined ||
			browserAccessPort === undefined ||
			projectFolderAcquisitionPort === undefined ||
			projectWorktreePort === undefined ||
			terminalLaunchProfilesPort === undefined ||
			terminalDefaultSettingsPort === undefined ||
			clipboardReadPort === undefined ||
			windowLayoutPort === undefined ||
			remoteTerminalCreationPort === undefined ||
			sshProjectEnrollmentPort === undefined
		) {
			closeTransferredPorts(event.ports);
			return;
		}
		try {
			targetWindow.postMessage(
				DesktopRendererBootstrapWindowMessageSchema.parse({
					bootstrap: bootstrap.data,
					security,
					type: DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE,
				}),
				targetWindow.location.origin === 'null' ? '*' : targetWindow.location.origin,
				[
					clientSessionPort,
					terminalProjectionPort,
					agentObservationPort,
					agentIntegrationManagementPort,
					applicationLifetimePort,
					browserAccessPort,
					projectFolderAcquisitionPort,
					projectWorktreePort,
					terminalLaunchProfilesPort,
					terminalDefaultSettingsPort,
					clipboardReadPort,
					windowLayoutPort,
					remoteTerminalCreationPort,
					sshProjectEnrollmentPort,
				],
			);
		} catch {
			closeTransferredPorts(event.ports);
		}
	});
}

function closeTransferredPorts(ports: readonly MessagePort[]): void {
	for (const port of ports) port.close();
}
