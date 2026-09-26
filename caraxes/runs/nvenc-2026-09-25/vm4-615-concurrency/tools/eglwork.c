// eglwork: GL ES 2 off-screen render without a display: EGL_EXT_platform_device (the NVIDIA
// device), a 512x512 pbuffer (or surfaceless), a shaded full-screen quad per frame, glFinish
// per frame, a glReadPixels check. For <secs> seconds; a glFinish over 30 s prints HANG.
//   gcc -O2 -o eglwork eglwork.c -lEGL -lGLESv2     ./eglwork [secs]
#include <EGL/egl.h>
#include <EGL/eglext.h>
#include <GLES2/gl2.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
static double now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+t.tv_nsec*1e-9;}
static double t0; static volatile long frames;
static void onalrm(int s){(void)s; char b[128]; int n=snprintf(b,sizeof b,"RESULT HANG frames=%ld t=%.2f\n",frames,now()-t0); write(1,b,n); _exit(3);}
#define FAIL(...) do{printf("RESULT FAIL " __VA_ARGS__); printf(" egl=0x%x t=%.2f\n",eglGetError(),now()-t0); fflush(stdout); exit(1);}while(0)
int main(int argc,char**argv){
  int secs=argc>1?atoi(argv[1]):60; t0=now(); struct timespec w; clock_gettime(CLOCK_REALTIME,&w);
  printf("START wall=%ld.%03ld pid=%d\n",(long)w.tv_sec,w.tv_nsec/1000000,getpid()); fflush(stdout);
  signal(SIGALRM,onalrm); alarm(60);
  PFNEGLQUERYDEVICESEXTPROC qd=(void*)eglGetProcAddress("eglQueryDevicesEXT");
  PFNEGLGETPLATFORMDISPLAYEXTPROC gpd=(void*)eglGetProcAddress("eglGetPlatformDisplayEXT");
  PFNEGLQUERYDEVICESTRINGEXTPROC qds=(void*)eglGetProcAddress("eglQueryDeviceStringEXT");
  if(!qd||!gpd) FAIL("step=no-device-ext");
  EGLDeviceEXT devs[8]; EGLint nd=0; if(!qd(8,devs,&nd)||nd<1) FAIL("step=eglQueryDevicesEXT n=%d",nd);
  EGLDisplay dpy=EGL_NO_DISPLAY; int pick=-1;
  for(int i=0;i<nd;i++){const char*e=qds?qds(devs[i],EGL_EXTENSIONS):""; printf("egl device %d: %s\n",i,e?e:"");
    if(pick<0&&e&&strstr(e,"EGL_NV_device_cuda")) pick=i;}
  if(pick<0) pick=0;
  dpy=gpd(EGL_PLATFORM_DEVICE_EXT,devs[pick],0); if(dpy==EGL_NO_DISPLAY) FAIL("step=eglGetPlatformDisplayEXT");
  EGLint ma,mi; if(!eglInitialize(dpy,&ma,&mi)) FAIL("step=eglInitialize");
  printf("vendor %s version %s\n",eglQueryString(dpy,EGL_VENDOR),eglQueryString(dpy,EGL_VERSION));
  EGLint ca[]={EGL_SURFACE_TYPE,EGL_PBUFFER_BIT,EGL_RENDERABLE_TYPE,EGL_OPENGL_ES2_BIT,EGL_RED_SIZE,8,EGL_GREEN_SIZE,8,EGL_BLUE_SIZE,8,EGL_NONE};
  EGLConfig cfg; EGLint nc=0; if(!eglChooseConfig(dpy,ca,&cfg,1,&nc)||nc<1) FAIL("step=eglChooseConfig");
  EGLint pa[]={EGL_WIDTH,512,EGL_HEIGHT,512,EGL_NONE}; EGLSurface sf=eglCreatePbufferSurface(dpy,cfg,pa);
  if(sf==EGL_NO_SURFACE) FAIL("step=eglCreatePbufferSurface");
  eglBindAPI(EGL_OPENGL_ES_API);
  const char*pe=getenv("EGL_PRIO"); EGLint cta[]={EGL_CONTEXT_CLIENT_VERSION,2,pe?0x3100:EGL_NONE,pe&&!strcmp(pe,"high")?0x3101:pe&&!strcmp(pe,"low")?0x3103:0x3102,EGL_NONE};
  if(pe) printf("context priority requested: %s\n",pe); EGLContext cx=eglCreateContext(dpy,cfg,EGL_NO_CONTEXT,cta);
  if(cx==EGL_NO_CONTEXT) FAIL("step=eglCreateContext");
  if(!eglMakeCurrent(dpy,sf,sf,cx)) FAIL("step=eglMakeCurrent");
  printf("GL_RENDERER %s\n",glGetString(GL_RENDERER)); if(pe){EGLint v=0; eglQueryContext(dpy,cx,0x3100,&v); printf("context priority got 0x%x\n",v);} fflush(stdout);
  const char*vs="attribute vec2 p; void main(){gl_Position=vec4(p,0.,1.);}";
  const char*fs="precision highp float; uniform float t; void main(){ vec2 u=gl_FragCoord.xy/512.; float a=0.; for(int i=0;i<64;i++) a+=sin(u.x*float(i)+t)*cos(u.y*float(i)-t); gl_FragColor=vec4(fract(a),u,1.);}";
  GLuint pr=glCreateProgram(); const char*src[2]={vs,fs}; GLenum ty[2]={GL_VERTEX_SHADER,GL_FRAGMENT_SHADER};
  for(int i=0;i<2;i++){GLuint s=glCreateShader(ty[i]); glShaderSource(s,1,&src[i],0); glCompileShader(s); GLint ok; glGetShaderiv(s,GL_COMPILE_STATUS,&ok); if(!ok) FAIL("step=shader%d",i); glAttachShader(pr,s);}
  glBindAttribLocation(pr,0,"p"); glLinkProgram(pr); glUseProgram(pr); GLint ut=glGetUniformLocation(pr,"t");
  float q[]={-1,-1,1,-1,-1,1,1,1}; glVertexAttribPointer(0,2,GL_FLOAT,0,0,q); glEnableVertexAttribArray(0);
  printf("setup in %.2f s\n",now()-t0); fflush(stdout); alarm(0);
  double ts=now(), maxf=0; long bad=0; unsigned char px[4];
  while(now()-ts<secs){
    double a=now(); alarm(30);
    glClearColor(0,0,1,1); glClear(GL_COLOR_BUFFER_BIT); glUniform1f(ut,(float)frames*0.01f); glDrawArrays(GL_TRIANGLE_STRIP,0,4);
    glFinish(); glReadPixels(256,256,1,1,GL_RGBA,GL_UNSIGNED_BYTE,px); alarm(0);
    if(px[3]!=255||glGetError()) bad++;
    double l=now()-a; if(l>maxf) maxf=l; frames++;
  }
  double el=now()-ts;
  printf("RESULT %s frames=%ld fps=%.1f max_frame_ms=%.1f bad=%ld secs=%.1f total=%.1f\n",bad?"FAIL":"PASS",frames,frames/el,maxf*1e3,bad,el,now()-t0);
  eglTerminate(dpy); return bad?1:0;
}
