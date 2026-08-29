module.exports = async function orchbeeAgentObservation(pi) {
	const extension = await import('./orchbee-agent-observation.mjs');
	await extension.orchbeeAgentObservation(pi);
};
