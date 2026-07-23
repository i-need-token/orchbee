import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { AlertIndicatorButton } from '../AlertIndicatorButton';

afterEach(cleanup);

describe('AlertIndicatorButton', () => {
	it('renders camera, capture, and device alert variants', () => {
		const { rerender, container } = render(<AlertIndicatorButton alertState='video-recording' />);
		expect(
			container.querySelector('[data-tab-alert-indicator]')?.getAttribute('data-alert-color'),
		).toBe('recording');

		rerender(<AlertIndicatorButton alertState='tab-capturing' />);
		expect(
			container.querySelector('[data-tab-alert-indicator]')?.getAttribute('data-alert-color'),
		).toBe('pip');

		rerender(<AlertIndicatorButton alertState='usb-connected' />);
		expect(
			container.querySelector('[data-tab-alert-indicator]')?.getAttribute('data-alert-color'),
		).toBe('default');
	});
});
