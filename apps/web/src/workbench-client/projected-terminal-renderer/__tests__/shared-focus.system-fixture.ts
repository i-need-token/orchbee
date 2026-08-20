import { ProjectedTerminal } from '@orchbee/terminal';
import type { ProjectedTerminalCommand } from '@orchbee/workbench-client-protocol';
import {
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
} from '@orchbee/terminal-protocol';

const firstAttachmentId = AttachmentIdSchema.parse('21'.repeat(16));
const secondAttachmentId = AttachmentIdSchema.parse('22'.repeat(16));
const creationId = CreationIdSchema.parse('31'.repeat(16));
const hostEpoch = HostEpochSchema.parse('41'.repeat(16));
const terminalId = TerminalIdSchema.parse('51'.repeat(16));
const capabilityRevision = CanonicalUint64TextSchema.parse('1');

const root = document.createElement('main');
root.id = 'terminal-shared-focus-witness';
root.style.display = 'grid';
root.style.gridTemplateColumns = '1fr 1fr';
root.style.height = '240px';
root.style.width = '720px';
document.body.replaceChildren(root);
document.body.style.margin = '0';

Object.defineProperty(navigator, 'gpu', { configurable: true, value: undefined });
root.dataset['witnessForcedCanvas'] = 'true';

const focusClaims = new Map<string, boolean>();
const projectionDemand = new Map<string, 'active' | 'paused'>();
const aggregateTransitions: string[] = [];
const focusCommands: string[] = [];
const demandCommands: string[] = [];

void runWitness();

async function runWitness(): Promise<void> {
	const terminals: ProjectedTerminal[] = [];
	try {
		const first = createTerminal(firstAttachmentId, 'first');
		const second = createTerminal(secondAttachmentId, 'second');
		terminals.push(first.terminal, second.terminal);
		await Promise.all([first.terminal.open(first.surface), second.terminal.open(second.surface)]);

		const firstInput = requireInput(first.terminal);
		const secondInput = requireInput(second.terminal);
		firstInput.dispatchEvent(new FocusEvent('focus'));
		secondInput.dispatchEvent(new FocusEvent('focus'));
		firstInput.dispatchEvent(new FocusEvent('blur'));
		const claimsAfterFirstRelease = activeFocusClaimCount();

		second.terminal.setProjectionDemand('paused');
		const claimsAfterPause = activeFocusClaimCount();
		const focusCountBeforeActivation = focusCommands.length;
		second.terminal.setProjectionDemand('active');
		const activationReplayDelta = focusCommands.length - focusCountBeforeActivation;

		if (aggregateTransitions.join(',') !== 'in,out') {
			throw new Error('Shared focus did not produce exactly one aggregate interval.');
		}
		if (claimsAfterFirstRelease !== 1 || claimsAfterPause !== 0) {
			throw new Error('Shared focus repair crossed or retained the wrong attachment claim.');
		}
		if (activationReplayDelta !== 0) {
			throw new Error('Projection activation replayed a repaired focus claim.');
		}

		root.dataset['witnessActivationReplayDelta'] = String(activationReplayDelta);
		root.dataset['witnessAggregateTransitions'] = aggregateTransitions.join(',');
		root.dataset['witnessClaimsAfterFirstRelease'] = String(claimsAfterFirstRelease);
		root.dataset['witnessClaimsAfterPause'] = String(claimsAfterPause);
		root.dataset['witnessDemandCommands'] = demandCommands.join(',');
		root.dataset['witnessFocusCommands'] = focusCommands.join(',');
		root.dataset['witnessSameTerminal'] = 'true';
		root.dataset['witnessSyntheticEventScope'] = 'application-layer-only';
		root.dataset['witnessReady'] = 'true';
	} catch (error) {
		root.dataset['witnessFailure'] = error instanceof Error ? error.message : String(error);
	} finally {
		for (const terminal of terminals) terminal.dispose();
	}
}

function createTerminal(
	attachmentId: typeof firstAttachmentId,
	label: string,
): { readonly surface: HTMLElement; readonly terminal: ProjectedTerminal } {
	const surface = document.createElement('section');
	surface.dataset['sharedFocusSurface'] = label;
	surface.style.height = '240px';
	surface.style.minWidth = '0';
	root.appendChild(surface);
	const terminal = new ProjectedTerminal({
		attachmentId,
		capabilityRevision,
		creationId,
		hostEpoch,
		labels: { input: `${label} input`, screen: `${label} screen` },
		port: { send: observeCommand },
		terminalId,
	});
	return { surface, terminal };
}

function observeCommand(command: ProjectedTerminalCommand): void {
	if (command.kind === 'set-projection-demand') {
		const attachmentId = command.payload.attachmentId;
		const before = aggregateFocused();
		projectionDemand.set(attachmentId, command.payload.demand);
		if (command.payload.demand === 'paused') focusClaims.set(attachmentId, false);
		demandCommands.push(`${shortId(attachmentId)}:${command.payload.demand}`);
		recordAggregateTransition(before);
		return;
	}
	if (command.kind !== 'focus-input') return;
	const attachmentId = command.payload.attachmentId;
	const before = aggregateFocused();
	if (command.payload.focused && projectionDemand.get(attachmentId) !== 'active') {
		throw new Error('A paused attachment attempted to create a focus claim.');
	}
	focusClaims.set(attachmentId, command.payload.focused);
	focusCommands.push(`${shortId(attachmentId)}:${String(command.payload.focused)}`);
	recordAggregateTransition(before);
}

function recordAggregateTransition(before: boolean): void {
	const after = aggregateFocused();
	if (before === after) return;
	aggregateTransitions.push(after ? 'in' : 'out');
}

function aggregateFocused(): boolean {
	for (const [attachmentId, focused] of focusClaims) {
		if (focused && projectionDemand.get(attachmentId) === 'active') return true;
	}
	return false;
}

function activeFocusClaimCount(): number {
	let count = 0;
	for (const [attachmentId, focused] of focusClaims) {
		if (focused && projectionDemand.get(attachmentId) === 'active') count += 1;
	}
	return count;
}

function requireInput(terminal: ProjectedTerminal): HTMLTextAreaElement {
	const input = terminal.textarea;
	if (input === undefined) throw new Error('Projected terminal input was not created.');
	return input;
}

function shortId(attachmentId: string): string {
	return attachmentId === firstAttachmentId ? 'first' : 'second';
}
