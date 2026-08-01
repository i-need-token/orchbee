export interface TextShapingCell {
	readonly text: string;
	readonly width: number;
	readonly styleKey: string;
	readonly eligible: boolean;
}

export interface TextShapingSpan {
	readonly start: number;
	readonly end: number;
	readonly columns: number;
	readonly text: string;
}

export type TextShapingPlan = readonly (TextShapingSpan | null)[];

export interface TextShapingReader {
	readonly length: number;
	eligible(index: number): boolean;
	sameStyle(left: number, right: number): boolean;
	text(index: number): string;
	width(index: number): number;
}

const PROGRAMMING_LIGATURES = [
	'<==>',
	'<-->',
	'<!--',
	'||||>',
	'====',
	'!==',
	'===',
	'==>',
	'<==',
	'<=>',
	'<->',
	'<--',
	'<~~',
	'<<=',
	'<<<',
	'-->',
	'->>',
	'-<<',
	'...',
	'..<',
	'..=',
	'/**',
	'///',
	':::',
	'>==',
	'>>=',
	'>>>',
	'<|>',
	'<||',
	'||>',
	'|||',
	'~~>',
	'~>>',
	'~~~',
	'###',
	'***',
	'+++',
	'---',
	'www',
	'!=',
	'==',
	'=>',
	'<=',
	'>=',
	'<>',
	'->',
	'<-',
	'<~',
	'~>',
	'-<',
	'>-',
	'..',
	'//',
	'/*',
	'*/',
	'/=',
	'/>',
	'</',
	'::',
	':=',
	'!!',
	'??',
	'?.',
	'?:',
	'&&',
	'||',
	'|=',
	'|>',
	'<|',
	'++',
	'--',
	'**',
	'##',
	'#{',
	'#[',
	'#(',
	'#!',
] as const;

/** Build bounded multi-cell runs that Canvas can shape without caching arbitrary terminal lines. */
export function buildTextShapingPlan(
	source: readonly TextShapingCell[] | TextShapingReader,
): TextShapingPlan {
	const reader = isTextShapingReader(source)
		? source
		: {
				length: source.length,
				eligible: (index: number) => source[index]?.eligible === true,
				sameStyle: (left: number, right: number) =>
					source[left]?.styleKey === source[right]?.styleKey,
				text: (index: number) => source[index]?.text ?? '',
				width: (index: number) => source[index]?.width ?? 0,
			};
	return buildTextShapingPlanFromReader(reader);
}

/** Build the same shaping plan without requiring one JavaScript object per terminal cell. */
function buildTextShapingPlanFromReader(reader: TextShapingReader): TextShapingPlan {
	const plan: (TextShapingSpan | null)[] = Array.from({ length: reader.length }, () => null);
	let column = 0;
	while (column < reader.length) {
		const span = findSpan(reader, column);
		if (!span) {
			column++;
			continue;
		}
		for (let index = span.start; index < span.end; index++) plan[index] = span;
		column = span.end;
	}
	return plan;
}

function isTextShapingReader(
	source: readonly TextShapingCell[] | TextShapingReader,
): source is TextShapingReader {
	return 'eligible' in source;
}

function findSpan(reader: TextShapingReader, start: number): TextShapingSpan | null {
	if (!reader.eligible(start) || reader.width(start) !== 1) return null;

	for (const candidate of PROGRAMMING_LIGATURES) {
		if (!matches(reader, start, candidate)) continue;
		if (extendsRepeatedCharacterRun(reader, start, candidate)) continue;
		return {
			start,
			end: start + candidate.length,
			columns: candidate.length,
			text: candidate,
		};
	}
	return null;
}

function extendsRepeatedCharacterRun(
	reader: TextShapingReader,
	start: number,
	candidate: string,
): boolean {
	const character = candidate[0];
	if (!character) return false;
	for (let index = 1; index < candidate.length; index++) {
		if (candidate[index] !== character) return false;
	}
	return (
		matchesRepeatedCharacter(reader, start - 1, character, start) ||
		matchesRepeatedCharacter(reader, start + candidate.length, character, start)
	);
}

function matchesRepeatedCharacter(
	reader: TextShapingReader,
	index: number,
	character: string,
	styleIndex: number,
): boolean {
	return (
		index >= 0 &&
		index < reader.length &&
		reader.eligible(index) &&
		reader.width(index) === 1 &&
		reader.sameStyle(styleIndex, index) &&
		reader.text(index) === character
	);
}

function matches(reader: TextShapingReader, start: number, candidate: string): boolean {
	if (start + candidate.length > reader.length) return false;
	for (let offset = 0; offset < candidate.length; offset++) {
		const index = start + offset;
		if (
			!reader.eligible(index) ||
			reader.width(index) !== 1 ||
			!reader.sameStyle(start, index) ||
			reader.text(index) !== candidate[offset]
		) {
			return false;
		}
	}
	return true;
}
