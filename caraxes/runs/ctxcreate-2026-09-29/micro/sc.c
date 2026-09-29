// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// Host micro-benchmark (docs/research/ctxcreate-2026-09-29.md): the BAR2 memfd scan of 128 pages against
// their snapshots (bar2area.rs `RemapTable::scan`), cold and warm caches. gcc -O2 -o sc sc.c
#define _GNU_SOURCE
#include <sys/mman.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <time.h>
#include <unistd.h>
static double now(){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec*1e6+t.tv_nsec/1e3;}
int main(){
  size_t M=32<<20; int fd=memfd_create("b",0); ftruncate(fd,M);
  uint64_t*base=mmap(0,M,PROT_READ|PROT_WRITE,MAP_SHARED,fd,0);
  for(size_t i=0;i<M/8;i++) base[i]=i*7;
  int N=128; long pg[128]; uint64_t*snap[128]; srand(2);
  for(int i=0;i<N;i++){pg[i]=rand()%(M/4096); snap[i]=aligned_alloc(4096,4096); memcpy(snap[i],base+pg[i]*512,4096);}
  char*junk=malloc(64<<20);
  for(int k=0;k<6;k++){
    memset(junk,k,64<<20); // evict caches
    uint64_t buf[512]; int diff=0;
    double t0=now();
    for(int i=0;i<N;i++){ memcpy(buf,base+pg[i]*512,4096); if(memcmp(buf,snap[i],4096)) diff++; }
    double t1=now();
    memset(junk,k+1,64<<20);
    double t2=now();
    for(int i=0;i<N;i++){ if(memcmp(base+pg[i]*512,snap[i],4096)) diff++; }
    double t3=now();
    double t4=now();
    for(int i=0;i<N;i++){ memcpy(buf,base+pg[i]*512,4096); if(memcmp(buf,snap[i],4096)) diff++; }
    double t5=now();
    printf("cold copy+cmp %.0f us, cold in-place %.0f us, warm copy+cmp %.0f us (%d)\n",t1-t0,t3-t2,t5-t4,diff);
  }
}
