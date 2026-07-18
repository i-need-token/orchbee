/** Collapse a node location to its first source character. */
export function startLoc(node) {
	const start = node.loc?.start;
	if (!start) return undefined;
	return { start, end: { line: start.line, column: start.column + 1 } };
}

/** Return whether an AST node introduces a JavaScript function boundary. */
export function isFunctionNode(node) {
	return (
		node?.type === 'ArrowFunctionExpression' ||
		node?.type === 'FunctionDeclaration' ||
		node?.type === 'FunctionExpression'
	);
}

/** Return whether a function node has no containing function. */
export function isTopLevelFunction(node) {
	let ancestor = node.parent;
	while (ancestor) {
		if (isFunctionNode(ancestor)) return false;
		if (ancestor.type === 'Program') return true;
		ancestor = ancestor.parent;
	}
	return false;
}
