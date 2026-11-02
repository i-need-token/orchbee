import type { DesktopDistributionProfile } from '../desktop-distribution-profile';

const SVG_CLOSING_TAG = '</svg>';
const BASE_ICON_OPENING_TAG = /^\s*<svg\s[^>]*\bviewBox="0 0 1024 1024"[^>]*>/u;
// The macOS application icon grid draws an 824 px body centred on the 1024 px canvas.
const DARWIN_ICON_BODY_TRANSFORM = 'translate(100 100) scale(0.8046875)';
const BADGE_RING_COLOR = '#16140F';
const BADGE_GLYPH_COLOR = '#F8F7F4';

const DISTRIBUTION_ICON_BADGES = {
	dev: (accentColor: string) =>
		`<g data-orchbee-distribution="dev"><circle cx="836" cy="836" r="140" fill="${accentColor}" stroke="${BADGE_RING_COLOR}" stroke-width="24"/><path d="M812 768 754 836l58 68m48-136 58 68-58 68" fill="none" stroke="${BADGE_GLYPH_COLOR}" stroke-linecap="round" stroke-linejoin="round" stroke-width="30"/></g>`,
	local: (accentColor: string) =>
		`<g data-orchbee-distribution="local"><rect x="696" y="696" width="280" height="280" rx="90" fill="${accentColor}" stroke="${BADGE_RING_COLOR}" stroke-width="24"/><path d="M766 856v-40a70 70 0 0 1 140 0v40m-160 0h180v70H746z" fill="none" stroke="${BADGE_GLYPH_COLOR}" stroke-linecap="round" stroke-linejoin="round" stroke-width="30"/></g>`,
} as const;

export function renderDesktopApplicationIconSvg(
	baseIconSvg: string,
	profile: DesktopDistributionProfile,
): string {
	const openingTag = BASE_ICON_OPENING_TAG.exec(baseIconSvg)?.[0];
	const closingTagIndex = baseIconSvg.lastIndexOf(SVG_CLOSING_TAG);
	if (openingTag === undefined || closingTagIndex < openingTag.length) {
		throw new Error(
			'The Desktop application base icon must be a complete SVG document on a 1024 px viewBox.',
		);
	}
	const artwork = baseIconSvg.slice(openingTag.length, closingTagIndex);
	const badge =
		profile.iconBadge === null
			? ''
			: DISTRIBUTION_ICON_BADGES[profile.iconBadge.variant](profile.iconBadge.accentColor);
	return `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024"><g transform="${DARWIN_ICON_BODY_TRANSFORM}">${artwork}</g>${badge}</svg>`;
}
