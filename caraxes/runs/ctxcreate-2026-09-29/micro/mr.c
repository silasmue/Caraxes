// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// Host micro-benchmark (docs/research/ctxcreate-2026-09-29.md): a gather window of 512 scattered pages of a memfd,
// built with mremap (hostmem.rs `gather`) or mmap(MAP_FIXED), and its munmap. gcc -O2 -o mr mr.c
#define _GNU_SOURCE
#include <sys/mman.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <unistd.h>
static double now(){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec*1e6+t.tv_nsec/1e3;}
int main(){
  size_t M=256<<20; int fd=memfd_create("g",0); ftruncate(fd,M);
  char*base=mmap(0,M,PROT_READ|PROT_WRITE,MAP_SHARED,fd,0);
  for(size_t i=0;i<M;i+=4096) base[i]=1;
  int N=512; long pg[512]; srand(1);
  for(int k=0;k<5;k++){
    for(int i=0;i<N;i++) pg[i]=(rand()%(M/4096));
    double t0=now();
    char*w=mmap(0,N*4096,PROT_NONE,MAP_PRIVATE|MAP_ANONYMOUS|MAP_NORESERVE,-1,0);
    for(int i=0;i<N;i++){ void*r=mremap(base+pg[i]*4096,0,4096,MREMAP_MAYMOVE|MREMAP_FIXED,w+i*4096); if(r!=w+i*4096){perror("mremap");return 1;} }
    double t1=now();
    munmap(w,N*4096);
    double t2=now();
    // variant b: mmap fixed from fd
    w=mmap(0,N*4096,PROT_NONE,MAP_PRIVATE|MAP_ANONYMOUS|MAP_NORESERVE,-1,0);
    double t3=now();
    for(int i=0;i<N;i++){ void*r=mmap(w+i*4096,4096,PROT_READ|PROT_WRITE,MAP_SHARED|MAP_FIXED,fd,pg[i]*4096); if(r!=w+i*4096){perror("mmap");return 1;} }
    double t4=now();
    munmap(w,N*4096);
    double t5=now();
    printf("mremap %.0f us munmap %.0f us | mmap-fixed %.0f us munmap %.0f us\n",t1-t0,t2-t1,t4-t3,t5-t4);
  }
  return 0;
}
