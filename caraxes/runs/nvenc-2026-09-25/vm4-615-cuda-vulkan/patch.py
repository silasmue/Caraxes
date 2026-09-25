#!/usr/bin/env python3
# CXDBG instrumentation of the 615.71.09 RM core (GR object path). Usage: patch.py <ogkm root>
import re, sys, os
root = sys.argv[1]
N = os.path.join(root, 'src/nvidia')
TARGETS = {
 'src/kernel/rmapi/alloc_free.c': ['serverAllocResourceUnderLock', 'rmapiAllocWithSecInfo'],
 'src/libraries/resserv/src/rs_client.c': ['clientAllocResource_IMPL'],
 'src/kernel/gpu/fifo/channel_descendant.c': ['chandesConstruct_IMPL'],
 'src/kernel/gpu/gpu_resource.c': ['gpuresConstruct_IMPL'],
 'src/kernel/gpu/gr/kernel_graphics_object.c': ['kgrobjConstruct_IMPL', 'kgrobjPromoteContext_IMPL',
     'kgrobjSetComputeMmio_IMPL', '_kgrAlloc'],
 'src/kernel/gpu/gr/kernel_graphics_context.c': ['kgrctxAllocCtxBuffers_IMPL', 'kgrctxMapCtxBuffers_IMPL',
     'kgrctxPrepareInitializeCtxBuffer_IMPL', 'kgrctxPreparePromoteCtxBuffer_IMPL',
     'kgrctxMapGlobalCtxBuffer_IMPL', 'kgrctxMapGlobalCtxBuffers_IMPL', 'kgrctxAllocMainCtxBuffer_IMPL',
     'kgrctxAllocPatchBuffer_IMPL', 'kgrctxAllocPmBuffer_IMPL'],
 'src/kernel/gpu/gr/kernel_graphics.c': ['kgraphicsMapCtxBuffer_IMPL', 'kgraphicsAllocKgraphicsBuffers_KERNEL',
     'kgraphicsInitializeDeferredStaticData_IMPL'],
 'src/kernel/gpu/gr/kernel_graphics_manager.c': ['kgrmgrCtrlRouteKGRWithDevice_IMPL'],
 'src/kernel/gpu/fifo/kernel_ctxshare.c': ['kctxshareapiConstruct_IMPL', 'kctxshareConstruct_IMPL'],
}
def wrap(src, fn, fname):
    m = re.search(r'\n(static\s+)?NV_STATUS\s*\n' + re.escape(fn) + r'\s*\n?\(\s*\n(.*?)\n\)\s*\n\{', src, re.S)
    if not m:
        raise SystemExit(f'{fname}: {fn} not found')
    static = m.group(1) or ''
    params = m.group(2)
    plist = [p.strip() for p in params.split(',') if p.strip()]
    names = [re.findall(r'(\w+)\s*$', p)[0] for p in plist]
    extra_fmt, extra_args = '', ''
    for p, n in zip(plist, names):
        if re.match(r'RS_RES_ALLOC_PARAMS(_INTERNAL)?\s*\*', p):
            extra_fmt = ' class 0x%x hClient 0x%x hParent 0x%x hObject 0x%x'
            extra_args = f', {n}->externalClassId, {n}->hClient, {n}->hParent, {n}->hResource'
    inner = fn + '__cxinner'
    # rename the original definition, keep it static
    start = m.start()
    head = src[start:m.end()]
    newhead = re.sub(r'\n(static\s+)?NV_STATUS\s*\n' + re.escape(fn), '\nstatic NV_STATUS\n' + inner, head, count=1)
    src = src[:start] + newhead + src[m.end():]
    proto = f'static NV_STATUS {inner}({params.strip()});\n'
    wrapper = (f'\n/* CXDBG wrapper */\n{static}NV_STATUS\n{fn}\n(\n{params}\n)\n{{\n'
               f'    NV_STATUS cxStatus = {inner}({", ".join(names)});\n'
               f'    if (cxStatus != NV_OK)\n'
               f'        NV_PRINTF(LEVEL_ERROR, "CXDBG {fn} -> 0x%x{extra_fmt}\\n", cxStatus{extra_args});\n'
               f'    return cxStatus;\n}}\n')
    return src + wrapper
for rel, fns in TARGETS.items():
    path = os.path.join(N, rel)
    src = open(path).read()
    if 'CXDBG' in src:
        print('already patched', rel); continue
    for fn in fns:
        src = wrap(src, fn, rel)
    # explicit sites
    if rel.endswith('alloc_free.c'):
        old = """        status = clientGetResourceRef(pClient, hParent, &pParentRef);
        if (status != NV_OK)
        {
"""
        assert old in src
        src = src.replace(old, old + """            NV_PRINTF(LEVEL_ERROR, "CXDBG alloc: parent 0x%x not in client 0x%x (class 0x%x hObject 0x%x) -> 0x%x\\n",
                      hParent, hClient, pRmAllocParams->externalClassId, pRmAllocParams->hResource, status);
""", 1)
        old = """            NV_RM_RPC_ALLOC_OBJECT(pGpu,"""
        assert old in src
        src = src.replace(old, """            NV_PRINTF(LEVEL_ERROR, "CXDBG alloc: RPC for class 0x%x hClient 0x%x hParent 0x%x hObject 0x%x\\n",
                      pRmAllocParams->externalClassId, pRmAllocParams->hClient, pRmAllocParams->hParent, pRmAllocParams->hResource);
""" + old, 1)
    if rel.endswith('kernel_graphics_object.c'):
        old = "        if (kgrctxFromKernelChannel(pkChannel, &pKernelGraphicsObject->pKernelGraphicsContext) == NV_OK)\n"
        assert old in src
        new = """        NV_STATUS cxCtx = kgrctxFromKernelChannel(pkChannel, &pKernelGraphicsObject->pKernelGraphicsContext);
        NV_PRINTF(LEVEL_ERROR, "CXDBG kgrobj: class 0x%x hClient 0x%x hParent 0x%x hObject 0x%x chid %u runqueue %u ctxLookup 0x%x hKGC chan 0x%x tsg 0x%x legacy %d vas %p\\n",
                  pParams->externalClassId, pParams->hClient, pParams->hParent, pParams->hResource,
                  pkChannel->ChID, kchannelGetRunqueue(pkChannel), cxCtx, pkChannel->hKernelGraphicsContext,
                  (pkChannel->pKernelChannelGroupApi != NULL) ? pkChannel->pKernelChannelGroupApi->hKernelGraphicsContext : 0,
                  (pkChannel->pKernelChannelGroupApi != NULL && pkChannel->pKernelChannelGroupApi->pKernelChannelGroup != NULL) ?
                      (int)pkChannel->pKernelChannelGroupApi->pKernelChannelGroup->bLegacyMode : -1,
                  pkChannel->pVAS);
        if (cxCtx == NV_OK)
"""
        src = src.replace(old, new, 1)
    open(path, 'w').write(src)
    print('patched', rel, len(fns))
