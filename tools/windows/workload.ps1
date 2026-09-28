# SPDX-License-Identifier: MIT
# The compute workloads of the Linux and native comparison (caraxes/runs/night-2026-09-28), on
# Windows. Everything lives under C:\caraxes\wl; the host serves the files over QEMU user
# networking (10.0.2.2 = the host's loopback) and MANIFEST.sha256 checks each one.
#   install   llama.cpp b10173 CUDA 13.3 + cudart, Python 3.12.10, torch 2.13.0+cu130, the models
#             (kept in C:\caraxes\wl\dl; a rerun fetches only what is missing or differs)
#   info      nvidia-smi -q, the driver's name for the device
#   llama     llama-bench -ngl <Ngl> -p 512 -n 128 -r 3 on -Model 1b|7b
#   torch     caraxes/tools/<Tool>.py --seconds <Seconds> (transformer or convnet, batch 32 defaults)
# Usage: workload.ps1 -Stage install [-Server http://10.0.2.2:8765]
#        workload.ps1 -Stage llama -Model 1b [-Ngl 99] | -Stage torch -Tool transformer [-Seconds 60]
param([Parameter(Mandatory)][ValidateSet('install', 'info', 'llama', 'torch')][string]$Stage,
      [string]$Server = 'http://10.0.2.2:8765', [ValidateSet('1b', '7b')][string]$Model = '1b',
      [int]$Ngl = 99, [ValidateSet('transformer', 'convnet')][string]$Tool = 'transformer',
      [int]$Seconds = 60, [int]$TimeoutS = 900)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$W = 'C:\caraxes\wl'
$out = "$W\out"
New-Item -ItemType Directory -Force $W, $out, "$W\dl" | Out-Null
$models = @{ '1b' = 'Llama-3.2-1B-Instruct-Q4_K_M.gguf'; '7b' = 'Qwen2.5-7B-Instruct-Q4_K_M.gguf' }
$py = "$W\py312\python.exe"

# A native command with stderr merged; PowerShell 5.1 would stop on its first stderr line.
function Invoke-Native {
    $ErrorActionPreference = 'Continue'
    $exe, $rest = $args
    & $exe @rest 2>&1 | ForEach-Object { "$_" }
    "exit $LASTEXITCODE"
}

# Runs a program with a deadline; stdout and stderr go to one log, which is printed.
function Invoke-Timed($exe, [string[]]$argv, $log) {
    $t0 = Get-Date
    $p = Start-Process $exe -ArgumentList $argv -NoNewWindow -PassThru -WorkingDirectory $W `
        -RedirectStandardOutput "$log.out" -RedirectStandardError "$log.err"
    $null = $p.Handle
    $done = $p.WaitForExit($TimeoutS * 1000)
    if (-not $done) { $p.Kill(); $code = 'TIMEOUT' } else { $code = $p.ExitCode }
    $s = ((Get-Date) - $t0).TotalSeconds
    $text = "== $exe $($argv -join ' ')`r`n== exit $code after $([math]::Round($s, 1)) s`r`n" +
        (Get-Content -Raw "$log.out") + "`r`n== stderr`r`n" + (Get-Content -Raw "$log.err")
    Set-Content -Path $log -Value $text
    $text
    "WORKLOAD_EXIT=$code"
}

function Get-Checked($name, $sum) {
    $dst = "$W\dl\$name"
    if ((Test-Path $dst) -and (Get-FileHash -Algorithm SHA256 $dst).Hash -eq $sum) { return $dst }
    & curl.exe -sS --fail -o $dst "$Server/$([uri]::EscapeDataString($name))"
    if ($LASTEXITCODE -ne 0) { throw "download failed (curl exit $LASTEXITCODE): $name" }
    $got = (Get-FileHash -Algorithm SHA256 $dst).Hash
    if ($got -ne $sum) { throw "sha256 mismatch for ${name}: $got" }
    "fetched $name ($((Get-Item $dst).Length) bytes, sha256 ok)" | Write-Host
    $dst
}

switch ($Stage) {
    'install' {
        & curl.exe -sS --fail -o "$W\dl\MANIFEST.sha256" "$Server/MANIFEST.sha256"
        if ($LASTEXITCODE -ne 0) { throw "no manifest at $Server (is the host server up?)" }
        $sums = @{}
        foreach ($l in Get-Content "$W\dl\MANIFEST.sha256") {
            $h, $n = $l -split '\s+', 2
            $sums[$n.TrimStart('*')] = $h.ToUpper()
        }
        $files = @{}
        foreach ($n in $sums.Keys) { $files[$n] = Get-Checked $n $sums[$n] }
        if (-not (Test-Path "$W\llama\llama-bench.exe")) {
            New-Item -ItemType Directory -Force "$W\llama" | Out-Null
            foreach ($z in 'llama-b10173-bin-win-cuda-13.3-x64.zip', 'cudart-llama-bin-win-cuda-13.3-x64.zip') {
                & tar.exe -xf $files[$z] -C "$W\llama"
                if ($LASTEXITCODE -ne 0) { throw "tar exit $LASTEXITCODE for $z" }
            }
        }
        New-Item -ItemType Directory -Force "$W\tools" | Out-Null
        foreach ($t in 'transformer.py', 'convnet.py') { Copy-Item $files[$t] "$W\tools\$t" -Force }
        if (-not (Test-Path $py)) {
            $sig = Get-AuthenticodeSignature $files['python-3.12.10-amd64.exe']
            if ($sig.Status -ne 'Valid') { throw "python installer signature: $($sig.Status)" }
            "python installer signed by $($sig.SignerCertificate.Subject)"
            $p = Start-Process $files['python-3.12.10-amd64.exe'] -Wait -PassThru -ArgumentList @(
                '/quiet', 'InstallAllUsers=1', "TargetDir=$W\py312", 'PrependPath=0', 'Include_launcher=0',
                'Include_test=0', 'Include_doc=0', 'Include_tcltk=0', 'Shortcuts=0', 'AssociateFiles=0')
            if ($p.ExitCode -ne 0) { throw "python installer exit $($p.ExitCode)" }
        }
        $pip = Invoke-Native $py -m pip install --disable-pip-version-check --no-index --find-links "$W\dl" 'torch==2.13.0+cu130'
        $pip
        if ($pip[-1] -ne 'exit 0') { throw 'pip install failed' }
        Invoke-Native $py -c "import sys, torch; print('python', sys.version.split()[0], 'torch', torch.__version__, 'cuda', torch.version.cuda)"
        Invoke-Native "$W\llama\llama-bench.exe" --version
        'INSTALL_OK'
    }
    'info' {
        Invoke-Native nvidia-smi.exe -q
        Invoke-Native nvidia-smi.exe
        Get-CimInstance Win32_VideoController | Format-List Name, DriverVersion, AdapterRAM, Status | Out-String
    }
    'llama' {
        $m = "$W\dl\$($models[$Model])"
        Invoke-Timed "$W\llama\llama-bench.exe" @('-m', $m, '-ngl', "$Ngl", '-p', '512', '-n', '128', '-r', '3') `
            "$out\llama-$Model-ngl$Ngl.txt"
    }
    'torch' {
        Invoke-Timed $py @("$W\tools\$Tool.py", '--seconds', "$Seconds") "$out\$Tool.txt"
    }
}
