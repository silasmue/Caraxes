// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
/*
 * nvenc-probe: the NVENC API and the NVDEC (cuvid) API used directly, for the video
 * engines of the Caraxes device (docs/caraxes/video-engines.md). It opens a CUDA context,
 * opens an NVENC session on it, encodes N frames of a moving test pattern to H.264 (or
 * HEVC), checks the bitstream (Annex-B start codes, SPS/PPS/IDR present, the SPS picture
 * size equals the request) and, unless -D, decodes the stream back with NVDEC and compares
 * the decoded luma with the pattern (PSNR).
 *
 *   nvenc-probe [-w 1920] [-h 1080] [-n 300] [-c h264|hevc] [-r fps] [-b kbit] [-o out]
 *               [-D] [-q]
 *     -r fps   pace the encode to fps (0: as fast as possible; default 0)
 *     -b kbit  CBR bitrate (default 20000)
 *     -o out   write the elementary stream there
 *     -D       no decode stage
 *     -q       no per-frame lines
 *
 * Output: "INFO ..." lines, one "ENC frames=.. bytes=.. fps=.. lat_us_med=.. lat_us_max=.."
 * line, one "BITSTREAM ..." line, one "DEC frames=.. fps=.. psnr_y_min=.. psnr_y_avg=.."
 * line, and "RESULT PASS" or "RESULT FAIL <why>" (exit 0 or 1). Errors name the call and
 * its status.
 *
 * Headers: FFmpeg's nv-codec-headers (MIT; include/ffnvcodec: nvEncodeAPI.h,
 * dynlink_loader.h and the cuda/cuvid dynlink headers). The libraries are loaded at run
 * time (libcuda.so.1, libnvidia-encode.so.1, libnvcuvid.so.1), so the build needs no
 * CUDA toolkit:
 *   gcc -O2 -Wall -o nvenc-probe nvenc-probe.c -I<dir of the ffnvcodec headers> -ldl -lm
 */
#include <dlfcn.h>
#include <errno.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include "dynlink_loader.h"

static CudaFunctions *cu;
static NvencFunctions *nvf;
static CuvidFunctions *cv;
static NV_ENCODE_API_FUNCTION_LIST nv = {NV_ENCODE_API_FUNCTION_LIST_VER};

static int W = 1920, H = 1080, N = 300, FPS = 0, KBIT = 20000, HEVC = 0, DECODE = 1, QUIET = 0;
static const char *OUT = NULL;

static double now_s(void)
{
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec + t.tv_nsec * 1e-9;
}

