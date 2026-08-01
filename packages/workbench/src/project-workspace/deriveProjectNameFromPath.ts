export function deriveProjectNameFromPath(path: string): string {
	const trimmedPath = path.trim();
	const pathWithoutTrailingSeparators = trimmedPath.replace(/[\\/]+$/u, '');
	if (pathWithoutTrailingSeparators.length === 0) return trimmedPath;
	return pathWithoutTrailingSeparators.split(/[\\/]/u).at(-1) ?? trimmedPath;
}
