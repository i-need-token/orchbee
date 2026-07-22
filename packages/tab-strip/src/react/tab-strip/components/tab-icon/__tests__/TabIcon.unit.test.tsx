import { act, render } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { TabIcon } from '../TabIcon';

const tab = {
	id: 'a',
	title: 'A',
	pinned: false,
	group: null,
	split: null,
} as const;

describe('TabIcon', () => {
	it('renders the synchronized spinner and suppresses the default favicon while loading', () => {
		const { container } = render(<TabIcon tab={{ ...tab, networkState: 'loading' }} />);
		const icon = container.querySelector('.ct-tab-icon');
		expect(icon?.getAttribute('data-loading')).toBe('true');
		expect(icon?.hasAttribute('data-show-favicon')).toBe(false);
		expect(container.querySelector('.ct-tab-icon__spinner')).toBeTruthy();
	});

	it('renders attention and discard decorations with a custom favicon', () => {
		const { container } = render(
			<TabIcon
				tab={{ ...tab, faviconUrl: '/favicon.png', needsAttention: true, discarded: true }}
			/>,
		);
		expect(container.querySelector('.ct-tab-icon')?.getAttribute('data-attention')).toBe('true');
		expect(container.querySelector('.ct-tab-icon')?.getAttribute('data-discarded')).toBe('true');
		expect(container.querySelector('img.ct-tab-icon__favicon')).toBeTruthy();
	});

	it('animates a later crash but shows an initially crashed tab directly', () => {
		vi.useFakeTimers();
		const { container, rerender } = render(<TabIcon tab={tab} />);
		rerender(<TabIcon tab={{ ...tab, crashed: true }} />);
		expect(container.querySelector('.ct-tab-icon')?.getAttribute('data-crash-transition')).toBe(
			'true',
		);
		act(() => vi.advanceTimersByTime(1_000));
		expect(container.querySelector('.ct-tab-icon')?.hasAttribute('data-crash-transition')).toBe(
			false,
		);
		vi.useRealTimers();

		const initial = render(<TabIcon tab={{ ...tab, crashed: true }} />);
		expect(
			initial.container.querySelector('.ct-tab-icon')?.hasAttribute('data-crash-transition'),
		).toBe(false);
	});
});
