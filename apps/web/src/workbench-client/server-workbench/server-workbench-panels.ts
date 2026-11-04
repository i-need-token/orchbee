import type { ActionResult } from '@orchbee/actions';
import type {
	WorkbenchAuthorityContext,
	WorkbenchAuthorityPanelTerminalCreation,
	WorkbenchAuthorityPanelTerminalReceipt,
	WorkbenchAuthoritySplitRatioChange,
} from '../../workbench';
import type {
	WorkbenchPanelNode,
	WorkbenchSnapshot,
	WorkbenchTerminalContext,
} from '@orchbee/workbench-client-protocol';

import { handOverServerWorkbenchAction } from './handOverServerWorkbenchAction';
import { readServerWorkbenchAction } from './readServerWorkbenchAction';
import type {
	ServerWorkbenchAction,
	ServerWorkbenchOutcome,
	ServerWorkbenchSession,
} from './ServerWorkbenchSession';
import { serverWorkbenchResult } from './serverWorkbenchResult';
import { DEFAULT_TERMINAL_GEOMETRY } from './workspace-launch';

type ServerTab = WorkbenchTerminalContext['tabs'][number];
type ServerPanel = Extract<WorkbenchPanelNode, { readonly kind: 'panel' }>;
type ServerSplit = Extract<WorkbenchPanelNode, { readonly kind: 'split' }>;

type RatioSession = Pick<ServerWorkbenchSession, 'snapshot' | 'submit'>;

/** The first slot's share of one split, changed from what the person saw it hold. */
interface RatioMove {
	readonly previousRatio: number;
	readonly ratio: number;
	readonly split: ServerSplit;
}

type RatioStep =
	| { readonly kind: 'move'; readonly move: RatioMove }
	/** Nothing is left to change. */
	| { readonly kind: 'settled' }
	/** What the change names is no longer there. */
	| { readonly kind: 'unavailable' };

type RatioCommit =
	| { readonly kind: 'committed' }
	| { readonly kind: 'settled' }
	| { readonly kind: 'stopped'; readonly result: ActionResult<void> };

/** What each turn of a submission found, which its answer alone cannot say. */
interface RatioTurn {
	move: RatioMove | null;
	stop: 'settled' | 'split-changed' | 'unavailable' | null;
}

export const SERVER_MINIMUM_SPLIT_PERCENT = 10;

const EVEN_RATIO = 0.5;
const MAXIMUM_RATIO = 0.9;
const MINIMUM_RATIO = SERVER_MINIMUM_SPLIT_PERCENT / 100;
/** Finer than any screen shows, and free of what repeatedly adding decimal fractions leaves behind. */
const RATIO_PRECISION = 100_000;
const RATIO_TOLERANCE = 0.5 / RATIO_PRECISION;
/** The TUI's keyboard step, which is also what the GUI's dialog has always moved by. */
const RATIO_STEP = 0.05;
/** The TUI's bound on going again when an unrelated commit moved the revision under a ratio. */
const RATIO_CONTENTION_MILLISECONDS = 5_000;

function findTab(
	snapshot: WorkbenchSnapshot,
	terminalContextId: string,
	tabId: string,
): ServerTab | undefined {
	return snapshot.terminalContexts
		.find((context) => context.terminalContextId === terminalContextId)
		?.tabs.find((tab) => tab.tabId === tabId);
}

function panelsOf(node: WorkbenchPanelNode): readonly ServerPanel[] {
	return node.kind === 'panel' ? [node] : node.children.flatMap((child) => panelsOf(child.node));
}

function splitsOf(node: WorkbenchPanelNode): readonly ServerSplit[] {
	return node.kind === 'panel'
		? []
		: [node, ...node.children.flatMap((child) => splitsOf(child.node))];
}

function ratioOf(split: ServerSplit): number {
	return split.ratio ?? EVEN_RATIO;
}

function admissibleRatio(ratio: number): number {
	const clamped = Math.min(MAXIMUM_RATIO, Math.max(MINIMUM_RATIO, ratio));
	return Math.round(clamped * RATIO_PRECISION) / RATIO_PRECISION;
}

function moveTo(split: ServerSplit, ratio: number): RatioStep {
	const previousRatio = ratioOf(split);
	const target = admissibleRatio(ratio);
	return Math.abs(target - previousRatio) <= RATIO_TOLERANCE
		? { kind: 'settled' }
		: { kind: 'move', move: { previousRatio, ratio: target, split } };
}

/** How many Panels sit side by side along this axis, so that each of them can get the same share. */
function axisUnits(node: WorkbenchPanelNode, orientation: ServerSplit['orientation']): number {
	return node.kind === 'split' && node.orientation === orientation
		? node.children.reduce((total, child) => total + axisUnits(child.node, orientation), 0)
		: 1;
}

