/** How the File Bar's column moves focus into Source Control. */
export interface SourceControlFocus {
	/**
	 * Focuses the section focused last, else Changes, else Repositories, as VS Code focuses its last
	 * focused view, or the Changes view while its Repositories view is hidden by default.
	 */
	focus(): void;
}
