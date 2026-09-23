// SPDX-License-Identifier: MIT
/*
 * dmacheck: look at the DMA setup a device gets from the driver core, from inside a
 * probe. Written 2026-09-23 for the ACPI _DMA tests (patches/ACPI-DMA.md §6).
 *
 * caraxes_dma_range_test.ko runs only with no driver bound, but a dma_range_map from
 * firmware (_DMA through acpi_arch_dma_setup) exists only between dma_configure in
 * really_probe() and device_unbind_cleanup(). This driver binds on request
 * (driver_override), logs what dma_configure left on the device (dma_range_map,
 * bus_dma_limit, masks), maps pages with the DMA API the way caraxes_dma_range_test
 * does and checks each result against guest physical address + the offset of the
 * range holding it, then declines the device (-ENODEV) so the driver core frees the
 * map again. The device is never touched: without a vIOMMU a mapping only computes an
 * address (dma-direct).
 *
 *   echo dmacheck > /sys/bus/pci/devices/$BDF/driver_override
 *   echo $BDF > /sys/bus/pci/drivers_probe; dmesg | grep dmacheck
 *   echo > /sys/bus/pci/devices/$BDF/driver_override
 */
#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/dma-direct.h>
#include <linux/dma-mapping.h>
#include <linux/gfp.h>
#include <linux/mm.h>
#include <linux/module.h>
#include <linux/pci.h>
#include <linux/sizes.h>
#include <linux/slab.h>

static unsigned int pages = 256;
module_param(pages, uint, 0444);
MODULE_PARM_DESC(pages, "pages to allocate and map (half GFP_DMA32)");

static unsigned int mask_bits = 47;
module_param(mask_bits, uint, 0444);
MODULE_PARM_DESC(mask_bits, "DMA mask to test with (default 47, as nvidia sets; 0 keeps "
		 "the mask dma_configure left)");

struct dc_run {
	struct device *dev;
	const struct bus_dma_region *map;
	unsigned int mapped, match, reverse, errors, fail;
	u64 lo_phys, hi_phys;	/* lowest and highest phys seen, with their dma */
	u64 lo_dma, hi_dma;
};

static u64 dc_expect(const struct bus_dma_region *map, u64 phys)
{
	int i;

	if (!map)
		return phys;
	for (i = 0; map[i].size; i++)
		if (phys >= map[i].cpu_start && phys - map[i].cpu_start < map[i].size)
			return map[i].dma_start + (phys - map[i].cpu_start);
	return DMA_MAPPING_ERROR;
}

static void dc_account(struct dc_run *r, u64 phys, dma_addr_t dma, bool err,
		       const char *what)
{
	u64 want = dc_expect(r->map, phys);

	if (err) {
		r->errors++;
		if (++r->fail <= 8)
			dev_err(r->dev, "%s phys %#llx: mapping failed\n", what, phys);
		return;
	}
	r->mapped++;
	if (dma == want)
		r->match++;
	else if (++r->fail <= 8)
		dev_err(r->dev, "%s phys %#llx: dma %#llx, want %#llx\n", what, phys,
			(u64)dma, want);
	if (dma_to_phys(r->dev, dma) == phys)
		r->reverse++;
	else
		r->fail++;
	if (!r->lo_phys && !r->lo_dma) {
		r->lo_phys = r->hi_phys = phys;
		r->lo_dma = r->hi_dma = dma;
	}
	if (phys < r->lo_phys) {
		r->lo_phys = phys;
		r->lo_dma = dma;
	}
	if (phys > r->hi_phys) {
		r->hi_phys = phys;
		r->hi_dma = dma;
	}
}

static void dc_pfn(struct dc_run *r, u64 phys, unsigned int off, size_t len)
{
	unsigned long pfn = PHYS_PFN(phys);
	dma_addr_t dma;

	if (!pfn_valid(pfn) || !page_is_ram(pfn))
		return;
	dma = dma_map_page(r->dev, pfn_to_page(pfn), off, len, DMA_TO_DEVICE);
	if (dma_mapping_error(r->dev, dma)) {
		dc_account(r, phys + off, dma, true, "edge");
		return;
	}
	dc_account(r, phys + off, dma, false, "edge");
	dma_unmap_page(r->dev, dma, len, DMA_TO_DEVICE);
}

