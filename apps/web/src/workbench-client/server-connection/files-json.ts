/**
 * The JSON rules of every File Bar record body, the `OBHS` payload rules `schemas/files` adopts:
 * UTF-8 without a byte order mark, one object, no member repeated at any depth, at most 32 levels
 * of nesting, finite numbers, and no string escape that leaves a lone surrogate. `JSON.parse` keeps
 * the last of repeated members and admits the rest, so a body is read here instead.
 */

/** The deepest nesting a body may reach, counting its object as level 1. */
const MAXIMUM_DEPTH = 32;
const NUMBER = /-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/y;
const HEX_ESCAPE = /^[0-9a-fA-F]{4}$/;
const ESCAPES: Readonly<Record<string, string>> = {
	'"': '"',
	'/': '/',
	'\\': '\\',
	b: '\b',
	f: '\f',
	n: '\n',
	r: '\r',
	t: '\t',
};
const utf8 = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

/** What reading a body found: its object, or nothing a File Bar record may carry. */
export type FilesJsonRead =
	| { readonly kind: 'malformed' }
	| { readonly kind: 'object'; readonly value: Readonly<Record<string, unknown>> };

/** Reads a record body as one JSON object under the File Bar's JSON rules. */
export function readFilesJson(body: Uint8Array): FilesJsonRead {
	let text: string;
	try {
		text = utf8.decode(body);
	} catch {
		return { kind: 'malformed' };
	}
	try {
		return { kind: 'object', value: new BodyReader(text).document() };
	} catch (error) {
		if (error instanceof MalformedBody) return { kind: 'malformed' };
		throw error;
	}
}

class MalformedBody extends Error {
	constructor() {
		super('The body breaks the File Bar JSON rules.');
		this.name = 'MalformedBody';
	}
}

/** Whether `value` pairs every surrogate, so it has a UTF-8 spelling. */
export function isWellFormedText(value: string): boolean {
	for (let index = 0; index < value.length; index += 1) {
		const code = value.charCodeAt(index);
		if (code >= 0xdc00 && code <= 0xdfff) return false;
		if (code < 0xd800 || code > 0xdbff) continue;
		// Past the end, `charCodeAt` is NaN, which no comparison admits.
		const next = value.charCodeAt(index + 1);
		if (!(next >= 0xdc00 && next <= 0xdfff)) return false;
		index += 1;
	}
	return true;
}

/** One pass over a body's text; any rule it breaks throws `MalformedBody`. */
class BodyReader {
	#index = 0;
	readonly #text: string;

	constructor(text: string) {
		this.#text = text;
	}

	document(): Record<string, unknown> {
		this.#skipWhitespace();
		// A byte order mark is not JSON whitespace, so it fails here.
		if (this.#text[this.#index] !== '{') throw new MalformedBody();
		const value = this.#object(1);
		this.#skipWhitespace();
		if (this.#index !== this.#text.length) throw new MalformedBody();
		return value;
	}

	#value(depth: number): unknown {
		this.#skipWhitespace();
		switch (this.#text[this.#index]) {
			case '{':
				return this.#object(depth + 1);
			case '[':
				return this.#array(depth + 1);
			case '"':
				return this.#string();
			case 't':
				return this.#literal('true', true);
			case 'f':
				return this.#literal('false', false);
			case 'n':
				return this.#literal('null', null);
			default:
				return this.#number();
		}
	}

	#object(depth: number): Record<string, unknown> {
		if (depth > MAXIMUM_DEPTH) throw new MalformedBody();
		this.#index += 1;
		const object: Record<string, unknown> = {};
		this.#skipWhitespace();
		if (this.#text[this.#index] === '}') {
			this.#index += 1;
			return object;
		}
		for (;;) {
			this.#skipWhitespace();
			if (this.#text[this.#index] !== '"') throw new MalformedBody();
			const name = this.#string();
			if (Object.hasOwn(object, name)) throw new MalformedBody();
			this.#skipWhitespace();
			if (this.#text[this.#index] !== ':') throw new MalformedBody();
			this.#index += 1;
			// Defined rather than assigned, so a member named `__proto__` stays a member.
			Object.defineProperty(object, name, {
				configurable: true,
				enumerable: true,
				value: this.#value(depth),
				writable: true,
			});
			if (this.#endsContainer('}')) return object;
		}
	}

	#array(depth: number): unknown[] {
		if (depth > MAXIMUM_DEPTH) throw new MalformedBody();
		this.#index += 1;
		const values: unknown[] = [];
		this.#skipWhitespace();
		if (this.#text[this.#index] === ']') {
			this.#index += 1;
			return values;
		}
		for (;;) {
			values.push(this.#value(depth));
			if (this.#endsContainer(']')) return values;
		}
	}

	/** Reads the separator after a member or element: true at the container's end, false at a comma. */
	#endsContainer(close: '}' | ']'): boolean {
		this.#skipWhitespace();
		const next = this.#text[this.#index];
		this.#index += 1;
		if (next === close) return true;
		if (next !== ',') throw new MalformedBody();
		return false;
	}

	#string(): string {
		const text = this.#text;
		let index = this.#index + 1;
		let start = index;
		let value = '';
		let escaped = false;
		for (;;) {
			const character = text[index];
			if (character === '"') break;
			if (character === undefined || character < ' ') throw new MalformedBody();
			if (character !== '\\') {
				index += 1;
				continue;
			}
			value += text.slice(start, index);
			const escape = text[index + 1];
			if (escape === 'u') {
				const hex = text.slice(index + 2, index + 6);
				if (!HEX_ESCAPE.test(hex)) throw new MalformedBody();
				value += String.fromCharCode(Number.parseInt(hex, 16));
				escaped = true;
				index += 6;
			} else {
				const replacement = escape === undefined ? undefined : ESCAPES[escape];
				if (replacement === undefined) throw new MalformedBody();
				value += replacement;
				index += 2;
			}
			start = index;
		}
		value += text.slice(start, index);
		this.#index = index + 1;
		// Decoded UTF-8 holds only whole pairs, so only an escape can leave a lone surrogate.
		if (escaped && !isWellFormedText(value)) throw new MalformedBody();
		return value;
	}

	#literal<Value>(word: string, value: Value): Value {
		if (!this.#text.startsWith(word, this.#index)) throw new MalformedBody();
		this.#index += word.length;
		return value;
	}

	#number(): number {
		NUMBER.lastIndex = this.#index;
		const match = NUMBER.exec(this.#text);
		if (match === null) throw new MalformedBody();
		const value = Number(match[0]);
		if (!Number.isFinite(value)) throw new MalformedBody();
		this.#index = NUMBER.lastIndex;
		return value;
	}

	#skipWhitespace(): void {
		const text = this.#text;
		let index = this.#index;
		for (;;) {
			const character = text[index];
			if (character !== ' ' && character !== '\t' && character !== '\n' && character !== '\r')
				break;
			index += 1;
		}
		this.#index = index;
	}
}
