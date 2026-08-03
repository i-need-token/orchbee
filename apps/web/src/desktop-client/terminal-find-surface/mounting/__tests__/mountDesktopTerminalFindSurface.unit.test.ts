import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopTerminalFindController,
	type DesktopTerminalSearchDriver,
} from '../../model/DesktopTerminalFindController';
import { mountDesktopTerminalFindSurface } from '../mountDesktopTerminalFindSurface';

const rootCalls = vi.hoisted(() => ({
	render: vi.fn<(node: ReactNode) => void>(),
	unmount: vi.fn<() => void>(),
}));

vi.mock('react-dom/client', () => ({
	createRoot: vi.fn<() => typeof rootCalls>(() => rootCalls),
}));

describe('mountDesktopTerminalFindSurface', () => {
	it('renders one localized surface and releases its React root', () => {
		const controller = new DesktopTerminalFindController({
			clearSearch: vi.fn<DesktopTerminalSearchDriver['clearSearch']>(),
			updateSearch: vi.fn<DesktopTerminalSearchDriver['updateSearch']>(),
		});
		const root = documentLikeElement();
		const mount = mountDesktopTerminalFindSurface({
			controller,
			locale: 'en-US',
			onNext: vi.fn<() => void>(),
			onPrevious: vi.fn<() => void>(),
			restoreFocus: vi.fn<() => void>(),
			root,
		});

		expect(rootCalls.render).toHaveBeenCalledOnce();
		mount.dispose();
		expect(rootCalls.unmount).toHaveBeenCalledOnce();
	});
});

function documentLikeElement(): HTMLElement {
	return Object.create(null) as HTMLElement;
}
