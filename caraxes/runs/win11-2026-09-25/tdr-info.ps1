# What Windows recorded about its last TDRs: the DxgKrnl channels, the live kernel dumps
# and the WER live-kernel-event reports (bug check code and parameters), newest first.
param([int]$Max = 6)
Get-WinEvent -ListLog *DxgKrnl*, *Display* -ErrorAction SilentlyContinue |
    Where-Object { $_.RecordCount -gt 0 } | ForEach-Object {
        "== log $($_.LogName) ($($_.RecordCount) records)"
        Get-WinEvent -LogName $_.LogName -MaxEvents $Max -ErrorAction SilentlyContinue |
            ForEach-Object { "{0:HH:mm:ss.fff} id {1} lvl {2}: {3}" -f $_.TimeCreated, $_.Id, $_.Level, (($_.Message -replace '\s+', ' ') -replace '^(.{0,400}).*$', '$1') }
    }
"== LiveKernelReports"
Get-ChildItem C:\Windows\LiveKernelReports -Recurse -File -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First $Max |
    ForEach-Object { "{0:HH:mm:ss} {1} {2}" -f $_.LastWriteTime, $_.Length, $_.FullName }
"== WER live kernel events"
Get-ChildItem C:\ProgramData\Microsoft\Windows\WER\ReportArchive, C:\ProgramData\Microsoft\Windows\WER\ReportQueue -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -like '*LiveKernelEvent*' -or $_.Name -like '*Kernel_1[14][17]*' } |
    Sort-Object LastWriteTime -Descending | Select-Object -First $Max | ForEach-Object {
        "-- $($_.Name) $($_.LastWriteTime)"
        $w = Join-Path $_.FullName 'Report.wer'
        if (Test-Path $w) {
            Get-Content $w -Encoding Unicode | Where-Object { $_ -match '^(EventType|Sig\[\d\]\.(Name|Value)|DynamicSig\[\d+\]\.(Name|Value)|EventTime)=' }
        }
    }
