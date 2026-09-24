# before-emu -> after-emu

## Controls whose answer changed (status, label)

## Non-zero answers only after (new refusals)
- none

## nvml gate
- before: ('fail', 'failed: temp-number pstate-number power-number util-number temp-matches-host pstate-family')
- after: ('fail', 'failed: temp-number pstate-number power-number util-number temp-matches-host pstate-family')

## cuda gate rows
- cuda-context: fail -> fail
- cuda-core: fail -> fail
- cuda-hostreg: fail -> fail
- cuda-init: fail -> fail
- cuda-jit: fail -> fail
- cuda-launch: fail -> fail
- cuda-managed: fail -> fail
- robustness: fail -> fail
