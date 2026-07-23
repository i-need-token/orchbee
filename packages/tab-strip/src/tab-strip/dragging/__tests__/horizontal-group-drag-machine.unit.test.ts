import { requireDefined } from '../../utils/requireDefined';
import fc from 'fast-check';
import './horizontal-group-drag-machine.examples';
import { describe, expect, it } from 'vitest';
import { TabModel, type TabData, type TabGroupData } from '../../model/tab-model';
import { moveContiguousBlock } from '../../model/moveContiguousBlock';
import {
	getHorizontalGroupDragActiveSurface,
	getHorizontalGroupDragInvariantViolations,
	transitionHorizontalGroupDrag,
	type HorizontalGroupDragState,
} from '../horizontal-group-drag-machine';
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