function equalRatio(split: ServerSplit): number {
	const [first, second] = split.children;
	const units = axisUnits(first.node, split.orientation);
	return units / (units + axisUnits(second.node, split.orientation));
}

/** One split at a time, because the Server changes one split per command. */
function firstUneven(root: WorkbenchPanelNode, target: (split: ServerSplit) => number): RatioStep {
	for (const split of splitsOf(root)) {
		const step = moveTo(split, target(split));
		if (step.kind === 'move') return step;
	}
	return { kind: 'settled' };
}

/** The deepest split on this axis that holds the Panel, the one whose divider touches its side. */
function nearestSplit(
	node: WorkbenchPanelNode,
	panelId: string,
	orientation: ServerSplit['orientation'],
): { readonly holdsPanelFirst: boolean; readonly split: ServerSplit } | null {
	if (node.kind === 'panel') return null;
	const [first, second] = node.children;
	const holdsPanelFirst = panelsOf(first.node).some((panel) => panel.panelId === panelId);
	if (!holdsPanelFirst && !panelsOf(second.node).some((panel) => panel.panelId === panelId)) {
		return null;
	}
	return (
		nearestSplit((holdsPanelFirst ? first : second).node, panelId, orientation) ??
		(node.orientation === orientation ? { holdsPanelFirst, split: node } : null)
	);
}

function adjustedStep(
	root: WorkbenchPanelNode,
	change: Extract<WorkbenchAuthoritySplitRatioChange, { type: 'panel-size-adjusted' }>,
): RatioStep {
	const { adjustment, multiplier, panelId } = change;
	const nearest = nearestSplit(
		root,
		panelId,
		adjustment === 'narrower' || adjustment === 'wider' ? 'horizontal' : 'vertical',
	);
	if (nearest === null) return { kind: 'settled' };
	const growth =
		(adjustment === 'taller' || adjustment === 'wider' ? 1 : -1) * RATIO_STEP * multiplier;
	const previous = ratioOf(nearest.split);
	return moveTo(nearest.split, nearest.holdsPanelFirst ? previous + growth : previous - growth);
}

function nextRatioStep(change: WorkbenchAuthoritySplitRatioChange, tab: ServerTab): RatioStep {
	switch (change.type) {
		case 'split-ratio-set': {
			const split = splitsOf(tab.root).find((candidate) => candidate.splitId === change.splitId);
			return split === undefined
				? { kind: 'unavailable' }
				: moveTo(split, change.leadingWeight / 100);
		}
		case 'panel-size-adjusted':
			return change.adjustment === 'reset'
				? firstUneven(tab.root, () => EVEN_RATIO)
				: adjustedStep(tab.root, change);
		case 'panels-equalized':
			return firstUneven(tab.root, equalRatio);
	}
}

function ratioAction(
	{ tabId, terminalContextId }: WorkbenchAuthoritySplitRatioChange,
	{ previousRatio, ratio, split }: RatioMove,
): ServerWorkbenchAction | null {
	return readServerWorkbenchAction({
		kind: 'layout.set-split-ratio',
		previousRatio,
		ratio,
		split: {
			firstSlotId: split.children[0].slotId,
			secondSlotId: split.children[1].slotId,
			splitId: split.splitId,
		},
		tabId,
		terminalContextId,
	});
}

/** The move is what the person asked for, so an older revision alone does not make it wrong. */
function stillApplies(tab: ServerTab, { previousRatio, split }: RatioMove): boolean {
	const current = splitsOf(tab.root).find((candidate) => candidate.splitId === split.splitId);
	return (
		current !== undefined &&
		current.children[0].slotId === split.children[0].slotId &&
		current.children[1].slotId === split.children[1].slotId &&
		ratioOf(current) === previousRatio
	);
}

/**
 * One ratio, decided when its turn comes. Every commit anywhere advances the revision, so another
 * Client's unrelated change can refuse this one before it had any effect; the move then goes again,
 * as long as the split still has what it moved from and for as long as the TUI keeps trying.
 */
