import {
	getHorizontalGroupDragActiveSurface,
	getHorizontalGroupDragInvariantViolations,
} from './horizontal-group-drag-invariants';
import {
	createHorizontalGroupDragState,
	type HorizontalGroupDragAuthoritativeState,
	transitionHorizontalGroupDrag,
	type HorizontalGroupDragState,
} from '../horizontal-group-drag-machine';
import { requireDefined } from '../../utils/requireDefined';
import fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { TabModel, type TabData, type TabGroupData } from '../../model/tab-model';
import { moveContiguousBlock } from '../../model/moveContiguousBlock';

import {
	applyHorizontalGroupDragCommands,
	assertHorizontalGroupDragRefinement,
	createArmedHorizontalGroupDrag,
} from '../horizontal-group-drag-model-adapter';
const PROPERTY_SEED = 20_260_721;

/**
 * Executable refinement of the two list normal forms proved in
 * formal/lean/MoveContiguousBlock.lean.
 */
function leanMoveNormalForm<T>(
	items: readonly T[],
	start: number,
	length: number,
	preRemovalTargetIndex: number,
): readonly T[] {
	const target = Math.min(Math.max(preRemovalTargetIndex, 0), items.length);
	const end = start + length;
	if (target >= start && target <= end) return [...items];
	const moving = items.slice(start, end);
	if (target < start) {
		const before = items.slice(0, target);
		const crossed = items.slice(target, start);
		const after = items.slice(end);
		return [...before, ...moving, ...crossed, ...after];
	}
	const before = items.slice(0, start);
	const crossed = items.slice(end, target);
	const after = items.slice(target);
	return [...before, ...crossed, ...moving, ...after];
}
interface GeneratedScenario {
	readonly activeTabId: string;
	readonly groups: readonly TabGroupData[];
	readonly sourceGroupId: string;
	readonly tabs: readonly TabData[];
	readonly targets: readonly number[];
}
const groupColors = ['blue', 'purple', 'green', 'red'] as const;
const scenarioArbitrary = fc
	.record({
		groupSizes: fc.array(
			fc.integer({
				min: 1,
				max: 4,
			}),
			{
				minLength: 1,
				maxLength: 4,
			},
		),
		collapsedBits: fc.array(fc.boolean(), {
			minLength: 1,
			maxLength: 4,
		}),
		pinnedCount: fc.integer({
			min: 0,
			max: 2,
		}),
		sourceIndex: fc.nat(),
		activeInSource: fc.boolean(),
		activeOffset: fc.nat(),
		targets: fc.array(
			fc.integer({
				min: -3,
				max: 28,
			}),
			{
				minLength: 0,
				maxLength: 24,
			},
		),
	})
	.map((input): GeneratedScenario => {
		const sourceIndex = input.sourceIndex % input.groupSizes.length;
		const groups: TabGroupData[] = input.groupSizes.map((_, index) => ({
			id: `g${index}`,
			title: `Group ${index}`,
			color: requireDefined(groupColors[index % groupColors.length]),
			collapsed: input.collapsedBits[index % input.collapsedBits.length] ?? false,
		}));
		const tabs: TabData[] = [];
		for (let index = 0; index < input.pinnedCount; index += 1) {
			tabs.push({
				id: `p${index}`,
				title: `Pinned ${index}`,
				pinned: true,
				group: null,
			});
		}
		tabs.push({
			id: 'loose-head',
			title: 'Loose head',
			pinned: false,
			group: null,
		});
		input.groupSizes.forEach((size, groupIndex) => {
			for (let member = 0; member < size; member += 1) {
				tabs.push({
					id: `g${groupIndex}-t${member}`,
					title: `Group ${groupIndex} tab ${member}`,
					pinned: false,
					group: `g${groupIndex}`,
				});
			}
			tabs.push({
				id: `loose-${groupIndex}`,
				title: `Loose ${groupIndex}`,
				pinned: false,
				group: null,
			});
		});
		const sourceMembers = tabs.filter((tab) => tab.group === `g${sourceIndex}`);
		const activeTabId = input.activeInSource
			? requireDefined(sourceMembers[input.activeOffset % sourceMembers.length]).id
			: 'loose-head';
		return {
			tabs,
			groups,
			activeTabId,
			sourceGroupId: `g${sourceIndex}`,
			targets: input.targets,
		};
	});
