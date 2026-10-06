# SPDX-License-Identifier: MIT
# Windows PowerShell 5.1. Run as SYSTEM in a dedicated scheduled-task process.
# Example arguments: -OutputDirectory C:\Users\Public\caraxes\process-trace -DurationSeconds 900
# The output directory must be fresh. Creating its "stop" file requests shutdown.
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$OutputDirectory,

    [ValidateRange(1, 900)]
    [int]$DurationSeconds = 900
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$script:StartedUtc = [DateTime]::UtcNow
$script:DeadlineUtc = $script:StartedUtc.AddSeconds($DurationSeconds)
$script:Sink = $null
$script:Writer = $null
$script:SelfTestPid = -1
$script:SelfTestMinimumUtc = [DateTime]::MinValue
$script:SelfTestStartSeen = $false
$script:SelfTestStopSeen = $false
$script:SelfTestObservedExitStatus = $null
$script:EventsWritten = 0
$script:StopReason = 'not_started'
$readyWritten = $false
$fatal = $null
$failurePath = $null
$finishedPath = $null
$mayWriteStatus = $false
$cleanupErrors = New-Object System.Collections.ArrayList
$utf8 = [System.Text.UTF8Encoding]::new($false)

# WMI callbacks copy event fields into a .NET concurrent queue. PowerShell alone
# drains that queue and writes JSONL; no Get-Event/Remove-Event IDs are involved.
$traceSinkSource = @'
using System;
using System.Collections.Concurrent;
using System.Management;
using System.Threading;

public sealed class CaraxesProcessEvent {
    public long Sequence;
    public string Kind;
    public string ProcessName;
    public uint ProcessId;
    public uint ParentProcessId;
    public uint SessionId;
    public ulong EventFileTime100ns;
    public uint? ExitStatus;
    public DateTime CallbackReceivedUtc;
    public string Error;
}

public sealed class CaraxesProcessTrace : IDisposable {
    private readonly ConcurrentQueue<CaraxesProcessEvent> queue =
        new ConcurrentQueue<CaraxesProcessEvent>();
    private ManagementEventWatcher startWatcher;
    private ManagementEventWatcher stopWatcher;
    private long sequence;
    private int stopping;
    private int stopped;

    public ConcurrentQueue<CaraxesProcessEvent> Queue { get { return queue; } }

    private void EnqueueError(string kind, Exception error) {
        queue.Enqueue(new CaraxesProcessEvent {
            Sequence = Interlocked.Increment(ref sequence),
            Kind = kind,
            CallbackReceivedUtc = DateTime.UtcNow,
            Error = error.ToString()
        });
    }

    private void Capture(EventArrivedEventArgs args, bool isStop) {
        try {
            ManagementBaseObject e = args.NewEvent;
            queue.Enqueue(new CaraxesProcessEvent {
                Sequence = Interlocked.Increment(ref sequence),
                Kind = isStop ? "stop" : "start",
                ProcessName = Convert.ToString(e["ProcessName"]),
                ProcessId = Convert.ToUInt32(e["ProcessID"]),
                ParentProcessId = Convert.ToUInt32(e["ParentProcessID"]),
                SessionId = Convert.ToUInt32(e["SessionID"]),
                EventFileTime100ns = Convert.ToUInt64(e["TIME_CREATED"]),
                ExitStatus = isStop ? (uint?)Convert.ToUInt32(e["ExitStatus"]) : null,
                CallbackReceivedUtc = DateTime.UtcNow
            });
        } catch (Exception ex) {
            EnqueueError("callback_error", ex);
        }
    }

    private void OnStart(object sender, EventArrivedEventArgs e) { Capture(e, false); }
    private void OnStop(object sender, EventArrivedEventArgs e) { Capture(e, true); }
    private void OnWatcherStopped(object sender, StoppedEventArgs e) {
        if (Interlocked.CompareExchange(ref stopping, 0, 0) == 0) {
            EnqueueError("watcher_stopped", new InvalidOperationException(
                "WMI process watcher stopped unexpectedly: " + e.Status));
        }
    }

