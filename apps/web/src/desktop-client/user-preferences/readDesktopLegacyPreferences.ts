import type { UserPreferencesPatch } from '@orchbee/workbench-client-protocol';

import { readLegacyLocalePreferencePatch } from '../../application-locale';
import { readLegacyAppearancePreferencePatch } from '../../terminal-settings';
import { readLegacyFeedbackPreferencePatch } from '../../terminal-feedback-settings';
import { readLegacyInteractionPreferencePatch } from '../../terminal-interaction-settings';

/** Invalid old data remains visible; it is never imported as defaults or silently discarded. */
export function readDesktopLegacyPreferences(
	storage: Pick<Storage, 'getItem' | 'setItem'>,
): UserPreferencesPatch | null {
	const locale = readLegacyLocalePreferencePatch(storage);
	const appearance = readLegacyAppearancePreferencePatch(storage);
	const interaction = readLegacyInteractionPreferencePatch(storage);
	const feedback = readLegacyFeedbackPreferencePatch(storage);
	if (locale === null || appearance === null || interaction === null || feedback === null)
		return null;
	return { ...locale, ...appearance, ...interaction, ...feedback };
}