async function commitNextRatio(
	session: RatioSession,
	context: WorkbenchAuthorityContext,
	change: WorkbenchAuthoritySplitRatioChange,
): Promise<RatioCommit> {
	const turn: RatioTurn = { move: null, stop: null };
	let contentionDeadline: number | null = null;
	for (;;) {
		const submission = await session.submit((snapshot) => {
			const tab = findTab(snapshot, change.terminalContextId, change.tabId);
			if (tab === undefined) {
				turn.stop = 'unavailable';
				return null;
			}
			if (turn.move === null) {
				const step = nextRatioStep(change, tab);
				if (step.kind !== 'move') {
					turn.stop = step.kind;
					return null;
				}
				turn.move = step.move;
			} else if (!stillApplies(tab, turn.move)) {
				turn.stop = 'split-changed';
				return null;
			}
			return handOverServerWorkbenchAction(context, ratioAction(change, turn.move));
		});
		if (turn.stop === 'settled') return { kind: 'settled' };
		if (turn.stop !== null) {
			return { kind: 'stopped', result: { code: turn.stop, kind: 'blocked' } };
		}
		if (
			submission.kind === 'settled' &&
			submission.outcome.kind === 'rejected' &&
			submission.outcome.reason === 'revision-mismatch'
		) {
			contentionDeadline ??= performance.now() + RATIO_CONTENTION_MILLISECONDS;
			if (performance.now() >= contentionDeadline) {
				return { kind: 'stopped', result: { code: 'ratio-contended', kind: 'blocked' } };
			}
			continue;
		}
		const result = serverWorkbenchResult<void>(context, submission, () => ({ kind: 'completed' }));
		switch (result.kind) {
			case 'completed':
				return { kind: 'committed' };
			case 'unchanged':
				return { kind: 'settled' };
			// A refused attempt was already handed over, so this can no longer say it never started.
			case 'cancelled':
				return {
					kind: 'stopped',
					result:
						contentionDeadline === null ? result : { code: 'revision-mismatch', kind: 'blocked' },
				};
			default:
				return { kind: 'stopped', result };
		}
	}
}

function changesOneSplit(change: WorkbenchAuthoritySplitRatioChange): boolean {
	return (
		change.type === 'split-ratio-set' ||
		(change.type === 'panel-size-adjusted' && change.adjustment !== 'reset')
	);
}

/**
 * Evening a Tab is several commands, because the Server changes one split at a time. Each is decided
 * from what the last one published, and the first that fails stops the rest. What already committed
 * stays, because a person who asked to even their Panels has no use for undoing part of it.
 */
export async function changeServerSplitRatios(
	session: RatioSession,
	context: WorkbenchAuthorityContext,
	change: WorkbenchAuthoritySplitRatioChange,
): Promise<ActionResult<void>> {
	const tab = findTab(session.snapshot, change.terminalContextId, change.tabId);
	const stepLimit = changesOneSplit(change) || tab === undefined ? 1 : splitsOf(tab.root).length;
	let committed = 0;
	while (committed < stepLimit) {
		const commit = await commitNextRatio(session, context, change);
		if (commit.kind === 'settled') break;
		if (commit.kind === 'stopped') {
			return committed > 0 && commit.result.kind === 'cancelled'
				? { kind: 'completed' }
				: commit.result;
		}
		committed += 1;
	}
	return committed === 0 ? { kind: 'unchanged' } : { kind: 'completed' };
}

/** A new terminal is named by its place, as every Client names it. The Server limits the children. */
export function panelTerminalCreateAction(
	creation: WorkbenchAuthorityPanelTerminalCreation,
	snapshot: WorkbenchSnapshot,
): ServerWorkbenchAction | null {
	const { cwd, panelId, tabId, terminalContextId } = creation;
	const tab = findTab(snapshot, terminalContextId, tabId);
	if (tab === undefined) return null;
	const panels = panelsOf(tab.root);
	const launch = {
		geometry: DEFAULT_TERMINAL_GEOMETRY,
		...(cwd === undefined ? {} : { options: { cwd } }),
		panelId,
		tabId,
		terminalContextId,
	};
	if (creation.type === 'panel-split') {
		return readServerWorkbenchAction({
			...launch,
			direction: creation.direction,
			kind: 'panel.split',
			label: `${panels.length + 1}`,
		});
	}
	const panel = panels.find((candidate) => candidate.panelId === panelId);
	return panel === undefined
		? null
		: readServerWorkbenchAction({
				...launch,
				kind: 'panel.create-terminal',
				target: { kind: 'child', label: `${panel.childPresentations.length + 2}` },
			});
}

export function panelTerminalCreated(
	outcome: ServerWorkbenchOutcome,
): ActionResult<WorkbenchAuthorityPanelTerminalReceipt> {
	switch (outcome.kind) {
		case 'panel-split-created':
		case 'panel-split-pending':
			return {
				kind: 'completed',
				value: {
					panelId: outcome.newPanelId,
					presentationId: outcome.presentationId,
					tabId: outcome.tabId,
					terminalContextId: outcome.terminalContextId,
					terminalStarted:
						outcome.kind === 'panel-split-created' && outcome.terminalOutcome.kind === 'started',
				},
			};
		case 'panel-terminal-created':
		case 'panel-terminal-pending':
			return {
				kind: 'completed',
				value: {
					panelId: outcome.panelId,
					presentationId: outcome.presentationId,
					tabId: outcome.tabId,
					terminalContextId: outcome.terminalContextId,
					terminalStarted:
						outcome.kind === 'panel-terminal-created' && outcome.terminalOutcome.kind === 'started',
				},
			};
		default:
			return { kind: 'completed' };
	}
}
