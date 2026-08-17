import { useMemoizedFn } from '@orchbee/foundation/react';
import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ProjectedPasteReview } from '@orchbee/terminal';
import type { WorkbenchTerminalPresentation } from '@orchbee/workbench';
import * as React from 'react';

import { startProjectedTerminalRenderer } from '../../projected-terminal-renderer';
import { ProjectedTerminalSurfaceCapabilityContext } from '../contexts/projected-terminal-surface-capability';
import {
	ProjectedTerminalRecoveryController,
	type ProjectedTerminalRecoveryState,
} from '../model/ProjectedTerminalRecoveryController';
import type { ProjectedTerminalSurfaceView } from '../types/projected-terminal-surface-view';

interface PendingPasteDecision {
	readonly abort: () => void;
	readonly resolve: (approved: boolean) => void;
	readonly signal: AbortSignal;
}

interface PresentationRecoveryState {
	readonly presentationId: string;
	readonly state: ProjectedTerminalRecoveryState;
}

export function useProjectedTerminalSurface(
	presentation: WorkbenchTerminalPresentation,
): ProjectedTerminalSurfaceView {
	const capability = React.useContext(ProjectedTerminalSurfaceCapabilityContext);
	const intl = useOrchbeeIntl();
	const root = React.useRef<HTMLDivElement>(null);
	const recoveryController = React.useRef<{
		readonly controller: ProjectedTerminalRecoveryController;
		readonly presentationId: string;
	} | null>(null);
	const [presentationRecovery, setPresentationRecovery] =
		React.useState<PresentationRecoveryState | null>(null);
	const [startFailurePresentationId, setStartFailurePresentationId] = React.useState<string | null>(
		null,
	);
	const [pasteReview, setPasteReview] = React.useState<ProjectedPasteReview | null>(null);
	const pendingPasteDecision = React.useRef<PendingPasteDecision | null>(null);
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
	const startNewSession = useMemoizedFn((): void => {
		try {
			const result = capability?.startStoppedTerminalSession(presentation.presentationId);
			setStartFailurePresentationId(
				result?.kind === 'rejected' ? presentation.presentationId : null,
			);
		} catch {
			setStartFailurePresentationId(presentation.presentationId);
		}
	});
	if (capability === null) {
		throw new Error('ProjectedTerminalSurface requires its client capability.');
	}

	React.useEffect(() => {
		if (!projectable) return;
		let cancelled = false;
		const controller = new ProjectedTerminalRecoveryController({
			onStateChange: (state) => {
				if (!cancelled) {
					setPresentationRecovery({
						presentationId: presentation.presentationId,
						state,
					});
				}
			},
			open: () => capability.projectionClient.open(presentation.presentationId),
			startRenderer: (attachment) =>
				startProjectedTerminalRenderer(
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
					capability.shortcutBindingSource,
				),
		});
		recoveryController.current = { controller, presentationId: presentation.presentationId };
		controller.start();
		return () => {
			cancelled = true;
			settlePasteReview(false);
			if (recoveryController.current?.controller === controller) {
				recoveryController.current = null;
			}
			controller.dispose();
		};
	}, [capability, presentation.presentationId, projectable, reviewPaste, settlePasteReview]);

	let projectionState: ProjectedTerminalSurfaceView['projectionState'] = 'inactive';
	const currentRecovery =
		presentationRecovery?.presentationId === presentation.presentationId
			? presentationRecovery.state
			: undefined;
	if (projectable) projectionState = 'requested';
	if (currentRecovery?.kind === 'recovering') projectionState = 'recovering';
	if (currentRecovery?.kind === 'failed') projectionState = 'failed';
	let failureMessage: string | null = null;
	if (currentRecovery?.kind === 'recovering') {
		failureMessage = intl.formatMessage(projectedTerminalMessages.connectionRecovering);
	} else if (currentRecovery?.kind === 'failed') {
		failureMessage = intl.formatMessage(projectedTerminalMessages.connectionRecoveryFailed);
	} else if (startFailurePresentationId === presentation.presentationId) {
		failureMessage = intl.formatMessage(projectedTerminalMessages.sessionStartFailed);
	}
	return {
		approvePaste: () => settlePasteReview(true),
		cancelPaste: () => settlePasteReview(false),
		failureMessage,
		pasteReview,
		projectionState,
		recoveryAvailable: currentRecovery?.kind === 'failed',
		retryProjection: () => {
			if (recoveryController.current?.presentationId === presentation.presentationId) {
				recoveryController.current.controller.retry();
			}
		},
		root,
		stopped: presentation.lifecycle.kind === 'stopped',
		startNewSession,
	};
}