    public void Start() {
        if (startWatcher != null || stopWatcher != null)
            throw new InvalidOperationException("Trace sink already started");
        try {
            ManagementScope scope = new ManagementScope(@"\\.\root\cimv2");
            startWatcher = new ManagementEventWatcher(scope,
                new WqlEventQuery("SELECT * FROM Win32_ProcessStartTrace"));
            stopWatcher = new ManagementEventWatcher(scope,
                new WqlEventQuery("SELECT * FROM Win32_ProcessStopTrace"));
            startWatcher.EventArrived += OnStart;
            stopWatcher.EventArrived += OnStop;
            startWatcher.Stopped += OnWatcherStopped;
            stopWatcher.Stopped += OnWatcherStopped;
            startWatcher.Start();
            stopWatcher.Start();
        } catch {
            try { Stop(); } catch { }
            throw;
        }
    }

    public void Stop() {
        if (Interlocked.Exchange(ref stopped, 1) != 0) return;
        Interlocked.Exchange(ref stopping, 1);
        Exception first = null;
        if (stopWatcher != null) {
            try { stopWatcher.Stop(); } catch (Exception ex) { if (first == null) first = ex; }
            try { stopWatcher.Dispose(); } catch (Exception ex) { if (first == null) first = ex; }
        }
        if (startWatcher != null) {
            try { startWatcher.Stop(); } catch (Exception ex) { if (first == null) first = ex; }
            try { startWatcher.Dispose(); } catch (Exception ex) { if (first == null) first = ex; }
        }
        if (first != null) throw first;
    }

    public void Dispose() { Stop(); }
}
'@

function Write-JsonLine {
    param([System.Collections.IDictionary]$Record)
    $script:Writer.WriteLine(($Record | ConvertTo-Json -Compress -Depth 8))
    $script:Writer.Flush()
    $script:EventsWritten++
}

