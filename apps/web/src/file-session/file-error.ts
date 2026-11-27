/**
 * Why a File Bar request failed, by the backend's code. `remote-disconnected` is the Client's own,
 * raised when the machine link drops, and a code the Client does not know arrives as `failed`.
 */
type FileErrorCode =
	| 'busy'
	| 'cancelled'
	| 'content-newer'
	| 'denied'
	| 'disk-full'
	| 'exists'
	| 'failed'
	| 'git-failed'
	| 'invalid-name'
	| 'invalid-regex'
	| 'invalid-request'
	| 'is-directory'
	| 'link-target-outside'
	| 'not-directory'
	| 'not-found'
	| 'outside-scope'
	| 'permission-denied'
	| 'read-only'
	| 'remote-disconnected'
	| 'stale-revision'
	| 'timeout'
	| 'too-large'
	| 'trash-failed'
	| 'trash-unavailable'
	| 'unavailable'
	| 'undo-expired'
	| 'unrepresentable-name'
	| 'unsafe-repository'
	| 'unsupported-location'
	| 'write-incomplete';

/** Why `orchbee-files` cannot serve, when an `unavailable` says; an unknown reason is absent. */
type FileUnavailableReason =
	| 'build-mismatch'
	| 'crashed-repeatedly'
	| 'restarting'
	| 'search-binary'
	| 'stuck-filesystem';

/**
 * A failed request. No presentation text crosses the wire: the frontend localizes the code, and
 * only `detail` is shown verbatim.
 */
export interface FileError {
	readonly code: FileErrorCode;
	/** Content Orchbee did not author, git's stderr or rg's pattern message, at most 4 KiB. */
	readonly detail?: string | undefined;
	/** VS Code's `GitErrorCodes` name for a Git failure, such as `repository-is-locked`. */
	readonly gitErrorCode?: string | undefined;
	/** The operating system's error number behind a filesystem failure. */
	readonly osError?: number | undefined;
	/** Why the backend is unavailable; only with `unavailable`. */
	readonly reason?: FileUnavailableReason | undefined;
}

/** What a request settles to: its value, or the error it failed with. Requests never reject. */
export type FileResult<Value> =
	| { readonly kind: 'error'; readonly error: FileError }
	| { readonly kind: 'ok'; readonly value: Value };
