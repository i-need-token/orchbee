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

function identifierName(identifier) {
	if (identifier?.type === 'Identifier') return identifier.name;
	if (identifier?.type === 'Literal' && typeof identifier.value === 'string')
		return identifier.value;
	return undefined;
}

function declarationSymbols(declaration) {
	if (declaration === null || declaration === undefined) return [];
	if (declaration?.declare === true) return [];
	if (declaration?.type === 'FunctionDeclaration') {
		const name = identifierName(declaration.id);
		return name === undefined ? [] : [{ name, kind: 'function' }];
	}
	if (declaration?.type === 'ClassDeclaration') {
		const name = identifierName(declaration.id);
		return name === undefined ? [] : [{ name, kind: 'class' }];
	}
	if (declaration?.type === 'VariableDeclaration') {
		return declaration.declarations.flatMap((declarator) => {
			const name = identifierName(declarator.id);
			if (name === undefined) return [];
			if (
				declarator.init?.type === 'ArrowFunctionExpression' ||
				declarator.init?.type === 'FunctionExpression'
			)
				return [{ name, kind: 'function' }];
			if (declarator.init?.type === 'ClassExpression') return [{ name, kind: 'class' }];
			return [{ name, kind: 'other' }];
		});
	}
	if (
		declaration?.type === 'TSInterfaceDeclaration' ||
		declaration?.type === 'TSTypeAliasDeclaration'
	)
		return [];

	const name = identifierName(declaration?.id);
	return name === undefined ? [{ name: undefined, kind: 'other' }] : [{ name, kind: 'other' }];
}

function localSymbols(program) {
	const symbols = new Map();
	for (const statement of program.body ?? []) {
		const declaration =
			statement.type === 'ExportNamedDeclaration' ? statement.declaration : statement;
		for (const symbol of declarationSymbols(declaration)) {
			if (symbol.name !== undefined) symbols.set(symbol.name, symbol.kind);
		}
	}
	return symbols;
}

export function runtimeExports(program) {
	const definitions = localSymbols(program);
	const exports = [];
	for (const statement of program.body ?? []) {
		if (statement.type === 'ExportDefaultDeclaration') {
			exports.push({ name: 'default', kind: 'other' });
			continue;
		}
		if (statement.type === 'ExportAllDeclaration') {
			if (statement.exportKind !== 'type') exports.push({ name: undefined, kind: 'other' });
			continue;
		}
		if (statement.type !== 'ExportNamedDeclaration' || statement.exportKind === 'type') continue;

		exports.push(...declarationSymbols(statement.declaration));
		for (const specifier of statement.specifiers ?? []) {
			if (specifier.exportKind === 'type') continue;
			const name = identifierName(specifier.exported);
			const localName = identifierName(specifier.local);
			const kind =
				!statement.source && localName !== undefined
					? (definitions.get(localName) ?? 'other')
					: 'other';
			exports.push({ name, kind });
		}
	}
	return exports;
}
