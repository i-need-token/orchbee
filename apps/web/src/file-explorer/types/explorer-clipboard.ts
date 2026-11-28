/** The window's clipboard, where the Explorer's copy commands write. */
export interface ExplorerClipboard {
	/** What joins several paths: the client system's line break, as in VS Code. */
	readonly lineBreak: '\n' | '\r\n';
	writeText(text: string): Promise<void>;
}
