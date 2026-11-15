/** Windows terminals copy a selection with Ctrl+C before the key can interrupt the program. */
export function resolveCtrlCCopiesSelection(platform: string): boolean {
	return platform.startsWith('Win');
}
