import { isValidElement, type ReactNode } from 'react';
import { describe, expect, it } from 'vitest';

import { ComboboxContent } from '../combobox';
import { SelectContent } from '../select';

function readPositionerClassName(content: ReactNode): string {
	if (!isValidElement<{ readonly children?: ReactNode }>(content)) {
		throw new Error('Floating content must render a Portal element.');
	}
	const positioner = content.props.children;
	if (!isValidElement<{ readonly className?: string }>(positioner)) {
		throw new Error('Floating content must render a Positioner inside its Portal.');
	}
	return positioner.props.className ?? '';
}

describe('Floating content layers', () => {
	it('allows dialog consumers to replace the default positioner layer', () => {
		const modalLayer = 'modal-floating-layer';

		for (const className of [
			readPositionerClassName(ComboboxContent({ positionerClassName: modalLayer })),
			readPositionerClassName(SelectContent({ positionerClassName: modalLayer })),
		]) {
			expect(className).toContain(modalLayer);
			expect(className).not.toContain('z-50');
		}
	});

	it('retains the generated default layer outside dialogs', () => {
		expect(readPositionerClassName(ComboboxContent({}))).toContain('z-50');
		expect(readPositionerClassName(SelectContent({}))).toContain('z-50');
	});
});
