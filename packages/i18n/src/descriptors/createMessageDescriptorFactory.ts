type MessageCatalog = Readonly<Record<string, string>>;

type MessageDescriptorFactory<Catalog extends MessageCatalog> = (
	id: Extract<keyof Catalog, string>,
	description?: string,
) => {
	readonly defaultMessage: string;
	readonly description?: string;
	readonly id: string;
};

export function createMessageDescriptorFactory<const Catalog extends MessageCatalog>(
	catalog: Catalog,
): MessageDescriptorFactory<Catalog> {
	return (id, description) => {
		const defaultMessage = catalog[id];
		if (defaultMessage === undefined) {
			throw new Error(`Message descriptor ${id} is absent from its English catalog.`);
		}
		return {
			defaultMessage,
			...(description === undefined ? {} : { description }),
			id,
		};
	};
}
