# SPDX-License-Identifier: MIT
# Read-only identity probe for the installed Fire Strike x64 workload file.
# No benchmark or UI is started. A live workload makes the observation invalid.
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

$expected = 'C:\Program Files (x86)\Steam\steamapps\common\3DMark\dlc\fire-strike-test\bin\x64\3DMarkICFWorkload.exe'
$maximumBytes = 256MB

function Assert-NoLiveWorkload {
    $active = @(Get-Process -ErrorAction SilentlyContinue | Where-Object {
        $_.ProcessName -match '^3DMarkICFWorkload(?:32|64)?$'
    })
    if ($active.Count -ne 0) { throw 'An ICF workload is live; file identity probe refused' }
}

function Get-StableFile {
    $item = Get-Item -LiteralPath $expected -ErrorAction Stop
    if (-not ($item -is [System.IO.FileInfo])) { throw 'Installed workload is not a file' }
    if (-not [string]::Equals($item.FullName, $expected,
            [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'Installed workload path changed'
    }
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw 'Installed workload is a reparse point'
    }
    $parent = $item.Directory
    while ($null -ne $parent) {
        if (($parent.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw 'Installed workload parent is a reparse point'
        }
        $parent = $parent.Parent
    }
    if ($item.Length -le 0 -or $item.Length -gt $maximumBytes) {
        throw 'Installed workload size is outside the bounded probe range'
    }
    return $item
}

Assert-NoLiveWorkload
$before = Get-StableFile
$version = $before.VersionInfo
$firstHash = (Get-FileHash -LiteralPath $expected -Algorithm SHA256 -ErrorAction Stop).Hash.ToLowerInvariant()
$signature = Get-AuthenticodeSignature -LiteralPath $expected -ErrorAction Stop
$secondHash = (Get-FileHash -LiteralPath $expected -Algorithm SHA256 -ErrorAction Stop).Hash.ToLowerInvariant()
$after = Get-StableFile
$afterVersion = $after.VersionInfo
Assert-NoLiveWorkload
if ($before.Length -ne $after.Length -or
    $before.LastWriteTimeUtc.Ticks -ne $after.LastWriteTimeUtc.Ticks -or
    $version.FileVersion -ne $afterVersion.FileVersion -or
    $version.ProductVersion -ne $afterVersion.ProductVersion -or
    $firstHash -ne $secondHash) {
    throw 'Installed workload changed during file identity probe'
}

$certificate = $signature.SignerCertificate
$statusMessage = [string]$signature.StatusMessage
$subject = if ($null -ne $certificate) { [string]$certificate.Subject } else { $null }
if ($statusMessage.Length -gt 2048 -or ($null -ne $subject -and $subject.Length -gt 2048)) {
    throw 'Signature text exceeds bounded probe output'
}
$result = [ordered]@{
    schema = 'caraxes-firestrike-file-identity-v1'
    observed_utc = [DateTime]::UtcNow.ToString('o')
    path = $after.FullName
    length = [long]$after.Length
    last_write_utc_ticks = [long]$after.LastWriteTimeUtc.Ticks
    file_version = [string]$version.FileVersion
    product_version = [string]$version.ProductVersion
    sha256 = $secondHash
    signature_status = $signature.Status.ToString()
    signature_status_message = $statusMessage
    signature_type = $signature.SignatureType.ToString()
    signer_subject = $subject
    signer_thumbprint = if ($null -ne $certificate) { [string]$certificate.Thumbprint } else { $null }
    signer_not_before_utc = if ($null -ne $certificate) { $certificate.NotBefore.ToUniversalTime().ToString('o') } else { $null }
    signer_not_after_utc = if ($null -ne $certificate) { $certificate.NotAfter.ToUniversalTime().ToString('o') } else { $null }
}
$json = ConvertTo-Json -InputObject $result -Depth 3 -Compress
if ([Text.Encoding]::UTF8.GetByteCount($json) -gt 8192) {
    throw 'File identity probe JSON exceeds output bound'
}
$json
