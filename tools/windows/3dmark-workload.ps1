# SPDX-License-Identifier: MIT
# Re-run the exact Steel Nomad workload input retained by 3DMark. This avoids UI automation and
# preserves the settings of the failing run. Invoke with run44.py upack so the child owns the
# signed-in user's interactive desktop.
param(
    [string]$WorkloadInput = 'C:\ProgramData\UL\3DMark\tmp\fm-workload-2533705837541784396.json',
    [int]$TimeoutSeconds = 600
)

$ErrorActionPreference = 'Stop'
$root = 'C:\Program Files (x86)\Steam\steamapps\common\3DMark\dlc\steel-nomad-test'
$exe = Join-Path $root 'windows\bin\x64\3DMarkSteelNomad.exe'
$adapterInfoExe = 'C:\Program Files (x86)\Steam\steamapps\common\3DMark\bin\x64\adapter_info.exe'

if (-not (Test-Path -LiteralPath $exe)) {
    throw "Steel Nomad executable is missing: $exe"
}
if (-not (Test-Path -LiteralPath $WorkloadInput)) {
    throw "Retained workload input is missing: $WorkloadInput"
}
if (-not (Test-Path -LiteralPath $adapterInfoExe)) {
    throw "3DMark adapter_info.exe is missing: $adapterInfoExe"
}

$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$runInput = "C:\ProgramData\UL\3DMark\tmp\caraxes-steel-nomad-$stamp.in.json"
$output = "C:\ProgramData\UL\3DMark\tmp\caraxes-steel-nomad-$stamp.json"
$stdout = "C:\ProgramData\UL\3DMark\tmp\caraxes-steel-nomad-$stamp.out.txt"
$stderr = "C:\ProgramData\UL\3DMark\tmp\caraxes-steel-nomad-$stamp.err.txt"
$inputHash = (Get-FileHash -LiteralPath $WorkloadInput -Algorithm SHA256).Hash.ToLowerInvariant()
$adapterText = & $adapterInfoExe | Out-String
# adapter_info's monitor paths contain JSON-invalid single backslashes. Extract only the NVIDIA
# LUID from its otherwise stable textual output instead of parsing the complete document.
$adapterMatch = [regex]::Match(
    $adapterText,
    '"vendorId"\s*:\s*4318.*?"adapterUid"\s*:\s*\{\s*"lowPart"\s*:\s*(\d+)',
    [Text.RegularExpressions.RegexOptions]::Singleline
)
if (-not $adapterMatch.Success) {
    throw '3DMark does not see an NVIDIA adapter'
}
$adapterLuid = [long]$adapterMatch.Groups[1].Value
$retainedJson = Get-Content -Raw -LiteralPath $WorkloadInput
$luidPattern = '"adapter_luid"\s*:\s*\d+'
if ([regex]::Matches($retainedJson, $luidPattern).Count -ne 1) {
    throw 'Retained workload input does not contain exactly one adapter_luid'
}
$runJson = [regex]::Replace($retainedJson, $luidPattern, "`"adapter_luid`":$adapterLuid")
[IO.File]::WriteAllText($runInput, $runJson, [Text.UTF8Encoding]::new($false))
$runInputHash = (Get-FileHash -LiteralPath $runInput -Algorithm SHA256).Hash.ToLowerInvariant()

"STEEL_NOMAD_BEGIN $(Get-Date -Format o)"
"input=$WorkloadInput sha256=$inputHash"
"run_input=$runInput sha256=$runInputHash adapter_luid=$adapterLuid"
"output=$output"

$arguments = "--in `"$runInput`" --out `"$output`""
$process = Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $root `
    -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
$null = $process.Handle
if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
    $process.Kill()
    throw "Steel Nomad timed out after $TimeoutSeconds seconds"
}

"exit=$($process.ExitCode) end=$(Get-Date -Format o)"
if (Test-Path -LiteralPath $output) {
    "result_size=$((Get-Item -LiteralPath $output).Length)"
    Get-Content -LiteralPath $output
} else {
    'result_missing=true'
}
foreach ($path in $stdout, $stderr) {
    if ((Test-Path -LiteralPath $path) -and (Get-Item -LiteralPath $path).Length -gt 0) {
        "--- $path"
        Get-Content -LiteralPath $path
    }
}
