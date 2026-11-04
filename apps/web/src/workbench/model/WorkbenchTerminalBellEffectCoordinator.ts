export interface WorkbenchTerminalBellEffect {
	readonly sequence: string;
	readonly terminalId: string;
}

interface BellSequenceRange {
	end: bigint;
	start: bigint;
}

const MAXIMUM_UINT64 = (1n << 64n) - 1n;
const POSITIVE_CANONICAL_INTEGER = /^[1-9]\d*$/u;

export class WorkbenchTerminalBellEffectCoordinator {
	readonly #rangesByTerminal = new Map<string, BellSequenceRange[]>();
	#disposed = false;

	admit({ sequence: sequenceText, terminalId }: WorkbenchTerminalBellEffect): boolean {
		if (
			this.#disposed ||
			terminalId.length === 0 ||
			!POSITIVE_CANONICAL_INTEGER.test(sequenceText)
		) {
			return false;
		}
		const sequence = BigInt(sequenceText);
		if (sequence > MAXIMUM_UINT64) return false;

		const ranges = this.#rangesByTerminal.get(terminalId) ?? [];
		let insertionIndex = 0;
		while (
			insertionIndex < ranges.length &&
			(ranges[insertionIndex]?.start ?? sequence) < sequence
		) {
			insertionIndex += 1;
		}
		const previous = ranges[insertionIndex - 1];
		const next = ranges[insertionIndex];
		if (
			(previous !== undefined && sequence <= previous.end) ||
			(next !== undefined && sequence === next.start)
		) {
			return false;
		}

		const joinsPrevious = previous !== undefined && previous.end + 1n === sequence;
		const joinsNext = next !== undefined && sequence + 1n === next.start;
		if (joinsPrevious && joinsNext) {
			previous.end = next.end;
			ranges.splice(insertionIndex, 1);
		} else if (joinsPrevious) {
			previous.end = sequence;
		} else if (joinsNext) {
			next.start = sequence;
		} else {
			ranges.splice(insertionIndex, 0, { end: sequence, start: sequence });
		}
		this.#rangesByTerminal.set(terminalId, ranges);
		return true;
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#rangesByTerminal.clear();
	}
}
