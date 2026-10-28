// Report-only Rust module graph check: production module cycles in every Cargo member, plus layer
// direction for members that declare a module-layer contract. Findings never fail the command;
// unreadable manifests, unresolvable inputs, and malformed contracts do.
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPOSITORY_ROOT = path.resolve(import.meta.dirname, '..');
export const CONTRACT_PATH = 'docs/module-layers.md';
const LAYERS_HEADING = 'Layers';
const ASSIGNMENT_HEADING = 'Module assignment';
const SUMMARY_CYCLE_LIMIT = 4;
const SUMMARY_MEMBER_LIMIT = 10;
const SAMPLE_LIMIT = 3;
const ALIAS_DEPTH_LIMIT = 8;

const IDENTIFIER_CHAR = /[A-Za-z0-9_]/u;
const ITEM_KEYWORDS = new Set([
	'async',
	'const',
	'enum',
	'extern',
	'fn',
	'impl',
	'let',
	'macro_rules',
	'mod',
	'static',
	'struct',
	'trait',
	'type',
	'unsafe',
	'use',
]);
const PATH_PATTERN =
	/(?<![A-Za-z0-9_$:])(\$crate|[A-Za-z_][A-Za-z0-9_]*)((?:\s*::\s*[A-Za-z_][A-Za-z0-9_]*)+)/gu;
const RESTRICTED_VISIBILITY_PATTERN = /\bpub\s*\(\s*in\s+[^)]*\)/gu;

function normalizeSeparators(value) {
	return value.split(path.sep).join('/');
}

function blankRange(chars, start, end) {
	for (let index = start; index < end; index += 1) {
		if (chars[index] !== '\n' && chars[index] !== '\r') chars[index] = ' ';
	}
}

function stringLiteralEnd(source, start) {
	for (let index = start; index < source.length; index += 1) {
		if (source[index] === '\\') index += 1;
		else if (source[index] === '"') return index + 1;
	}
	return source.length;
}

function rawStringLiteralEnd(source, index) {
	const before = source[index - 1];
	if (before !== undefined && IDENTIFIER_CHAR.test(before)) {
		const prefixed =
			(before === 'b' || before === 'c') && (index < 2 || !IDENTIFIER_CHAR.test(source[index - 2]));
		if (!prefixed) return -1;
	}
	let cursor = index + 1;
	while (source[cursor] === '#') cursor += 1;
	if (source[cursor] !== '"') return -1;
	const terminator = `"${'#'.repeat(cursor - index - 1)}`;
	const end = source.indexOf(terminator, cursor + 1);
	return end === -1 ? source.length : end + terminator.length;
}

function charLiteralEnd(source, index) {
	if (source[index + 1] === '\\') {
		let cursor = index + 2;
		if (source[cursor] === 'u' && source[cursor + 1] === '{') {
			const close = source.indexOf('}', cursor);
			if (close === -1) return -1;
			cursor = close + 1;
		} else if (source[cursor] === 'x') cursor += 3;
		else cursor += 1;
		return source[cursor] === "'" ? cursor + 1 : -1;
	}
	const codePoint = source.codePointAt(index + 1);
	if (codePoint === undefined || source[index + 1] === "'" || source[index + 1] === '\n') return -1;
	const width = codePoint > 0xffff ? 2 : 1;
	return source[index + 1 + width] === "'" ? index + 2 + width : -1;
}

/**
 * Replaces comments and string, raw-string, and character literals with spaces while preserving
 * every offset and line break, so later scans never match text inside them.
 */
export function maskRustSource(source) {
	const chars = source.split('');
	let index = 0;
	while (index < source.length) {
		const current = source[index];
		const next = source[index + 1];
		if (current === '/' && next === '/') {
			const end = source.indexOf('\n', index);
			const stop = end === -1 ? source.length : end;
			blankRange(chars, index, stop);
			index = stop;
		} else if (current === '/' && next === '*') {
			let depth = 1;
			let cursor = index + 2;
			while (cursor < source.length && depth > 0) {
				if (source[cursor] === '/' && source[cursor + 1] === '*') {
					depth += 1;
					cursor += 2;
				} else if (source[cursor] === '*' && source[cursor + 1] === '/') {
					depth -= 1;
					cursor += 2;
				} else cursor += 1;
			}
			blankRange(chars, index, cursor);
			index = cursor;
		} else if (current === '"') {
			const end = stringLiteralEnd(source, index + 1);
			blankRange(chars, index, end);
			index = end;
		} else if (current === 'r' && (next === '"' || next === '#')) {
			const end = rawStringLiteralEnd(source, index);
			if (end === -1) index += 1;
			else {
				blankRange(chars, index, end);
				index = end;
			}
		} else if (current === "'") {
			const end = charLiteralEnd(source, index);
			if (end === -1) index += 1;
			else {
				blankRange(chars, index, end);
				index = end;
			}
		} else index += 1;
	}
	return chars.join('');
}

