// @vitest-environment jsdom

import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
	type measureMountedTerminalGeometry,
} from '@orchbee/terminal';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { TerminalPresentationConfigurationSource } from '../../../../terminal-presentation';
import type { TerminalProjection, TerminalProjectionClient } from '../../../terminal-projection';
import { MountedTerminalProjectionRequest } from '../MountedTerminalProjectionRequest';

const mountedGeometry = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 96, rows: 32 } as const;

afterEach(() => {
	vi.unstubAllGlobals();
});

describe('MountedTerminalProjectionRequest', () => {
	it('opens with measured geometry and pins that measurement configuration for first presentation', async () => {
		const initial = DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION;
		const updated = { ...initial, fontSize: initial.fontSize + 1 };
		let current = initial;
		const unsubscribe = vi.fn<() => void>();
		const source: TerminalPresentationConfigurationSource = {
			current: () => current,
			subscribe: () => unsubscribe,
		};
		const projection = {} as TerminalProjection;
		const delegateRelease = vi.fn<() => void>();
		const open = vi.fn<TerminalProjectionClient['open']>(() => ({
			release: delegateRelease,
			result: Promise.resolve(projection),
		}));
		const measure = vi
			.fn<typeof measureMountedTerminalGeometry>()
			.mockResolvedValue(mountedGeometry);
		const presentationId = terminalPresentationId('presentation-mounted-geometry');
		const request = new MountedTerminalProjectionRequest(
			document.createElement('div'),
			presentationId,
			{ open },
			source,
			measure,
		);

		await expect(request.result).resolves.toBe(projection);
		expect(measure).toHaveBeenCalledWith(expect.any(HTMLDivElement), initial);
		expect(open).toHaveBeenCalledWith(presentationId, mountedGeometry);

		current = updated;
		const pinned = request.takePresentationConfigurationSource();
		const listener = vi.fn<(configuration: typeof initial) => void>();
		expect(pinned.current()).toBe(initial);
		expect(pinned.subscribe(listener)).toBe(unsubscribe);
		expect(listener).toHaveBeenCalledWith(updated);
		request.release();
		expect(delegateRelease).toHaveBeenCalledOnce();
	});

	it('releases a hidden mount without opening or inventing fallback geometry', async () => {
		const disconnect = vi.fn<() => void>();
		class TestResizeObserver {
			disconnect = disconnect;
			observe(): void {}
			unobserve(): void {}
		}
		vi.stubGlobal('ResizeObserver', TestResizeObserver);
		const open = vi.fn<TerminalProjectionClient['open']>();
		const measure = vi.fn<typeof measureMountedTerminalGeometry>().mockResolvedValue(undefined);
		const source: TerminalPresentationConfigurationSource = {
			current: () => DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
			subscribe: () => () => undefined,
		};
		const request = new MountedTerminalProjectionRequest(
			document.createElement('div'),
			terminalPresentationId('presentation-hidden-before-start'),
			{ open },
			source,
			measure,
		);
		await vi.waitFor(() => expect(measure).toHaveBeenCalledOnce());
		request.release();
		await expect(request.result).rejects.toThrow('Terminal projection measurement was released.');
		expect(open).not.toHaveBeenCalled();
		expect(disconnect).toHaveBeenCalledOnce();
	});
});
