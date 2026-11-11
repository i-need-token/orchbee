import { ProductIcon } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import { WORKBENCH_AGENT_PROVIDER_ORDER } from '../../constants/workbench-agent-providers';
import styles from './WorkbenchAgentProviderIcon.module.css';

interface WorkbenchAgentProviderIconProps {
	readonly gradientPrefix: string;
	/**
	 * Reported Agent labels or Agent start provider ids; one without a product mark shows the generic
	 * Agent icon.
	 */
	readonly agents: readonly string[];
}

/**
 * Curated Agent product marks derived from Lobe Icons 1.94.0 and the OMP website favicon.
 * Source revisions and license notices are recorded in the repository third-party notices.
 */
export function WorkbenchAgentProviderIcon({
	agents,
	gradientPrefix,
}: WorkbenchAgentProviderIconProps): ReactNode {
	const renderProviderIcon = (agent: string): ReactNode => {
		const icon = (contents: ReactNode, viewBox = '0 0 24 24'): ReactNode => (
			<svg
				aria-hidden='true'
				data-agent-provider-icon={agent}
				focusable='false'
				height='1em'
				viewBox={viewBox}
				width='1em'
				xmlns='http://www.w3.org/2000/svg'
			>
				{contents}
			</svg>
		);

		switch (agent) {
			case 'claude':
			case 'claude-code':
				return icon(
					<path
						clipRule='evenodd'
						d='M20.998 10.949H24v3.102h-3v3.028h-1.487V20H18v-2.921h-1.487V20H15v-2.921H9V20H7.488v-2.921H6V20H4.487v-2.921H3V14.05H0V10.95h3V5h17.998v5.949zM6 10.949h1.488V8.102H6v2.847zm10.51 0H18V8.102h-1.49v2.847z'
						fill='#D97757'
						fillRule='evenodd'
					/>,
				);
			case 'cline':
				return icon(
					<g fill='currentColor' fillRule='evenodd'>
						<path d='M17.035 3.991c2.75 0 4.98 2.24 4.98 5.003v1.667l1.45 2.896a1.01 1.01 0 01-.002.909l-1.448 2.864v1.668c0 2.762-2.23 5.002-4.98 5.002H7.074c-2.751 0-4.98-2.24-4.98-5.002V17.33l-1.48-2.855a1.01 1.01 0 01-.003-.927l1.482-2.887V8.994c0-2.763 2.23-5.003 4.98-5.003h9.962zM8.265 9.6a2.274 2.274 0 00-2.274 2.274v4.042a2.274 2.274 0 004.547 0v-4.042A2.274 2.274 0 008.265 9.6zm7.326 0a2.274 2.274 0 00-2.274 2.274v4.042a2.274 2.274 0 104.548 0v-4.042A2.274 2.274 0 0015.59 9.6z' />
						<path d='M12.054 5.558a2.779 2.779 0 100-5.558 2.779 2.779 0 000 5.558z' />
					</g>,
				);
			case 'codex': {
				const gradientId = `${gradientPrefix}-codex`;
				return icon(
					<>
						<path
							d='M19.503 0H4.496A4.496 4.496 0 000 4.496v15.007A4.496 4.496 0 004.496 24h15.007A4.496 4.496 0 0024 19.503V4.496A4.496 4.496 0 0019.503 0z'
							fill='#fff'
						/>
						<path
							d='M9.064 3.344a4.578 4.578 0 012.285-.312c1 .115 1.891.54 2.673 1.275.01.01.024.017.037.021a.09.09 0 00.043 0 4.55 4.55 0 013.046.275l.047.022.116.057a4.581 4.581 0 012.188 2.399c.209.51.313 1.041.315 1.595a4.24 4.24 0 01-.134 1.223.123.123 0 00.03.115c.594.607.988 1.33 1.183 2.17.289 1.425-.007 2.71-.887 3.854l-.136.166a4.548 4.548 0 01-2.201 1.388.123.123 0 00-.081.076c-.191.551-.383 1.023-.74 1.494-.9 1.187-2.222 1.846-3.711 1.838-1.187-.006-2.239-.44-3.157-1.302a.107.107 0 00-.105-.024c-.388.125-.78.143-1.204.138a4.441 4.441 0 01-1.945-.466 4.544 4.544 0 01-1.61-1.335c-.152-.202-.303-.392-.414-.617a5.81 5.81 0 01-.37-.961 4.582 4.582 0 01-.014-2.298.124.124 0 00.006-.056.085.085 0 00-.027-.048 4.467 4.467 0 01-1.034-1.651 3.896 3.896 0 01-.251-1.192 5.189 5.189 0 01.141-1.6c.337-1.112.982-1.985 1.933-2.618.212-.141.413-.251.601-.33.215-.089.43-.164.646-.227a.098.098 0 00.065-.066 4.51 4.51 0 01.829-1.615 4.535 4.535 0 011.837-1.388zm3.482 10.565a.637.637 0 000 1.272h3.636a.637.637 0 100-1.272h-3.636zM8.462 9.23a.637.637 0 00-1.106.631l1.272 2.224-1.266 2.136a.636.636 0 101.095.649l1.454-2.455a.636.636 0 00.005-.64L8.462 9.23z'
							fill={`url(#${gradientId})`}
						/>
						<defs>
							<linearGradient
								gradientUnits='userSpaceOnUse'
								id={gradientId}
								x1='12'
								x2='12'
								y1='3'
								y2='21'
							>
								<stop stopColor='#B1A7FF' />
								<stop offset='.5' stopColor='#7A9DFF' />
								<stop offset='1' stopColor='#3941FF' />
							</linearGradient>
						</defs>
					</>,
				);
			}
			case 'gemini': {
				const gradientId = `${gradientPrefix}-gemini-cli`;
				return icon(
					<>
						<path
							d='M0 4.391A4.391 4.391 0 014.391 0h15.217A4.391 4.391 0 0124 4.391v15.217A4.391 4.391 0 0119.608 24H4.391A4.391 4.391 0 010 19.608V4.391z'
							fill={`url(#${gradientId})`}
						/>
						<path
							clipRule='evenodd'
							d='M19.74 1.444a2.816 2.816 0 012.816 2.816v15.48a2.816 2.816 0 01-2.816 2.816H4.26a2.816 2.816 0 01-2.816-2.816V4.26A2.816 2.816 0 014.26 1.444h15.48zM7.236 8.564l7.752 3.728-7.752 3.727v2.802l9.557-4.596v-3.866L7.236 5.763v2.801z'
							fill='#1E1E2E'
							fillRule='evenodd'
						/>
						<defs>
							<linearGradient
								gradientUnits='userSpaceOnUse'
								id={gradientId}
								x1='24'
								x2='0'
								y1='6.587'
								y2='16.494'
							>
								<stop stopColor='#EE4D5D' />
								<stop offset='.328' stopColor='#B381DD' />
								<stop offset='.476' stopColor='#207CFE' />
							</linearGradient>
						</defs>
					</>,
				);
			}
			case 'copilot':
			case 'github-copilot':
				return icon(
					<path
						d='M19.245 5.364c1.322 1.36 1.877 3.216 2.11 5.817.622 0 1.2.135 1.592.654l.73.964c.21.278.323.61.323.955v2.62c0 .339-.173.669-.453.868C20.239 19.602 16.157 21.5 12 21.5c-4.6 0-9.205-2.583-11.547-4.258-.28-.2-.452-.53-.453-.868v-2.62c0-.345.113-.679.321-.956l.73-.963c.392-.517.974-.654 1.593-.654l.029-.297c.25-2.446.81-4.213 2.082-5.52 2.461-2.54 5.71-2.851 7.146-2.864h.198c1.436.013 4.685.323 7.146 2.864zm-7.244 4.328c-.284 0-.613.016-.962.05-.123.447-.305.85-.57 1.108-1.05 1.023-2.316 1.18-2.994 1.18-.638 0-1.306-.13-1.851-.464-.516.165-1.012.403-1.044.996a65.882 65.882 0 00-.063 2.884l-.002.48c-.002.563-.005 1.126-.013 1.69.002.326.204.63.51.765 2.482 1.102 4.83 1.657 6.99 1.657 2.156 0 4.504-.555 6.985-1.657a.854.854 0 00.51-.766c.03-1.682.006-3.372-.076-5.053-.031-.596-.528-.83-1.046-.996-.546.333-1.212.464-1.85.464-.677 0-1.942-.157-2.993-1.18-.266-.258-.447-.661-.57-1.108-.32-.032-.64-.049-.96-.05zm-2.525 4.013c.539 0 .976.426.976.95v1.753c0 .525-.437.95-.976.95a.964.964 0 01-.976-.95v-1.752c0-.525.437-.951.976-.951zm5 0c.539 0 .976.426.976.95v1.753c0 .525-.437.95-.976.95a.964.964 0 01-.976-.95v-1.752c0-.525.437-.951.976-.951zM7.635 5.087c-1.05.102-1.935.438-2.385.906-.975 1.037-.765 3.668-.21 4.224.405.394 1.17.657 1.995.657h.09c.649-.013 1.785-.176 2.73-1.11.435-.41.705-1.433.675-2.47-.03-.834-.27-1.52-.63-1.813-.39-.336-1.275-.482-2.265-.394zm6.465.394c-.36.292-.6.98-.63 1.813-.03 1.037.24 2.06.675 2.47.968.957 2.136 1.104 2.776 1.11h.044c.825 0 1.59-.263 1.995-.657.555-.556.765-3.187-.21-4.224-.45-.468-1.335-.804-2.385-.906-.99-.088-1.875.058-2.265.394zM12 7.615c-.24 0-.525.015-.84.044.03.16.045.336.06.526l-.001.159a2.94 2.94 0 01-.014.25c.225-.022.425-.027.612-.028h.366c.187 0 .387.006.612.028-.015-.146-.015-.277-.015-.409.015-.19.03-.365.06-.526a9.29 9.29 0 00-.84-.044z'
						fill='currentColor'
						fillRule='evenodd'
					/>,
				);
			case 'grok':
				return icon(
					<path
						d='M9.27 15.29l7.978-5.897c.391-.29.95-.177 1.137.272.98 2.369.542 5.215-1.41 7.169-1.951 1.954-4.667 2.382-7.149 1.406l-2.711 1.257c3.889 2.661 8.611 2.003 11.562-.953 2.341-2.344 3.066-5.539 2.388-8.42l.006.007c-.983-4.232.242-5.924 2.75-9.383.06-.082.12-.164.179-.248l-3.301 3.305v-.01L9.267 15.292M7.623 16.723c-2.792-2.67-2.31-6.801.071-9.184 1.761-1.763 4.647-2.483 7.166-1.425l2.705-1.25a7.808 7.808 0 00-1.829-1A8.975 8.975 0 005.984 5.83c-2.533 2.536-3.33 6.436-1.962 9.764 1.022 2.487-.653 4.246-2.34 6.022-.599.63-1.199 1.259-1.682 1.925l7.62-6.815'
						fill='currentColor'
						fillRule='evenodd'
					/>,
				);
			case 'omp': {
				const gradientId = `${gradientPrefix}-omp`;
				return icon(
					<>
						<defs>
							<linearGradient id={gradientId} x1='0' x2='1' y1='0' y2='1'>
								<stop offset='0' stopColor='#ed4abf' />
								<stop offset='.5' stopColor='#9b4dff' />
								<stop offset='1' stopColor='#5ad8e6' />
							</linearGradient>
						</defs>
						<rect fill='#0f0a14' height='64' rx='12' width='64' />
						<path d='M14 16h36v8H40v32h-8V24h-6v22h-8V24h-4z' fill={`url(#${gradientId})`} />
					</>,
					'0 0 64 64',
				);
			}
			case 'opencode':
				return icon(
					<path d='M16 6H8v12h8V6zm4 16H4V2h16v20z' fill='currentColor' fillRule='evenodd' />,
				);
			case 'pi':
				return icon(
					<g fill='currentColor' fillRule='evenodd'>
						<path clipRule='evenodd' d='M1 1h16.5v11H12v5.5H6.5V23H1V1zm5.5 5.5V12H12V6.5H6.5z' />
						<path d='M17.5 12H23v11h-5.5V12z' />
					</g>,
				);
			case 'qwen':
			case 'qwen-code': {
				const gradientId = `${gradientPrefix}-qwen`;
				return icon(
					<>
						<path
							d='M12.604 1.34c.393.69.784 1.382 1.174 2.075a.18.18 0 00.157.091h5.552c.174 0 .322.11.446.327l1.454 2.57c.19.337.24.478.024.837-.26.43-.513.864-.76 1.3l-.367.658c-.106.196-.223.28-.04.512l2.652 4.637c.172.301.111.494-.043.77-.437.785-.882 1.564-1.335 2.34-.159.272-.352.375-.68.37-.777-.016-1.552-.01-2.327.016a.099.099 0 00-.081.05 575.097 575.097 0 01-2.705 4.74c-.169.293-.38.363-.725.364-.997.003-2.002.004-3.017.002a.537.537 0 01-.465-.271l-1.335-2.323a.09.09 0 00-.083-.049H4.982c-.285.03-.553-.001-.805-.092l-1.603-2.77a.543.543 0 01-.002-.54l1.207-2.12a.198.198 0 000-.197 550.951 550.951 0 01-1.875-3.272l-.79-1.395c-.16-.31-.173-.496.095-.965.465-.813.927-1.625 1.387-2.436.132-.234.304-.334.584-.335a338.3 338.3 0 012.589-.001.124.124 0 00.107-.063l2.806-4.895a.488.488 0 01.422-.246c.524-.001 1.053 0 1.583-.006L11.704 1c.341-.003.724.032.9.34zm-3.432.403a.06.06 0 00-.052.03L6.254 6.788a.157.157 0 01-.135.078H3.253c-.056 0-.07.025-.041.074l5.81 10.156c.025.042.013.062-.034.063l-2.795.015a.218.218 0 00-.2.116l-1.32 2.31c-.044.078-.021.118.068.118l5.716.008c.046 0 .08.02.104.061l1.403 2.454c.046.081.092.082.139 0l5.006-8.76.783-1.382a.055.055 0 01.096 0l1.424 2.53a.122.122 0 00.107.062l2.763-.02a.04.04 0 00.035-.02.041.041 0 000-.04l-2.9-5.086a.108.108 0 010-.113l.293-.507 1.12-1.977c.024-.041.012-.062-.035-.062H9.2c-.059 0-.073-.026-.043-.077l1.434-2.505a.107.107 0 000-.114L9.225 1.774a.06.06 0 00-.053-.031zm6.29 8.02c.046 0 .058.02.034.06l-.832 1.465-2.613 4.585a.056.056 0 01-.05.029.058.058 0 01-.05-.029L8.498 9.841c-.02-.034-.01-.052.028-.054l.216-.012 6.722-.012z'
							fill={`url(#${gradientId})`}
							fillRule='nonzero'
						/>
						<defs>
							<linearGradient id={gradientId} x1='0%' x2='100%' y1='0%' y2='0%'>
								<stop offset='0%' stopColor='#6336E7' stopOpacity='.84' />
								<stop offset='100%' stopColor='#6F69F7' stopOpacity='.84' />
							</linearGradient>
						</defs>
					</>,
				);
			}
			default:
				return <ProductIcon name='agent' />;
		}
	};
	const distinctProviderIds = [...new Set(agents)];
	distinctProviderIds.sort((left, right) => {
		const leftIndex = WORKBENCH_AGENT_PROVIDER_ORDER.indexOf(left);
		const rightIndex = WORKBENCH_AGENT_PROVIDER_ORDER.indexOf(right);
		if (leftIndex === -1 && rightIndex === -1) return left.localeCompare(right);
		if (leftIndex === -1) return 1;
		if (rightIndex === -1) return -1;
		return leftIndex - rightIndex;
	});
	if (distinctProviderIds.length === 0 || distinctProviderIds.length >= 3) {
		return <ProductIcon name='agent' />;
	}
	const [firstProviderId, secondProviderId] = distinctProviderIds;
	if (firstProviderId === undefined) return <ProductIcon name='agent' />;
	if (secondProviderId === undefined) return renderProviderIcon(firstProviderId);
	return (
		<span aria-hidden='true' className={styles['group']} data-agent-provider-composition='pair'>
			<span className={styles['provider']} data-agent-provider={firstProviderId}>
				{renderProviderIcon(firstProviderId)}
			</span>
			<span className={styles['provider']} data-agent-provider={secondProviderId}>
				{renderProviderIcon(secondProviderId)}
			</span>
		</span>
	);
}
