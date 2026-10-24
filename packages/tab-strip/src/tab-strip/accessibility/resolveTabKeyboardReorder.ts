interface TabKeyboardReorderInput {
	readonly commandOrControl: boolean;
	readonly horizontal: boolean;
	readonly key: string;
	readonly mac: boolean;
	readonly shift: boolean;
}

interface TabKeyboardReorder {
	readonly direction: -1 | 1;
	readonly moveToEnd: boolean;
}

/** Mirrors Chromium's platform-normalized keyboard reorder command. */
export function resolveTabKeyboardReorder(
	input: Readonly<TabKeyboardReorderInput>,
): TabKeyboardReorder | null {
	if (input.mac && input.shift && (input.key === 'Home' || input.key === 'End')) {
		return {
			direction: input.key === 'Home' ? -1 : 1,
			moveToEnd: true,
		};
	}
	if (!input.commandOrControl) return null;
	const previousKey = input.horizontal ? 'ArrowLeft' : 'ArrowUp';
	const nextKey = input.horizontal ? 'ArrowRight' : 'ArrowDown';
	if (input.key !== previousKey && input.key !== nextKey) return null;
	return {
		direction: input.key === previousKey ? -1 : 1,
		moveToEnd: input.shift,
	};
}