static int dc_probe(struct pci_dev *pdev, const struct pci_device_id *id)
{
	struct device *dev = &pdev->dev;
	struct dc_run r = { .dev = dev, .map = dev->dma_range_map };
	unsigned int i, n = 0, coh_ok = 0, coh_dma32 = 0;
	u64 mask = *dev->dma_mask, cmask = dev->coherent_dma_mask;
	struct page **pg;
	dma_addr_t dma;
	void *cpu;

	for (i = 0; r.map && r.map[i].size; i++)
		n++;
	dev_info(dev, "at probe: dma_range_map %s (%u range(s)), bus_dma_limit %#llx, "
		 "dma_mask %#llx, coherent_dma_mask %#llx\n",
		 r.map ? "present" : "absent", n, (u64)dev->bus_dma_limit, mask, cmask);
	for (i = 0; i < n; i++)
		dev_info(dev, "range %u: cpu %#llx dma %#llx size %#llx\n", i,
			 (u64)r.map[i].cpu_start, (u64)r.map[i].dma_start,
			 (u64)r.map[i].size);

	/* By default the mask nvidia sets for this GPU (47 bits). */
	if (mask_bits && dma_set_mask_and_coherent(dev, DMA_BIT_MASK(mask_bits))) {
		dev_err(dev, "cannot set a %u-bit DMA mask\n", mask_bits);
		return -ENODEV;
	}
	dev_info(dev, "testing with dma_mask %#llx, coherent %#llx, bus_dma_limit %#llx "
		 "(mask_bits=%u)\n", *dev->dma_mask, dev->coherent_dma_mask,
		 (u64)dev->bus_dma_limit, mask_bits);

	pg = kcalloc(pages, sizeof(*pg), GFP_KERNEL);
	for (i = 0; pg && i < pages; i++)
		pg[i] = alloc_page((i & 1) ? GFP_KERNEL : GFP_DMA32);
	for (i = 0; pg && i < pages; i++) {
		if (!pg[i])
			continue;
		dma = dma_map_page(dev, pg[i], 0, PAGE_SIZE, DMA_BIDIRECTIONAL);
		dc_account(&r, page_to_phys(pg[i]), dma, dma_mapping_error(dev, dma),
			   "page");
		if (!dma_mapping_error(dev, dma))
			dma_unmap_page(dev, dma, PAGE_SIZE, DMA_BIDIRECTIONAL);
	}
	for (i = 0; pg && i < pages; i++)
		if (pg[i])
			__free_page(pg[i]);
	kfree(pg);

	/* Coherent allocations, as nvidia and UVM make them; note where they land. */
	for (i = 0; i < 8; i++) {
		cpu = dma_alloc_coherent(dev, SZ_64K, &dma, GFP_KERNEL);
		if (!cpu) {
			r.errors++;
			if (++r.fail <= 8)
				dev_err(dev, "dma_alloc_coherent(64K) %u failed\n", i);
			continue;
		}
		coh_ok++;
		if (virt_to_phys(cpu) < SZ_4G)
			coh_dma32++;
		dc_account(&r, virt_to_phys(cpu), dma, false, "coherent");
		dma_free_coherent(dev, SZ_64K, cpu, dma);
	}

	/* First, middle and last page of every range, with sub-page offsets. */
	for (i = 0; i < n; i++) {
		u64 s = r.map[i].cpu_start, z = r.map[i].size;

		dc_pfn(&r, s, 0, PAGE_SIZE);
		dc_pfn(&r, s + z / 2, 0x123, 16);
		dc_pfn(&r, s + z - PAGE_SIZE, PAGE_SIZE - 1, 1);
	}

	if (n) {
		dma_addr_t top = 0;

		for (i = 0; i < n; i++)
			top = max_t(dma_addr_t, top, r.map[i].dma_start + r.map[i].size);
		dev_info(dev, "dma_to_phys(%#llx, above every range) = %#llx\n", (u64)top,
			 (u64)dma_to_phys(dev, top));
		if (dma_to_phys(dev, top) != (phys_addr_t)-1)
			r.fail++;
	}
	dev_info(dev, "%u mapped, %u dma == expected, %u dma_to_phys ok, %u errors; "
		 "coherent %u/8 ok, %u below 4 GiB; lowest phys %#llx -> dma %#llx, "
		 "highest phys %#llx -> dma %#llx\n",
		 r.mapped, r.match, r.reverse, r.errors, coh_ok, coh_dma32, r.lo_phys,
		 r.lo_dma, r.hi_phys, r.hi_dma);
	dev_info(dev, "%s (%u failures) against %s\n", r.fail ? "FAIL" : "PASS", r.fail,
		 n ? "the installed dma_range_map" : "identity (no map)");

	*dev->dma_mask = mask;
	dev->coherent_dma_mask = cmask;
	return -ENODEV;	/* decline: the driver core frees the map */
}

static struct pci_driver dc_driver = {
	.name = "dmacheck",
	.probe = dc_probe,	/* no id_table: binds only through driver_override */
};
module_pci_driver(dc_driver);

MODULE_LICENSE("Dual MIT/GPL");
MODULE_DESCRIPTION("Caraxes: DMA API results inside a probe, against dev->dma_range_map");
