import { cleanup, render, screen } from '@testing-library/react';
import { ProductIcon } from '@orchbee/ui/components';
import { afterEach, describe, expect, it } from 'vitest';
import { TabStripControlButton } from '../TabStripControlButton';

afterEach(cleanup);

describe('TabStripControlButton', () => {
	it('renders one stable system visual target without Chromium corner geometry', () => {
		render(
			<TabStripControlButton
				aria-label='Control'
				icon={<ProductIcon name='add' />}
				inlineStart={12}
			/>,
		);
		const button = screen.getByRole('button', { name: 'Control' });
		const target = button.querySelector('[data-tab-strip-control-target]');
		expect(button.getAttribute('tabindex')).toBe('-1');
		expect(button.style.getPropertyValue('--ob-comp-tab-strip-inline-start')).toBe('12px');
		expect(button.style.getPropertyValue('--ob-comp-tab-strip-corner-top-left')).toBe('');
		expect(target).not.toBeNull();
		expect(target?.querySelector('[data-icon="add"]')).not.toBeNull();
	});

	it('keeps the same visual target inside the condensed frame hit area', () => {
		render(
			<TabStripControlButton
				aria-label='Control'
				frameCondensed
				icon={<ProductIcon name='add' />}
			/>,
		);
		const button = screen.getByRole('button', { name: 'Control' });
		expect(button.getAttribute('data-frame-condensed')).toBe('true');
		expect(button.querySelectorAll('[data-tab-strip-control-target]')).toHaveLength(1);
	});
});
