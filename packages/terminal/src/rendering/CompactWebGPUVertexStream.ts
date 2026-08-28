export interface CompactWebGPUVertexStreamUpdate {
	readonly buffer: GPUBuffer | null;
	readonly floatCount: number;
	readonly uploadedBytes: number;
}

/**
 * Retains one compact vertex stream while row-owned segment identities expose exact compact update ranges.
 * The segment values are derived staging data; the caller remains responsible for their authority.
 */
export class CompactWebGPUVertexStream {
	#buffer: GPUBuffer | null = null;
	#bufferSize = 0;
	#lengths: number[] = [];
	#nextOffsets: number[] = [0];
	#segments: Array<readonly number[]> = [];
	#upload = new Float32Array(0);

	update(
		device: GPUDevice,
		segments: readonly (readonly number[])[],
	): CompactWebGPUVertexStreamUpdate {
		const previousSegmentCount = this.#segments.length;
		this.#nextOffsets.length = segments.length + 1;
		let firstChangedSegment = -1;
		let firstLengthChange = -1;
		let floatCount = 0;
		for (const [index, segment] of segments.entries()) {
			this.#nextOffsets[index] = floatCount;
			if (segment !== this.#segments[index] && firstChangedSegment === -1) {
				firstChangedSegment = index;
			}
			if (segment.length !== this.#lengths[index] && firstLengthChange === -1) {
				firstLengthChange = index;
			}
			floatCount += segment.length;
		}
		this.#nextOffsets[segments.length] = floatCount;
		if (segments.length !== previousSegmentCount) {
			const changedIndex = Math.min(segments.length, previousSegmentCount);
			if (firstChangedSegment === -1) firstChangedSegment = changedIndex;
			if (firstLengthChange === -1) firstLengthChange = changedIndex;
		}

		if (this.#upload.length < floatCount) {
			let capacity = 64;
			while (capacity < floatCount) capacity *= 2;
			this.#upload = new Float32Array(capacity);
		}

		const requiredSize = floatCount * Float32Array.BYTES_PER_ELEMENT;
		let bufferCreated = false;
		if (floatCount > 0 && (this.#buffer === null || this.#bufferSize < requiredSize)) {
			this.#buffer?.destroy();
			let size = 256;
			while (size < requiredSize) size *= 2;
			this.#buffer = device.createBuffer({
				size,
				usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
			});
			this.#bufferSize = size;
			bufferCreated = true;
		}

		let uploadedBytes = 0;
		if (floatCount > 0 && this.#buffer !== null) {
			if (bufferCreated) {
				this.#copySegments(segments, 0, segments.length);
				uploadedBytes += this.#writeRange(device, 0, floatCount);
			} else if (firstLengthChange !== -1) {
				uploadedBytes += this.#writeChangedSegmentsBefore(device, segments, firstLengthChange);
				this.#copySegments(segments, firstLengthChange, segments.length);
				uploadedBytes += this.#writeRange(
					device,
					this.#nextOffsets[firstLengthChange] ?? 0,
					floatCount,
				);
			} else if (firstChangedSegment !== -1) {
				uploadedBytes += this.#writeChangedSegmentsBefore(device, segments, segments.length);
			}
		}

		this.#segments.length = segments.length;
		this.#lengths.length = segments.length;
		for (const [index, segment] of segments.entries()) {
			this.#segments[index] = segment;
			this.#lengths[index] = segment.length;
		}

		return {
			buffer: floatCount === 0 ? null : this.#buffer,
			floatCount,
			uploadedBytes,
		};
	}

	releaseGPUBuffer(): void {
		this.#buffer?.destroy();
		this.#buffer = null;
		this.#bufferSize = 0;
	}

	releaseStaging(): void {
		this.#lengths = [];
		this.#nextOffsets = [0];
		this.#segments = [];
		this.#upload = new Float32Array(0);
	}

	dispose(): void {
		this.releaseGPUBuffer();
		this.releaseStaging();
	}

	#copySegments(
		segments: readonly (readonly number[])[],
		startSegment: number,
		endSegment: number,
	): void {
		for (let index = startSegment; index < endSegment; index++) {
			const segment = segments[index];
			if (segment === undefined || segment.length === 0) continue;
			this.#upload.set(segment, this.#nextOffsets[index] ?? 0);
		}
	}

	#writeChangedSegmentsBefore(
		device: GPUDevice,
		segments: readonly (readonly number[])[],
		endSegment: number,
	): number {
		let uploadedBytes = 0;
		let runStart = -1;
		for (let index = 0; index <= endSegment; index++) {
			const changed = index < endSegment && segments[index] !== this.#segments[index];
			if (changed && runStart === -1) {
				runStart = index;
				continue;
			}
			if (changed || runStart === -1) continue;
			this.#copySegments(segments, runStart, index);
			uploadedBytes += this.#writeRange(
				device,
				this.#nextOffsets[runStart] ?? 0,
				this.#nextOffsets[index] ?? 0,
			);
			runStart = -1;
		}
		return uploadedBytes;
	}

	#writeRange(device: GPUDevice, startFloat: number, endFloat: number): number {
		if (this.#buffer === null || endFloat <= startFloat) return 0;
		device.queue.writeBuffer(
			this.#buffer,
			startFloat * Float32Array.BYTES_PER_ELEMENT,
			this.#upload.subarray(startFloat, endFloat),
		);
		return (endFloat - startFloat) * Float32Array.BYTES_PER_ELEMENT;
	}
}
