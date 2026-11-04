import { useMemoizedFn } from '@orchbee/foundation/react';
import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { ProjectedPasteReview } from '@orchbee/terminal';
import type {
	WorkbenchTerminalPresentation,
	WorkbenchTerminalSurfaceProps,
} from '../../../workbench';
import * as React from 'react';

import type { ProjectedTerminalControlPresentation } from '../../projected-terminal-actions';
import {
	createTerminalTrailingBlankRowsPresentationSource,
	startProjectedTerminalRenderer,
	type MobileTerminalInputState,
	type MobileTerminalKey,
	type MobileTerminalModifier,
	type ProjectedTerminalOutcome,
} from '../../projected-terminal-renderer';
import {
	ProjectedTerminalSurfaceCapabilityContext,
	type ProjectedTerminalSurfaceCapability,
} from '../contexts/projected-terminal-surface-capability';
import { MountedTerminalProjectionRequest } from '../model/MountedTerminalProjectionRequest';
import {
	ProjectedTerminalRecoveryController,
	type ProjectedTerminalRecoveryState,
} from '../model/ProjectedTerminalRecoveryController';
import { runApplicationCleanup } from '../../../runApplicationCleanup';
import type {
	ProjectedTerminalLaunchFailureView,
	ProjectedTerminalSurfaceView,
} from '../types/projected-terminal-surface-view';

interface PendingPasteDecision {
	readonly abort: () => void;
	readonly resolve: (approved: boolean) => void;
	readonly signal: AbortSignal;
}

interface PresentationRecoveryState {
	readonly presentationId: string;
	readonly state: ProjectedTerminalRecoveryState;
}

interface PresentationMobileInputState {
	readonly presentationId: string;
	readonly state: MobileTerminalInputState;
}

interface PresentationControl {
	readonly liveCropped: boolean;
	readonly outcome: ProjectedTerminalOutcome | undefined;
	readonly presentation: ProjectedTerminalControlPresentation;
	readonly presentationId: string;
}

const unavailableMobileInputState: MobileTerminalInputState = {
	alt: false,
	control: false,
	focused: false,
	readiness: { kind: 'unavailable' },
};

type ProjectedTerminalSessionPolicy = 'project' | 'retain';

type ProjectedTerminalSurfaceWorkbenchHost = Pick<
	WorkbenchTerminalSurfaceProps,
	'actionRuntime' | 'actionScope' | 'admitBellEffect' | 'autoFocus' | 'bindActionContextProvider'
> &
	Partial<
		Pick<
			WorkbenchTerminalSurfaceProps,
			| 'closeTerminal'
			| 'createTab'
			| 'focusRequest'
			| 'onPresentationReady'
			| 'presentationMode'
			| 'onTerminalEdit'
		>
	>;

