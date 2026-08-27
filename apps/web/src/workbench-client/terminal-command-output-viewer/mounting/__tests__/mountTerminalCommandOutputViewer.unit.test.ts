import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { TerminalCommandOutputViewerController } from '../../model/terminal-command-output-viewer-controller';
import { mountTerminalCommandOutputViewer } from '../mountTerminalCommandOutputViewer';

const rootCalls = vi.hoisted(() => ({
	render: vi.fn<(node: ReactNode) => void>(),
	unmount: vi.fn<() => void>(),
}));

vi.mock('react-dom/client', () => ({
	createRoot: vi.fn<() => typeof rootCalls>(() => rootCalls),
}));

describe('mountTerminalCommandOutputViewer', () => {
	it('renders one localized viewer and releases its React root', () => {
		const controller = new TerminalCommandOutputViewerController();
		const mount = mountTerminalCommandOutputViewer({
			controller,
			locale: 'en-US',
			onCopy: vi.fn<() => void>(),
			platform: 'apple',
			restoreFocus: vi.fn<() => void>(),
			root: Object.create(null) as HTMLElement,
		});

		expect(rootCalls.render).toHaveBeenCalledOnce();
		mount.dispose();
		expect(rootCalls.unmount).toHaveBeenCalledOnce();
	});
});
