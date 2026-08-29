module.exports = async function orchbeeAgentObservation(omp) {
	const extension = await import('./orchbee-agent-observation.mjs');
	await extension.orchbeeAgentObservation(omp);
};
