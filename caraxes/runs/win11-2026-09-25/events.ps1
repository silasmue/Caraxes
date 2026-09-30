# The display driver's events (nvlddmkm, Display, dxgkrnl TDR 4101/4097/141/117) between two
# local times, with their insertion strings (the device's RC messages are among them).
param([string]$From = '2026-09-29 08:50', [string]$To = '2026-09-29 09:15')
$f = [datetime]$From; $t = [datetime]$To
Get-WinEvent -FilterHashtable @{ LogName = 'System'; StartTime = $f; EndTime = $t } -ErrorAction SilentlyContinue |
    Where-Object { $_.ProviderName -match 'nvlddmkm|Display|DxgKrnl|dxgkrnl' -or $_.Id -in 4101, 4097, 141, 117 } |
    Sort-Object TimeCreated |
    ForEach-Object { "{0:HH:mm:ss.fff} {1} id {2} lvl {3}: {4} | {5}" -f $_.TimeCreated, $_.ProviderName, $_.Id, $_.Level, ($_.Message -replace '\s+', ' '), (($_.Properties | ForEach-Object { $_.Value }) -join ' ; ') }
Get-WinEvent -FilterHashtable @{ LogName = 'Application'; StartTime = $f; EndTime = $t } -ErrorAction SilentlyContinue |
    Where-Object { $_.ProviderName -match 'Windows Error Reporting|Application Error|Application Hang' } |
    ForEach-Object { "{0:HH:mm:ss} {1} id {2}: {3}" -f $_.TimeCreated, $_.ProviderName, $_.Id, ($_.Message -replace '\s+', ' ').Substring(0, [Math]::Min(300, ($_.Message -replace '\s+', ' ').Length)) }
