// cuwork: headless CUDA compute through the driver API without headers or nvcc: libcuda via
// dlopen, a hand-written PTX kernel JIT-compiled by the driver. For <secs> seconds: launch an
// FMA loop over blocks*256 threads, synchronize, read back and verify 4 values.
// A synchronize that does not return within 30 s prints HANG (SIGALRM).
//   gcc -O2 -o cuwork cuwork.c -ldl -lm      ./cuwork [secs] [iters] [blocks]
#include <dlfcn.h>
#include <math.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
typedef int CUresult; typedef int CUdevice; typedef void *CUcontext, *CUmodule, *CUfunction, *CUstream; typedef unsigned long long CUdeviceptr;
static const char ptx[] =
".version 6.4\n.target sm_75\n.address_size 64\n"
".visible .entry busy(.param .u64 out, .param .u32 iters)\n{\n"
" .reg .pred %p<2>; .reg .f32 %f<4>; .reg .b32 %r<8>; .reg .b64 %rd<5>;\n"
" ld.param.u64 %rd1, [out]; ld.param.u32 %r1, [iters];\n"
" mov.u32 %r2, %tid.x; mov.u32 %r3, %ctaid.x; mov.u32 %r4, %ntid.x;\n"
" mad.lo.s32 %r5, %r3, %r4, %r2; cvt.rn.f32.u32 %f1, %r5; mov.f32 %f2, 0f3F800000; mov.u32 %r6, 0;\n"
"L1:\n fma.rn.f32 %f2, %f2, 0f3F7FFFF0, %f1; add.s32 %r6, %r6, 1; setp.lt.u32 %p1, %r6, %r1; @%p1 bra L1;\n"
" cvta.to.global.u64 %rd2, %rd1; mul.wide.u32 %rd3, %r5, 4; add.s64 %rd4, %rd2, %rd3; st.global.f32 [%rd4], %f2; ret;\n}\n";
static double now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+t.tv_nsec*1e-9;}
static double t0; static volatile long launches; static volatile int phase;
static void onalrm(int s){(void)s; char b[160]; int n=snprintf(b,sizeof b,"RESULT HANG launches=%ld phase=%d t=%.2f\n",launches,phase,now()-t0); write(1,b,n); _exit(3);}
#define SYM(n) n##_t n = (n##_t)dlsym(h, #n); if(!n){printf("RESULT FAIL nosym %s\n",#n);return 1;}
typedef CUresult (*cuInit_t)(unsigned); typedef CUresult (*cuDeviceGet_t)(CUdevice*,int);
typedef CUresult (*cuDevicePrimaryCtxRetain_t)(CUcontext*,CUdevice); typedef CUresult (*cuCtxSetCurrent_t)(CUcontext);
typedef CUresult (*cuModuleLoadData_t)(CUmodule*,const void*); typedef CUresult (*cuModuleGetFunction_t)(CUfunction*,CUmodule,const char*);
typedef CUresult (*cuMemAlloc_v2_t)(CUdeviceptr*,size_t); typedef CUresult (*cuLaunchKernel_t)(CUfunction,unsigned,unsigned,unsigned,unsigned,unsigned,unsigned,unsigned,CUstream,void**,void**);
typedef CUresult (*cuCtxSynchronize_t)(void); typedef CUresult (*cuMemcpyDtoH_v2_t)(void*,CUdeviceptr,size_t);
typedef CUresult (*cuGetErrorString_t)(CUresult,const char**); typedef CUresult (*cuDevicePrimaryCtxRelease_v2_t)(CUdevice);
int main(int argc,char**argv){
  int secs=argc>1?atoi(argv[1]):60; unsigned iters=argc>2?atoi(argv[2]):20000, blocks=argc>3?atoi(argv[3]):144;
  t0=now(); struct timespec w; clock_gettime(CLOCK_REALTIME,&w);
  printf("START wall=%ld.%03ld pid=%d\n",(long)w.tv_sec,w.tv_nsec/1000000,getpid()); fflush(stdout);
  signal(SIGALRM,onalrm);
  void *h=dlopen("libcuda.so.1",RTLD_NOW); if(!h){printf("RESULT FAIL dlopen %s\n",dlerror());return 1;}
  SYM(cuInit) SYM(cuDeviceGet) SYM(cuDevicePrimaryCtxRetain) SYM(cuCtxSetCurrent) SYM(cuModuleLoadData) SYM(cuModuleGetFunction)
  SYM(cuMemAlloc_v2) SYM(cuLaunchKernel) SYM(cuCtxSynchronize) SYM(cuMemcpyDtoH_v2) SYM(cuGetErrorString) SYM(cuDevicePrimaryCtxRelease_v2)
#define CK(x) do{CUresult r_=(x); if(r_){const char*s=0; cuGetErrorString(r_,&s); printf("RESULT FAIL step=%s cu=%d %s t=%.2f\n",#x,r_,s?s:"?",now()-t0); fflush(stdout); exit(1);}}while(0)
  alarm(60); phase=1;
  CUdevice d; CUcontext c; CUmodule m; CUfunction f; CUdeviceptr buf; size_t nth=(size_t)blocks*256;
  CK(cuInit(0)); CK(cuDeviceGet(&d,0)); CK(cuDevicePrimaryCtxRetain(&c,d)); CK(cuCtxSetCurrent(c));
  printf("context in %.2f s\n",now()-t0); fflush(stdout);
  phase=2; CK(cuModuleLoadData(&m,ptx)); CK(cuModuleGetFunction(&f,m,"busy")); CK(cuMemAlloc_v2(&buf,nth*4));
  float *host=malloc(nth*4); size_t idx[4]={0,777,nth/2+3,nth-1}; float exp[4];
  for(int j=0;j<4;j++){float v=1.0f, x=(float)idx[j]; for(unsigned k=0;k<iters;k++) v=fmaf(v,0x1.ffffep-1f,x); exp[j]=v;}
  phase=3; long bad=0; double ts=now(), maxl=0;
  while(now()-ts<secs){
    void *args[2]={&buf,&iters}; double a=now(); alarm(30);
    CK(cuLaunchKernel(f,blocks,1,1,256,1,1,0,0,args,0)); CK(cuCtxSynchronize());
    CK(cuMemcpyDtoH_v2(host,buf,nth*4)); alarm(0);
    double l=now()-a; if(l>maxl) maxl=l;
    for(int j=0;j<4;j++) if(host[idx[j]]!=exp[j]) bad++;
    launches++;
  }
  double el=now()-ts;
  printf("RESULT %s launches=%ld per_s=%.1f gflops=%.1f max_ms=%.1f bad=%ld secs=%.1f total=%.1f\n",bad?"FAIL":"PASS",launches,launches/el,2.0*iters*nth*launches/el/1e9,maxl*1e3,bad,el,now()-t0);
  cuDevicePrimaryCtxRelease_v2(d); return bad?1:0;
}
