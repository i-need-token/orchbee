import { isValidElement } from 'react';
import { describe, expect, it } from 'vitest';

import { AlertDialogOverlay } from '../alert-dialog';
import { DialogOverlay } from '../dialog';

describe('Dialog overlays', () => {
	it('keeps ordinary and alert dialogs on the same scrim without backdrop blur', () => {
		const dialogOverlay = DialogOverlay({});
		const alertDialogOverlay = AlertDialogOverlay({});
		if (
			!isValidElement<{ readonly className?: string }>(dialogOverlay) ||
			!isValidElement<{ readonly className?: string }>(alertDialogOverlay)
		) {
			throw new Error('Dialog overlays must return React elements.');
		}

		expect(dialogOverlay.props.className).toContain('bg-black/15');
		expect(alertDialogOverlay.props.className).toContain('bg-black/15');
		expect(dialogOverlay.props.className).toContain('z-(--ob-ref-layer-modal)');
		expect(alertDialogOverlay.props.className).toContain('z-(--ob-ref-layer-modal)');
		expect(dialogOverlay.props.className).not.toContain('backdrop-blur');
		expect(alertDialogOverlay.props.className).not.toContain('backdrop-blur');
	});
});
