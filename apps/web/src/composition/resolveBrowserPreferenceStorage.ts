type BrowserPreferenceStorage = Pick<Storage, 'getItem' | 'setItem'>;

export function resolveBrowserPreferenceStorage(
	readStorage: () => BrowserPreferenceStorage,
): BrowserPreferenceStorage {
	try {
		return readStorage();
	} catch {
		return createEphemeralPreferenceStorage();
	}
}

function createEphemeralPreferenceStorage(): BrowserPreferenceStorage {
	const records = new Map<string, string>();
	return {
		getItem: (key) => records.get(key) ?? null,
		setItem: (key, value) => {
			records.set(key, value);
		},
	};
}
