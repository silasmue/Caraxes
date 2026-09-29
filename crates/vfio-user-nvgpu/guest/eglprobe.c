// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//
// Guest probe for display phase E (docs/caraxes/display-e.md): does the NVIDIA EGL/GLES
// driver render on the GPU? Opens the EGL default display (the device platform when no
// window system is set), makes a WxH pbuffer with a GLES2 context, and for each of N
// frames clears it to a frame-dependent colour and glFinish()es; at the end clears to
// green and reads the whole frame back. Prints the vendor/renderer, per-frame times and whether the pixels are right.
// The image has no EGL/GLES headers, so the few declarations needed are written here.
// Build: cc -O2 -o eglprobe eglprobe.c /usr/lib/x86_64-linux-gnu/libEGL.so.1
//        /usr/lib/x86_64-linux-gnu/libGLESv2.so.2
// Usage: eglprobe [width] [height] [frames]
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

typedef void *EGLDisplay, *EGLConfig, *EGLSurface, *EGLContext;
typedef int32_t EGLint;
typedef unsigned int EGLBoolean, EGLenum;
EGLDisplay eglGetDisplay(void *);
EGLBoolean eglInitialize(EGLDisplay, EGLint *, EGLint *);
const char *eglQueryString(EGLDisplay, EGLint);
EGLBoolean eglChooseConfig(EGLDisplay, const EGLint *, EGLConfig *, EGLint, EGLint *);
EGLSurface eglCreatePbufferSurface(EGLDisplay, EGLConfig, const EGLint *);
EGLBoolean eglBindAPI(EGLenum);
EGLContext eglCreateContext(EGLDisplay, EGLConfig, EGLContext, const EGLint *);
EGLBoolean eglMakeCurrent(EGLDisplay, EGLSurface, EGLSurface, EGLContext);
EGLint eglGetError(void);
EGLBoolean eglTerminate(EGLDisplay);
#define EGL_VENDOR 0x3053
#define EGL_VERSION 0x3054
#define EGL_NONE 0x3038
#define EGL_SURFACE_TYPE 0x3033
#define EGL_PBUFFER_BIT 0x0001
#define EGL_RENDERABLE_TYPE 0x3040
#define EGL_OPENGL_ES2_BIT 0x0004
#define EGL_RED_SIZE 0x3024
#define EGL_GREEN_SIZE 0x3023
#define EGL_BLUE_SIZE 0x3022
#define EGL_ALPHA_SIZE 0x3021
#define EGL_WIDTH 0x3057
#define EGL_HEIGHT 0x3056
#define EGL_OPENGL_ES_API 0x30A0
#define EGL_CONTEXT_CLIENT_VERSION 0x3098

typedef unsigned int GLenum, GLuint, GLbitfield;
typedef int GLint, GLsizei;
typedef float GLfloat;
typedef unsigned char GLubyte;
const GLubyte *glGetString(GLenum);
void glViewport(GLint, GLint, GLsizei, GLsizei);
void glClearColor(GLfloat, GLfloat, GLfloat, GLfloat);
void glClear(GLbitfield);
void glFinish(void);
void glReadPixels(GLint, GLint, GLsizei, GLsizei, GLenum, GLenum, void *);
GLenum glGetError(void);
#define GL_VENDOR 0x1F00
#define GL_RENDERER 0x1F01
#define GL_VERSION 0x1F02
#define GL_COLOR_BUFFER_BIT 0x4000
#define GL_RGBA 0x1908
#define GL_UNSIGNED_BYTE 0x1401

static double now(void)
{
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec + t.tv_nsec * 1e-9;
}

int main(int argc, char **argv)
{
    int w = argc > 1 ? atoi(argv[1]) : 1920, h = argc > 2 ? atoi(argv[2]) : 1080;
    int frames = argc > 3 ? atoi(argv[3]) : 60;
    EGLDisplay d = eglGetDisplay(NULL);
    EGLint ma, mi;
    if (!d || !eglInitialize(d, &ma, &mi)) {
        printf("EGL-INIT-FAIL error %#x\n", eglGetError());
        return 1;
    }
    printf("EGL %d.%d vendor %s version %s\n", ma, mi, eglQueryString(d, EGL_VENDOR), eglQueryString(d, EGL_VERSION));
    const EGLint ca[] = {EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT,
                         EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_ALPHA_SIZE, 8, EGL_NONE};
    EGLConfig cfg;
    EGLint n = 0;
    if (!eglChooseConfig(d, ca, &cfg, 1, &n) || n < 1) {
        printf("EGL-CONFIG-FAIL error %#x\n", eglGetError());
        return 1;
    }
    const EGLint sa[] = {EGL_WIDTH, w, EGL_HEIGHT, h, EGL_NONE};
    EGLSurface s = eglCreatePbufferSurface(d, cfg, sa);
    eglBindAPI(EGL_OPENGL_ES_API);
    const EGLint xa[] = {EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE};
    EGLContext c = eglCreateContext(d, cfg, NULL, xa);
    if (!s || !c || !eglMakeCurrent(d, s, s, c)) {
        printf("EGL-CONTEXT-FAIL error %#x\n", eglGetError());
        return 1;
    }
    printf("GL vendor %s renderer %s version %s\n", glGetString(GL_VENDOR), glGetString(GL_RENDERER), glGetString(GL_VERSION));
    glViewport(0, 0, w, h);
    double tmin = 1e9, tmax = 0, tsum = 0, t0 = now();
    for (int f = 0; f < frames; f++) {
        double a = now();
        glClearColor((f & 1) ? 1.f : 0.f, 0.25f, (f & 2) ? 1.f : 0.f, 1.f);
        glClear(GL_COLOR_BUFFER_BIT);
        glFinish();
        double b = now() - a;
        tsum += b;
        if (b < tmin) tmin = b;
        if (b > tmax) tmax = b;
    }
    double total = now() - t0;
    // Last frame: clear to a known colour, then draw nothing more; read back.
    glClearColor(0.f, 1.f, 0.f, 1.f);
    glClear(GL_COLOR_BUFFER_BIT);
    glFinish();
    uint32_t *px = malloc((size_t)w * h * 4);
    double a = now();
    glReadPixels(0, 0, w, h, GL_RGBA, GL_UNSIGNED_BYTE, px);
    double rb = now() - a;
    uint64_t sum = 0;
    size_t good = 0;
    for (size_t i = 0; i < (size_t)w * h; i++) {
        sum = sum * 1099511628211ULL ^ px[i];
        good += px[i] == 0xff00ff00u;
    }
    printf("gl error %#x\n", glGetError());
    printf("FRAMES %d at %dx%d: clear+finish mean %.3f ms min %.3f max %.3f; total %.3f s\n", frames, w, h,
           tsum / frames * 1e3, tmin * 1e3, tmax * 1e3, total);
    printf("READBACK %.3f ms; centre pixel %#010x; pixels equal to green %zu of %zu (%s)\n", rb * 1e3,
           px[(size_t)(h / 2) * w + w / 2], good, (size_t)w * h, good == (size_t)w * h ? "GPU-RENDERED-OK" : "WRONG");
    eglTerminate(d);
    return good == (size_t)w * h ? 0 : 2;
}
