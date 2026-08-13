import { useMemoizedFn } from '@orchbee/foundation/react';
import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ProjectedPasteReview } from '@orchbee/terminal';
import type { WorkbenchTerminalPresentation } from '@orchbee/workbench';
import * as React from 'react';

import {
	startProjectedTerminalRenderer,
	type ProjectedTerminalRendererSession,
} from '../../projected-terminal-renderer';
import { ProjectedTerminalSurfaceCapabilityContext } from '../contexts/projected-terminal-surface-capability';
import type { ProjectedTerminalSurfaceView } from '../types/projected-terminal-surface-view';

interface PendingPasteDecision {
	readonly abort: () => void;
	readonly resolve: (approved: boolean) => void;
	readonly signal: AbortSignal;
}

export function useProjectedTerminalSurface(
	presentation: WorkbenchTerminalPresentation,
): ProjectedTerminalSurfaceView {
	const capability = React.useContext(ProjectedTerminalSurfaceCapabilityContext);
	const intl = useOrchbeeIntl();
	const root = React.useRef<HTMLDivElement>(null);
	const [failed, setFailed] = React.useState(false);
	const [pasteReview, setPasteReview] = React.useState<ProjectedPasteReview | null>(null);
	const pendingPasteDecision = React.useRef<PendingPasteDecision | null>(null);
	const [projectionGeneration, setProjectionGeneration] = React.useState(0);
	const projectable =
		presentation.lifecycle.kind === 'running' ||
		presentation.lifecycle.kind === 'exited' ||
		presentation.lifecycle.kind === 'failed';

	const settlePasteReview = useMemoizedFn((approved: boolean): void => {
		const pending = pendingPasteDecision.current;
		if (pending === null) return;
		pendingPasteDecision.current = null;
		pending.signal.removeEventListener('abort', pending.abort);
		setPasteReview(null);
		pending.resolve(approved);
	});
	const reviewPaste = useMemoizedFn(
		(review: ProjectedPasteReview, signal: AbortSignal): Promise<boolean> => {
			settlePasteReview(false);
			if (signal.aborted) return Promise.resolve(false);
			return new Promise<boolean>((resolve) => {
				const pending: PendingPasteDecision = {
					abort: () => {
						if (pendingPasteDecision.current === pending) settlePasteReview(false);
					},
					resolve,
					signal,
				};
				pendingPasteDecision.current = pending;
				setPasteReview(review);
				signal.addEventListener('abort', pending.abort, { once: true });
				if (signal.aborted) pending.abort();
			});
		},
	);
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
					undefined,
					capability.platformEffects,
					capability.presentationConfigurationSource,
					reviewPaste,
					(metadata) => capability.observeTerminalMetadata(presentation.presentationId, metadata),
				);
				if (cancelled) rendererSession.dispose();
				else {
					session = rendererSession;
					void rendererSession.transportUnavailable.then(() => {
						if (!cancelled) setProjectionGeneration((generation) => generation + 1);
					});
				}
			})
			.catch(() => {
				if (!cancelled) setFailed(true);
			});
		return () => {
			cancelled = true;
			settlePasteReview(false);
			session?.dispose();
			projection.release();
		};
	}, [
		capability,
		presentation.presentationId,
		projectable,
		projectionGeneration,
		reviewPaste,
		settlePasteReview,
	]);

	let projectionState: ProjectedTerminalSurfaceView['projectionState'] = 'inactive';
	if (projectable) projectionState = 'requested';
	if (failed) projectionState = 'failed';
	return {
		approvePaste: () => settlePasteReview(true),
		cancelPaste: () => settlePasteReview(false),
		failureMessage: failed ? intl.formatMessage(projectedTerminalMessages.connectionFailure) : null,
		pasteReview,
		projectionState,
		root,
		stopped: presentation.lifecycle.kind === 'stopped',
		startNewSession: () => capability.startStoppedTerminalSession(presentation.presentationId),
	};
}
