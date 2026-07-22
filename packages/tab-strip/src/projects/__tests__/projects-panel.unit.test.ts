import { describe, expect, it, vi } from 'vitest';
import {
	calculateProjectsPanelLayout,
	getProjectsPanelCloseButtonOpacity,
	PROJECTS_PANEL_MIN_WIDTH,
} from '../projects-panel-layout';
import {
	ProjectsPanelController,
	type ProjectThread,
	type SavedProjectTabGroup,
} from '../ProjectsPanelController';

function group(id: string, position?: number): SavedProjectTabGroup {
	return {
		id,
		title: id,
		color: 'blue',
		...(position === undefined ? {} : { position }),
	};
}

function thread(
	id: string,
	lastTurnTime: number,
	type: ProjectThread['type'] = 'ai-mode',
): ProjectThread {
	return { taskId: `task-${id}`, serverId: id, title: id, lastTurnTime, type };
}

describe('Projects panel ports', () => {
	it('ports the controls-only proposed layout', () => {
		expect(
			calculateProjectsPanelLayout({ controlsVisible: true, controlsPreferredHeight: 50 }),
		).toEqual({
			hostSize: { width: PROJECTS_PANEL_MIN_WIDTH, height: 74 },
			controlsBounds: { x: 12, y: 12, width: 216, height: 50 },
		});
		expect(
			calculateProjectsPanelLayout({
				width: 300,
				height: 500,
				controlsVisible: false,
				controlsPreferredHeight: 50,
			}),
		).toEqual({ hostSize: { width: 300, height: 500 }, controlsBounds: null });
	});

	it('delays the macOS close-button fade until the panel is half open', () => {
		expect(getProjectsPanelCloseButtonOpacity(0, true)).toBe(0);
		expect(getProjectsPanelCloseButtonOpacity(0.5, true)).toBe(0);
		expect(getProjectsPanelCloseButtonOpacity(0.75, true)).toBe(0.5);
		expect(getProjectsPanelCloseButtonOpacity(1, true)).toBe(1);
		expect(getProjectsPanelCloseButtonOpacity(0, false)).toBe(1);
	});

	it('preserves initialization order and clamps inserted positions', () => {
		const controller = new ProjectsPanelController();
		controller.initializeTabGroups([group('one'), group('three')]);
		controller.addTabGroup(group('two', 1));
		controller.addTabGroup(group('zero', -20));
		expect(controller.getSnapshot().tabGroups.map((item) => item.id)).toEqual([
			'zero',
			'one',
			'two',
			'three',
		]);
		controller.updateTabGroup({ ...group('two'), title: 'updated' });
		controller.removeTabGroup('one');
		expect(controller.getSnapshot().tabGroups.map((item) => item.title)).toEqual([
			'zero',
			'updated',
			'three',
		]);
	});

	it('delegates up/down reorders to before/after service calls', () => {
		const before = vi.fn<() => void>();
		const after = vi.fn<() => void>();
		const controller = new ProjectsPanelController({
			onReorderGroupBefore: before,
			onReorderGroupAfter: after,
		});
		controller.initializeTabGroups([group('one'), group('two'), group('three')]);
		controller.moveTabGroup('three', 0);
		controller.moveTabGroup('one', 2);
		expect(before).toHaveBeenCalledWith('three', 'one');
		expect(after).toHaveBeenCalledWith('one', 'three');
	});

	it('sorts threads newest-first and filters them through panel state', () => {
		const controller = new ProjectsPanelController({ canShowGeminiThreads: () => false });
		controller.initializeThreads([
			thread('old', 1),
			thread('gemini', 3, 'gemini'),
			thread('new', 2),
		]);
		expect(controller.getSnapshot().threads.map((item) => item.serverId)).toEqual(['new', 'old']);
		controller.updateThread({ ...thread('old', 4), title: 'updated' });
		expect(controller.getSnapshot().threads.map((item) => item.title)).toEqual(['updated', 'new']);
		controller.removeThreadByTaskId('task-old');
		expect(controller.getSnapshot().threads.map((item) => item.serverId)).toEqual(['new']);
	});
});
