import { afterEach, describe, expect, it, vi } from 'vitest';

import { CompactWebGPUVertexStream } from '../CompactWebGPUVertexStream';

afterEach(() => {
	vi.unstubAllGlobals();
});

describe('CompactWebGPUVertexStream', () => {
	it('skips unchanged data and writes exact ranges while compact offsets stay stable', () => {
		const rendering = createDevice();
		const stream = new CompactWebGPUVertexStream();
		const first = [1, 2];
		const second = [3, 4];
		const third = [5];

		expect(stream.update(rendering.device, [first, second, third])).toMatchObject({
			floatCount: 5,
			uploadedBytes: 20,
		});
		expect(rendering.writes).toEqual([{ data: [1, 2, 3, 4, 5], offset: 0 }]);

		rendering.writes.length = 0;
		expect(stream.update(rendering.device, [first, second, third])).toMatchObject({
			floatCount: 5,
			uploadedBytes: 0,
		});
		expect(rendering.writes).toEqual([]);

		expect(stream.update(rendering.device, [first, [30, 40], third])).toMatchObject({
			floatCount: 5,
			uploadedBytes: 8,
		});
		expect(rendering.writes).toEqual([{ data: [30, 40], offset: 8 }]);
		stream.dispose();
	});

	it('rewrites the suffix after the first segment-length change', () => {
		const rendering = createDevice();
		const stream = new CompactWebGPUVertexStream();
		const first = [1, 2];
		const second = [3, 4];
		const third = [5, 6];
		stream.update(rendering.device, [first, second, third]);
		rendering.writes.length = 0;

		expect(stream.update(rendering.device, [first, [30, 40, 50], third])).toMatchObject({
			floatCount: 7,
			uploadedBytes: 20,
		});
		expect(rendering.writes).toEqual([{ data: [30, 40, 50, 5, 6], offset: 8 }]);
		stream.dispose();
	});

	it('uploads the complete retained stream after its GPU buffer is released', () => {
		const rendering = createDevice();
		const stream = new CompactWebGPUVertexStream();
		const segments = [
			[1, 2],
			[3, 4],
		];
		stream.update(rendering.device, segments);
		rendering.writes.length = 0;
		stream.releaseGPUBuffer();

		expect(stream.update(rendering.device, segments)).toMatchObject({
			floatCount: 4,
			uploadedBytes: 16,
		});
		expect(rendering.writes).toEqual([{ data: [1, 2, 3, 4], offset: 0 }]);
		expect(rendering.createBuffer).toHaveBeenCalledTimes(2);
		stream.dispose();
	});

	it('preserves compact stream content across disjoint changes and segment insertion or removal', () => {
		const rendering = createDevice();
		const stream = new CompactWebGPUVertexStream();
		const gpuMirror: number[] = [];
		const first = [1, 2];
		const changedFirst = [10, 20];
		const second = [3, 4];
		const grownSecond = [30, 40, 50];
		const third = [5, 6];
		const changedThird = [50, 60];
		const fourth = [7, 8];
		const insertedFourth = [9];
		const update = (segments: readonly (readonly number[])[]): number => {
			rendering.writes.length = 0;
			const result = stream.update(rendering.device, segments);
			for (const write of rendering.writes) {
				gpuMirror.splice(
					write.offset / Float32Array.BYTES_PER_ELEMENT,
					write.data.length,
					...write.data,
				);
			}
			expect(gpuMirror.slice(0, result.floatCount)).toEqual(segments.flat());
			return result.uploadedBytes;
		};

		expect(update([first, second, third, fourth])).toBe(32);
		expect(update([changedFirst, second, changedThird, fourth])).toBe(16);
		expect(rendering.writes.map(({ offset }) => offset)).toEqual([0, 16]);
		expect(update([changedFirst, grownSecond, changedThird, fourth])).toBe(28);
		expect(rendering.writes).toEqual([{ data: [30, 40, 50, 50, 60, 7, 8], offset: 8 }]);
		expect(update([changedFirst, grownSecond, changedThird])).toBe(0);
		expect(rendering.writes).toEqual([]);
		expect(update([changedFirst, grownSecond, changedThird, insertedFourth])).toBe(4);
		expect(rendering.writes).toEqual([{ data: [9], offset: 28 }]);
		stream.dispose();
	});
});

function createDevice(): {
	readonly createBuffer: ReturnType<typeof vi.fn<GPUDevice['createBuffer']>>;
	readonly device: GPUDevice;
	readonly writes: Array<{ readonly data: readonly number[]; readonly offset: number }>;
} {
	vi.stubGlobal('GPUBufferUsage', { COPY_DST: 2, VERTEX: 1 });
	const writes: Array<{ readonly data: readonly number[]; readonly offset: number }> = [];
	const createBuffer = vi.fn<GPUDevice['createBuffer']>(
		() => ({ destroy: () => undefined }) as GPUBuffer,
	);
	const device = {
		createBuffer,
		queue: {
			writeBuffer: (_buffer: GPUBuffer, offset: number, data: AllowSharedBufferSource) => {
				if (!ArrayBuffer.isView(data)) throw new TypeError('Expected a typed vertex upload');
				writes.push({ data: Array.from(data as Float32Array), offset });
			},
		},
	} as unknown as GPUDevice;
	return { createBuffer, device, writes };
}
