import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import {
	TerminalTextViewerController,
	type TerminalTextViewerSource,
} from '../../model/terminal-text-viewer-controller';
import { mountTerminalTextViewer } from '../mountTerminalTextViewer';

const rootCalls = vi.hoisted(() => ({
	render: vi.fn<(node: ReactNode) => void>(),
	unmount: vi.fn<() => void>(),
}));

vi.mock('react-dom/client', () => ({
	createRoot: vi.fn<() => typeof rootCalls>(() => rootCalls),
}));

describe('mountTerminalTextViewer', () => {
	it('renders one localized viewer and releases its React root', () => {
		const controller = new TerminalTextViewerController();
		const mount = mountTerminalTextViewer({
			controller,
			locale: 'en-US',
			onCopy: vi.fn<(source: TerminalTextViewerSource) => void>(),
			platform: 'apple',
			restoreFocus: vi.fn<() => void>(),
			root: Object.create(null) as HTMLElement,
		});

		expect(rootCalls.render).toHaveBeenCalledOnce();
		mount.dispose();
		expect(rootCalls.unmount).toHaveBeenCalledOnce();
	});
});
