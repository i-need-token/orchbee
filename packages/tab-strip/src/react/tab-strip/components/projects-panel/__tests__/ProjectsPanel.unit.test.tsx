import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ProjectsPanelController } from '../../../../../projects';
import { ProjectsPanel } from '../ProjectsPanel';

afterEach(cleanup);

function createController(
	onOpenTabGroup = vi.fn<() => void>(),
	onOpenThread = vi.fn<() => void>(),
) {
	const controller = new ProjectsPanelController({ onOpenTabGroup, onOpenThread });
	controller.initializeTabGroups([
		{ id: 'research', title: 'Research', color: 'blue', tabCount: 2 },
	]);
	controller.initializeThreads([
		{
			taskId: 'task-one',
			serverId: 'one',
			title: 'First thread',
			lastTurnTime: 2,
			type: 'ai-mode',
		},
		{
			taskId: 'task-two',
			serverId: 'two',
			title: 'Second thread',
			lastTurnTime: 1,
			type: 'gemini',
		},
	]);
	return controller;
}

describe('ProjectsPanel', () => {
	it('renders controller data and delegates open actions', () => {
		const openGroup = vi.fn<() => void>();
		const openThread = vi.fn<() => void>();
		const controller = createController(openGroup, openThread);
		render(<ProjectsPanel controller={controller} open onClose={vi.fn<() => void>()} />);

		expect(screen.getByRole('region', { name: 'Projects' }).hasAttribute('data-open')).toBe(true);
		fireEvent.click(screen.getByRole('button', { name: 'Open Research, 2 tabs' }));
		fireEvent.click(screen.getByRole('button', { name: 'First thread' }));
		expect(openGroup).toHaveBeenCalledWith('research');
		expect(openThread).toHaveBeenCalledWith(expect.objectContaining({ serverId: 'one' }));
	});

	it('closes with Escape, its control, and an outside pointer', () => {
		const controller = createController();
		const onClose = vi.fn<() => void>();
		const { rerender } = render(<ProjectsPanel controller={controller} open onClose={onClose} />);
		fireEvent.keyDown(window, { key: 'Escape' });
		expect(onClose).toHaveBeenLastCalledWith('escape');

		rerender(<ProjectsPanel controller={controller} open={false} onClose={onClose} />);
		rerender(<ProjectsPanel controller={controller} open onClose={onClose} />);
		fireEvent.click(screen.getByRole('button', { name: 'Close Projects' }));
		expect(onClose).toHaveBeenLastCalledWith('button');

		rerender(<ProjectsPanel controller={controller} open={false} onClose={onClose} />);
		rerender(<ProjectsPanel controller={controller} open onClose={onClose} />);
		fireEvent.pointerDown(document.body);
		expect(onClose).toHaveBeenLastCalledWith('outside-pointer');
	});
});
