const zeroWidthProperty = /^(?:\p{Mark}|\p{Default_Ignorable_Code_Point})$/u;

/**
 * Return the pinned Ghostty-compatible standalone width of one Unicode code point.
 *
 * The caller supplies the package-pinned Unicode East Asian Width classifier. Ghostty-specific
 * zero-width and clamped three-em-dash cases mirror `src/unicode/main.zig` and
 * `src/simd/codepoint_width.cpp` at the repository's pinned Ghostty revision. This is presentation
 * data for browser-native preedit geometry; Terminal Host remains the only VT owner.
 */
export function ghosttyCodepointWidth(
	codepoint: number,
	eastAsianWidthOf: (codepoint: number) => number,
): 0 | 1 | 2 {
	if (!Number.isInteger(codepoint) || codepoint < 0 || codepoint > 0x10_ffff) return 1;
	if (
		codepoint <= 0x1f ||
		(codepoint >= 0x7f && codepoint <= 0x9f) ||
		(codepoint >= 0xd800 && codepoint <= 0xdfff) ||
		(codepoint >= 0x1160 && codepoint <= 0x11ff) ||
		(codepoint >= 0x2060 && codepoint <= 0x206f) ||
		(codepoint >= 0xfff0 && codepoint <= 0xfff8)
	) {
		return 0;
	}
	if (codepoint === 0x2e3a || codepoint === 0x2e3b) return 2;
	if (codepoint >= 0x1f1e6 && codepoint <= 0x1f1ff) return 2;
	if (zeroWidthProperty.test(String.fromCodePoint(codepoint))) return 0;
	return eastAsianWidthOf(codepoint) === 2 ? 2 : 1;
}
