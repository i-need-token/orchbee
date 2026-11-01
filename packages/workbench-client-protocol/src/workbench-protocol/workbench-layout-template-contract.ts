import { z } from 'zod';
import { WorkbenchLabelSchema, WorkbenchPresentationSchema } from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_ENCODED_LAYOUT_COMMAND_BYTES = 1024 * 1024;
const WorkbenchLayoutPresentationSchema = z
	.strictObject({
		label: WorkbenchLabelSchema,
		customLabel: WorkbenchPresentationSchema.unwrap().shape.customLabel,
	})
	.readonly();
type WorkbenchLayoutPresentation = z.infer<typeof WorkbenchLayoutPresentationSchema>;
type WorkbenchLayoutNode =
	| {
			readonly kind: 'panel';
			readonly primary: WorkbenchLayoutPresentation;
			readonly children: readonly WorkbenchLayoutPresentation[];
			readonly rightClick: 'orchbee' | 'panel';
	  }
	| {
			readonly kind: 'split';
			readonly orientation: 'horizontal' | 'vertical';
			readonly children: readonly [WorkbenchLayoutNode, WorkbenchLayoutNode];
	  };
const nodeSchema: z.ZodType<WorkbenchLayoutNode> = z.lazy(() =>
	z.discriminatedUnion('kind', [
		z
			.strictObject({
				kind: z.literal('panel'),
				primary: WorkbenchLayoutPresentationSchema,
				children: z.array(WorkbenchLayoutPresentationSchema).max(16).readonly(),
				rightClick: z.enum(['orchbee', 'panel']),
			})
			.readonly(),
		z
			.strictObject({
				kind: z.literal('split'),
				orientation: z.enum(['horizontal', 'vertical']),
				children: z.tuple([nodeSchema, nodeSchema]).readonly(),
			})
			.readonly(),
	]),
);
export const WorkbenchLayoutTemplateSchema = nodeSchema.refine((root) => {
	let panels = 0;
	const walk = (node: WorkbenchLayoutNode, depth: number): boolean => {
		if (depth > 16) return false;
		if (node.kind === 'panel') {
			panels += 1;
			return panels <= 24;
		}
		return node.children.every((child) => walk(child, depth + 1));
	};
	return walk(root, 1);
}, 'Layout exceeds its Panel or depth limit');
