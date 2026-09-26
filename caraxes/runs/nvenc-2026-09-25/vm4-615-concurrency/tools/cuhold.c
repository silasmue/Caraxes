// cuhold: hold a CUDA driver-API state for <secs> seconds, no work. Modes:
//   init   cuInit only;  ctx  cuCtxCreate (held, no allocation);  alloc  ctx plus one 1 MiB cuMemAlloc
//   gcc -O2 -o cuhold cuhold.c -ldl      ./cuhold <mode> <secs>
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
typedef int CUresult; typedef void *CUcontext; typedef unsigned long long CUdeviceptr;
int main(int argc,char**argv){
  const char*mode=argc>1?argv[1]:"init"; int secs=argc>2?atoi(argv[2]):60;
  struct timespec w; clock_gettime(CLOCK_REALTIME,&w);
  printf("START wall=%ld.%03ld pid=%d mode=%s\n",(long)w.tv_sec,w.tv_nsec/1000000,getpid(),mode); fflush(stdout);
  void*h=dlopen("libcuda.so.1",RTLD_NOW); if(!h){printf("RESULT FAIL dlopen\n");return 1;}
  CUresult (*init)(unsigned)=dlsym(h,"cuInit"); CUresult (*dget)(int*,int)=dlsym(h,"cuDeviceGet");
  CUresult (*cc)(CUcontext*,unsigned,int)=dlsym(h,"cuCtxCreate_v2"); CUresult (*ma)(CUdeviceptr*,size_t)=dlsym(h,"cuMemAlloc_v2");
  CUresult r=init(0); printf("cuInit=%d\n",r); if(r){printf("RESULT FAIL cuInit %d\n",r);return 1;}
  if(strcmp(mode,"init")){ int d; CUcontext c; if(!cc){printf("RESULT FAIL no cuCtxCreate_v2\n");return 1;}
    r=dget(&d,0); r=r?r:cc(&c,0,d); printf("cuCtxCreate_v2=%d\n",r); if(r){printf("RESULT FAIL ctx %d\n",r);return 1;}
    if(!strcmp(mode,"alloc")){CUdeviceptr p; r=ma(&p,1<<20); printf("cuMemAlloc=%d\n",r); if(r){printf("RESULT FAIL alloc %d\n",r);return 1;}}}
  printf("HOLDING\n"); fflush(stdout); sleep(secs); printf("RESULT PASS held=%d\n",secs); return 0;
}
