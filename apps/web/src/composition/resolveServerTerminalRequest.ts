/** What the Server terminal page should open; only that page's Main entry asks for it. */
export function resolveServerTerminalRequest(search: string): string | null {
	return new URLSearchParams(search).get('orchbee-server-terminal');
}