function matchingClose(text, openIndex) {
	const open = text[openIndex];
	const close = { '(': ')', '[': ']', '{': '}' }[open];
	let depth = 0;
	for (let index = openIndex; index < text.length; index += 1) {
		if (text[index] === open) depth += 1;
		else if (text[index] === close) {
			depth -= 1;
			if (depth === 0) return index;
		}
	}
	return text.length - 1;
}

function tokenizeCfg(text) {
	const tokens = [];
	const pattern = /\s*(?:([A-Za-z_][A-Za-z0-9_]*)|("(?:[^"\\]|\\.)*")|([(),=]))/gsuy;
	let match;
	while ((match = pattern.exec(text)) !== null) {
		if (match[0].length === 0) break;
		if (match[1] !== undefined) tokens.push({ kind: 'identifier', value: match[1] });
		else if (match[2] !== undefined) tokens.push({ kind: 'string', value: match[2].slice(1, -1) });
		else tokens.push({ kind: 'punctuation', value: match[3] });
	}
	return tokens;
}

/**
 * Evaluates a `cfg` predicate for production code: `test` and the `test-support` feature are
 * false, every other atom is unknown. Returns `true`, `false`, or `unknown`.
 */
export function evaluateProductionCfg(predicate) {
	const tokens = tokenizeCfg(predicate);
	let position = 0;
	function parseList() {
		const values = [];
		if (tokens[position]?.value !== '(') return values;
		position += 1;
		while (position < tokens.length && tokens[position].value !== ')') {
			values.push(parsePredicate());
			if (tokens[position]?.value === ',') position += 1;
		}
		position += 1;
		return values;
	}
	function parsePredicate() {
		const token = tokens[position];
		if (token === undefined || token.kind !== 'identifier') {
			position += 1;
			return 'unknown';
		}
		position += 1;
		if (token.value === 'all' || token.value === 'any' || token.value === 'not') {
			const values = parseList();
			if (token.value === 'not') {
				const [value] = values;
				if (value === 'true') return 'false';
				return value === 'false' ? 'true' : 'unknown';
			}
			if (token.value === 'all') {
				if (values.includes('false')) return 'false';
				return values.every((value) => value === 'true') ? 'true' : 'unknown';
			}
			if (values.includes('true')) return 'true';
			return values.every((value) => value === 'false') ? 'false' : 'unknown';
		}
		if (tokens[position]?.value === '=') {
			const value = tokens[position + 1]?.value;
			position += 2;
			return token.value === 'feature' && value === 'test-support' ? 'false' : 'unknown';
		}
		return token.value === 'test' ? 'false' : 'unknown';
	}
	return parsePredicate();
}

function skipAttributesAndSpace(text, start) {
	let index = start;
	for (;;) {
		while (index < text.length && /\s/u.test(text[index])) index += 1;
		if (text[index] === '#' && text[index + 1] === '[') {
			index = matchingClose(text, index + 1) + 1;
			continue;
		}
		return index;
	}
}

function itemEnd(text, start) {
	let index = skipAttributesAndSpace(text, start);
	let cursor = index;
	const word = () => {
		const match = /^[A-Za-z_][A-Za-z0-9_]*/u.exec(text.slice(cursor, cursor + 64));
		return match?.[0];
	};
	let keyword = word();
	if (keyword === 'pub') {
		cursor += 3;
		while (/\s/u.test(text[cursor] ?? '')) cursor += 1;
		if (text[cursor] === '(') cursor = matchingClose(text, cursor) + 1;
		while (/\s/u.test(text[cursor] ?? '')) cursor += 1;
		keyword = word();
	}
	const itemMode = keyword !== undefined && ITEM_KEYWORDS.has(keyword);
	let depth = 0;
	let angleDepth = 0;
	for (; index < text.length; index += 1) {
		const current = text[index];
		if (current === '(' || current === '[' || current === '{') depth += 1;
		else if (current === ')' || current === ']' || current === '}') {
			if (depth === 0) return index;
			depth -= 1;
			if (depth === 0 && current === '}') return index + 1;
		} else if (depth === 0 && current === ';') return index + 1;
		else if (!itemMode && depth === 0) {
			if (current === '<') angleDepth += 1;
			else if (current === '>' && text[index - 1] !== '-' && text[index - 1] !== '=')
				angleDepth = Math.max(0, angleDepth - 1);
			else if (current === ',' && angleDepth === 0) return index + 1;
		}
	}
	return text.length;
}

/**
 * Blanks items whose `cfg` attributes cannot hold in production builds. Returns the cleaned text
 * and the number of line breaks removed from production accounting.
 */
