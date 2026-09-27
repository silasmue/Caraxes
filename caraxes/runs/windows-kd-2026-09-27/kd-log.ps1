# SPDX-License-Identifier: MIT
param(
    [int]$Lines = 80,
    [string]$Snapshot
)
$ErrorActionPreference = 'Stop'
$stream = [IO.FileStream]::new('C:\Temp\kd-console.log', [IO.FileMode]::Open,
    [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
$reader = [IO.StreamReader]::new($stream)
try {
    $text = $reader.ReadToEnd()
} finally {
    $reader.Dispose()
}
if ($Snapshot) {
    [IO.File]::WriteAllText($Snapshot, $text, [Text.UTF8Encoding]::new($false))
} else {
    $text -split '\r?\n' | Select-Object -Last $Lines
}
