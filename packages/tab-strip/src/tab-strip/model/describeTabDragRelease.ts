import type { TabStripArrangementChange } from '../types/tab-strip-options';
import type { TabDragRestoreState } from './tab-model';

type TabArrangement = Pick<TabDragRestoreState, 'tabs'>;

function arrangementsMatch(before: TabArrangement, after: TabArrangement): boolean {
	return (
		before.tabs.length === after.tabs.length &&
		before.tabs.every((tab, index) => {
			const candidate = after.tabs[index];
			return candidate?.id === tab.id && candidate.group === tab.group;
		})
	);
}

/** A group whose every member moved travels as a collection, so it cannot join another group. */
function carriesWholeGroup(before: TabArrangement, dragged: ReadonlySet<string>): boolean {
	const partialGroups = new Set(
		before.tabs.filter((tab) => !dragged.has(tab.id)).map((tab) => tab.group),
	);
	return before.tabs.some(
		(tab) => tab.group !== null && dragged.has(tab.id) && !partialGroups.has(tab.group),
	);
}

/**
 * A drag moves the model many times, and an authority takes it as one change: which Tabs moved, the
 * boundary in the order they started from, and where they ended up. Tabs that did not move keep their
 * order throughout, so the first one after the dragged block names that boundary. Null when the
 * release changed nothing an authority holds.
 */
export function describeTabDragRelease(
	before: TabArrangement,
	after: TabArrangement,
	tabIds: readonly string[],
): Extract<TabStripArrangementChange, { kind: 'tabs-dragged' }> | null {
	if (arrangementsMatch(before, after)) return null;
	const dragged = new Set(tabIds);
	const blockEnd = after.tabs.map((tab) => dragged.has(tab.id)).lastIndexOf(true);
	const follower = after.tabs[blockEnd + 1];
	const followerIndex =
		follower === undefined ? -1 : before.tabs.findIndex((tab) => tab.id === follower.id);
	return {
		groupId: carriesWholeGroup(before, dragged)
			? null
			: (after.tabs.find((tab) => dragged.has(tab.id))?.group ?? null),
		insertIndex: followerIndex === -1 ? before.tabs.length : followerIndex,
		kind: 'tabs-dragged',
		sourceOrder: before.tabs.map((tab) => tab.id),
		tabIds,
	};
}
