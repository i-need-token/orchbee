& {
if ($c = $env:ORCHBEE_AGENT_START) {
	$env:ORCHBEE_AGENT_START = $null
	$global:__OrchbeeRunId = $env:ORCHBEE_RUN_TOKEN = [Guid]::NewGuid().ToString('N')
	[Console]::Write("$([char]27)_orchbee.command.v1;start;$global:__OrchbeeRunId$([char]27)\")
	Invoke-Expression $c
}
}
