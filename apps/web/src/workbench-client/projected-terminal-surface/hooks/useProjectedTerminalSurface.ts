import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { WorkbenchTerminalPresentation } from '@orchbee/workbench';
import * as React from 'react';

import {
	startProjectedTerminalRenderer,
	type ProjectedTerminalRendererSession,
} from '../../projected-terminal-renderer';
import { ProjectedTerminalSurfaceCapabilityContext } from '../contexts/projected-terminal-surface-capability';
import type { ProjectedTerminalSurfaceView } from '../types/projected-terminal-surface-view';

export function useProjectedTerminalSurface(
	presentation: WorkbenchTerminalPresentation,
): ProjectedTerminalSurfaceView {
	const capability = React.useContext(ProjectedTerminalSurfaceCapabilityContext);
	const intl = useOrchbeeIntl();
	const root = React.useRef<HTMLDivElement>(null);
	const [failed, setFailed] = React.useState(false);
	const projectable =
		presentation.lifecycle.kind === 'running' || presentation.lifecycle.kind === 'exited';

	if (capability === null) {
		throw new Error('ProjectedTerminalSurface requires its client capability.');
	}

	React.useEffect(() => {
		if (!projectable) return;
		const projection = capability.projectionClient.open(presentation.presentationId);
		let cancelled = false;
		let session: ProjectedTerminalRendererSession | undefined;
		setFailed(false);
		void projection.result
			.then(async (attachment) => {
				if (cancelled) {
					attachment.transport.close();
					return;
				}
				const rendererSession = await startProjectedTerminalRenderer(
					root.current,
					capability.documentElement,
					capability.preferredLocales,
					capability.shortcutPlatform,
					attachment.bootstrap,
					attachment.transport,
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

	let projectionState: ProjectedTerminalSurfaceView['projectionState'] = 'inactive';
	if (projectable) projectionState = 'requested';
	if (failed) projectionState = 'failed';
	return {
		failureMessage: failed ? intl.formatMessage(projectedTerminalMessages.connectionFailure) : null,
		projectionState,
		root,
		stopped: presentation.lifecycle.kind === 'stopped',
		startNewSession: () => capability.startStoppedTerminalSession(presentation.presentationId),
	};
}
