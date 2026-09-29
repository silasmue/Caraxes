// SPDX-License-Identifier: GPL-2.0-only
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
/*
 * Check that the guest kernel's DMA API honours the GPU's dma_range_map:
 * dma_map_page() and dma_alloc_coherent() on the GPU's PCI device must return
 * guest physical address + the offset of the range that holds it, and
 * dma_to_phys() must invert it. The device itself is never touched: with no
 * vIOMMU a mapping only computes an address (dma-direct).
 *
 * Load only while NO driver is bound to the GPU (refused otherwise); the test
 * holds the device lock throughout so no driver can bind meanwhile. The
 * result is logged per range; the module loads (0) on PASS and fails with
 * -EIO on FAIL. Unload it afterwards.
 */
#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/device.h>
#include <linux/dma-direct.h>
#include <linux/dma-mapping.h>
#include <linux/gfp.h>
#include <linux/io.h>
#include <linux/mm.h>
#include <linux/module.h>
#include <linux/moduleparam.h>
#include <linux/pci.h>
#include <linux/sizes.h>
#include <linux/slab.h>

#include "caraxes_dma_range_pci.h"

#define CDT_MAX 64

static char *bdf;
module_param(bdf, charp, 0444);
MODULE_PARM_DESC(bdf, "PCI device [DDDD:]BB:DD.F (default: first NVIDIA VGA, "
		      "then 3D controller)");

static unsigned int pages = 256;
module_param(pages, uint, 0444);
MODULE_PARM_DESC(pages, "pages to allocate and map (half GFP_DMA32)");

struct cdt_stat {
	unsigned int mapped, match, reverse, errors;
	u64 ex_phys, ex_dma;
};

static struct cdt_stat cdt_stats[CDT_MAX + 1]; /* [n] = not covered */
static unsigned int cdt_n, cdt_fail;

/* Independent of dma-direct.h's translate_phys_to_dma(). */
static int cdt_index(const struct bus_dma_region *map, u64 phys)
{
	int i;

	for (i = 0; map && map[i].size; i++)
		if (phys >= map[i].cpu_start &&
		    phys - map[i].cpu_start < map[i].size)
			return i;
	return -1;
}

static u64 cdt_expect(const struct bus_dma_region *map, u64 phys, int i)
{
	if (!map)
		return phys;
	if (i < 0)
		return DMA_MAPPING_ERROR;
	return map[i].dma_start + (phys - map[i].cpu_start);
}

static void cdt_account(struct device *dev, const struct bus_dma_region *map,
			u64 phys, dma_addr_t dma, bool err)
{
	int i = cdt_index(map, phys);
	struct cdt_stat *st = &cdt_stats[i < 0 ? cdt_n : i];
	u64 want = cdt_expect(map, phys, i);

	if (err) {
		st->errors++;
		cdt_fail++;
		dev_err(dev, "phys %#llx: mapping failed\n", phys);
		return;
	}
	st->mapped++;
	if (dma == want) {
		st->match++;
	} else {
		cdt_fail++;
		if (cdt_fail <= 8)
			dev_err(dev, "phys %#llx: dma %#llx, want %#llx\n",
				phys, (u64)dma, want);
	}
	if (dma_to_phys(dev, dma) == phys)
		st->reverse++;
	else
		cdt_fail++;
	if (!st->ex_dma && !st->ex_phys) {
		st->ex_phys = phys;
		st->ex_dma = dma;
	}
}

/*
 * Map a page the test does not own: DMA_TO_DEVICE only, so even a swiotlb
 * bounce would only read it, never write it back.
 */
static void cdt_probe_pfn(struct device *dev, const struct bus_dma_region *map,
			  u64 phys, unsigned int off, size_t len)
{
	unsigned long pfn = PHYS_PFN(phys);
	dma_addr_t dma;

	if (!pfn_valid(pfn) || !page_is_ram(pfn))
		return;
	dma = dma_map_page(dev, pfn_to_page(pfn), off, len, DMA_TO_DEVICE);
	if (dma_mapping_error(dev, dma)) {
		cdt_account(dev, map, phys + off, dma, true);
		return;
	}
	cdt_account(dev, map, phys + off, dma, false);
	dma_unmap_page(dev, dma, len, DMA_TO_DEVICE);
}