#define CK(call)                                                                   \
    do {                                                                           \
        CUresult r_ = (call);                                                      \
        if (r_ != CUDA_SUCCESS) {                                                  \
            const char *s_ = "?";                                                  \
            if (cu->cuGetErrorName) cu->cuGetErrorName(r_, &s_);                   \
            printf("RESULT FAIL %s: CUDA %d %s\n", #call, (int)r_, s_);            \
            exit(1);                                                               \
        }                                                                          \
    } while (0)

#define NK(call)                                                                   \
    do {                                                                           \
        NVENCSTATUS r_ = (call);                                                   \
        if (r_ != NV_ENC_SUCCESS) {                                                \
            const char *s_ = enc && nv.nvEncGetLastErrorString                     \
                                 ? nv.nvEncGetLastErrorString(enc) : "";           \
            printf("RESULT FAIL %s: NVENCSTATUS %d %s\n", #call, (int)r_, s_);     \
            exit(1);                                                               \
        }                                                                          \
    } while (0)

/* The pattern: diagonal luma ramps moving 4 pixels per frame, a bright box moving
 * across, chroma slowly varying. Smooth enough that 20 Mbit/s keeps it above 30 dB. */
static uint8_t pat_y(int x, int y, int i)
{
    int bx = (i * 8) % (W > 256 ? W - 256 : 1), by = H / 3;
    if (x >= bx && x < bx + 256 && y >= by && y < by + 256)
        return 235;
    return (uint8_t)(16 + ((x + y + 4 * i) / 4) % 200);
}

static void fill_nv12(uint8_t *p, int pitch, int i)
{
    for (int y = 0; y < H; y++) {
        uint8_t *row = p + (size_t)y * pitch;
        for (int x = 0; x < W; x++)
            row[x] = pat_y(x, y, i);
    }
    uint8_t *uv = p + (size_t)H * pitch;
    for (int y = 0; y < H / 2; y++) {
        uint8_t *row = uv + (size_t)y * pitch;
        for (int x = 0; x < W / 2; x++) {
            row[2 * x] = (uint8_t)(128 + ((x / 8 + i) % 32) - 16);
            row[2 * x + 1] = (uint8_t)(128 + ((y / 8) % 32) - 16);
        }
    }
}

/* ---- bitstream check ---- */

struct bits {
    const uint8_t *p;
    size_t n, pos; /* pos in bits */
};

static int getbit(struct bits *b)
{
    if (b->pos >= b->n * 8)
        return -1;
    int v = (b->p[b->pos / 8] >> (7 - b->pos % 8)) & 1;
    b->pos++;
    return v;
}

static uint32_t getbits(struct bits *b, int k)
{
    uint32_t v = 0;
    while (k--)
        v = v << 1 | (uint32_t)(getbit(b) & 1);
    return v;
}

static int64_t ue(struct bits *b)
{
    int z = 0;
    while (getbit(b) == 0 && z < 32)
        z++;
    if (z >= 32)
        return -1;
    return (int64_t)((1ull << z) - 1 + getbits(b, z));
}

static int64_t se(struct bits *b)
{
    int64_t k = ue(b);
    return (k & 1) ? (k + 1) / 2 : -(k / 2);
}

/* NAL payload without emulation-prevention bytes. */
static size_t unescape(const uint8_t *s, size_t n, uint8_t *d)
{
    size_t o = 0, z = 0;
    for (size_t i = 0; i < n; i++) {
        if (z >= 2 && s[i] == 3) {
            z = 0;
            continue;
        }
        d[o++] = s[i];
        z = s[i] == 0 ? z + 1 : 0;
    }
    return o;
}

static void skip_scaling_list(struct bits *b, int size)
{
    int last = 8, next = 8;
    for (int j = 0; j < size; j++) {
        if (next != 0)
            next = (last + (int)se(b) + 256) % 256;
        last = next == 0 ? last : next;
    }
}

/* H.264 SPS (7.3.2.1.1): profile, level and the cropped picture size. */
static int h264_sps(const uint8_t *nal, size_t n, int *prof, int *lvl, int *w, int *h)
{
    uint8_t *buf = malloc(n);
    size_t m = unescape(nal + 1, n - 1, buf);
    struct bits b = {buf, m, 0};
    *prof = (int)getbits(&b, 8);
    getbits(&b, 8);
    *lvl = (int)getbits(&b, 8);
    ue(&b);
    int chroma = 1;
    if (*prof == 100 || *prof == 110 || *prof == 122 || *prof == 244 || *prof == 44 ||
        *prof == 83 || *prof == 86 || *prof == 118 || *prof == 128 || *prof == 138) {
        chroma = (int)ue(&b);
        if (chroma == 3)
            getbit(&b);
        ue(&b);
        ue(&b);
        getbit(&b);
        if (getbit(&b))
            for (int i = 0; i < (chroma != 3 ? 8 : 12); i++)
                if (getbit(&b))
                    skip_scaling_list(&b, i < 6 ? 16 : 64);
    }
    ue(&b); /* log2_max_frame_num_minus4 */
    int64_t poc = ue(&b);
    if (poc == 0) {
        ue(&b);
    } else if (poc == 1) {
        getbit(&b);
        se(&b);
        se(&b);
        int64_t k = ue(&b);
        for (int64_t i = 0; i < k && i < 256; i++)
            se(&b);
    }
    ue(&b);
    getbit(&b);
    int64_t wmb = ue(&b) + 1, hmu = ue(&b) + 1;
    int frame_mbs_only = getbit(&b);
    if (!frame_mbs_only)
        getbit(&b);
    getbit(&b);
    int64_t cl = 0, cr = 0, ct = 0, cb = 0;
    if (getbit(&b)) {
        cl = ue(&b);
        cr = ue(&b);
        ct = ue(&b);
        cb = ue(&b);
    }
    int sx = chroma == 1 || chroma == 2 ? 2 : 1, sy = (chroma == 1 ? 2 : 1) * (2 - frame_mbs_only);
    *w = (int)(wmb * 16 - sx * (cl + cr));
    *h = (int)(hmu * 16 * (2 - frame_mbs_only) - sy * (ct + cb));
    free(buf);
    return b.pos <= m * 8 ? 0 : -1;
}

struct nalstat {
    long nal, sps, pps, vps, idr, slice, other, bad_start;
    int prof, lvl, w, h, sps_ok;
};

static void scan_nals(const uint8_t *p, size_t n, struct nalstat *st)
{
    size_t i = 0;
    if (n < 4 || !(p[0] == 0 && p[1] == 0 && (p[2] == 1 || (p[2] == 0 && p[3] == 1))))
        st->bad_start++;
    while (i + 3 <= n) {
        if (p[i] == 0 && p[i + 1] == 0 && p[i + 2] == 1) {
            size_t s = i + 3, e = s;
            while (e + 3 <= n && !(p[e] == 0 && p[e + 1] == 0 && (p[e + 2] == 1 || (p[e + 2] == 0 && e + 3 < n && p[e + 3] == 1))))
                e++;
            if (e + 3 > n)
                e = n;
            st->nal++;
            if (s < e) {
                if (!HEVC) {
                    int t = p[s] & 0x1f;
                    if (t == 7) {
                        st->sps++;
                        st->sps_ok = h264_sps(p + s, e - s, &st->prof, &st->lvl, &st->w, &st->h) == 0;
                    } else if (t == 8) st->pps++;
                    else if (t == 5) st->idr++;
                    else if (t == 1) st->slice++;
                    else st->other++;
                } else {
                    int t = (p[s] >> 1) & 0x3f;
                    if (t == 32) st->vps++;
                    else if (t == 33) st->sps++;
                    else if (t == 34) st->pps++;
                    else if (t == 19 || t == 20) st->idr++;
                    else if (t < 32) st->slice++;
                    else st->other++;
                }
            }
            i = e;
        } else {
            i++;
        }
    }
}

/* ---- decode stage ---- */

struct dec {
    CUvideodecoder dec;
    CUVIDEOFORMAT fmt;
    long frames;
    double psnr_min, psnr_sum;
    uint8_t *host;
    int err;
};

static int CUDAAPI on_seq(void *u, CUVIDEOFORMAT *f)
{
    struct dec *d = u;
    CUVIDDECODECAPS caps = {0};
    caps.eCodecType = f->codec;
    caps.eChromaFormat = f->chroma_format;
    caps.nBitDepthMinus8 = f->bit_depth_luma_minus8;
    if (cv->cuvidGetDecoderCaps(&caps) != CUDA_SUCCESS || !caps.bIsSupported) {
        printf("INFO NVDEC: codec %d not supported by the decoder caps (bIsSupported %d)\n",
               f->codec, caps.bIsSupported);
        d->err = 1;
        return 0;
    }
    printf("INFO NVDEC caps: max %ux%u, %u NVDEC engine(s), output formats mask %#x\n",
           caps.nMaxWidth, caps.nMaxHeight, caps.nNumNVDECs, caps.nOutputFormatMask);
    if (d->dec)
        return f->min_num_decode_surfaces;
    d->fmt = *f;
    CUVIDDECODECREATEINFO ci = {0};
    ci.ulWidth = f->coded_width;
    ci.ulHeight = f->coded_height;
    ci.ulNumDecodeSurfaces = f->min_num_decode_surfaces + 2;
    ci.CodecType = f->codec;
    ci.ChromaFormat = f->chroma_format;
    ci.ulCreationFlags = cudaVideoCreate_PreferCUVID;
    ci.bitDepthMinus8 = f->bit_depth_luma_minus8;
    ci.OutputFormat = cudaVideoSurfaceFormat_NV12;
    ci.DeinterlaceMode = cudaVideoDeinterlaceMode_Weave;
    ci.ulTargetWidth = f->coded_width;
    ci.ulTargetHeight = f->coded_height;
    ci.ulNumOutputSurfaces = 2;
    ci.display_area.left = (short)f->display_area.left;
    ci.display_area.top = (short)f->display_area.top;
    ci.display_area.right = (short)f->display_area.right;
    ci.display_area.bottom = (short)f->display_area.bottom;
    ci.ulTargetWidth = f->display_area.right - f->display_area.left;
    ci.ulTargetHeight = f->display_area.bottom - f->display_area.top;
    CUresult r = cv->cuvidCreateDecoder(&d->dec, &ci);
    if (r != CUDA_SUCCESS) {
        printf("INFO cuvidCreateDecoder: CUDA %d\n", (int)r);
        d->err = 1;
        return 0;
    }
    printf("INFO NVDEC decoder %ux%u coded, %lux%lu target, %lu surfaces\n", f->coded_width,
           f->coded_height, ci.ulTargetWidth, ci.ulTargetHeight, ci.ulNumDecodeSurfaces);
    return (int)ci.ulNumDecodeSurfaces;
}

static int CUDAAPI on_decode(void *u, CUVIDPICPARAMS *pp)
{
    struct dec *d = u;
    if (!d->dec)
        return 0;
    CUresult r = cv->cuvidDecodePicture(d->dec, pp);
    if (r != CUDA_SUCCESS) {
        printf("INFO cuvidDecodePicture: CUDA %d\n", (int)r);
        d->err = 1;
        return 0;
    }
    return 1;
}

static int CUDAAPI on_display(void *u, CUVIDPARSERDISPINFO *di)
{
    struct dec *d = u;
    if (!d->dec || !di)
        return 1;
    CUVIDPROCPARAMS pp = {0};
    pp.progressive_frame = di->progressive_frame;
    pp.top_field_first = di->top_field_first;
    CUdeviceptr dp = 0;
    unsigned pitch = 0;
    CUresult r = cv->cuvidMapVideoFrame(d->dec, di->picture_index, &dp, &pitch, &pp);
    if (r != CUDA_SUCCESS) {
        printf("INFO cuvidMapVideoFrame: CUDA %d\n", (int)r);
        d->err = 1;
        return 0;
    }
    CUDA_MEMCPY2D c = {0};
    c.srcMemoryType = CU_MEMORYTYPE_DEVICE;
    c.srcDevice = dp;
    c.srcPitch = pitch;
    c.dstMemoryType = CU_MEMORYTYPE_HOST;
    c.dstHost = d->host;
    c.dstPitch = W;
    c.WidthInBytes = W;
    c.Height = H;
    r = cu->cuMemcpy2D(&c);
    cv->cuvidUnmapVideoFrame(d->dec, dp);
    if (r != CUDA_SUCCESS) {
        printf("INFO cuMemcpy2D of the decoded frame: CUDA %d\n", (int)r);
        d->err = 1;
        return 0;
    }
    int i = (int)di->timestamp;
    double se2 = 0;
    for (int y = 0; y < H; y++)
        for (int x = 0; x < W; x++) {
            int e = (int)d->host[(size_t)y * W + x] - (int)pat_y(x, y, i);
            se2 += e * e;
        }
    double mse = se2 / ((double)W * H);
    double psnr = mse == 0 ? 99.0 : 10 * log10(255.0 * 255.0 / mse);
    if (d->frames == 0 || psnr < d->psnr_min)
        d->psnr_min = psnr;
    d->psnr_sum += psnr;
    d->frames++;
    return 1;
}

static int cmp_d(const void *a, const void *b)
{
    double x = *(const double *)a, y = *(const double *)b;
    return x < y ? -1 : x > y;
}

int main(int argc, char **argv)
{
    int o;
    while ((o = getopt(argc, argv, "w:h:n:c:r:b:o:Dq")) != -1) {
        switch (o) {
        case 'w': W = atoi(optarg); break;
        case 'h': H = atoi(optarg); break;
        case 'n': N = atoi(optarg); break;
        case 'c': HEVC = strcmp(optarg, "hevc") == 0; break;
        case 'r': FPS = atoi(optarg); break;
        case 'b': KBIT = atoi(optarg); break;
        case 'o': OUT = optarg; break;
        case 'D': DECODE = 0; break;
        case 'q': QUIET = 1; break;
        default:
            fprintf(stderr, "usage: nvenc-probe [-w W] [-h H] [-n N] [-c h264|hevc] [-r fps] [-b kbit] [-o out] [-D] [-q]\n");
            return 2;
        }
    }
    void *enc = NULL;
    if (cuda_load_functions(&cu, NULL) < 0) {
        printf("RESULT FAIL libcuda.so.1 not loaded\n");
        return 1;
    }
    if (nvenc_load_functions(&nvf, NULL) < 0) {
        printf("RESULT FAIL libnvidia-encode.so.1 not loaded\n");
        return 1;
    }
    if (DECODE && cuvid_load_functions(&cv, NULL) < 0) {
        printf("INFO libnvcuvid.so.1 not loaded: no decode stage\n");
        DECODE = 0;
    }
    double t0 = now_s();
    CK(cu->cuInit(0));
    CUdevice dev;
    CK(cu->cuDeviceGet(&dev, 0));
    char name[128] = "";
    cu->cuDeviceGetName(name, sizeof name, dev);
    CUcontext ctx;
    CK(cu->cuCtxCreate(&ctx, 0, dev));
    printf("INFO device \"%s\", CUDA context in %.1f ms\n", name, (now_s() - t0) * 1e3);

    uint32_t maxver = 0;
    NK(nvf->NvEncodeAPIGetMaxSupportedVersion(&maxver));
    uint32_t hdrver = (NVENCAPI_MAJOR_VERSION << 4) | NVENCAPI_MINOR_VERSION;
    printf("INFO NVENC API: driver supports %u.%u, header %u.%u\n", maxver >> 4, maxver & 0xf,
           NVENCAPI_MAJOR_VERSION, NVENCAPI_MINOR_VERSION);
    if (maxver < hdrver) {
        printf("RESULT FAIL the driver's NVENC API %u.%u is older than the header's\n", maxver >> 4, maxver & 0xf);
        return 1;
    }
    NK(nvf->NvEncodeAPICreateInstance(&nv));

    double t1 = now_s();
    NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS op = {0};
    op.version = NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS_VER;
    op.deviceType = NV_ENC_DEVICE_TYPE_CUDA;
    op.device = ctx;
    op.apiVersion = NVENCAPI_VERSION;
    NK(nv.nvEncOpenEncodeSessionEx(&op, &enc));
    printf("INFO NVENC session opened in %.1f ms\n", (now_s() - t1) * 1e3);

    GUID codec = HEVC ? NV_ENC_CODEC_HEVC_GUID : NV_ENC_CODEC_H264_GUID;
    uint32_t ng = 0;
    NK(nv.nvEncGetEncodeGUIDCount(enc, &ng));
    GUID gl[16];
    uint32_t got = 0;
    NK(nv.nvEncGetEncodeGUIDs(enc, gl, ng < 16 ? ng : 16, &got));
    int have = 0;
    for (uint32_t i = 0; i < got; i++)
        have |= memcmp(&gl[i], &codec, sizeof codec) == 0;
    printf("INFO NVENC codecs: %u, %s %s\n", got, HEVC ? "HEVC" : "H.264", have ? "present" : "ABSENT");
    if (!have) {
        printf("RESULT FAIL codec not offered by the encoder\n");
        return 1;
    }
    NV_ENC_CAPS_PARAM cp = {NV_ENC_CAPS_PARAM_VER};
    int wmax = 0, hmax = 0;
    cp.capsToQuery = NV_ENC_CAPS_WIDTH_MAX;
    nv.nvEncGetEncodeCaps(enc, codec, &cp, &wmax);
    cp.capsToQuery = NV_ENC_CAPS_HEIGHT_MAX;
    nv.nvEncGetEncodeCaps(enc, codec, &cp, &hmax);
    printf("INFO NVENC caps: max %dx%d\n", wmax, hmax);

    NV_ENC_PRESET_CONFIG pc = {0};
    pc.version = NV_ENC_PRESET_CONFIG_VER;
    pc.presetCfg.version = NV_ENC_CONFIG_VER;
    NK(nv.nvEncGetEncodePresetConfigEx(enc, codec, NV_ENC_PRESET_P1_GUID,
                                       NV_ENC_TUNING_INFO_ULTRA_LOW_LATENCY, &pc));
    NV_ENC_CONFIG cfg = pc.presetCfg;
    cfg.rcParams.rateControlMode = NV_ENC_PARAMS_RC_CBR;
    cfg.rcParams.averageBitRate = (uint32_t)KBIT * 1000;
    cfg.rcParams.maxBitRate = (uint32_t)KBIT * 1000;
    cfg.gopLength = 120;
    cfg.frameIntervalP = 1;
    if (HEVC)
        cfg.encodeCodecConfig.hevcConfig.idrPeriod = 120;
    else
        cfg.encodeCodecConfig.h264Config.idrPeriod = 120;

    NV_ENC_INITIALIZE_PARAMS ip = {0};
    ip.version = NV_ENC_INITIALIZE_PARAMS_VER;
    ip.encodeGUID = codec;
    ip.presetGUID = NV_ENC_PRESET_P1_GUID;
    ip.tuningInfo = NV_ENC_TUNING_INFO_ULTRA_LOW_LATENCY;
    ip.encodeWidth = W;
    ip.encodeHeight = H;
    ip.darWidth = W;
    ip.darHeight = H;
    ip.frameRateNum = FPS ? FPS : 60;
    ip.frameRateDen = 1;
    ip.enablePTD = 1;
    ip.encodeConfig = &cfg;
    ip.maxEncodeWidth = W;
    ip.maxEncodeHeight = H;
    t1 = now_s();
    NK(nv.nvEncInitializeEncoder(enc, &ip));
    printf("INFO NVENC encoder initialised %dx%d %s P1 ultra-low-latency CBR %d kbit/s in %.1f ms\n",
           W, H, HEVC ? "HEVC" : "H.264", KBIT, (now_s() - t1) * 1e3);

    NV_ENC_CREATE_INPUT_BUFFER ib = {0};
    ib.version = NV_ENC_CREATE_INPUT_BUFFER_VER;
    ib.width = W;
    ib.height = H;
    ib.bufferFmt = NV_ENC_BUFFER_FORMAT_NV12;
    NK(nv.nvEncCreateInputBuffer(enc, &ib));
    NV_ENC_CREATE_BITSTREAM_BUFFER bb = {0};
    bb.version = NV_ENC_CREATE_BITSTREAM_BUFFER_VER;
    NK(nv.nvEncCreateBitstreamBuffer(enc, &bb));

    size_t cap = 1 << 20, len = 0;
    uint8_t *es = malloc(cap);
    size_t *off = calloc((size_t)N + 1, sizeof *off);
    double *lat = calloc((size_t)N, sizeof *lat);
    double fill_s = 0, te0 = now_s();
    for (int i = 0; i < N; i++) {
        if (FPS) {
            double due = te0 + (double)i / FPS;
            double d = due - now_s();
            if (d > 0)
                usleep((useconds_t)(d * 1e6));
        }
        double tf = now_s();
        NV_ENC_LOCK_INPUT_BUFFER li = {0};
        li.version = NV_ENC_LOCK_INPUT_BUFFER_VER;
        li.inputBuffer = ib.inputBuffer;
        NK(nv.nvEncLockInputBuffer(enc, &li));
        fill_nv12(li.bufferDataPtr, (int)li.pitch, i);
        NK(nv.nvEncUnlockInputBuffer(enc, ib.inputBuffer));
        fill_s += now_s() - tf;

        double ts = now_s();
        NV_ENC_PIC_PARAMS pp = {0};
        pp.version = NV_ENC_PIC_PARAMS_VER;
        pp.inputWidth = W;
        pp.inputHeight = H;
        pp.inputPitch = li.pitch;
        pp.inputBuffer = ib.inputBuffer;
        pp.outputBitstream = bb.bitstreamBuffer;
        pp.bufferFmt = NV_ENC_BUFFER_FORMAT_NV12;
        pp.pictureStruct = NV_ENC_PIC_STRUCT_FRAME;
        pp.inputTimeStamp = (uint64_t)i;
        NK(nv.nvEncEncodePicture(enc, &pp));
        NV_ENC_LOCK_BITSTREAM lb = {0};
        lb.version = NV_ENC_LOCK_BITSTREAM_VER;
        lb.outputBitstream = bb.bitstreamBuffer;
        NK(nv.nvEncLockBitstream(enc, &lb));
        lat[i] = (now_s() - ts) * 1e6;
        if (len + lb.bitstreamSizeInBytes > cap) {
            while (len + lb.bitstreamSizeInBytes > cap)
                cap *= 2;
            es = realloc(es, cap);
        }
        memcpy(es + len, lb.bitstreamBufferPtr, lb.bitstreamSizeInBytes);
        off[i] = len;
        len += lb.bitstreamSizeInBytes;
        if (!QUIET && (i < 3 || i % 60 == 0))
            printf("FRAME %d type %d bytes %u lat_us %.0f\n", i, (int)lb.pictureType,
                   lb.bitstreamSizeInBytes, lat[i]);
        NK(nv.nvEncUnlockBitstream(enc, bb.bitstreamBuffer));
    }
    off[N] = len;
    double te = now_s() - te0;
    double *ls = malloc((size_t)N * sizeof *ls);
    memcpy(ls, lat, (size_t)N * sizeof *ls);
    qsort(ls, (size_t)N, sizeof *ls, cmp_d);
    printf("ENC frames=%d bytes=%zu seconds=%.3f fps=%.1f fill_ms_total=%.0f lat_us_med=%.0f lat_us_p99=%.0f lat_us_max=%.0f kbit_per_s_at_%dfps=%.0f\n",
           N, len, te, N / te, fill_s * 1e3, ls[N / 2], ls[(N * 99) / 100], ls[N - 1],
           FPS ? FPS : 60, len * 8.0 / 1000 / ((double)N / (FPS ? FPS : 60)));
    NK(nv.nvEncDestroyInputBuffer(enc, ib.inputBuffer));
    NK(nv.nvEncDestroyBitstreamBuffer(enc, bb.bitstreamBuffer));
    NK(nv.nvEncDestroyEncoder(enc));
    enc = NULL;

    if (OUT) {
        FILE *f = fopen(OUT, "wb");
        if (!f || fwrite(es, 1, len, f) != len) {
            printf("RESULT FAIL cannot write %s: %s\n", OUT, strerror(errno));
            return 1;
        }
        fclose(f);
    }
    struct nalstat st = {0};
    scan_nals(es, len, &st);
    printf("BITSTREAM codec=%s nal=%ld vps=%ld sps=%ld pps=%ld idr=%ld slice=%ld other=%ld bad_start=%ld",
           HEVC ? "hevc" : "h264", st.nal, st.vps, st.sps, st.pps, st.idr, st.slice, st.other, st.bad_start);
    if (!HEVC)
        printf(" profile=%d level=%d sps_size=%dx%d sps_parsed=%d", st.prof, st.lvl, st.w, st.h, st.sps_ok);
    printf("\n");
    const char *why = NULL;
    if (len == 0) why = "no bitstream";
    else if (st.bad_start) why = "the stream does not begin with an Annex-B start code";
    else if (!st.sps || !st.pps || !st.idr) why = "SPS, PPS or IDR missing";
    else if (HEVC && !st.vps) why = "VPS missing";
    else if (st.idr + st.slice < N) why = "fewer slices than frames";
    else if (!HEVC && (!st.sps_ok || st.w != W || st.h != H)) why = "SPS picture size differs from the request";

    if (!why && DECODE) {
        struct dec d = {0};
        d.host = malloc((size_t)W * H);
        CUvideoparser parser;
        CUVIDPARSERPARAMS pp = {0};
        pp.CodecType = HEVC ? cudaVideoCodec_HEVC : cudaVideoCodec_H264;
        pp.ulMaxNumDecodeSurfaces = 1;
        pp.ulMaxDisplayDelay = 0;
        pp.pUserData = &d;
        pp.pfnSequenceCallback = on_seq;
        pp.pfnDecodePicture = on_decode;
        pp.pfnDisplayPicture = on_display;
        CK(cv->cuvidCreateVideoParser(&parser, &pp));
        double td = now_s();
        for (int i = 0; i < N && !d.err; i++) {
            CUVIDSOURCEDATAPACKET pk = {0};
            pk.flags = CUVID_PKT_TIMESTAMP;
            pk.payload_size = (unsigned long)(off[i + 1] - off[i]);
            pk.payload = es + off[i];
            pk.timestamp = i;
            CK(cv->cuvidParseVideoData(parser, &pk));
        }
        CUVIDSOURCEDATAPACKET eos = {0};
        eos.flags = CUVID_PKT_ENDOFSTREAM;
        CK(cv->cuvidParseVideoData(parser, &eos));
        td = now_s() - td;
        cv->cuvidDestroyVideoParser(parser);
        if (d.dec)
            cv->cuvidDestroyDecoder(d.dec);
        printf("DEC frames=%ld seconds=%.3f fps=%.1f psnr_y_min=%.2f psnr_y_avg=%.2f (the decoded luma against the pattern; includes the copy to the host and the comparison)\n",
               d.frames, td, d.frames / td, d.psnr_min, d.frames ? d.psnr_sum / d.frames : 0.0);
        if (d.err) why = "NVDEC stage failed";
        else if (d.frames != N) why = "NVDEC returned fewer frames than encoded";
        else if (d.psnr_min < 30.0) why = "decoded luma differs from the pattern (PSNR < 30 dB)";
    }
    cu->cuCtxDestroy(ctx);
    if (why) {
        printf("RESULT FAIL %s\n", why);
        return 1;
    }
    printf("RESULT PASS\n");
    return 0;
}