function runScenario(
	scenario: Readonly<GeneratedScenario>,
	sourceCollapsed: boolean,
	outcome: 'cancel' | 'commit',
): Readonly<{
	final: HorizontalGroupDragState;
	initialActiveTabId: string;
	initialIds: readonly string[];
}> {
	const groups = scenario.groups.map((group) =>
		group.id === scenario.sourceGroupId
			? {
					...group,
					collapsed: sourceCollapsed,
				}
			: group,
	);
	const activeTabId = scenario.activeTabId;
	const model = new TabModel({
		tabs: scenario.tabs,
		groups,
		activeTabId,
		selectedTabIds: [activeTabId],
	});
	const restoreState = model.captureDragRestoreState();
	const initialIds = model.getSnapshot().tabs.map((tab) => tab.id);
	let state = createArmedHorizontalGroupDrag(model.getSnapshot(), scenario.sourceGroupId);
	assertHorizontalGroupDragRefinement(model, state);
	expect(getHorizontalGroupDragInvariantViolations(state)).toEqual([]);
	let result = transitionHorizontalGroupDrag(state, {
		type: 'activate-drag',
	});
	applyHorizontalGroupDragCommands(model, restoreState, result.commands);
	state = result.state;
	assertHorizontalGroupDragRefinement(model, state);
	expect(getHorizontalGroupDragInvariantViolations(state)).toEqual([]);
	for (const target of scenario.targets) {
		result = transitionHorizontalGroupDrag(state, {
			type: 'move-to',
			preRemovalTargetIndex: target,
		});
		applyHorizontalGroupDragCommands(model, restoreState, result.commands);
		state = result.state;
		assertHorizontalGroupDragRefinement(model, state);
		expect(getHorizontalGroupDragInvariantViolations(state)).toEqual([]);
	}
	result = transitionHorizontalGroupDrag(state, {
		type: outcome,
	});
	applyHorizontalGroupDragCommands(model, restoreState, result.commands);
	state = result.state;
	assertHorizontalGroupDragRefinement(model, state);
	expect(getHorizontalGroupDragInvariantViolations(state)).toEqual([]);
	return {
		final: state,
		initialActiveTabId: activeTabId,
		initialIds,
	};
}
describe('horizontal group drag properties', () => {
	it('moves any valid contiguous block without losing, duplicating, or reordering its members', () => {
		const moveArbitrary = fc
			.uniqueArray(fc.integer(), {
				minLength: 1,
				maxLength: 24,
			})
			.chain((items) =>
				fc
					.record({
						start: fc.integer({
							min: 0,
							max: items.length - 1,
						}),
						lengthSeed: fc.nat(),
						target: fc.integer({
							min: -4,
							max: items.length + 4,
						}),
					})
					.map(({ start, lengthSeed, target }) => ({
						items,
						start,
						length: 1 + (lengthSeed % (items.length - start)),
						target,
					})),
			);
		fc.assert(
			fc.property(moveArbitrary, ({ items, start, length, target }) => {
				const original = [...items];
				const block = items.slice(start, start + length);
				const moved = moveContiguousBlock(items, {
					start,
					length,
					preRemovalTargetIndex: target,
				});
				expect(items).toEqual(original);
				expect(moved).toEqual(leanMoveNormalForm(items, start, length, target));
				expect([...moved].sort((a, b) => a - b)).toEqual([...items].sort((a, b) => a - b));
				const movedStart = moved.findIndex((value) => value === block[0]);
				expect(moved.slice(movedStart, movedStart + block.length)).toEqual(block);
			}),
			{
				seed: PROPERTY_SEED,
				numRuns: 500,
			},
		);
	});
	it.each([false, true])(
		'preserves every invariant and restores collapsed intent on commit (collapsed=%s)',
		(sourceCollapsed) => {
			fc.assert(
				fc.property(scenarioArbitrary, (scenario) => {
					const { final, initialActiveTabId, initialIds } = runScenario(
						scenario,
						sourceCollapsed,
						'commit',
					);
					expect(final.authoritative.tabs.map((tab) => tab.id).sort()).toEqual(
						[...initialIds].sort(),
					);
					expect(
						final.authoritative.groups.find((group) => group.id === scenario.sourceGroupId)
							?.collapsed,
					).toBe(sourceCollapsed);
					expect(final.authoritative.selectedTabIds).toEqual([final.authoritative.activeTabId]);
					const sourceMembers = final.authoritative.tabs
						.filter((tab) => tab.group === scenario.sourceGroupId)
						.map((tab) => tab.id);
					const expectedActive = sourceMembers.includes(initialActiveTabId)
						? initialActiveTabId
						: sourceMembers[0];
					expect(final.authoritative.activeTabId).toBe(expectedActive);
					expect(getHorizontalGroupDragActiveSurface(final.authoritative)).toEqual(
						sourceCollapsed
							? {
									kind: 'group-header',
									id: scenario.sourceGroupId,
									activeTabId: expectedActive,
								}
							: {
									kind: 'tab',
									id: expectedActive,
									activeTabId: expectedActive,
								},
					);
				}),
				{
					seed: PROPERTY_SEED + Number(sourceCollapsed),
					numRuns: 300,
				},
			);
		},
	);
	it('restores the complete authoritative state after arbitrary moves followed by cancel', () => {
		fc.assert(
			fc.property(scenarioArbitrary, fc.boolean(), (scenario, sourceCollapsed) => {
				const initialGroups = scenario.groups.map((group) =>
					group.id === scenario.sourceGroupId
						? {
								...group,
								collapsed: sourceCollapsed,
							}
						: group,
				);
				const expected = createArmedHorizontalGroupDrag(
					new TabModel({
						tabs: scenario.tabs,
						groups: initialGroups,
						activeTabId: scenario.activeTabId,
						selectedTabIds: [scenario.activeTabId],
					}).getSnapshot(),
					scenario.sourceGroupId,
				).authoritative;
				const { final } = runScenario(scenario, sourceCollapsed, 'cancel');
				expect(final.drag.kind).toBe('idle');
				expect(final.authoritative).toEqual(expected);
			}),
			{
				seed: PROPERTY_SEED + 2,
				numRuns: 300,
			},
		);
	});
	it('keeps every active group member represented by its header across collapse and expand', () => {
		fc.assert(
			fc.property(
				fc.integer({
					min: 1,
					max: 8,
				}),
				fc.nat(),
				(groupSize, activeSeed) => {
					const tabs = Array.from(
						{
							length: groupSize,
						},
						(_, index): TabData => ({
							id: `member-${index}`,
							title: `Member ${index}`,
							pinned: false,
							group: 'work',
						}),
					);
					tabs.push({
						id: 'outside',
						title: 'Outside',
						pinned: false,
						group: null,
					});
					const activeTabId = `member-${activeSeed % groupSize}`;
					const model = new TabModel({
						tabs,
						groups: [
							{
								id: 'work',
								title: 'Work',
								color: 'blue',
								collapsed: false,
							},
						],
						activeTabId,
						selectedTabIds: [activeTabId],
					});
					model.toggleGroupCollapsed('work');
					expect(model.getSnapshot().activeTabId).toBe(activeTabId);
					expect(getHorizontalGroupDragActiveSurface(model.getSnapshot())).toEqual({
						kind: 'group-header',
						id: 'work',
						activeTabId,
					});
					model.toggleGroupCollapsed('work');
					expect(model.getSnapshot().activeTabId).toBe(activeTabId);
					expect(getHorizontalGroupDragActiveSurface(model.getSnapshot())).toEqual({
						kind: 'tab',
						id: activeTabId,
						activeTabId,
					});
				},
			),
			{
				seed: PROPERTY_SEED + 3,
				numRuns: 300,
			},
		);
	});
});

