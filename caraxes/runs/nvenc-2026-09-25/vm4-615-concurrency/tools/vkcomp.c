// vkcomp: headless Vulkan compute. Instance, NVIDIA physical device, device with a compute
// queue, then for <secs> seconds: dispatch an LCG shader over N uints, submit, wait on a fence
// (10 s timeout = HANG), verify. Prints one RESULT line.
//   gcc -O2 -o vkcomp vkcomp.c -lvulkan      ./vkcomp [secs] [iters]
#include <vulkan/vulkan.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <time.h>
#include <unistd.h>
#include "comp_spv.h"
#define N (1u << 20)
static double now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+t.tv_nsec*1e-9;}
static double t0;
#define CK(x) do{VkResult r_=(x); if(r_!=VK_SUCCESS){printf("RESULT FAIL step=%s vkresult=%d t=%.2f\n",#x,r_,now()-t0);fflush(stdout);exit(1);}}while(0)
int main(int argc,char**argv){
  int secs=argc>1?atoi(argv[1]):60; uint32_t iters=argc>2?atoi(argv[2]):4096;
  t0=now(); struct timespec w; clock_gettime(CLOCK_REALTIME,&w);
  printf("START wall=%ld.%03ld pid=%d\n",(long)w.tv_sec,w.tv_nsec/1000000,getpid()); fflush(stdout);
  VkApplicationInfo ai={VK_STRUCTURE_TYPE_APPLICATION_INFO,0,"vkcomp",1,0,0,VK_API_VERSION_1_1};
  VkInstanceCreateInfo ici={VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,0,0,&ai};
  VkInstance inst; CK(vkCreateInstance(&ici,0,&inst));
  uint32_t n=8; VkPhysicalDevice pds[8]; CK(vkEnumeratePhysicalDevices(inst,&n,pds));
  VkPhysicalDevice pd=0; VkPhysicalDeviceProperties pp;
  for(uint32_t i=0;i<n;i++){vkGetPhysicalDeviceProperties(pds[i],&pp); if(pp.vendorID==0x10de){pd=pds[i];break;}}
  if(!pd){printf("RESULT FAIL step=no-nvidia-device n=%u\n",n);return 1;}
  printf("device %s\n",pp.deviceName);
  uint32_t qn=16; VkQueueFamilyProperties qf[16]; vkGetPhysicalDeviceQueueFamilyProperties(pd,&qn,qf);
  uint32_t fam=UINT32_MAX; for(uint32_t i=0;i<qn;i++) if((qf[i].queueFlags&VK_QUEUE_COMPUTE_BIT)&&!(qf[i].queueFlags&VK_QUEUE_GRAPHICS_BIT)){fam=i;break;}
  if(fam==UINT32_MAX) for(uint32_t i=0;i<qn;i++) if(qf[i].queueFlags&VK_QUEUE_COMPUTE_BIT){fam=i;break;}
  float prio=1; VkDeviceQueueCreateInfo qci={VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,0,0,fam,1,&prio};
  VkDeviceCreateInfo dci={VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,0,0,1,&qci};
  double td=now(); VkDevice dev; CK(vkCreateDevice(pd,&dci,0,&dev));
  printf("device created fam=%u in %.2f s\n",fam,now()-td); fflush(stdout);
  VkQueue q; vkGetDeviceQueue(dev,fam,0,&q);
  VkBufferCreateInfo bci={VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO,0,0,N*4,VK_BUFFER_USAGE_STORAGE_BUFFER_BIT,VK_SHARING_MODE_EXCLUSIVE};
  VkBuffer buf; CK(vkCreateBuffer(dev,&bci,0,&buf));
  VkMemoryRequirements mr; vkGetBufferMemoryRequirements(dev,buf,&mr);
  VkPhysicalDeviceMemoryProperties mp; vkGetPhysicalDeviceMemoryProperties(pd,&mp);
  uint32_t mt=UINT32_MAX; VkMemoryPropertyFlags want=VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT|VK_MEMORY_PROPERTY_HOST_COHERENT_BIT;
  for(uint32_t i=0;i<mp.memoryTypeCount;i++) if((mr.memoryTypeBits&(1u<<i))&&(mp.memoryTypes[i].propertyFlags&want)==want){mt=i;break;}
  VkMemoryAllocateInfo mai={VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,0,mr.size,mt};
  VkDeviceMemory mem; CK(vkAllocateMemory(dev,&mai,0,&mem)); CK(vkBindBufferMemory(dev,buf,mem,0));
  uint32_t *p; CK(vkMapMemory(dev,mem,0,N*4,0,(void**)&p));
  VkShaderModuleCreateInfo smi={VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,0,0,sizeof comp_spv,comp_spv};
  VkShaderModule sm; CK(vkCreateShaderModule(dev,&smi,0,&sm));
  VkDescriptorSetLayoutBinding b={0,VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,1,VK_SHADER_STAGE_COMPUTE_BIT};
  VkDescriptorSetLayoutCreateInfo dli={VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO,0,0,1,&b};
  VkDescriptorSetLayout dl; CK(vkCreateDescriptorSetLayout(dev,&dli,0,&dl));
  VkPushConstantRange pcr={VK_SHADER_STAGE_COMPUTE_BIT,0,4};
  VkPipelineLayoutCreateInfo pli={VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO,0,0,1,&dl,1,&pcr};
  VkPipelineLayout pl; CK(vkCreatePipelineLayout(dev,&pli,0,&pl));
  VkComputePipelineCreateInfo cpi={VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO,0,0,{VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,0,0,VK_SHADER_STAGE_COMPUTE_BIT,sm,"main"},pl};
  VkPipeline pipe; CK(vkCreateComputePipelines(dev,0,1,&cpi,0,&pipe));
  VkDescriptorPoolSize ps={VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,1};
  VkDescriptorPoolCreateInfo dpi={VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO,0,0,1,1,&ps};
  VkDescriptorPool dp; CK(vkCreateDescriptorPool(dev,&dpi,0,&dp));
  VkDescriptorSetAllocateInfo dai={VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO,0,dp,1,&dl};
  VkDescriptorSet ds; CK(vkAllocateDescriptorSets(dev,&dai,&ds));
  VkDescriptorBufferInfo dbi={buf,0,N*4};
  VkWriteDescriptorSet wds={VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET,0,ds,0,0,1,VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,0,&dbi};
  vkUpdateDescriptorSets(dev,1,&wds,0,0);
  VkCommandPoolCreateInfo cpci={VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,0,VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT,fam};
  VkCommandPool cp; CK(vkCreateCommandPool(dev,&cpci,0,&cp));
  VkCommandBufferAllocateInfo cbai={VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,0,cp,VK_COMMAND_BUFFER_LEVEL_PRIMARY,1};
  VkCommandBuffer cb; CK(vkAllocateCommandBuffers(dev,&cbai,&cb));
  VkCommandBufferBeginInfo cbi={VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO};
  CK(vkBeginCommandBuffer(cb,&cbi)); vkCmdBindPipeline(cb,VK_PIPELINE_BIND_POINT_COMPUTE,pipe);
  vkCmdBindDescriptorSets(cb,VK_PIPELINE_BIND_POINT_COMPUTE,pl,0,1,&ds,0,0);
  vkCmdPushConstants(cb,pl,VK_SHADER_STAGE_COMPUTE_BIT,0,4,&iters); vkCmdDispatch(cb,N/64,1,1); CK(vkEndCommandBuffer(cb));
  VkFenceCreateInfo fci={VK_STRUCTURE_TYPE_FENCE_CREATE_INFO}; VkFence f; CK(vkCreateFence(dev,&fci,0,&f));
  VkSubmitInfo si={VK_STRUCTURE_TYPE_SUBMIT_INFO,0,0,0,0,1,&cb};
  // expected values for a few indices
  uint32_t idx[4]={0,12345,N/2+7,N-1}, exp[4];
  for(int j=0;j<4;j++){uint32_t v=idx[j]; for(uint32_t k=0;k<iters;k++) v=v*1664525u+1013904223u; exp[j]=v;}
  long disp=0, bad=0; double ts=now(), maxw=0;
  while(now()-ts<secs){
    for(int j=0;j<4;j++) p[idx[j]]=0xdeadbeef;
    double a=now(); CK(vkQueueSubmit(q,1,&si,f));
    VkResult r=vkWaitForFences(dev,1,&f,1,10000000000ull);
    if(r!=VK_SUCCESS){printf("RESULT HANG dispatch=%ld wait=%d t=%.2f\n",disp,r,now()-t0);fflush(stdout);_exit(3);}
    double wt=now()-a; if(wt>maxw) maxw=wt;
    CK(vkResetFences(dev,1,&f));
    for(int j=0;j<4;j++) if(p[idx[j]]!=exp[j]) bad++;
    disp++;
  }
  double el=now()-ts;
  printf("RESULT %s dispatches=%ld per_s=%.1f max_wait_ms=%.1f bad=%ld secs=%.1f total=%.1f\n",bad?"FAIL":"PASS",disp,disp/el,maxw*1e3,bad,el,now()-t0);
  vkDestroyDevice(dev,0); vkDestroyInstance(inst,0); return bad?1:0;
}
