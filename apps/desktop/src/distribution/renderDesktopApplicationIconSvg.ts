import type { DesktopDistributionProfile } from './desktop-distribution-profile';

const SVG_CLOSING_TAG = '</svg>';

const DISTRIBUTION_ICON_OVERLAYS = {
	dev: (accentColor: string) =>
		`<g data-orchbee-distribution="dev"><circle cx="522" cy="522" r="82" fill="${accentColor}" stroke="#fcfcfb" stroke-width="18"/><path d="m508 482-34 40 34 40m28-80 34 40-34 40" fill="none" stroke="#fcfcfb" stroke-linecap="round" stroke-linejoin="round" stroke-width="18"/></g>`,
	local: (accentColor: string) =>
		`<g data-orchbee-distribution="local"><rect x="431" y="431" width="182" height="182" rx="58" fill="${accentColor}" stroke="#fcfcfb" stroke-width="18"/><path d="M476 527v-26a46 46 0 0 1 92 0v26m-104 0h116v42H464z" fill="none" stroke="#fcfcfb" stroke-linecap="round" stroke-linejoin="round" stroke-width="18"/></g>`,
} as const;

export function renderDesktopApplicationIconSvg(
	baseIconSvg: string,
	profile: DesktopDistributionProfile,
): string {
	const closingTagIndex = baseIconSvg.lastIndexOf(SVG_CLOSING_TAG);
	if (closingTagIndex < 0 || !/^\s*<svg(?:\s|>)/u.test(baseIconSvg)) {
		throw new Error('The Desktop application base icon must be a complete SVG document.');
	}
	if (profile.profileId === 'release') return baseIconSvg;
	const accentColor = profile.icon.accentColor;
	if (accentColor === null) {
		throw new Error(`The ${profile.profileId} Desktop icon requires an accent color.`);
	}
	const overlay = DISTRIBUTION_ICON_OVERLAYS[profile.profileId](accentColor);
	return `${baseIconSvg.slice(0, closingTagIndex)}${overlay}${baseIconSvg.slice(closingTagIndex)}`;
}
