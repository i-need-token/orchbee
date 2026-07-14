/** Collapse a node location to its first source character. */
export function startLoc(node) {
	const start = node.loc?.start;
	if (!start) return undefined;
	return { start, end: { line: start.line, column: start.column + 1 } };
}
