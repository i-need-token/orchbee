import { useMemoizedFn } from '@orchbee/foundation/react';
import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ProjectedPasteReview } from '@orchbee/terminal';
import type { TerminalClipboardWriteRequest } from '@orchbee/terminal-protocol';
import type { WorkbenchTerminalPresentation } from '@orchbee/workbench';
import * as React from 'react';

import {
	MAXIMUM_TERMINAL_FILE_DROP_FILES,
	MAXIMUM_TERMINAL_FILE_DROP_PREVIEW_CHARACTERS,
	type TerminalFileDropRejectionReason,
} from '../../../terminal-file-drop';
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

interface PendingClipboardWriteDecision {
	readonly abort: () => void;
	readonly resolve: (approved: boolean) => void;
	readonly signal: AbortSignal;
}

interface PresentationRecoveryState {
	readonly presentationId: string;
	readonly state: ProjectedTerminalRecoveryState;
}

interface PresentationFileDropFailure {
	readonly count: number;
	readonly presentationId: string;
	readonly reason: TerminalFileDropRejectionReason | 'terminal-unavailable';
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
	const fileDropEpoch = React.useRef(0);
	const [presentationFileDropFailure, setPresentationFileDropFailure] =
		React.useState<PresentationFileDropFailure | null>(null);
	const [presentationRecovery, setPresentationRecovery] =
		React.useState<PresentationRecoveryState | null>(null);
	const [startFailurePresentationId, setStartFailurePresentationId] = React.useState<string | null>(
		null,
	);
	const [pasteReview, setPasteReview] = React.useState<ProjectedPasteReview | null>(null);
	const pendingPasteDecision = React.useRef<PendingPasteDecision | null>(null);
	const [clipboardWriteRequest, setClipboardWriteRequest] =
		React.useState<TerminalClipboardWriteRequest | null>(null);
	const pendingClipboardWriteDecision = React.useRef<PendingClipboardWriteDecision | null>(null);
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
	const settleClipboardWriteReview = useMemoizedFn((approved: boolean): void => {
		const pending = pendingClipboardWriteDecision.current;
		if (pending === null) return;
		pendingClipboardWriteDecision.current = null;
		pending.signal.removeEventListener('abort', pending.abort);
		setClipboardWriteRequest(null);
		pending.resolve(approved);
	});
	const reviewPaste = useMemoizedFn(
		(review: ProjectedPasteReview, signal: AbortSignal): Promise<boolean> => {
			settlePasteReview(false);
			settleClipboardWriteReview(false);
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
	const reviewClipboardWrite = useMemoizedFn(
		(request: TerminalClipboardWriteRequest, signal: AbortSignal): Promise<boolean> => {
			if (
				signal.aborted ||
				pendingClipboardWriteDecision.current !== null ||
				pendingPasteDecision.current !== null
			) {
				return Promise.resolve(false);
			}
			return new Promise<boolean>((resolve) => {
				const pending: PendingClipboardWriteDecision = {
					abort: () => {
						if (pendingClipboardWriteDecision.current === pending) {
							settleClipboardWriteReview(false);
						}
					},
					resolve,
					signal,
				};
				pendingClipboardWriteDecision.current = pending;
				setClipboardWriteRequest(request);
				signal.addEventListener('abort', pending.abort, { once: true });
				if (signal.aborted) pending.abort();
			});
		},
	);
	const handleFileDragOver = useMemoizedFn((event: React.DragEvent<HTMLDivElement>): void => {
		if (
			capability?.terminalFileDropCapability === undefined ||
			event.dataTransfer.files.length === 0
		) {
			return;
		}
		event.preventDefault();
		event.dataTransfer.dropEffect = 'copy';
	});
	const handleFileDrop = useMemoizedFn((event: React.DragEvent<HTMLDivElement>): void => {
		const fileDropCapability = capability?.terminalFileDropCapability;
		const files = Array.from(event.dataTransfer.files);
		if (fileDropCapability === undefined || files.length === 0) return;
		event.preventDefault();
		event.stopPropagation();
		settlePasteReview(false);
		settleClipboardWriteReview(false);
		const epoch = ++fileDropEpoch.current;
		setPresentationFileDropFailure(null);
		void (async () => {
			let preparation;
			try {
				preparation = await fileDropCapability.prepare(files);
			} catch {
				preparation = { kind: 'rejected', reason: 'unavailable-path' } as const;
			}
			if (fileDropEpoch.current !== epoch) return;
			if (preparation.kind === 'rejected') {
				setPresentationFileDropFailure({
					count: files.length,
					presentationId: presentation.presentationId,
					reason: preparation.reason,
				});
				return;
			}
			const recovery = recoveryController.current;
			if (recovery?.presentationId !== presentation.presentationId) {
				setPresentationFileDropFailure({
					count: files.length,
					presentationId: presentation.presentationId,
					reason: 'terminal-unavailable',
				});
				return;
			}
			const result = await recovery.controller.pasteFileDrop(preparation.text);
			if (fileDropEpoch.current !== epoch || result.kind !== 'unavailable') return;
			setPresentationFileDropFailure({
				count: files.length,
				presentationId: presentation.presentationId,
				reason: 'terminal-unavailable',
			});
		})().catch(() => {
			if (fileDropEpoch.current !== epoch) return;
			setPresentationFileDropFailure({
				count: files.length,
				presentationId: presentation.presentationId,
				reason: 'terminal-unavailable',
			});
		});
	});
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
					reviewClipboardWrite,
					capability.terminalCompletionReminderObserver,
				),
		});
		recoveryController.current = { controller, presentationId: presentation.presentationId };
		controller.start();
		return () => {
			cancelled = true;
			fileDropEpoch.current += 1;
			settlePasteReview(false);
			settleClipboardWriteReview(false);
			if (recoveryController.current?.controller === controller) {
				recoveryController.current = null;
			}
			controller.dispose();
		};
	}, [
		capability,
		presentation.presentationId,
		projectable,
		reviewClipboardWrite,
		reviewPaste,
		settleClipboardWriteReview,
		settlePasteReview,
	]);

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
	const currentFileDropFailure =
		presentationFileDropFailure?.presentationId === presentation.presentationId
			? presentationFileDropFailure
			: null;
	if (failureMessage === null && currentFileDropFailure !== null) {
		switch (currentFileDropFailure.reason) {
			case 'too-many-files':
				failureMessage = intl.formatMessage(projectedTerminalMessages.fileDropFailureTooMany, {
					count: currentFileDropFailure.count,
					maximum: MAXIMUM_TERMINAL_FILE_DROP_FILES,
				});
				break;
			case 'preview-too-large':
				failureMessage = intl.formatMessage(
					projectedTerminalMessages.fileDropFailurePreviewTooLarge,
					{ maximum: MAXIMUM_TERMINAL_FILE_DROP_PREVIEW_CHARACTERS },
				);
				break;
			case 'unsupported':
				failureMessage = intl.formatMessage(projectedTerminalMessages.fileDropFailureUnsupported);
				break;
			case 'terminal-unavailable':
				failureMessage = intl.formatMessage(
					projectedTerminalMessages.fileDropFailureTerminalUnavailable,
				);
				break;
			case 'empty':
			case 'invalid-path':
			case 'unavailable-path':
				failureMessage = intl.formatMessage(projectedTerminalMessages.fileDropFailureInvalid);
				break;
		}
	}
	return {
		approveClipboardWrite: () => settleClipboardWriteReview(true),
		approvePaste: () => settlePasteReview(true),
		cancelClipboardWrite: () => settleClipboardWriteReview(false),
		cancelPaste: () => settlePasteReview(false),
		clipboardWriteRequest,
		failureMessage,
		handleFileDragOver,
		handleFileDrop,
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
