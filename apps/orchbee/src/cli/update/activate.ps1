# Activates an already admitted immutable release. Values are data, never interpolated code.
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath($env:ORCHBEE_INSTALL_ROOT).TrimEnd('\')
$release = [IO.Path]::GetFullPath($env:ORCHBEE_INSTALL_RELEASE)
if ([IO.Path]::GetDirectoryName($release) -ne $root) { throw 'Invalid release parent' }
$leaf = [IO.Path]::GetFileName($release)
if ($leaf -cnotmatch '^[a-f0-9]{64}$') { throw 'Invalid release identity' }
$key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment')
try {
    $before = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
    $kind = [Microsoft.Win32.RegistryValueKind]::ExpandString
    if ($key.GetValueNames() -contains 'Path') { $kind = $key.GetValueKind('Path') }
    if ($kind -notin @([Microsoft.Win32.RegistryValueKind]::String, [Microsoft.Win32.RegistryValueKind]::ExpandString)) {
        throw 'User Path is not a string'
    }
    $parts = [Collections.Generic.List[string]]::new()
    $parts.Add($release)
    foreach ($entry in $before.Split(';')) {
        # Remove only exact digest-named paths owned by this installation root.
        $managed = $false
        try {
            $entryPath = [IO.Path]::GetFullPath($entry)
            $managed = [IO.Path]::GetDirectoryName($entryPath) -eq $root -and
                [IO.Path]::GetFileName($entryPath) -cmatch '^[a-f0-9]{64}$'
        } catch { $managed = $false }
        if (-not $managed -and $before.Length -gt 0) { $parts.Add($entry) }
    }
    $after = [string]::Join(';', $parts)
    if ($after.Length -gt 32760) { throw 'User Path exceeds bound' }
    if ([string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) -cne $before) {
        throw 'User Path changed'
    }
    $key.SetValue('Path', $after, $kind)
    $key.Flush()
    if ([string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) -cne $after) {
        throw 'User Path activation not observed'
    }
} finally { $key.Dispose() }
