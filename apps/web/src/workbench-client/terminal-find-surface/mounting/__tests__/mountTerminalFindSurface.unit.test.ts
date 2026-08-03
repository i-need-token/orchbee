import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import {
	TerminalFindController,
	type TerminalSearchDriver,
} from '../../model/TerminalFindController';
import { mountTerminalFindSurface } from '../mountTerminalFindSurface';

const rootCalls = vi.hoisted(() => ({
	render: vi.fn<(node: ReactNode) => void>(),
	unmount: vi.fn<() => void>(),
}));

vi.mock('react-dom/client', () => ({
	createRoot: vi.fn<() => typeof rootCalls>(() => rootCalls),
}));

describe('mountTerminalFindSurface', () => {
	it('renders one localized surface and releases its React root', () => {
		const controller = new TerminalFindController({
			clearSearch: vi.fn<TerminalSearchDriver['clearSearch']>(),
			updateSearch: vi.fn<TerminalSearchDriver['updateSearch']>(),
		});
		const root = documentLikeElement();
		const mount = mountTerminalFindSurface({
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
