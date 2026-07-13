/**
 * Link detection and caching system.
 *
 * The LinkDetector coordinates between multiple link providers and caches
 * results for performance. Links are cached by position range, and earlier
 * providers (e.g. OSC8) take precedence over later ones (e.g. regex).
 */

import type { ILink, ILinkBuffer, ILinkProvider } from "./types.js";

/**
 * Manages link detection across multiple providers with intelligent caching.
 */
export class LinkDetector {
  private providers: ILinkProvider[] = [];

  // Cache links by position range: `r${row}:${startX}-${endX}`
  private linkCache = new Map<string, ILink>();

  // Track which rows have been scanned to avoid redundant provider calls
  private scannedRows = new Set<number>();

  private terminal: ITerminalForLinkDetector;

  constructor(terminal: ITerminalForLinkDetector) {
    this.terminal = terminal;
  }

  /**
   * Register a link provider.
   */
  registerProvider(provider: ILinkProvider): void {
    this.providers.push(provider);
    this.invalidateCache(); // New provider may detect different links
  }

  /**
   * Get link at the specified buffer position.
   * @param col Column (0-based)
   * @param row Absolute row in buffer (0-based)
   * @returns Link at position, or undefined if none
   */
  async getLinkAt(col: number, row: number): Promise<ILink | undefined> {
    const line = this.terminal.buffer.active.getLine(row);
    if (!line || col < 0 || col >= line.length) {
      return undefined;
    }

    const cell = line.getCell(col);
    if (!cell) {
      return undefined;
    }

    // Check if any cached link contains this position (fast path)
    for (const link of this.linkCache.values()) {
      if (this.isPositionInLink(col, row, link)) {
        return link;
      }
    }

    // Slow path: scan this row if not already scanned
    if (!this.scannedRows.has(row)) {
      await this.scanRow(row);
    }

    // Check cache again after scanning
    for (const link of this.linkCache.values()) {
      if (this.isPositionInLink(col, row, link)) {
        return link;
      }
    }

    return undefined;
  }

  /**
   * Scan a row for links using all registered providers.
   */
  private async scanRow(row: number): Promise<void> {
    this.scannedRows.add(row);

    const provided = await Promise.all(
      this.providers.map(
        async (provider) =>
          await new Promise<ILink[] | undefined>((resolve) => {
            provider.provideLinks(row, resolve);
          }),
      ),
    );
    const allLinks = provided.flatMap((links) => links ?? []);

    for (const link of allLinks) {
      this.cacheLink(link);
    }
  }

  /**
   * Cache a link for fast lookup.
   *
   * We cache by position range, not hyperlink_id, because the WASM
   * returns hyperlink_id as a boolean (0 or 1), not a unique identifier.
   * The actual unique identifier is the URI which is retrieved separately.
   */
  private cacheLink(link: ILink): void {
    const { start: s, end: e } = link.range;
    const cacheKey = `r${s.y}:${s.x}-${e.y}:${e.x}`;
    // Don't overwrite existing entries - earlier providers (OSC8) take
    // precedence over later ones (regex) for the same range
    if (!this.linkCache.has(cacheKey)) {
      this.linkCache.set(cacheKey, link);
    }
  }

  /**
   * Check if a position is within a link's range.
   */
  private isPositionInLink(col: number, row: number, link: ILink): boolean {
    const { start, end } = link.range;

    if (row < start.y || row > end.y) {
      return false;
    }

    // Single-line link
    if (start.y === end.y) {
      return col >= start.x && col <= end.x;
    }

    // Multi-line link
    if (row === start.y) {
      return col >= start.x; // First line: from start.x to end of line
    }
    if (row === end.y) {
      return col <= end.x; // Last line: from start of line to end.x
    }
    return true; // Middle line: entire line is part of link
  }

  /**
   * Invalidate cache when terminal content changes.
   * Should be called on terminal write, resize, or clear.
   */
  invalidateCache(): void {
    this.linkCache.clear();
    this.scannedRows.clear();
  }

  /**
   * Invalidate cache for specific rows.
   * Used when only part of the terminal changed.
   */
  invalidateRows(startRow: number, endRow: number): void {
    for (let row = startRow; row <= endRow; row++) {
      this.scannedRows.delete(row);
    }

    // Remove cached links that touch these rows
    const toDelete: string[] = [];
    for (const [key, link] of this.linkCache.entries()) {
      const { start, end } = link.range;
      if (
        (start.y >= startRow && start.y <= endRow) ||
        (end.y >= startRow && end.y <= endRow) ||
        (start.y < startRow && end.y > endRow)
      ) {
        toDelete.push(key);
      }
    }

    for (const key of toDelete) {
      this.linkCache.delete(key);
    }
  }

  /**
   * Dispose and cleanup.
   */
  dispose(): void {
    this.linkCache.clear();
    this.scannedRows.clear();

    for (const provider of this.providers) {
      provider.dispose?.();
    }
    this.providers = [];
  }
}

/**
 * Minimal terminal interface required by LinkDetector.
 * Uses ILinkBuffer/ILinkBufferLine/ILinkBufferCell from types.js
 * to keep coupling low and testing easy.
 */
export interface ITerminalForLinkDetector {
  buffer: {
    active: ILinkBuffer;
  };
}
