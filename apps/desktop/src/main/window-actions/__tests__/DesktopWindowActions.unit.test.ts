import { desktopWindowMessages } from '@orchbee/i18n';
import { describe, expect, it, vi } from 'vitest';

import { DesktopWindowActions, type DesktopWindowActionTarget } from '../DesktopWindowActions';

describe('DesktopWindowActions', () => {
	it('routes menu and keyboard invocations through semantic actions', async () => {
		const target = createTarget();
		const actions = new DesktopWindowActions(target);

		expect(actions.invokeToggleMaximized('menu')).toBe(true);
		expect(actions.invokeToggleFullScreen('keyboard')).toBe(true);
		expect(actions.invokeCreateWindow('keyboard')).toBe(true);
		await vi.waitFor(() => {
			expect(target.createWindow).toHaveBeenCalledOnce();
			expect(target.toggleWindowMaximized).toHaveBeenCalledOnce();
			expect(target.toggleWindowFullScreen).toHaveBeenCalledOnce();
		});
		actions.dispose();
	});

	it('projects checked state from the owned Desktop window', () => {
		const target = createTarget();
		const actions = new DesktopWindowActions(target);

		target.getWindowState.mockReturnValue('maximized');
		expect(actions.getToggleMaximizedAvailability()).toEqual({
			checked: true,
			kind: 'enabled',
		});
		target.getWindowState.mockReturnValue('full-screen');
		expect(actions.getToggleFullScreenAvailability()).toEqual({
			checked: true,
			kind: 'enabled',
		});
		actions.dispose();
	});

	it('blocks unavailable windows and maximization while full screen', () => {
		const target = createTarget();
		const actions = new DesktopWindowActions(target);

		target.getWindowState.mockReturnValue('unavailable');
		target.canCreateWindow.mockReturnValue(false);
		expect(actions.getCreateWindowAvailability()).toEqual({
			kind: 'disabled',
			reason: desktopWindowMessages.unavailable,
		});
		expect(actions.invokeCreateWindow('keyboard')).toBe(false);
		expect(actions.getToggleFullScreenAvailability()).toEqual({
			kind: 'disabled',
			reason: desktopWindowMessages.unavailable,
		});
		expect(actions.invokeToggleFullScreen('menu')).toBe(false);

		target.getWindowState.mockReturnValue('full-screen');
		expect(actions.getToggleMaximizedAvailability()).toEqual({
			kind: 'disabled',
			reason: desktopWindowMessages.maximizedUnavailableInFullScreen,
		});
		expect(actions.invokeToggleMaximized('keyboard')).toBe(false);
		expect(target.toggleWindowFullScreen).not.toHaveBeenCalled();
		expect(target.toggleWindowMaximized).not.toHaveBeenCalled();
		actions.dispose();
	});

	it('reports unchanged when a window disappears between resolution and execution', async () => {
		const target = createTarget();
		target.toggleWindowMaximized.mockReturnValue(false);
		const actions = new DesktopWindowActions(target);

		expect(actions.invokeToggleMaximized('menu')).toBe(true);
		await vi.waitFor(() => expect(target.toggleWindowMaximized).toHaveBeenCalledOnce());
		actions.dispose();
	});
});

function createTarget() {
	return {
		canCreateWindow: vi.fn<DesktopWindowActionTarget['canCreateWindow']>().mockReturnValue(true),
		createWindow: vi.fn<DesktopWindowActionTarget['createWindow']>().mockResolvedValue(true),
		getWindowState: vi.fn<DesktopWindowActionTarget['getWindowState']>().mockReturnValue('normal'),
		toggleWindowFullScreen: vi
			.fn<DesktopWindowActionTarget['toggleWindowFullScreen']>()
			.mockReturnValue(true),
		toggleWindowMaximized: vi
			.fn<DesktopWindowActionTarget['toggleWindowMaximized']>()
			.mockReturnValue(true),
	};
}