export function removeTestOnlyCode(masked, original) {
	const chars = masked.split('');
	let removedLines = 0;
	let depth = 0;
	for (let index = 0; index < masked.length; index += 1) {
		const current = chars[index];
		if (current === '{') depth += 1;
		else if (current === '}') depth = Math.max(0, depth - 1);
		if (current !== '#') continue;
		const inner = chars[index + 1] === '!';
		const open = inner ? index + 2 : index + 1;
		if (chars[open] !== '[') continue;
		const close = matchingClose(masked, open);
		const attribute = /^\s*cfg\s*\(([\s\S]*)\)\s*$/u.exec(original.slice(open + 1, close));
		// An inner attribute inside an inline module body gates that module, which its declaration
		// scan already owns; only a file-level inner `cfg` removes the whole file.
		if (
			attribute === null ||
			(inner && depth > 0) ||
			evaluateProductionCfg(attribute[1]) !== 'false'
		) {
			index = close;
			continue;
		}
		const end = inner ? masked.length : itemEnd(masked, close + 1);
		const start = inner ? 0 : index;
		for (let cursor = start; cursor < end; cursor += 1)
			if (chars[cursor] === '\n') removedLines += 1;
		blankRange(chars, start, end);
		if (inner) break;
		index = end - 1;
	}
	return { text: chars.join(''), removedLines };
}

function physicalLines(source) {
	if (source.length === 0) return 0;
	const breaks = source.split('\n').length - 1;
	return source.endsWith('\n') ? breaks : breaks + 1;
}

function lineStarts(source) {
	const starts = [0];
	for (let index = 0; index < source.length; index += 1) {
		if (source[index] === '\n') starts.push(index + 1);
	}
	return starts;
}

function lineAt(starts, offset) {
	let low = 0;
	let high = starts.length - 1;
	while (low < high) {
		const middle = Math.ceil((low + high) / 2);
		if (starts[middle] <= offset) low = middle;
		else high = middle - 1;
	}
	return low + 1;
}