export function useProjectedTerminalSurface(
	presentation: WorkbenchTerminalPresentation,
	workbenchHost?: ProjectedTerminalSurfaceWorkbenchHost,
): ProjectedTerminalSurfaceView {
	const actionRuntime = workbenchHost?.actionRuntime;
	const actionScope = workbenchHost?.actionScope;
	const admitBellEffect = workbenchHost?.admitBellEffect;
	const autoFocus = workbenchHost?.autoFocus ?? false;
	const bindActionContextProvider = workbenchHost?.bindActionContextProvider;
	const closeTerminal = workbenchHost?.closeTerminal;
	const createTab = workbenchHost?.createTab;
	const focusRequest = workbenchHost?.focusRequest ?? 0;
	const onPresentationReady = workbenchHost?.onPresentationReady;
	const onTerminalEdit = workbenchHost?.onTerminalEdit;
	const presentationMode = workbenchHost?.presentationMode ?? 'active';
	const capability = React.useContext(ProjectedTerminalSurfaceCapabilityContext);
	const intl = useOrchbeeIntl();
	const root = React.useRef<HTMLDivElement>(null);
	const launchFailureRoot = React.useRef<HTMLElement>(null);
	const recoveryController = React.useRef<{
		readonly controller: ProjectedTerminalRecoveryController;
		readonly presentationId: string;
	} | null>(null);
	const [presentationRecovery, setPresentationRecovery] =
		React.useState<PresentationRecoveryState | null>(null);
	const [presentationMobileInput, setPresentationMobileInput] =
		React.useState<PresentationMobileInputState | null>(null);
	const [presentationControl, setPresentationControl] = React.useState<PresentationControl | null>(
		null,
	);
	const [pasteReview, setPasteReview] = React.useState<ProjectedPasteReview | null>(null);
	const pendingPasteDecision = React.useRef<PendingPasteDecision | null>(null);
	const projectable =
		presentation.lifecycle.kind === 'running' ||
		presentation.lifecycle.kind === 'exited' ||
		presentation.lifecycle.kind === 'failed';
	const sessionPolicy: ProjectedTerminalSessionPolicy =
		presentationMode === 'retained' ? 'retain' : 'project';
	const sessionPolicyReference = React.useRef(sessionPolicy);
	sessionPolicyReference.current = sessionPolicy;
	// The renderer hears an end first, but the surface names it only once the Presentation has
	// ended too, so a composition decides when the person sees it, as the Server path holds a pane
	// whose shell exited cleanly while the Server closes it.
	const ended =
		presentation.lifecycle.kind === 'exited' || presentation.lifecycle.kind === 'failed';
	const creationFailed =
		presentation.lifecycle.kind === 'launch-failed' || presentation.lifecycle.kind === 'unbound';
	// Read on every render: the record changes only with a new lifecycle, which renders again.
	const launchFailure = creationFailed
		? describeLaunchFailure(capability?.readTerminalLaunchFailure, presentation.presentationId)
		: null;
	const currentRecovery =
		presentationRecovery?.presentationId === presentation.presentationId
			? presentationRecovery.state
			: undefined;
	const reportPresentationReady = useMemoizedFn(
		(readiness: Parameters<WorkbenchTerminalSurfaceProps['onPresentationReady']>[0]): void => {
			if (presentationMode !== 'retained') onPresentationReady?.(readiness);
		},
	);
	const reportTerminalEdit = useMemoizedFn(() => {
		if (presentationMode === 'active') onTerminalEdit?.();
	});

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
	const sendMobileKey = useMemoizedFn((key: MobileTerminalKey): boolean => {
		const recovery = recoveryController.current;
		return recovery?.presentationId === presentation.presentationId
			? recovery.controller.sendMobileKey(key)
			: false;
	});
	const toggleMobileKeyboard = useMemoizedFn((): boolean => {
		const recovery = recoveryController.current;
		return recovery?.presentationId === presentation.presentationId
			? recovery.controller.toggleMobileKeyboard()
			: false;
	});
	const toggleMobileModifier = useMemoizedFn((modifier: MobileTerminalModifier): void => {
		const recovery = recoveryController.current;
		if (recovery?.presentationId === presentation.presentationId) {
			recovery.controller.toggleMobileModifier(modifier);
		}
	});
	// The pressed button leaves with the view-only notice, so the keyboard goes to the terminal
	// unless the person has moved it elsewhere meanwhile.
	const takeControl = useMemoizedFn((): void => {
		const recovery = recoveryController.current;
		if (recovery?.presentationId !== presentation.presentationId) return;
		void recovery.controller.takeControl().then((taken) => {
			const ownerDocument = root.current?.ownerDocument;
			const focused = ownerDocument?.activeElement;
			if (
				!taken ||
				recoveryController.current !== recovery ||
				(focused !== ownerDocument?.body && !root.current?.contains(focused ?? null))
			) {
				return;
			}
			recovery.controller.focusTerminal();
		});
	});
	if (capability === null) {
		throw new Error('ProjectedTerminalSurface requires its client capability.');
	}

	React.useEffect(() => {
		if (!projectable) return;
		let cancelled = false;
		let preparedProjectionRequest: MountedTerminalProjectionRequest | undefined;
		const controller = new ProjectedTerminalRecoveryController({
			onActivationReady: () => reportPresentationReady('activation-ready'),
			onDisplayReady: () => reportPresentationReady('display-ready'),
			onRetainedSnapshot: (snapshot) => {
				capability.presentationSnapshots?.set(presentation.presentationId, snapshot);
			},
			onStateChange: (state) => {
				if (!cancelled) {
					setPresentationRecovery({
						presentationId: presentation.presentationId,
						state,
					});
				}
			},
			open: () => {
				const request = new MountedTerminalProjectionRequest(
					root.current,
					presentation.presentationId,
					capability.projectionClient,
					capability.presentationConfigurationSource,
				);
				preparedProjectionRequest = request;
				return request;
			},
			startRenderer: (attachment) => {
				const presentationConfigurationSource =
					preparedProjectionRequest?.takePresentationConfigurationSource() ??
					capability.presentationConfigurationSource;
				preparedProjectionRequest = undefined;
				const trailingBlankRowsPresentationSource =
					createTerminalTrailingBlankRowsPresentationSource({
						agentObservationSource: capability.agentObservationSource,
						preferenceSource: capability.trailingBlankRowsPreferenceSource,
						presentationId: presentation.presentationId,
					});
				return startProjectedTerminalRenderer(
					root.current,
					capability.documentElement,
					capability.preferredLocales,
					capability.shortcutPlatform,
					attachment.bootstrap,
					attachment.transport,
					undefined,
					capability.platformEffects,
					presentationConfigurationSource,
					reviewPaste,
					(metadata) => capability.observeTerminalMetadata(presentation.presentationId, metadata),
					capability.shortcutBindingSource,
					capability.terminalCompletionReminderObserver,
					capability.inputConfigurationSource,
					actionRuntime === undefined ||
						actionScope === undefined ||
						admitBellEffect === undefined ||
						bindActionContextProvider === undefined
						? undefined
						: {
								admitBellEffect,
								onTerminalEdit: reportTerminalEdit,
								bindContextProvider: bindActionContextProvider,
								runtime: actionRuntime,
								scope: actionScope,
							},
					capability.interactionConfigurationSource,
					capability.presentationSnapshots?.get(presentation.presentationId),
					capability.applicationLocaleSource,
					trailingBlankRowsPresentationSource,
				);
			},
		});
		recoveryController.current = { controller, presentationId: presentation.presentationId };
		const mobileInputSubscription = controller.subscribeMobileInputState(() => {
			if (cancelled) return;
			setPresentationMobileInput({
				presentationId: presentation.presentationId,
				state: controller.getMobileInputState(),
			});
		});
		setPresentationMobileInput({
			presentationId: presentation.presentationId,
			state: controller.getMobileInputState(),
		});
		const controlSubscription = controller.subscribeControlPresentation(() => {
			if (cancelled) return;
			setPresentationControl({
				liveCropped: controller.isLiveCropped(),
				outcome: controller.getTerminalOutcome(),
				presentation: controller.getControlPresentation(),
				presentationId: presentation.presentationId,
			});
		});
		if (sessionPolicyReference.current === 'retain') controller.retain();
		else controller.start();
		return () => {
			if (cancelled) return;
			cancelled = true;
			setPresentationControl((current) =>
				current?.presentationId === presentation.presentationId ? null : current,
			);
			if (recoveryController.current?.controller === controller) {
				recoveryController.current = null;
			}
			runApplicationCleanup(
				[
					() => settlePasteReview(false),
					() => mobileInputSubscription.dispose(),
					() => controlSubscription.dispose(),
					() => controller.dispose(),
				],
				'Projected terminal cleanup failed',
			);
		};
	}, [
		actionRuntime,
		actionScope,
		admitBellEffect,
		bindActionContextProvider,
		capability,
		presentation.presentationId,
		projectable,
		reviewPaste,
		reportPresentationReady,
		reportTerminalEdit,
		settlePasteReview,
	]);
	React.useLayoutEffect(() => {
		const recovery = recoveryController.current;
		if (recovery?.presentationId !== presentation.presentationId) return;
		if (sessionPolicy === 'retain') recovery.controller.retain();
		else {
			recovery.controller.resume();
			recovery.controller.start();
		}
	}, [presentation.presentationId, sessionPolicy]);
	React.useLayoutEffect(() => {
		if (presentationMode === 'retained') return;
		if (!projectable || currentRecovery?.kind === 'failed') {
			reportPresentationReady('display-ready');
			reportPresentationReady('activation-ready');
		}
	}, [currentRecovery?.kind, presentationMode, projectable, reportPresentationReady]);
	React.useEffect(() => {
		if (presentationMode !== 'active' || !autoFocus || currentRecovery?.kind !== 'ready') return;
		const recovery = recoveryController.current;
		if (recovery?.presentationId !== presentation.presentationId) return;
		recovery.controller.focusTerminal();
	}, [
		autoFocus,
		currentRecovery?.kind,
		focusRequest,
		presentation.presentationId,
		presentationMode,
	]);
	// Without this, focus stays on the document body, where Tab starts from the sidebar.
	React.useEffect(() => {
		if (presentationMode !== 'active' || !autoFocus || !creationFailed) return;
		launchFailureRoot.current?.focus({ preventScroll: true });
	}, [autoFocus, creationFailed, focusRequest, presentationMode]);

	let projectionState: ProjectedTerminalSurfaceView['projectionState'] = 'inactive';
	if (projectable) projectionState = 'requested';
	if (currentRecovery?.kind === 'recovering') projectionState = 'recovering';
	if (currentRecovery?.kind === 'failed') projectionState = 'failed';
	let failureMessage: string | null = null;
	if (currentRecovery?.kind === 'recovering') {
		failureMessage = intl.formatMessage(projectedTerminalMessages.connectionRecovering);
	} else if (currentRecovery?.kind === 'failed') {
		failureMessage = intl.formatMessage(projectedTerminalMessages.disconnected);
	}
	const mobileInputState =
		presentationMobileInput?.presentationId === presentation.presentationId
			? presentationMobileInput.state
			: unavailableMobileInputState;
	const currentControl =
		presentationControl?.presentationId === presentation.presentationId
			? presentationControl
			: null;
	const controlPresentation = currentControl?.presentation ?? 'none';
	return {
		approvePaste: () => settlePasteReview(true),
		cancelPaste: () => settlePasteReview(false),
		closeTerminal: () => closeTerminal?.('button'),
		controlPresentation,
		createTab: () => createTab?.('button'),
		failureMessage,
		launchFailure,
		launchFailureRoot,
		liveCropped: currentControl?.liveCropped === true,
		mobileInputState,
		pasteReview,
		projectionState,
		recoveryAvailable: currentRecovery?.kind === 'failed',
		retryProjection: () => {
			if (recoveryController.current?.presentationId === presentation.presentationId) {
				recoveryController.current.controller.retry();
			}
		},
		root,
		sendMobileKey,
		takeControl,
		terminalOutcome: ended ? (currentControl?.outcome ?? null) : null,
		toggleMobileKeyboard,
		toggleMobileModifier,
	};
}

function describeLaunchFailure(
	read: ProjectedTerminalSurfaceCapability['readTerminalLaunchFailure'],
	presentationId: WorkbenchTerminalPresentation['presentationId'],
): ProjectedTerminalLaunchFailureView {
	if (read === undefined) return { executable: null, kind: 'failed' };
	const failure = read(presentationId);
	return failure === null
		? { kind: 'unavailable' }
		: { executable: failure.executable, kind: 'failed' };
}