function Drain-TraceEvents {
    $item = [CaraxesProcessEvent]$null
    while ($script:Sink.Queue.TryDequeue([ref]$item)) {
        if ($item.Kind -eq 'callback_error' -or $item.Kind -eq 'watcher_stopped') {
            Write-JsonLine ([ordered]@{
                kind = $item.Kind
                callback_sequence = $item.Sequence
                observed_utc = $item.CallbackReceivedUtc.ToString('o')
                error = $item.Error
            })
            throw ('WMI trace callback failed: {0}' -f $item.Error)
        }
        $kind = $item.Kind
        $pidValue = [int]$item.ProcessId
        $eventUtc = [DateTime]::FromFileTimeUtc([long]$item.EventFileTime100ns)
        $record = [ordered]@{
            kind = $kind
            process_name = [string]$item.ProcessName
            pid = $pidValue
            parent_pid = [int]$item.ParentProcessId
            session_id = [int]$item.SessionId
            event_utc = $eventUtc.ToString('o')
            event_filetime_100ns = [string]$item.EventFileTime100ns
            callback_received_utc = $item.CallbackReceivedUtc.ToString('o')
            dequeued_utc = [DateTime]::UtcNow.ToString('o')
            callback_sequence = [long]$item.Sequence
        }

        if ($kind -eq 'stop') {
            # WMI defines this as uint32; preserve the exact value and its bit pattern.
            $status = [uint32]$item.ExitStatus
            $record['exit_status_uint32'] = [string]$status
            $record['exit_status_hex'] = ('0x{0:X8}' -f $status)
        }

        if ($kind -eq 'start' -and
            ($record['process_name'] -ieq '3DMarkICFWorkload.exe' -or
             $pidValue -eq $script:SelfTestPid)) {
            # StartTrace has no command-line field. A live Win32_Process query is
            # opportunistic and must never be mistaken for proof of absence.
            $record['command_line'] = $null
            $record['command_line_source'] = 'live_Win32_Process_best_effort'
            $record['command_line_observed_utc'] = [DateTime]::UtcNow.ToString('o')
            try {
                $live = Get-WmiObject -Namespace root\cimv2 -Class Win32_Process `
                    -Filter ('ProcessId = {0}' -f $pidValue) -ErrorAction Stop
                if ($null -eq $live) {
                    $record['command_line_note'] = 'process_not_live_at_lookup'
                } else {
                    $createdUtc = [System.Management.ManagementDateTimeConverter]::ToDateTime(
                        [string]$live.CreationDate).ToUniversalTime()
                    $record['live_creation_utc'] = $createdUtc.ToString('o')
                    if ([Math]::Abs(($createdUtc - $eventUtc).TotalSeconds) -gt 5) {
                        $record['command_line_note'] = 'live_pid_creation_time_mismatch'
                    } else {
                        $record['command_line'] = [string]$live.CommandLine
                        if ([string]::IsNullOrEmpty($record['command_line'])) {
                            $record['command_line_note'] = 'live_command_line_unavailable'
                        }
                    }
                }
            } catch {
                $record['command_line_note'] = 'live_query_error'
                $record['command_line_error'] = $_.Exception.ToString()
            }
        }

        Write-JsonLine $record

        if ($pidValue -eq $script:SelfTestPid -and
            $record['process_name'] -ieq 'cmd.exe' -and
            $eventUtc -ge $script:SelfTestMinimumUtc) {
            if ($kind -eq 'start') { $script:SelfTestStartSeen = $true }
            if ($kind -eq 'stop') {
                $script:SelfTestStopSeen = $true
                $script:SelfTestObservedExitStatus = [uint32]$item.ExitStatus
            }
        }
        $item = [CaraxesProcessEvent]$null
    }
}

try {
    $OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
    [IO.Directory]::CreateDirectory($OutputDirectory) | Out-Null
    $eventPath = Join-Path $OutputDirectory 'process-events.jsonl'
    $readyPath = Join-Path $OutputDirectory 'ready.json'
    $stopPath = Join-Path $OutputDirectory 'stop'
    $finishedPath = Join-Path $OutputDirectory 'finished.json'
    $failurePath = Join-Path $OutputDirectory 'failure.json'
    foreach ($path in @($eventPath, $readyPath, $stopPath, $finishedPath, $failurePath)) {
        if (Test-Path -LiteralPath $path) {
            throw ('Output directory is not fresh: {0}' -f [IO.Path]::GetFileName($path))
        }
    }
    $mayWriteStatus = $true

    $stream = [IO.File]::Open($eventPath, [IO.FileMode]::CreateNew,
        [IO.FileAccess]::Write, [IO.FileShare]::Read)
    $script:Writer = [IO.StreamWriter]::new($stream, $utf8)
    $script:Writer.AutoFlush = $true

    if (-not [Security.Principal.WindowsIdentity]::GetCurrent().IsSystem) {
        throw 'Collector must run as LocalSystem'
    }

    Add-Type -TypeDefinition $traceSinkSource -ReferencedAssemblies @('System.dll', 'System.Management.dll') `
        -ErrorAction Stop
    $script:Sink = [CaraxesProcessTrace]::new()
    $script:Sink.Start()

    # A successful subscription call alone is insufficient: prove both streams
    # can report one owned short-lived process, including its exact exit code.
    $cmd = Join-Path $env:SystemRoot 'System32\cmd.exe'
    if (-not [IO.File]::Exists($cmd)) { throw 'Self-test cmd.exe is unavailable' }
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $cmd
    $startInfo.Arguments = '/d /c exit 37'
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $selfTest = [Diagnostics.Process]::new()
    $selfTest.StartInfo = $startInfo
    $script:SelfTestMinimumUtc = [DateTime]::UtcNow.AddSeconds(-1)
    try {
        if (-not $selfTest.Start()) { throw 'Self-test process did not start' }
        $script:SelfTestPid = $selfTest.Id
        if (-not $selfTest.WaitForExit(10000)) {
            $selfTest.Kill()
            $selfTest.WaitForExit(2000) | Out-Null
            throw 'Self-test process exceeded ten seconds'
        }
        $directExitStatus = $selfTest.ExitCode
    } finally {
        $selfTest.Dispose()
    }

    $selfTestDeadlineUtc = [DateTime]::UtcNow.AddSeconds(20)
    if ($selfTestDeadlineUtc -gt $script:DeadlineUtc) {
        $selfTestDeadlineUtc = $script:DeadlineUtc
    }
    while ([DateTime]::UtcNow -lt $selfTestDeadlineUtc) {
        Drain-TraceEvents
        if ($script:SelfTestStartSeen -and $script:SelfTestStopSeen) { break }
        Start-Sleep -Milliseconds 100
    }
    Drain-TraceEvents
    if ($directExitStatus -ne 37 -or
        -not $script:SelfTestStartSeen -or
        -not $script:SelfTestStopSeen -or
        $script:SelfTestObservedExitStatus -ne 37) {
        throw ('Trace self-test failed: direct={0}, start={1}, stop={2}, trace_exit={3}' -f
            $directExitStatus, $script:SelfTestStartSeen, $script:SelfTestStopSeen,
            $script:SelfTestObservedExitStatus)
    }
    if (Test-Path -LiteralPath $stopPath) { throw 'Stop requested before readiness' }
    if ([DateTime]::UtcNow -ge $script:DeadlineUtc) {
        throw 'Deadline reached before readiness'
    }

    $ready = [ordered]@{
        schema = 'caraxes-process-trace-v1'
        ready_utc = [DateTime]::UtcNow.ToString('o')
        deadline_utc = $script:DeadlineUtc.ToString('o')
        watchers = @('Win32_ProcessStartTrace', 'Win32_ProcessStopTrace')
        self_test_pid = $script:SelfTestPid
        self_test_expected_exit_status = 37
        self_test_direct_exit_status = $directExitStatus
        self_test_trace_exit_status = $script:SelfTestObservedExitStatus
        self_test_start_seen = $script:SelfTestStartSeen
        self_test_stop_seen = $script:SelfTestStopSeen
    }
    $readyTemp = Join-Path $OutputDirectory ('ready.' + [Guid]::NewGuid().ToString('N') + '.tmp')
    [IO.File]::WriteAllText($readyTemp, ($ready | ConvertTo-Json -Depth 5), $utf8)
    [IO.File]::Move($readyTemp, $readyPath)
    $readyWritten = $true

    while ([DateTime]::UtcNow -lt $script:DeadlineUtc) {
        Drain-TraceEvents
        if (Test-Path -LiteralPath $stopPath) {
            $script:StopReason = 'stop_marker'
            break
        }
        Start-Sleep -Milliseconds 100
    }
    if ($script:StopReason -ne 'stop_marker') { $script:StopReason = 'deadline' }
} catch {
    $fatal = $_.Exception.ToString()
    $script:StopReason = 'failure'
    if ($null -ne $script:Writer) {
        try {
            Write-JsonLine ([ordered]@{
                kind = 'collector_error'
                observed_utc = [DateTime]::UtcNow.ToString('o')
                error = $fatal
            })
        } catch { [void]$cleanupErrors.Add('Could not write collector error to JSONL: ' + $_.Exception.ToString()) }
    }
} finally {
    if ($null -ne $script:Sink) {
        try { $script:Sink.Stop() }
        catch { [void]$cleanupErrors.Add('Stop WMI watchers: ' + $_.Exception.ToString()) }
    }
    if ($null -ne $script:Writer) {
        try { Drain-TraceEvents }
        catch { [void]$cleanupErrors.Add('Final event drain: ' + $_.Exception.ToString()) }
        try { $script:Writer.Dispose() }
        catch { [void]$cleanupErrors.Add('Writer close: ' + $_.Exception.ToString()) }
    }
    if ($null -ne $fatal -and $mayWriteStatus) {
        try {
            [IO.File]::WriteAllText($failurePath, ([ordered]@{
                failed_utc = [DateTime]::UtcNow.ToString('o')
                error = $fatal
            } | ConvertTo-Json -Depth 5), $utf8)
        } catch { [void]$cleanupErrors.Add('Failure file: ' + $_.Exception.ToString()) }
    }
    if ($null -ne $script:Writer -and $mayWriteStatus) {
        try {
            [IO.File]::WriteAllText($finishedPath, ([ordered]@{
                schema = 'caraxes-process-trace-v1'
                started_utc = $script:StartedUtc.ToString('o')
                finished_utc = [DateTime]::UtcNow.ToString('o')
                ready_written = $readyWritten
                reason = $script:StopReason
                events_written = $script:EventsWritten
                cleanup_errors = @($cleanupErrors.ToArray())
            } | ConvertTo-Json -Depth 5), $utf8)
        } catch { [void]$cleanupErrors.Add('Finished file: ' + $_.Exception.ToString()) }
    }
}

if ($null -ne $fatal) { throw $fatal }
if ($cleanupErrors.Count -gt 0) {
    throw ('Trace cleanup failed: {0}' -f ($cleanupErrors -join '; '))
}
