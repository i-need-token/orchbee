import { keyboardShortcutSequenceKey } from '@orchbee/keyboard-shortcuts';
import { describe, expect, it } from 'vitest';

import { defaultKeyboardShortcutBindings } from '../keyboard-shortcut-catalog';
import {
	findKeyboardShortcutScopedConflict,
	keyboardShortcutScopedDeclarations,
	normalizeKeyboardShortcutScopedSequence,
	replaceKeyboardShortcutScopedOverride,
	type KeyboardShortcutScopedDeclaration,
} from '../keyboard-shortcut-scoped-catalog';

const PLATFORMS = ['apple', 'standard'] as const;

function exclusive(
	first: KeyboardShortcutScopedDeclaration,
	second: KeyboardShortcutScopedDeclaration,
): boolean {
	return (
		first.definition.exclusiveWith === second.shortcutId ||
		second.definition.exclusiveWith === first.shortcutId
	);
}

describe('keyboard shortcut scoped catalog', () => {
	it.each(PLATFORMS)(
		'gives %s scoped defaults that Settings could record and no key press reaches twice',
		(platform) => {
			expect(
				findKeyboardShortcutScopedConflict(defaultKeyboardShortcutBindings(platform), [], platform),
			).toBeNull();
			const unrecordable = keyboardShortcutScopedDeclarations(platform).filter((declaration) => {
				const recorded = normalizeKeyboardShortcutScopedSequence(
					declaration,
					declaration.defaultSequence,
					platform,
				);
				return (
					recorded === null ||
					keyboardShortcutSequenceKey(recorded, platform) !==
						keyboardShortcutSequenceKey(declaration.defaultSequence, platform)
				);
			});
			expect(unrecordable.map((declaration) => declaration.shortcutId)).toEqual([]);
		},
	);

	it.each(PLATFORMS)(
		'lets two %s shortcuts share keys in one scope only when they are never offered together',
		(platform) => {
			const bindings = defaultKeyboardShortcutBindings(platform);
			const declarations = keyboardShortcutScopedDeclarations(platform);
			const misjudged: string[] = [];
			for (const holder of declarations) {
				for (const taker of declarations) {
					if (taker === holder || taker.definition.scope !== holder.definition.scope) continue;
					const overrides = replaceKeyboardShortcutScopedOverride(
						[],
						taker.shortcutId,
						holder.defaultSequence,
					);
					const accepted =
						findKeyboardShortcutScopedConflict(bindings, overrides, platform) === null;
					if (accepted !== exclusive(holder, taker)) {
						misjudged.push(`${taker.shortcutId} on the keys of ${holder.shortcutId}`);
					}
				}
			}
			expect(misjudged).toEqual([]);
		},
	);
});
