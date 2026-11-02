import { describe, expect, it } from 'vitest';

import {
	createDesktopDevelopmentScope,
	resolveDesktopDistributionProfile,
} from '../../desktop-distribution-profile';
import { renderDesktopApplicationIconSvg } from '../renderDesktopApplicationIconSvg';

const baseArtwork = '<path d="M0 0h1024v1024H0z"/>';
const baseIcon = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">${baseArtwork}</svg>`;

describe('renderDesktopApplicationIconSvg', () => {
	it('places the release artwork on the macOS grid without a badge', () => {
		const icon = renderDesktopApplicationIconSvg(
			baseIcon,
			resolveDesktopDistributionProfile('release'),
		);
		expect(icon).toContain(
			`<g transform="translate(100 100) scale(0.8046875)">${baseArtwork}</g></svg>`,
		);
		expect(icon).not.toContain('data-orchbee-distribution');
	});

	it('adds the distribution badge after the unchanged artwork', () => {
		const dev = renderDesktopApplicationIconSvg(
			baseIcon,
			resolveDesktopDistributionProfile(
				'dev',
				createDesktopDevelopmentScope('/source/orchbee', 'darwin'),
			),
		);
		const local = renderDesktopApplicationIconSvg(
			baseIcon,
			resolveDesktopDistributionProfile('local'),
		);
		expect(dev).toContain(`${baseArtwork}</g><g data-orchbee-distribution="dev">`);
		expect(dev).toContain('fill="#625bf6"');
		expect(local).toContain(`${baseArtwork}</g><g data-orchbee-distribution="local">`);
		expect(local).toContain('fill="#168f5b"');
	});

	it('rejects artwork that is incomplete or not on the 1024 px grid', () => {
		const release = resolveDesktopDistributionProfile('release');
		expect(() => renderDesktopApplicationIconSvg('<svg>', release)).toThrow('1024 px viewBox');
		expect(() =>
			renderDesktopApplicationIconSvg(
				'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 628 628"></svg>',
				release,
			),
		).toThrow('1024 px viewBox');
	});
});
