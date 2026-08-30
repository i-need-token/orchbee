import { describe, expect, it } from 'vitest';

import {
	createDesktopDevelopmentScope,
	resolveDesktopDistributionProfile,
} from '../desktop-distribution-profile';
import { renderDesktopApplicationIconSvg } from '../renderDesktopApplicationIconSvg';

const baseIcon = '<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0h1v1z"/></svg>';

describe('renderDesktopApplicationIconSvg', () => {
	it('rejects a release distribution without a configured application icon', () => {
		expect(() =>
			renderDesktopApplicationIconSvg(baseIcon, resolveDesktopDistributionProfile('release')),
		).toThrow('does not configure an application icon');
	});

	it('adds the checkout-scoped dev badge without changing the base artwork', () => {
		const profile = resolveDesktopDistributionProfile(
			'dev',
			createDesktopDevelopmentScope('/source/orchbee', 'darwin'),
		);
		const result = renderDesktopApplicationIconSvg(baseIcon, profile);

		expect(result).toContain('<path d="M0 0h1v1z"/>');
		expect(result).toContain('data-orchbee-distribution="dev"');
		expect(result).toContain('fill="#625bf6"');
		expect(result.endsWith('</svg>')).toBe(true);
	});

	it('adds a structurally distinct local badge', () => {
		const result = renderDesktopApplicationIconSvg(
			baseIcon,
			resolveDesktopDistributionProfile('local'),
		);

		expect(result).toContain('data-orchbee-distribution="local"');
		expect(result).toContain('fill="#168f5b"');
		expect(result).toContain('<rect x="431"');
		expect(result).not.toContain('data-orchbee-distribution="dev"');
	});

	it('rejects incomplete source artwork', () => {
		expect(() =>
			renderDesktopApplicationIconSvg('<svg>', resolveDesktopDistributionProfile('release')),
		).toThrow('complete SVG document');
	});
});
