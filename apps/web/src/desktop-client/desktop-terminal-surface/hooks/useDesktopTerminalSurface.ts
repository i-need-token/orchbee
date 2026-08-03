import { desktopTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { WorkbenchTerminalPresentation } from '@orchbee/workbench';
import * as React from 'react';

import { startDesktopRenderer, type DesktopRendererSession } from '../../terminal-renderer';
import { DesktopTerminalSurfaceCapabilityContext } from '../contexts/desktop-terminal-surface-capability';
import type { DesktopTerminalSurfaceView } from '../types/desktop-terminal-surface-view';

export function useDesktopTerminalSurface(
	presentation: WorkbenchTerminalPresentation,
): DesktopTerminalSurfaceView {
	const capability = React.useContext(DesktopTerminalSurfaceCapabilityContext);
	const intl = useOrchbeeIntl();
	const root = React.useRef<HTMLDivElement>(null);
	const [failed, setFailed] = React.useState(false);
	const projectable =
		presentation.lifecycle.kind === 'running' || presentation.lifecycle.kind === 'exited';

	if (capability === null) {
		throw new Error('DesktopTerminalSurface requires its Desktop capability.');
	}

	React.useEffect(() => {
		if (!projectable) return;
		const projection = capability.projectionClient.open(presentation.presentationId);
		let cancelled = false;
		let session: DesktopRendererSession | undefined;
		setFailed(false);
		void projection.result
			.then(async (attachment) => {
				if (cancelled) {
					attachment.port.close();
					return;
				}
				const rendererSession = await startDesktopRenderer(
					root.current,
					capability.documentElement,
					capability.preferredLocales,
					capability.shortcutPlatform,
					attachment.bootstrap,
					attachment.port,
				);
				if (cancelled) rendererSession.dispose();
				else session = rendererSession;
			})
			.catch(() => {
				if (!cancelled) setFailed(true);
			});
		return () => {
			cancelled = true;
			session?.dispose();
			projection.release();
		};
	}, [capability, presentation.presentationId, projectable]);

	let projectionState: DesktopTerminalSurfaceView['projectionState'] = 'inactive';
	if (projectable) projectionState = 'requested';
	if (failed) projectionState = 'failed';
	return {
		failureMessage: failed ? intl.formatMessage(desktopTerminalMessages.connectionFailure) : null,
		projectionState,
		root,
		stopped: presentation.lifecycle.kind === 'stopped',
		startNewSession: () => capability.startStoppedTerminalSession(presentation.presentationId),
	};
}