static void cdt_run(struct device *dev, const struct bus_dma_region *map)
{
	struct page **pg;
	unsigned int i;
	dma_addr_t dma;
	void *cpu;

	/* Pages of our own, bidirectional. */
	pg = kcalloc(pages, sizeof(*pg), GFP_KERNEL);
	for (i = 0; pg && i < pages; i++)
		pg[i] = alloc_page((i & 1) ? GFP_KERNEL : GFP_DMA32);
	for (i = 0; pg && i < pages; i++) {
		if (!pg[i])
			continue;
		dma = dma_map_page(dev, pg[i], 0, PAGE_SIZE, DMA_BIDIRECTIONAL);
		if (dma_mapping_error(dev, dma)) {
			cdt_account(dev, map, page_to_phys(pg[i]), dma, true);
			continue;
		}
		cdt_account(dev, map, page_to_phys(pg[i]), dma, false);
		dma_unmap_page(dev, dma, PAGE_SIZE, DMA_BIDIRECTIONAL);
	}
	for (i = 0; pg && i < pages; i++)
		if (pg[i])
			__free_page(pg[i]);
	kfree(pg);

	/* Coherent allocations, as nvidia and UVM make them. */
	for (i = 0; i < 8; i++) {
		cpu = dma_alloc_coherent(dev, SZ_64K, &dma, GFP_KERNEL);
		if (!cpu)
			continue;
		cdt_account(dev, map, virt_to_phys(cpu), dma, false);
		dma_free_coherent(dev, SZ_64K, cpu, dma);
	}

	/* First, middle and last page of every range, and sub-page offsets. */
	for (i = 0; map && map[i].size; i++) {
		u64 s = map[i].cpu_start, z = map[i].size;

		cdt_probe_pfn(dev, map, s, 0, PAGE_SIZE);
		cdt_probe_pfn(dev, map, s + z / 2, 0x123, 16);
		cdt_probe_pfn(dev, map, s + z - PAGE_SIZE, PAGE_SIZE - 1, 1);
	}
}

static int __init cdt_init(void)
{
	const struct bus_dma_region *map;
	u64 mask, cmask, off;
	struct pci_dev *pdev;
	struct device *dev;
	unsigned int i;
	int ret = 0;

	pdev = cdr_find_pci_dev(bdf);
	if (IS_ERR(pdev)) {
		pr_err("no device (%ld)\n", PTR_ERR(pdev));
		return PTR_ERR(pdev);
	}
	dev = &pdev->dev;

	device_lock(dev);
	if (dev->driver) {
		dev_err(dev,
			"bound to %s; run this only with no driver bound\n",
			dev->driver->name);
		ret = -EBUSY;
		goto out;
	}
	map = dev->dma_range_map;
	for (cdt_n = 0; map && map[cdt_n].size && cdt_n < CDT_MAX; cdt_n++)
		;
	if (!map)
		dev_warn(dev, "no dma_range_map: expecting dma == phys\n");

	/* The mask nvidia sets for this GPU (47 bits), restored after. */
	mask = *dev->dma_mask;
	cmask = dev->coherent_dma_mask;
	ret = dma_set_mask_and_coherent(dev, DMA_BIT_MASK(47));
	if (ret) {
		dev_err(dev, "cannot set a 47-bit DMA mask (%d)\n", ret);
		goto out;
	}
	cdt_run(dev, map);
	*dev->dma_mask = mask;
	dev->coherent_dma_mask = cmask;

	for (i = 0; i < cdt_n; i++) {
		const struct bus_dma_region *r = &map[i];
		struct cdt_stat *st = &cdt_stats[i];

		off = r->dma_start - r->cpu_start;
		dev_info(dev,
			 "range %u cpu %#llx dma %#llx size %#llx offset "
			 "%s%#llx: %u mapped, %u dma == phys + offset, %u "
			 "dma_to_phys ok, %u errors; e.g. phys %#llx -> dma "
			 "%#llx\n",
			 i, (u64)r->cpu_start, (u64)r->dma_start, (u64)r->size,
			 r->dma_start >= r->cpu_start ? "+" : "-",
			 r->dma_start >= r->cpu_start ? off : -off, st->mapped,
			 st->match, st->reverse, st->errors, st->ex_phys,
			 st->ex_dma);
	}
	if (cdt_stats[cdt_n].mapped || cdt_stats[cdt_n].errors)
		dev_info(dev, "%s: %u mapped, %u as expected, %u errors\n",
			 map ? "not covered" : "no map",
			 cdt_stats[cdt_n].mapped, cdt_stats[cdt_n].match,
			 cdt_stats[cdt_n].errors);
	if (map) {
		/* One past the highest range: no translation exists. */
		dma_addr_t top = 0;

		for (i = 0; i < cdt_n; i++)
			top = max_t(dma_addr_t, top,
				    map[i].dma_start + map[i].size);
		dev_info(dev, "dma_to_phys(%#llx, above every range) = %#llx\n",
			 (u64)top, (u64)dma_to_phys(dev, top));
		if (dma_to_phys(dev, top) != (phys_addr_t)-1)
			cdt_fail++;
	}
	dev_info(dev, "%s (%u failures)\n", cdt_fail ? "FAIL" : "PASS",
		 cdt_fail);
	ret = cdt_fail ? -EIO : 0;
out:
	device_unlock(dev);
	pci_dev_put(pdev);
	return ret;
}

static void __exit cdt_exit(void)
{
}

module_init(cdt_init);
module_exit(cdt_exit);

MODULE_LICENSE("GPL");
MODULE_DESCRIPTION("Caraxes: check the DMA API against the GPU's "
		   "dma_range_map");
