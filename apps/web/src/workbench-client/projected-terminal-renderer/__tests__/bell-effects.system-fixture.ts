import { ActionRuntime } from '@orchbee/actions';
import {
	WorkbenchTerminalBellEffectCoordinator,
	type WorkbenchTerminalBellEffect,
} from '@orchbee/workbench';
import type { ProjectedAttachmentBootstrap } from '@orchbee/workbench-client-protocol';
import {
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
	TerminalThemeStateSchema,
	type AttachmentId,
} from '@orchbee/terminal-protocol';

import type { ProjectedTerminalPlatformEffects } from '../../projected-terminal-platform-effects';
import type { ProjectedTerminalTransport } from '../../terminal-projection';
import { startProjectedTerminalRenderer } from '../startProjectedTerminalRenderer';

const firstAttachmentId = AttachmentIdSchema.parse('61'.repeat(16));
const secondAttachmentId = AttachmentIdSchema.parse('62'.repeat(16));
const reconnectAttachmentId = AttachmentIdSchema.parse('63'.repeat(16));
const creationId = CreationIdSchema.parse('71'.repeat(16));
const hostEpoch = HostEpochSchema.parse('81'.repeat(16));
const terminalId = TerminalIdSchema.parse('91'.repeat(16));
const capabilityRevision = CanonicalUint64TextSchema.parse('1');
const theme = TerminalThemeStateSchema.parse({
	operationId: null,
	revision: '0',
	terminal: {
		colorScheme: 'dark',
		defaultBackgroundRgb: 0x10_20_30,
		defaultCursorRgb: null,
		defaultForegroundRgb: 0xf4_f4_f4,
		palette: Array.from({ length: 256 }, (_, index) => index),
	},
});

const root = document.createElement('main');
root.id = 'terminal-bell-effects-witness';
root.style.display = 'grid';
root.style.gridTemplateColumns = 'repeat(3, 1fr)';
root.style.height = '240px';
root.style.width = '900px';
document.body.replaceChildren(root);
document.body.style.margin = '0';

Object.defineProperty(navigator, 'gpu', { configurable: true, value: undefined });
root.dataset['witnessForcedCanvas'] = 'true';

const actionRuntime = new ActionRuntime();
const bellCoordinator = new WorkbenchTerminalBellEffectCoordinator();
const soundEffects: string[] = [];
const notificationEffects: string[] = [];
const platformEffects: ProjectedTerminalPlatformEffects = {
	playBellSound: (effect) => {
		recordEffect(soundEffects, effect);
		if (effect.sequence === '7') throw new Error('synthetic sound failure');
	},
	showBellNotification: (effect) => {
		recordEffect(notificationEffects, effect);
	},
};

void runWitness();

async function runWitness(): Promise<void> {
	const renderers: Awaited<ReturnType<typeof createRenderer>>[] = [];
	try {
		const first = await createRenderer(firstAttachmentId, 'first');
		const second = await createRenderer(secondAttachmentId, 'second');
		renderers.push(first, second);

		first.publishBell('5');
		const firstVisualPhases = [first.surface.dataset['projectedTerminalBellPhase']];
		second.publishBell('5');
		const duplicateSurfaceInactive =
			second.surface.dataset['projectedTerminalBellActive'] === undefined;
		second.publishBell('7');
		first.publishBell('6');
		firstVisualPhases.push(first.surface.dataset['projectedTerminalBellPhase']);

		const reconnect = await createRenderer(reconnectAttachmentId, 'reconnect');
		renderers.push(reconnect);
		reconnect.publishBell('7');
		const reconnectReplayInactive =
			reconnect.surface.dataset['projectedTerminalBellActive'] === undefined;
		reconnect.publishBell('8');

		root.dataset['witnessDuplicateSurfaceInactive'] = String(duplicateSurfaceInactive);
		root.dataset['witnessEffectSequences'] = [
			first.surface.dataset['projectedTerminalBellSequence'],
			second.surface.dataset['projectedTerminalBellSequence'],
			reconnect.surface.dataset['projectedTerminalBellSequence'],
		].join(',');
		root.dataset['witnessNotificationEffects'] = notificationEffects.join(',');
		root.dataset['witnessReconnectReplayInactive'] = String(reconnectReplayInactive);
		root.dataset['witnessSameTerminal'] = 'true';
		root.dataset['witnessSoundEffects'] = soundEffects.join(',');
		root.dataset['witnessSoundFailureIsolated'] = String(notificationEffects.includes('7'));
		root.dataset['witnessVisualPhases'] = firstVisualPhases.join(',');
		root.dataset['witnessReady'] = 'true';
	} catch (error) {
		root.dataset['witnessFailure'] = error instanceof Error ? error.message : String(error);
	} finally {
		for (const renderer of renderers) renderer.session.dispose();
		bellCoordinator.dispose();
		actionRuntime.dispose();
	}
}

async function createRenderer(attachmentId: AttachmentId, label: string) {
	const surface = document.createElement('section');
	surface.dataset['bellSurface'] = label;
	surface.style.height = '240px';
	surface.style.minWidth = '0';
	root.appendChild(surface);
	let transportListener: (value: unknown) => void = () => undefined;
	const transport: ProjectedTerminalTransport = {
		close: () => undefined,
		projectionBufferTrust: 'local-host',
		send: () => undefined,
		subscribe: (listener) => {
			transportListener = listener;
			return () => undefined;
		},
	};
	const bootstrap: ProjectedAttachmentBootstrap = {
		attachmentId,
		canInput: true,
		canResize: false,
		capabilityRevision,
		creationId,
		hostEpoch,
		terminalId,
		terminalOutcome: null,
		theme,
	};
	const session = await startProjectedTerminalRenderer(
		surface,
		document.documentElement,
		['en-US'],
		'apple',
		bootstrap,
		transport,
		undefined,
		platformEffects,
		undefined,
		undefined,
		undefined,
		undefined,
		undefined,
		undefined,
		{
			admitBellEffect: (effect) => bellCoordinator.admit(effect),
			bindContextProvider: () => ({ dispose: () => undefined }),
			runtime: actionRuntime,
			scope: actionRuntime.createScope(`workbench.fixture.bell.${label}`),
		},
	);
	return {
		publishBell: (sequence: string): void => {
			transportListener({ event: { attachmentId, sequence }, kind: 'bell' });
		},
		session,
		surface,
	};
}

function recordEffect(target: string[], effect: WorkbenchTerminalBellEffect): void {
	if (effect.terminalId !== terminalId) throw new Error('Bell effect lost terminal identity.');
	target.push(effect.sequence);
}