function makeState(
	input: Partial<HorizontalGroupDragAuthoritativeState> = {},
): HorizontalGroupDragState {
	return createHorizontalGroupDragState({
		tabs: [
			{ id: 'r1', pinned: false, group: 'research' },
			{ id: 'r2', pinned: false, group: 'research' },
			{ id: 'b1', pinned: false, group: 'build' },
			{ id: 'b2', pinned: false, group: 'build' },
		],
		groups: [
			{ id: 'research', collapsed: false },
			{ id: 'build', collapsed: false },
		],
		activeTabId: 'b1',
		selectedTabIds: ['b1'],
		selectionAnchorId: 'b1',
		...input,
	});
}

function apply(
	state: HorizontalGroupDragState,
	event: Parameters<typeof transitionHorizontalGroupDrag>[1],
): HorizontalGroupDragState {
	return transitionHorizontalGroupDrag(state, event).state;
}

describe('horizontal group drag machine', () => {
	it('moves an expanded group as a full visual and restores one active selection', () => {
		let state = makeState();
		state = apply(state, { type: 'arm-group', groupId: 'research' });
		expect(state.drag).toMatchObject({
			kind: 'armed',
			recordedActiveTabId: 'r1',
			visualKind: 'expanded-group',
		});

		const activated = transitionHorizontalGroupDrag(state, { type: 'activate-drag' });
		state = activated.state;
		expect(activated.commands).toEqual([
			{ type: 'set-selection', selectedTabIds: ['r1', 'r2'], activeTabId: 'r1' },
			{ type: 'set-group-collapsed', groupId: 'research', collapsed: true },
		]);
		expect(state.authoritative.groups[0]?.collapsed).toBe(true);
		expect(getHorizontalGroupDragInvariantViolations(state)).toEqual([]);

		state = apply(state, { type: 'move-to', preRemovalTargetIndex: 4 });
		expect(state.authoritative.tabs.map((tab) => tab.id)).toEqual(['b1', 'b2', 'r1', 'r2']);
		expect(getHorizontalGroupDragInvariantViolations(state)).toEqual([]);

		const committed = transitionHorizontalGroupDrag(state, { type: 'commit' });
		state = committed.state;
		expect(committed.commands).toEqual([
			{ type: 'set-group-collapsed', groupId: 'research', collapsed: false },
			{ type: 'set-selection', selectedTabIds: ['r1'], activeTabId: 'r1' },
		]);
		expect(state.authoritative.selectedTabIds).toEqual(['r1']);
		expect(state.authoritative.groups[0]?.collapsed).toBe(false);
		expect(state.drag.kind).toBe('settling');
		expect(getHorizontalGroupDragInvariantViolations(state)).toEqual([]);

		state = apply(state, { type: 'settle' });
		expect(state.drag.kind).toBe('idle');
	});

	it('keeps a previously collapsed group compact and projects its recorded active through the header', () => {
		let state = makeState({
			groups: [
				{ id: 'research', collapsed: true },
				{ id: 'build', collapsed: false },
			],
		});
		state = apply(state, { type: 'arm-group', groupId: 'research' });
		const activated = transitionHorizontalGroupDrag(state, { type: 'activate-drag' });
		state = activated.state;

		expect(state.drag).toMatchObject({ kind: 'dragging', visualKind: 'collapsed-header' });
		expect(activated.commands).toEqual([
			{ type: 'set-selection', selectedTabIds: ['r1', 'r2'], activeTabId: 'r1' },
		]);
		expect(state.authoritative.groups[0]?.collapsed).toBe(true);

		const committed = transitionHorizontalGroupDrag(state, { type: 'commit' });
		state = committed.state;
		expect(state.authoritative.groups[0]?.collapsed).toBe(true);
		expect(committed.commands).toEqual([
			{ type: 'set-selection', selectedTabIds: ['r1'], activeTabId: 'r1' },
		]);
		expect(state.authoritative.activeTabId).toBe('r1');
		expect(state.authoritative.selectedTabIds).toEqual(['r1']);
		expect(getHorizontalGroupDragActiveSurface(state.authoritative)).toEqual({
			kind: 'group-header',
			id: 'research',
			activeTabId: 'r1',
		});
		expect(getHorizontalGroupDragInvariantViolations(state)).toEqual([]);
	});

	it('maps a grouped active identity onto its collapsed header surface', () => {
		const state = makeState({
			groups: [
				{ id: 'research', collapsed: true },
				{ id: 'build', collapsed: false },
			],
			activeTabId: 'r1',
			selectedTabIds: ['r1'],
			selectionAnchorId: 'r1',
		});

		expect(getHorizontalGroupDragActiveSurface(state.authoritative)).toEqual({
			kind: 'group-header',
			id: 'research',
			activeTabId: 'r1',
		});
		expect(getHorizontalGroupDragInvariantViolations(state)).toEqual([]);
	});

	it('records the active member when the active tab already belongs to the group', () => {
		let state = makeState({ activeTabId: 'r2', selectedTabIds: ['r2'], selectionAnchorId: 'r2' });
		state = apply(state, { type: 'arm-group', groupId: 'research' });
		state = apply(state, { type: 'activate-drag' });
		state = apply(state, { type: 'commit' });

		expect(state.authoritative.activeTabId).toBe('r2');
		expect(state.authoritative.selectedTabIds).toEqual(['r2']);
	});

	it('rejects a target inside a neighboring group', () => {
		let state = makeState();
		state = apply(state, { type: 'arm-group', groupId: 'research' });
		state = apply(state, { type: 'activate-drag' });
		const result = transitionHorizontalGroupDrag(state, {
			type: 'move-to',
			preRemovalTargetIndex: 3,
		});

		expect(result.commands).toEqual([]);
		expect(result.state.authoritative.tabs.map((tab) => tab.id)).toEqual(['r1', 'r2', 'b1', 'b2']);
	});

	it('restores the complete authoritative state when canceled', () => {
		const original = makeState();
		let state = apply(original, { type: 'arm-group', groupId: 'research' });
		state = apply(state, { type: 'activate-drag' });
		state = apply(state, { type: 'move-to', preRemovalTargetIndex: 4 });
		const canceled = transitionHorizontalGroupDrag(state, { type: 'cancel' });

		expect(canceled.commands).toEqual([{ type: 'restore' }]);
		expect(canceled.state).toEqual(original);
		expect(getHorizontalGroupDragInvariantViolations(canceled.state)).toEqual([]);
	});

	it('makes repeated and out-of-phase events explicit no-ops', () => {
		const state = makeState();
		expect(transitionHorizontalGroupDrag(state, { type: 'activate-drag' })).toEqual({
			state,
			commands: [],
		});
		expect(transitionHorizontalGroupDrag(state, { type: 'commit' })).toEqual({
			state,
			commands: [],
		});
	});
});
