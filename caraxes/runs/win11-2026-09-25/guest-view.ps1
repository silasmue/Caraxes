# SPDX-License-Identifier: MIT
# The guest's view of the NVIDIA adapter start (night-win4.sh, after the device trace stalls).
Get-WinEvent -FilterHashtable @{LogName='System'; ProviderName='nvlddmkm','Microsoft-Windows-DxgKrnl','Microsoft-Windows-Kernel-PnP'} -MaxEvents 40 -ErrorAction SilentlyContinue | Where-Object { $_.Id -ne 225 -and $_.TimeCreated -gt (Get-Date).AddMinutes(-15) } | Format-List TimeCreated,ProviderName,Id,Message | Out-String -Width 200
Get-WinEvent -LogName 'Microsoft-Windows-DxgKrnl-Admin' -MaxEvents 10 -ErrorAction SilentlyContinue | Where-Object { $_.TimeCreated -gt (Get-Date).AddMinutes(-15) } | Format-List TimeCreated,Id,Message | Out-String -Width 200
Get-PnpDevice -Class Display -PresentOnly | Where-Object { $_.InstanceId -like 'PCI\VEN_10DE*' } | Format-List InstanceId,Status,Problem | Out-String
