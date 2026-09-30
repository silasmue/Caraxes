# Guest CPU every 2 s, language-independent (WMI perf class); -Count samples.
param([int]$Count = 120)
for ($i = 0; $i -lt $Count; $i++) {
    $p = Get-CimInstance -ClassName Win32_PerfFormattedData_PerfOS_Processor | Where-Object Name -eq '_Total'
    "{0:HH:mm:ss} total {1} priv {2} intr {3} dpc {4}" -f (Get-Date), $p.PercentProcessorTime, $p.PercentPrivilegedTime, $p.PercentInterruptTime, $p.PercentDPCTime
    Start-Sleep 2
}
