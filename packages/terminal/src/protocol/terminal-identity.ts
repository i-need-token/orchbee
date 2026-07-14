export interface TerminalProtocolIdentity {
	readonly name: string;
	readonly version: string;
}

/** Build a bounded control-free XTVERSION identity string. */
export function formatTerminalIdentity(identity: Partial<TerminalProtocolIdentity> = {}): string {
	const name = sanitize(identity.name ?? 'orchbee') || 'orchbee';
	const version = sanitize(identity.version ?? '0.1.0') || '0.1.0';
	return `${name} ${version}`.slice(0, 256);
}

function sanitize(value: string): string {
	return [...value]
		.filter((character) => {
			const codepoint = character.codePointAt(0) ?? 0;
			return codepoint >= 0x20 && codepoint !== 0x7f;
		})
		.join('')
		.trim();
}