function tokenizeUse(text) {
	const tokens = [];
	const pattern = /\s*(?:(\$?[A-Za-z_][A-Za-z0-9_]*|r#[A-Za-z_][A-Za-z0-9_]*)|(::)|([{},*]))/gsuy;
	let match;
	while ((match = pattern.exec(text)) !== null) {
		if (match[0].length === 0) break;
		tokens.push(match[1] ?? match[2] ?? match[3]);
	}
	return tokens;
}

/** Expands one `use` tree into leaf paths with their bound names. */
export function expandUseTree(text) {
	const tokens = tokenizeUse(text);
	const leaves = [];
	let position = 0;
	function parseTree(prefix) {
		const segments = [...prefix];
		if (tokens[position] === '::') position += 1;
		for (;;) {
			const token = tokens[position];
			if (token === '{') {
				position += 1;
				while (position < tokens.length && tokens[position] !== '}') {
					parseTree(segments);
					if (tokens[position] === ',') position += 1;
					else if (tokens[position] !== '}') position += 1;
				}
				position += 1;
				return;
			}
			if (token === '*') {
				position += 1;
				leaves.push({ segments, binding: undefined, glob: true });
				return;
			}
			if (token === undefined || token === ',' || token === '}') {
				if (segments.length > prefix.length) {
					leaves.push({ segments, binding: segments.at(-1), glob: false });
				}
				return;
			}
			position += 1;
			if (token === 'as') {
				const alias = tokens[position];
				position += 1;
				leaves.push({ segments, binding: alias === '_' ? undefined : alias, glob: false });
				return;
			}
			if (token === 'self' && segments.length > 0 && segments.length === prefix.length) {
				const alias = tokens[position] === 'as' ? tokens[position + 1] : segments.at(-1);
				if (tokens[position] === 'as') position += 2;
				leaves.push({ segments, binding: alias, glob: false });
				return;
			}
			segments.push(token.replace(/^r#/u, ''));
			if (tokens[position] === '::') position += 1;
			else {
				if (tokens[position] === 'as') continue;
				leaves.push({ segments, binding: segments.at(-1), glob: false });
				return;
			}
		}
	}
	parseTree([]);
	return leaves;
}

function createModule(name, parent, directory) {
	return {
		name,
		parent,
		directory,
		path: parent === undefined ? [] : [...parent.path, name],
		children: new Map(),
		files: new Set(),
		segments: [],
		aliases: new Map(),
		localAliases: new Map(),
		productionLines: 0,
	};
}

function moduleKey(module) {
	return module.path.join('::');
}

function declarationAttributes(original, start, end) {
	const window = original.slice(start, end);
	const paths = [];
	let replace = false;
	for (const match of window.matchAll(/#\s*\[\s*path\s*=\s*"([^"]*)"\s*\]/gu)) {
		paths.push(match[1]);
		replace = true;
	}
	for (const match of window.matchAll(/#\s*\[\s*cfg_attr\s*\([^\]]*?\bpath\s*=\s*"([^"]*)"/gu)) {
		paths.push(match[1]);
	}
	return { paths: [...new Set(paths)], replace };
}

function scanScope(text, original, scopeStart, scopeEnd) {
	const declarations = [];
	const inlineRanges = [];
	let depth = 0;
	let boundary = scopeStart;
	for (let index = scopeStart; index < scopeEnd; index += 1) {
		const current = text[index];
		if (current === '{' || current === '(' || current === '[') {
			depth += 1;
			continue;
		}
		if (current === '}' || current === ')' || current === ']') {
			depth -= 1;
			if (depth === 0 && current === '}') boundary = index + 1;
			continue;
		}
		if (depth !== 0) continue;
		if (current === ';') {
			boundary = index + 1;
			continue;
		}
		if (current !== 'm' || IDENTIFIER_CHAR.test(text[index - 1] ?? ' ')) continue;
		const match = /^mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*([;{])/u.exec(
			text.slice(index, Math.min(scopeEnd, index + 256)),
		);
		if (match === null) continue;
		const name = match[1];
		const attributes = declarationAttributes(original, boundary, index);
		const terminator = index + match[0].length - 1;
		if (match[2] === ';') {
			declarations.push({ name, inline: false, attributes, offset: index });
			boundary = terminator + 1;
			index = terminator;
		} else {
			const close = matchingClose(text, terminator);
			declarations.push({
				name,
				inline: true,
				attributes,
				offset: index,
				bodyStart: terminator + 1,
				bodyEnd: close,
			});
			inlineRanges.push([terminator + 1, close]);
			boundary = close + 1;
			index = close;
		}
	}
	return { declarations, inlineRanges };
}

function ownText(text, scopeStart, scopeEnd, inlineRanges) {
	const chars = text.split('');
	blankRange(chars, 0, scopeStart);
	blankRange(chars, scopeEnd, chars.length);
	for (const [start, end] of inlineRanges) blankRange(chars, start, end);
	return chars.join('').replace(RESTRICTED_VISIBILITY_PATTERN, (match) => ' '.repeat(match.length));
}

function moduleFileCandidates(directory, name) {
	return [path.join(directory, `${name}.rs`), path.join(directory, name, 'mod.rs')];
}

function childDirectory(file, modRs) {
	if (modRs) return path.dirname(file);
	return path.join(path.dirname(file), path.basename(file, '.rs'));
}

function loadSourceFile(state, file) {
	const cached = state.files.get(file);
	if (cached !== undefined) return cached;
	const original = readFileSync(file, 'utf8').replaceAll('\r\n', '\n');
	const masked = maskRustSource(original);
	const { text, removedLines } = removeTestOnlyCode(masked, original);
	const loaded = {
		file,
		original,
		text,
		starts: lineStarts(original),
		productionLines: physicalLines(original) - removedLines,
	};
	state.files.set(file, loaded);
	return loaded;
}

function attachScope(state, module, source, scopeStart, scopeEnd, directory, fileDirectory) {
	const { declarations, inlineRanges } = scanScope(
		source.text,
		source.original,
		scopeStart,
		scopeEnd,
	);
	module.segments.push({ source, text: ownText(source.text, scopeStart, scopeEnd, inlineRanges) });
	for (const declaration of declarations) {
		const child =
			module.children.get(declaration.name) ??
			createModule(declaration.name, module, path.join(directory, declaration.name));
		module.children.set(declaration.name, child);
		if (declaration.inline) {
			attachScope(
				state,
				child,
				source,
				declaration.bodyStart,
				declaration.bodyEnd,
				path.join(directory, declaration.name),
				path.join(directory, declaration.name),
			);
			continue;
		}
		const files = [];
		const { paths, replace } = declaration.attributes;
		if (!replace) {
			const found = moduleFileCandidates(directory, declaration.name).find((candidate) =>
				existsSync(candidate),
			);
			if (found !== undefined) files.push({ file: found, modRs: found.endsWith('mod.rs') });
		}
		for (const relative of paths) {
			const candidate = path.resolve(fileDirectory, relative);
			if (existsSync(candidate)) files.push({ file: candidate, modRs: true });
		}
		if (files.length === 0) {
			state.unresolved.push(
				`${normalizeSeparators(path.relative(state.root, source.file))}:${lineAt(source.starts, declaration.offset)} mod ${declaration.name}`,
			);
			continue;
		}
		for (const { file, modRs } of files) loadModuleFile(state, child, file, modRs);
	}
}

function loadModuleFile(state, module, file, modRs) {
	// A platform `cfg_attr(path)` variant adds a file to the same module; never count one twice.
	if (module.files.has(file)) return;
	module.files.add(file);
	const source = loadSourceFile(state, file);
	module.productionLines += source.productionLines;
	attachScope(
		state,
		module,
		source,
		0,
		source.text.length,
		childDirectory(file, modRs),
		path.dirname(file),
	);
}

function collectUses(module) {
	for (const segment of module.segments) {
		const { text } = segment;
		const chars = text.split('');
		let depth = 0;
		segment.uses = [];
		for (let index = 0; index < text.length; index += 1) {
			const current = text[index];
			if (current === '{') depth += 1;
			else if (current === '}') depth = Math.max(0, depth - 1);
			if (current !== 'u' || IDENTIFIER_CHAR.test(text[index - 1] ?? ' ')) continue;
			if (!/^use\s/u.test(text.slice(index, index + 4))) continue;
			let cursor = index + 4;
			let braces = 0;
			while (cursor < text.length && !(text[cursor] === ';' && braces === 0)) {
				if (text[cursor] === '{') braces += 1;
				else if (text[cursor] === '}') braces -= 1;
				if (braces < 0) break;
				cursor += 1;
			}
			const leaves = expandUseTree(text.slice(index + 4, cursor));
			for (const leaf of leaves) {
				segment.uses.push({ ...leaf, offset: index });
				if (leaf.binding === undefined || leaf.glob) continue;
				const table = depth === 0 ? module.aliases : module.localAliases;
				if (!table.has(leaf.binding)) table.set(leaf.binding, leaf.segments);
			}
			blankRange(chars, index, cursor);
			index = cursor;
		}
		segment.pathText = chars.join('');
	}
	for (const child of module.children.values()) collectUses(child);
}

function resolvePath(module, segments, depth = 0) {
	if (segments.length === 0 || depth > ALIAS_DEPTH_LIMIT) return undefined;
	let node;
	let index = 0;
	const [first] = segments;
	if (first === 'crate' || first === '$crate') {
		node = module;
		while (node.parent !== undefined) node = node.parent;
		index = 1;
	} else if (first === 'self') {
		node = module;
		index = 1;
	} else if (first === 'super') {
		node = module;
		while (segments[index] === 'super') {
			node = node.parent ?? node;
			index += 1;
		}
	} else if (module.children.has(first)) {
		node = module;
	} else {
		const aliased = module.aliases.get(first) ?? module.localAliases.get(first);
		if (aliased === undefined) return undefined;
		const target = resolvePath(module, aliased, depth + 1);
		if (target === undefined) return undefined;
		if (target.item !== undefined || segments.length === 1) return target;
		node = target.module;
		index = 1;
	}
	while (index < segments.length && node.children.has(segments[index])) {
		node = node.children.get(segments[index]);
		index += 1;
	}
	const item = segments[index];
	if (item !== undefined && node.aliases.has(item) && node.aliases.get(item).length > 0) {
		const followed = resolvePath(node, node.aliases.get(item), depth + 1);
		if (followed !== undefined && followed.module !== node) return followed;
	}
	return { module: node, item };
}

function collectReferences(module, references) {
	for (const segment of module.segments) {
		const { source } = segment;
		const record = (target, offset) => {
			if (target === undefined || target.module === module) return;
			references.push({
				from: module,
				to: target.module,
				file: source.file,
				line: lineAt(source.starts, offset),
			});
		};
		for (const use of segment.uses) record(resolvePath(module, use.segments), use.offset);
		for (const match of segment.pathText.matchAll(PATH_PATTERN)) {
			const segments = [match[1], ...match[2].split('::').slice(1)].map((value) => value.trim());
			record(resolvePath(module, segments), match.index);
		}
	}
	for (const child of module.children.values()) collectReferences(child, references);
}

function* walkModules(module) {
	yield module;
	for (const child of module.children.values()) yield* walkModules(child);
}

function subtreeLines(module) {
	let total = 0;
	for (const node of walkModules(module)) total += node.productionLines;
	return total;
}

function stronglyConnectedComponents(nodes, edges) {
	let counter = 0;
	const index = new Map();
	const low = new Map();
	const stack = [];
	const onStack = new Set();
	const components = [];
	function connect(node) {
		index.set(node, counter);
		low.set(node, counter);
		counter += 1;
		stack.push(node);
		onStack.add(node);
		for (const target of edges.get(node) ?? []) {
			if (!index.has(target)) {
				connect(target);
				low.set(node, Math.min(low.get(node), low.get(target)));
			} else if (onStack.has(target)) {
				low.set(node, Math.min(low.get(node), index.get(target)));
			}
		}
		if (low.get(node) === index.get(node)) {
			const component = [];
			let member;
			do {
				member = stack.pop();
				onStack.delete(member);
				component.push(member);
			} while (member !== node);
			if (component.length > 1) components.push(component.toSorted());
		}
	}
	for (const node of nodes) if (!index.has(node)) connect(node);
	return components;
}

function siblingCycles(root, references) {
	const levels = new Map();
	for (const reference of references) {
		const from = reference.from.path;
		const to = reference.to.path;
		let common = 0;
		while (common < from.length && common < to.length && from[common] === to[common]) common += 1;
		if (common >= from.length || common >= to.length) continue;
		const parent = from.slice(0, common).join('::');
		const level = levels.get(parent) ?? new Map();
		levels.set(parent, level);
		const targets = level.get(from[common]) ?? new Set();
		level.set(from[common], targets);
		targets.add(to[common]);
	}
	const modulesByKey = new Map([...walkModules(root)].map((module) => [moduleKey(module), module]));
	const cycles = [];
	for (const [parent, edges] of levels) {
		const parentModule = modulesByKey.get(parent);
		const nodes = [...parentModule.children.keys()].toSorted();
		for (const members of stronglyConnectedComponents(nodes, edges)) {
			const memberSet = new Set(members);
			let internalEdges = 0;
			for (const member of members) {
				for (const target of edges.get(member) ?? []) if (memberSet.has(target)) internalEdges += 1;
			}
			cycles.push({
				parent,
				members,
				edges: internalEdges,
				productionLines: members.reduce(
					(total, member) => total + subtreeLines(parentModule.children.get(member)),
					0,
				),
			});
		}
	}
	return cycles.toSorted(
		(left, right) =>
			left.parent.split('::').filter(Boolean).length -
				right.parent.split('::').filter(Boolean).length ||
			right.members.length - left.members.length ||
			left.parent.localeCompare(right.parent),
	);
}

function markdownSection(markdown, heading) {
	const lines = markdown.replaceAll('\r\n', '\n').split('\n');
	const start = lines.findIndex((line) => line.trim() === `## ${heading}`);
	if (start === -1) return undefined;
	const end = lines.findIndex((line, index) => index > start && /^##\s/u.test(line));
	return lines.slice(start + 1, end === -1 ? lines.length : end);
}

function tableRows(lines) {
	const rows = [];
	for (const line of lines) {
		const trimmed = line.trim();
		if (!trimmed.startsWith('|')) continue;
		const cells = trimmed
			.slice(1, trimmed.endsWith('|') ? -1 : undefined)
			.split('|')
			.map((cell) => cell.trim());
		if (cells.every((cell) => /^:?-+:?$/u.test(cell))) continue;
		rows.push(cells);
	}
	return rows.slice(1);
}

function codeToken(cell) {
	const match = /^`([^`]+)`/u.exec(cell ?? '');
	return match?.[1];
}

function layerTable(lines, label, problems) {
	const layers = new Map();
	for (const cells of tableRows(lines)) {
		const layer = codeToken(cells[0]);
		if (layer === undefined) problems.push(`${label}: layer row "${cells[0]}" has no code ID`);
		else if (layers.has(layer)) problems.push(`${label}: duplicate layer \`${layer}\``);
		else layers.set(layer, layers.size);
	}
	return layers;
}

/**
 * Reads a module-layer contract. The first table under `## Layers` lists layer IDs from lowest to
 * highest; without a table, the section's first link to another contract reuses that contract's
 * layer table. Tables under `## Module assignment` map module paths to layer IDs.
 * `readLinked(target)` returns `{ markdown, label }` for a relative Markdown link, or `undefined`.
 */
export function parseLayerContract(markdown, label, readLinked = () => undefined) {
	const problems = [];
	const layerLines = markdownSection(markdown, LAYERS_HEADING);
	const assignmentLines = markdownSection(markdown, ASSIGNMENT_HEADING);
	if (layerLines === undefined) problems.push(`${label}: missing "## ${LAYERS_HEADING}" section`);
	if (assignmentLines === undefined)
		problems.push(`${label}: missing "## ${ASSIGNMENT_HEADING}" section`);
	if (problems.length > 0) return { problems };
	let layers = layerTable(layerLines, label, problems);
	if (layers.size === 0) {
		const link = /\]\(([^)#\s]+\.md)(?:#[^)\s]*)?\)/u.exec(layerLines.join('\n'));
		const linked = link === null ? undefined : readLinked(link[1]);
		const linkedLines =
			linked === undefined ? undefined : markdownSection(linked.markdown, LAYERS_HEADING);
		if (linkedLines !== undefined) layers = layerTable(linkedLines, linked.label, problems);
	}
	if (layers.size === 0)
		problems.push(
			`${label}: "## ${LAYERS_HEADING}" has neither a layer table nor a link to a contract with one`,
		);
	const assignments = new Map();
	for (const cells of tableRows(assignmentLines)) {
		const module = codeToken(cells[0]);
		const layer = codeToken(cells[1]);
		if (module === undefined || layer === undefined) {
			problems.push(`${label}: assignment row "${cells.join(' | ')}" needs code module and layer`);
		} else if (!layers.has(layer)) {
			problems.push(`${label}: \`${module}\` uses unknown layer \`${layer}\``);
		} else if (assignments.has(module)) {
			problems.push(`${label}: duplicate assignment for \`${module}\``);
		} else assignments.set(module, layer);
	}
	return { problems, layers, assignments };
}

function assignedKey(assignments, modulePath) {
	for (let length = modulePath.length; length > 0; length -= 1) {
		const key = modulePath.slice(0, length).join('::');
		if (assignments.has(key)) return key;
	}
	return undefined;
}

function layerFindings(root, references, contract, repositoryRoot) {
	const { layers, assignments } = contract;
	const existing = new Set([...walkModules(root)].map(moduleKey));
	const violations = new Map();
	for (const reference of references) {
		const from = assignedKey(assignments, reference.from.path);
		const to = assignedKey(assignments, reference.to.path);
		if (from === undefined || to === undefined || from === to) continue;
		const fromLayer = assignments.get(from);
		const toLayer = assignments.get(to);
		if (layers.get(fromLayer) >= layers.get(toLayer)) continue;
		const key = `${from}\0${to}`;
		const entry = violations.get(key) ?? {
			from,
			fromLayer,
			to,
			toLayer,
			references: 0,
			samples: [],
		};
		violations.set(key, entry);
		entry.references += 1;
		const sample = `${normalizeSeparators(path.relative(repositoryRoot, reference.file))}:${reference.line}`;
		if (entry.samples.length < SAMPLE_LIMIT && !entry.samples.includes(sample)) {
			entry.samples.push(sample);
		}
	}
	return {
		violations: [...violations.values()].toSorted(
			(left, right) => left.from.localeCompare(right.from) || left.to.localeCompare(right.to),
		),
		unassigned: [...root.children.keys()].filter((name) => !assignments.has(name)).toSorted(),
		stale: [...assignments.keys()].filter((key) => !existing.has(key)).toSorted(),
	};
}

function expandMember(root, pattern) {
	let directories = [root];
	for (const part of pattern.split('/')) {
		const next = [];
		for (const directory of directories) {
			if (part === '*') {
				if (!existsSync(directory)) continue;
				for (const entry of readdirSync(directory, { withFileTypes: true })) {
					if (entry.isDirectory()) next.push(path.join(directory, entry.name));
				}
			} else next.push(path.join(directory, part));
		}
		directories = next;
	}
	return directories.filter((directory) => existsSync(path.join(directory, 'Cargo.toml')));
}

function tomlStringArray(manifest, key) {
	const match = new RegExp(`^${key}\\s*=\\s*\\[([^\\]]*)\\]`, 'mu').exec(manifest);
	if (match === null) return [];
	return [...match[1].matchAll(/"([^"]+)"/gu)].map((entry) => entry[1]);
}

/** Lists Cargo workspace members from the root manifest's literal `members` and `exclude` arrays. */
export function workspaceMembers(repositoryRoot) {
	const manifest = readFileSync(path.join(repositoryRoot, 'Cargo.toml'), 'utf8');
	const workspace = /^\[workspace\]$([\s\S]*?)(?=^\[(?!workspace\])|(?![\s\S]))/mu.exec(manifest);
	if (workspace === null) throw new Error('Cargo.toml has no [workspace] table');
	const excluded = new Set(
		tomlStringArray(workspace[1], 'exclude').map((entry) => path.join(repositoryRoot, entry)),
	);
	const members = new Set();
	for (const pattern of tomlStringArray(workspace[1], 'members')) {
		for (const directory of expandMember(repositoryRoot, pattern)) {
			if (!excluded.has(directory)) members.add(directory);
		}
	}
	return [...members].toSorted();
}

/**
 * Builds the production module tree of one crate root and every resolved crate-internal reference
 * between distinct modules, each with its source location.
 */
export function buildModuleGraph(repositoryRoot, rootFile) {
	const state = { root: repositoryRoot, files: new Map(), unresolved: [] };
	const root = createModule('', undefined, path.dirname(rootFile));
	loadModuleFile(state, root, rootFile, true);
	collectUses(root);
	const references = [];
	collectReferences(root, references);
	return { root, references, unresolved: state.unresolved };
}

/** Builds the production module graph and findings for one crate target. */
export function analyzeTarget(repositoryRoot, memberDirectory, rootFile, contract) {
	const { root, references, unresolved } = buildModuleGraph(repositoryRoot, rootFile);
	const modules = [...walkModules(root)];
	return {
		member: normalizeSeparators(path.relative(repositoryRoot, memberDirectory)),
		target: path.basename(rootFile, '.rs') === 'main' ? 'bin' : 'lib',
		topLevelModules: root.children.size,
		modules: modules.length - 1,
		productionLines: subtreeLines(root),
		unresolvedModules: unresolved,
		cycles: siblingCycles(root, references),
		layers:
			contract === undefined
				? undefined
				: layerFindings(root, references, contract, repositoryRoot),
	};
}

/** Analyzes every selected Cargo member; contract problems are returned, not thrown. */
export function analyzeWorkspace(repositoryRoot, selectedMembers = []) {
	const selection = new Set(selectedMembers.map((member) => path.resolve(repositoryRoot, member)));
	const members = workspaceMembers(repositoryRoot).filter(
		(member) => selection.size === 0 || selection.has(member),
	);
	for (const member of selection) {
		if (!members.includes(member))
			throw new Error(
				`${normalizeSeparators(path.relative(repositoryRoot, member))} is not a Cargo member`,
			);
	}
	const results = [];
	const problems = [];
	for (const member of members) {
		const contractFile = path.join(member, CONTRACT_PATH);
		let contract;
		if (existsSync(contractFile)) {
			const label = normalizeSeparators(path.relative(repositoryRoot, contractFile));
			const readLinked = (target) => {
				const file = path.resolve(path.dirname(contractFile), target);
				if (!existsSync(file) || !statSync(file).isFile()) return undefined;
				return {
					markdown: readFileSync(file, 'utf8'),
					label: normalizeSeparators(path.relative(repositoryRoot, file)),
				};
			};
			const parsed = parseLayerContract(readFileSync(contractFile, 'utf8'), label, readLinked);
			if (parsed.problems.length > 0) problems.push(...parsed.problems);
			else contract = { ...parsed, path: label };
		}
		for (const rootName of ['lib.rs', 'main.rs']) {
			const rootFile = path.join(member, 'src', rootName);
			if (!existsSync(rootFile) || !statSync(rootFile).isFile()) continue;
			const result = analyzeTarget(repositoryRoot, member, rootFile, contract);
			if (contract !== undefined) result.layers.contract = contract.path;
			problems.push(...result.unresolvedModules.map((entry) => `unresolved module file: ${entry}`));
			results.push(result);
		}
	}
	return { results, problems };
}

function formatCount(value) {
	return value.toLocaleString('en-US');
}

function formatMembers(members, limit) {
	if (members.length <= limit) return members.join(', ');
	return `${members.slice(0, limit).join(', ')}, ... ${members.length - limit} more`;
}

function cycleScope(cycle) {
	return cycle.parent === '' ? 'top level' : `in ${cycle.parent}`;
}

/** Renders the bounded summary, or every cycle and violating pair with `details`. */
export function formatReport({ results, problems }, { details = false } = {}) {
	const lines = ['Rust module graph (report only; findings never fail this command)'];
	for (const result of results) {
		const { layers } = result;
		lines.push(
			`${result.member} (${result.target}): ${result.topLevelModules} top-level modules, ${formatCount(result.modules)} modules, ${formatCount(result.productionLines)} production lines`,
		);
		const findings = [];
		if (result.cycles.length > 0) {
			const largest = result.cycles.reduce((best, cycle) =>
				cycle.members.length > best.members.length ? cycle : best,
			);
			findings.push(
				`${result.cycles.length} module cycle(s), largest ${largest.members.length} modules ${cycleScope(largest)}`,
			);
		}
		if (layers !== undefined) {
			const references = layers.violations.reduce((total, entry) => total + entry.references, 0);
			if (layers.violations.length > 0)
				findings.push(
					`${layers.violations.length} module pair(s) depend on a higher layer (${formatCount(references)} references)`,
				);
			if (layers.unassigned.length > 0)
				findings.push(`${layers.unassigned.length} unassigned top-level module(s)`);
			if (layers.stale.length > 0) findings.push(`${layers.stale.length} stale assignment(s)`);
		}
		if (findings.length > 0) lines.push(`  warning: ${findings.join('; ')}`);
		const cycles = details ? result.cycles : result.cycles.slice(0, SUMMARY_CYCLE_LIMIT);
		for (const cycle of cycles) {
			lines.push(
				`  cycle ${cycleScope(cycle)}: ${cycle.members.length} modules, ${cycle.edges} edges, ${formatCount(cycle.productionLines)} lines: ${details ? cycle.members.join(', ') : formatMembers(cycle.members, SUMMARY_MEMBER_LIMIT)}`,
			);
		}
		if (cycles.length < result.cycles.length)
			lines.push(`  ... ${result.cycles.length - cycles.length} more cycle(s); use --details`);
		if (layers === undefined) continue;
		lines.push(`  layer contract: ${layers.contract}`);
		if (layers.unassigned.length > 0) lines.push(`  unassigned: ${layers.unassigned.join(', ')}`);
		if (layers.stale.length > 0) lines.push(`  stale: ${layers.stale.join(', ')}`);
		if (!details) continue;
		for (const entry of layers.violations) {
			lines.push(
				`  ${entry.from} (${entry.fromLayer}) -> ${entry.to} (${entry.toLayer}): ${entry.references} references, e.g. ${entry.samples.join(', ')}`,
			);
		}
	}
	for (const problem of problems) lines.push(`error: ${problem}`);
	if (
		!details &&
		results.some((result) => result.cycles.length > 0 || result.layers?.violations.length)
	)
		lines.push('Run `pnpm lint:rust-modules -- --details` for every cycle and violating pair.');
	return `${lines.join('\n')}\n`;
}

function parseArguments(argv) {
	const options = { details: false, json: false, members: [] };
	for (let index = 0; index < argv.length; index += 1) {
		const argument = argv[index];
		if (argument === '--') continue;
		if (argument === '--details') options.details = true;
		else if (argument === '--json') options.json = true;
		else if (argument === '--member') {
			const value = argv[index + 1];
			if (value === undefined) throw new Error('--member requires a Cargo member directory');
			options.members.push(value);
			index += 1;
		} else throw new Error(`Unknown argument ${argument}`);
	}
	return options;
}

function main() {
	const options = parseArguments(process.argv.slice(2));
	const report = analyzeWorkspace(REPOSITORY_ROOT, options.members);
	process.stdout.write(
		options.json ? `${JSON.stringify(report, undefined, '\t')}\n` : formatReport(report, options),
	);
	if (report.problems.length > 0) process.exitCode = 1;
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	try {
		main();
	} catch (error) {
		process.stderr.write(`error: ${error instanceof Error ? error.message : String(error)}\n`);
		process.exitCode = 1;
	}
}
