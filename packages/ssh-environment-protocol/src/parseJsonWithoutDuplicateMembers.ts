type JsonObjectFrame = {
	kind: 'object';
	state: 'key-or-end' | 'colon' | 'value' | 'comma-or-end';
	keys: Set<string>;
};

type JsonArrayFrame = {
	kind: 'array';
	state: 'value-or-end' | 'comma-or-end';
};

type JsonFrame = JsonObjectFrame | JsonArrayFrame;

function isJsonWhitespace(code: number): boolean {
	return code === 0x20 || code === 0x09 || code === 0x0a || code === 0x0d;
}

function isJsonDigit(character: string | undefined): boolean {
	return character !== undefined && character >= '0' && character <= '9';
}

function skipJsonWhitespace(text: string, index: number): number {
	while (index < text.length && isJsonWhitespace(text.charCodeAt(index))) index += 1;
	return index;
}

function scanJsonString(text: string, start: number): number | undefined {
	if (text.charCodeAt(start) !== 0x22) return undefined;
	let index = start + 1;
	while (index < text.length) {
		const code = text.charCodeAt(index);
		if (code < 0x20) return undefined;
		if (code === 0x22) return index + 1;
		if (code === 0x5c) {
			index += 2;
			continue;
		}
		index += 1;
	}
	return undefined;
}

function scanJsonNumber(text: string, start: number): number | undefined {
	let index = start;
	if (text[index] === '-') index += 1;
	const firstDigit = text[index];
	if (firstDigit === '0') {
		index += 1;
	} else if (firstDigit !== undefined && firstDigit >= '1' && firstDigit <= '9') {
		index += 1;
		while (isJsonDigit(text[index])) index += 1;
	} else {
		return undefined;
	}
	if (text[index] === '.') {
		index += 1;
		const fractionStart = index;
		while (isJsonDigit(text[index])) index += 1;
		if (index === fractionStart) return undefined;
	}
	if (text[index] === 'e' || text[index] === 'E') {
		index += 1;
		if (text[index] === '+' || text[index] === '-') index += 1;
		const exponentStart = index;
		while (isJsonDigit(text[index])) index += 1;
		if (index === exponentStart) return undefined;
	}
	return index;
}

function scanJsonScalar(text: string, start: number): number | undefined {
	const stringEnd = scanJsonString(text, start);
	if (stringEnd !== undefined) return stringEnd;
	const numberEnd = scanJsonNumber(text, start);
	if (numberEnd !== undefined) return numberEnd;
	for (const literal of ['true', 'false', 'null']) {
		if (text.startsWith(literal, start)) return start + literal.length;
	}
	return undefined;
}

function containsDuplicateJsonObjectMember(text: string): boolean {
	const stack: JsonFrame[] = [];
	let index = 0;
	let rootComplete = false;

	while (true) {
		if (stack.length === 0) {
			if (rootComplete) return false;
			index = skipJsonWhitespace(text, index);
			const code = text.charCodeAt(index);
			if (code === 0x7b) {
				stack.push({ kind: 'object', keys: new Set(), state: 'key-or-end' });
				index += 1;
				continue;
			}
			if (code === 0x5b) {
				stack.push({ kind: 'array', state: 'value-or-end' });
				index += 1;
				continue;
			}
			const scalarEnd = scanJsonScalar(text, index);
			if (scalarEnd === undefined) return false;
			index = scalarEnd;
			rootComplete = true;
			continue;
		}

		const frame = stack[stack.length - 1];
		if (frame === undefined) return false;
		if (frame.kind === 'object') {
			switch (frame.state) {
				case 'key-or-end': {
					index = skipJsonWhitespace(text, index);
					if (text.charCodeAt(index) === 0x7d) {
						stack.pop();
						index += 1;
						if (stack.length === 0) rootComplete = true;
						continue;
					}
					const keyEnd = scanJsonString(text, index);
					if (keyEnd === undefined) return false;
					let key: unknown;
					try {
						key = JSON.parse(text.slice(index, keyEnd));
					} catch {
						return false;
					}
					if (typeof key !== 'string') return false;
					if (frame.keys.has(key)) return true;
					frame.keys.add(key);
					index = keyEnd;
					frame.state = 'colon';
					continue;
				}
				case 'colon':
					index = skipJsonWhitespace(text, index);
					if (text.charCodeAt(index) !== 0x3a) return false;
					index += 1;
					frame.state = 'value';
					continue;
				case 'value': {
					index = skipJsonWhitespace(text, index);
					const code = text.charCodeAt(index);
					frame.state = 'comma-or-end';
					if (code === 0x7b) {
						stack.push({ kind: 'object', keys: new Set(), state: 'key-or-end' });
						index += 1;
						continue;
					}
					if (code === 0x5b) {
						stack.push({ kind: 'array', state: 'value-or-end' });
						index += 1;
						continue;
					}
					const scalarEnd = scanJsonScalar(text, index);
					if (scalarEnd === undefined) return false;
					index = scalarEnd;
					continue;
				}
				case 'comma-or-end':
					index = skipJsonWhitespace(text, index);
					if (text.charCodeAt(index) === 0x2c) {
						index += 1;
						frame.state = 'key-or-end';
						continue;
					}
					if (text.charCodeAt(index) === 0x7d) {
						stack.pop();
						index += 1;
						if (stack.length === 0) rootComplete = true;
						continue;
					}
					return false;
			}
		} else {
			switch (frame.state) {
				case 'value-or-end': {
					index = skipJsonWhitespace(text, index);
					if (text.charCodeAt(index) === 0x5d) {
						stack.pop();
						index += 1;
						if (stack.length === 0) rootComplete = true;
						continue;
					}
					const code = text.charCodeAt(index);
					frame.state = 'comma-or-end';
					if (code === 0x7b) {
						stack.push({ kind: 'object', keys: new Set(), state: 'key-or-end' });
						index += 1;
						continue;
					}
					if (code === 0x5b) {
						stack.push({ kind: 'array', state: 'value-or-end' });
						index += 1;
						continue;
					}
					const scalarEnd = scanJsonScalar(text, index);
					if (scalarEnd === undefined) return false;
					index = scalarEnd;
					continue;
				}
				case 'comma-or-end':
					index = skipJsonWhitespace(text, index);
					if (text.charCodeAt(index) === 0x2c) {
						index += 1;
						frame.state = 'value-or-end';
						continue;
					}
					if (text.charCodeAt(index) === 0x5d) {
						stack.pop();
						index += 1;
						if (stack.length === 0) rootComplete = true;
						continue;
					}
					return false;
			}
		}
	}
}

export function parseJsonWithoutDuplicateMembers(text: string): unknown {
	if (containsDuplicateJsonObjectMember(text)) {
		throw new SyntaxError('JSON contains a duplicate object member');
	}
	const value: unknown = JSON.parse(text);
	return value;
}
