type BrowserPreferenceStorage = Pick<Storage, 'getItem' | 'setItem'>;

export function resolveBrowserPreferenceStorage(
	readStorage: () => BrowserPreferenceStorage,
): BrowserPreferenceStorage {
	try {
		return readStorage();
	} catch {
		return createUnavailablePreferenceStorage();
	}
}

function createUnavailablePreferenceStorage(): BrowserPreferenceStorage {
	return {
		getItem: () => {
			throw new Error('Browser preference storage is unavailable.');
		},
		setItem: () => {
			throw new Error('Browser preference storage is unavailable.');
		},
	};
}
