// SPDX-License-Identifier: GPL-2.0-only
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
/*
 * Install a DMA range map (dev->dma_range_map) on the NVIDIA GPU's PCI device
 * before the nvidia driver binds, so that every DMA address the guest kernel
 * computes for that device is guest physical address + a per-range offset.
 *
 * Temporary aid (decision D13 in docs/caraxes/README.md); why and how it is
 * removed: docs/caraxes/sysmem-iova.md sections 3(b), 4 and 5.
 *
 * Ownership of the map follows the driver core: device_unbind_cleanup() in
 * drivers/base/dd.c kfree()s dev->dma_range_map and clears it whenever a
 * driver unbinds or a probe fails, and device_release() kfree()s it when the
 * device goes away. So every installed map is a fresh kmalloc() copy of the
 * table, and a PCI bus notifier installs a new copy at BUS_NOTIFY_BIND_DRIVER
 * (after the bus's dma_configure, before the driver's probe) whenever the
 * device has none. The module keeps a reference on itself while a driver is
 * bound, so rmmod fails with "in use" until that driver is gone.
 */
#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/device.h>
#include <linux/dma-direct.h>
#include <linux/ioport.h>
#include <linux/kernel.h>
#include <linux/module.h>
#include <linux/moduleparam.h>
#include <linux/mutex.h>
#include <linux/notifier.h>
#include <linux/pci.h>
#include <linux/sizes.h>
#include <linux/slab.h>
#include <linux/string.h>
#include <linux/version.h>

#include "caraxes_dma_range_pci.h"

#define CDR_MAX_RANGES 64

static char *bdf;
module_param(bdf, charp, 0444);
MODULE_PARM_DESC(bdf, "PCI device [DDDD:]BB:DD.F (default: first NVIDIA VGA, "
		      "then 3D controller)");

static bool force;
module_param(force, bool, 0444);
MODULE_PARM_DESC(force, "replace a DMA range map the device already has "
			"(never overrides a bound driver)");

/*
 * Lock order: device_lock(&cdr_pdev->dev), then cdr_lock. The bus notifier
 * is called with the device lock held.
 */
static DEFINE_MUTEX(cdr_lock);
static struct pci_dev *cdr_pdev;
/* The parsed table, cdr_n entries plus a zero terminator. */
static struct bus_dma_region *cdr_table;
static unsigned int cdr_n;
/* The copy on the device, or NULL once the driver core has freed it. */
static const struct bus_dma_region *cdr_installed;
/* A map the device had before force=1 replaced it; restored on unload. */
static const struct bus_dma_region *cdr_previous;
static bool cdr_pinned;
static unsigned int cdr_installs;

/*
 * Not a charp: param_set_charp() refuses strings over 1024 bytes, and 64
 * entries of 64-bit IOVAs need about 2.5 KiB.
 */
static char *ranges;

static int cdr_ranges_set(const char *val, const struct kernel_param *kp)
{
	char *s = kstrdup(val ? val : "", GFP_KERNEL);

	if (!s)
		return -ENOMEM;
	kfree(ranges);
	ranges = s;
	return 0;
}

static int cdr_ranges_get(char *buf, const struct kernel_param *kp)
{
	return scnprintf(buf, PAGE_SIZE, "%s\n", ranges ? ranges : "");
}

/* Called by the module loader after the parameters left sysfs. */
static void cdr_ranges_free(void *arg)
{
	kfree(ranges);
	ranges = NULL;
}

static const struct kernel_param_ops cdr_ranges_ops = {
	.set = cdr_ranges_set,
	.get = cdr_ranges_get,
	.free = cdr_ranges_free,
};
module_param_cb(ranges, &cdr_ranges_ops, NULL, 0444);
MODULE_PARM_DESC(ranges, "cpu_start:dma_start:size[,...] in hex, at most 64 "
			 "entries, covering all System RAM");

static u64 cdr_end(u64 start, u64 size)
{
	return start + size - 1;
}

static const struct bus_dma_region *cdr_find(u64 cpu)
{
	unsigned int i;

	for (i = 0; i < cdr_n; i++)
		if (cpu >= cdr_table[i].cpu_start &&
		    cpu - cdr_table[i].cpu_start < cdr_table[i].size)
			return &cdr_table[i];
	return NULL;
}

static int cdr_parse_one(char *tok, unsigned int n)
{
	u64 v[3];
	char *f;
	int i;

	for (i = 0; i < 3; i++) {
		f = strsep(&tok, ":");
		if (!f || kstrtou64(strim(f), 16, &v[i])) {
			pr_err("ranges entry %u: want cpu_start:dma_start:size "
			       "in hex\n",
			       n);
			return -EINVAL;
		}
	}
	if (tok) {
		pr_err("ranges entry %u: more than three fields\n", n);
		return -EINVAL;
	}
	cdr_table[n].cpu_start = v[0];
	cdr_table[n].dma_start = v[1];
	cdr_table[n].size = v[2];
#if LINUX_VERSION_CODE < KERNEL_VERSION(6, 8, 0)
	/* The field went away in v6.8; before it, lookups used it. */
	cdr_table[n].offset = v[0] - v[1];
#endif
	return 0;
}

static int cdr_parse(const char *s)
{
	char *buf, *cur, *tok;
	int ret = 0;

	if (!s || !*s) {
		pr_err("ranges= is required\n");
		return -EINVAL;
	}
	buf = kstrdup(s, GFP_KERNEL);
	cdr_table = kcalloc(CDR_MAX_RANGES + 1, sizeof(*cdr_table), GFP_KERNEL);
	if (!buf || !cdr_table) {
		ret = -ENOMEM;
		goto out;
	}
	cdr_n = 0;
	cur = strim(buf);
	while ((tok = strsep(&cur, ",")) != NULL) {
		if (cdr_n == CDR_MAX_RANGES) {
			pr_err("more than %d ranges\n", CDR_MAX_RANGES);
			ret = -E2BIG;
			goto out;
		}
		ret = cdr_parse_one(strim(tok), cdr_n);
		if (ret)
			goto out;
		cdr_n++;
	}
out:
	kfree(buf);
	if (ret) {
		kfree(cdr_table);
		cdr_table = NULL;
	}
	return ret;
}

static bool cdr_overlap(u64 a, u64 b, u64 size_a, u64 size_b)
{
	return a <= cdr_end(b, size_b) && b <= cdr_end(a, size_a);
}

static int cdr_validate(void)
{
	const struct bus_dma_region *r, *q;
	unsigned int i, j;

	for (i = 0; i < cdr_n; i++) {
		r = &cdr_table[i];
		if (!r->size || !IS_ALIGNED(r->size, SZ_2M)) {
			pr_err("range %u: size %#llx is not a non-zero multiple "
			       "of 2 MiB\n",
			       i, (u64)r->size);
			return -EINVAL;
		}
		if (!PAGE_ALIGNED(r->cpu_start) ||
		    !PAGE_ALIGNED(r->dma_start)) {
			pr_err("range %u: start %#llx or %#llx is not page "
			       "aligned\n",
			       i, (u64)r->cpu_start, (u64)r->dma_start);
			return -EINVAL;
		}
		if (cdr_end(r->cpu_start, r->size) < r->cpu_start ||
		    cdr_end(r->dma_start, r->size) < r->dma_start) {
			pr_err("range %u wraps around the address space\n", i);
			return -EINVAL;
		}
		if (!IS_ALIGNED(r->cpu_start ^ r->dma_start, SZ_2M))
			pr_warn("range %u: cpu and dma start differ modulo "
				"2 MiB; GPU big pages lose their alignment\n",
				i);
		for (j = 0; j < i; j++) {
			q = &cdr_table[j];
			if (cdr_overlap(r->cpu_start, q->cpu_start, r->size,
					q->size)) {
				pr_err("ranges %u and %u overlap in CPU "
				       "physical space\n",
				       j, i);
				return -EINVAL;
			}
			if (cdr_overlap(r->dma_start, q->dma_start, r->size,
					q->size)) {
				pr_err("ranges %u and %u overlap in DMA space\n",
				       j, i);
				return -EINVAL;
			}
		}
	}
	return 0;
}

/*
 * Every System RAM page must translate: an uncovered page gives
 * DMA_MAPPING_ERROR on map, and v6.8's check_ram_in_range_map() (used by
 * dma_addressing_limited()) loops forever on a RAM range that is covered at
 * its start but not to its end. walk_system_ram_range() is not exported, so
 * the same walk is done with walk_iomem_res_desc().
 */
static int cdr_cover_one(struct resource *res, void *arg)
{
	u64 start = round_up(res->start, PAGE_SIZE);
	u64 end = round_down(res->end + 1, PAGE_SIZE);
	const struct bus_dma_region *r;

	while (start < end) {
		r = cdr_find(start);
		if (!r) {
			pr_err("System RAM [%#llx-%#llx]: %#llx is not covered "
			       "by any range\n",
			       (u64)res->start, (u64)res->end, start);
			return 1;
		}
		start = r->cpu_start + r->size;
	}
	return 0;
}

static int cdr_check_coverage(void)
{
	int ret;

	ret = walk_iomem_res_desc(IORES_DESC_NONE,
				  IORESOURCE_SYSTEM_RAM | IORESOURCE_BUSY, 0,
				  U64_MAX, NULL, cdr_cover_one);
	return ret > 0 ? -EINVAL : 0;
}

/* Called with the device lock and cdr_lock held. */
static int cdr_install(struct device *dev)
{
	struct bus_dma_region *map;

	map = kmemdup(cdr_table, (cdr_n + 1) * sizeof(*map), GFP_KERNEL);
	if (!map)
		return -ENOMEM;
	dev->dma_range_map = map;
	cdr_installed = map;
	cdr_installs++;
	return 0;
}

static void cdr_log_table(struct device *dev)
{
	const struct bus_dma_region *r;
	unsigned int i;
	u64 off;

	for (i = 0; i < cdr_n; i++) {
		r = &cdr_table[i];
		off = r->dma_start - r->cpu_start;
		dev_info(dev,
			 "range %u: cpu [%#llx-%#llx] -> dma [%#llx-%#llx] "
			 "size %#llx offset %s%#llx\n",
			 i, (u64)r->cpu_start, cdr_end(r->cpu_start, r->size),
			 (u64)r->dma_start, cdr_end(r->dma_start, r->size),
			 (u64)r->size, r->dma_start >= r->cpu_start ? "+" : "-",
			 r->dma_start >= r->cpu_start ? off : -off);
	}
}

static const char *cdr_drv_name(struct device *dev)
{
	return dev->driver ? dev->driver->name : "none";
}

static int cdr_notify(struct notifier_block *nb, unsigned long action,
		      void *data)
{
	struct device *dev = data;

	if (!cdr_pdev || dev != &cdr_pdev->dev)
		return NOTIFY_DONE;

	mutex_lock(&cdr_lock);
	switch (action) {
	case BUS_NOTIFY_BIND_DRIVER:
		if (!dev->dma_range_map) {
			if (cdr_install(dev))
				dev_err(dev,
					"cannot re-install the DMA range "
					"map; %s will see guest physical "
					"addresses\n",
					cdr_drv_name(dev));
			else
				dev_info(dev,
					 "re-installed %u ranges before %s "
					 "probes\n",
					 cdr_n, cdr_drv_name(dev));
		} else if (dev->dma_range_map != cdr_installed) {
			dev_warn(dev,
				 "%s binds with a DMA range map this "
				 "module did not install; left alone\n",
				 cdr_drv_name(dev));
		}
		break;
	case BUS_NOTIFY_BOUND_DRIVER:
		if (!cdr_pinned) {
			__module_get(THIS_MODULE);
			cdr_pinned = true;
		}
		dev_info(dev,
			 "%s bound, DMA range map %s; unload blocked "
			 "until it unbinds\n",
			 cdr_drv_name(dev),
			 dev->dma_range_map == cdr_installed ? "in place" :
							       "NOT ours");
		break;
	case BUS_NOTIFY_DRIVER_NOT_BOUND:
	case BUS_NOTIFY_UNBOUND_DRIVER:
		/* device_unbind_cleanup() frees the map (after NOT_BOUND,
		 * before UNBOUND); forget it either way. */
		cdr_installed = NULL;
		if (cdr_pinned) {
			cdr_pinned = false;
			module_put(THIS_MODULE);
		}
		dev_info(dev,
			 "driver %s; the driver core freed the map, the "
			 "next bind gets a new copy\n",
			 action == BUS_NOTIFY_UNBOUND_DRIVER ? "unbound" :
							       "probe failed");
		break;
	case BUS_NOTIFY_DEL_DEVICE:
		dev_warn(dev, "device is being removed\n");
		break;
	}
	mutex_unlock(&cdr_lock);
	return NOTIFY_OK;
}

static struct notifier_block cdr_nb = {
	.notifier_call = cdr_notify,
};

/* Readback through /sys/module/caraxes_dma_range/parameters/. */

static int cdr_ro_set(const char *val, const struct kernel_param *kp)
{
	return -EPERM;
}

/* One "0xcpu:0xdma:0xsize" line per range: joined with ',' it is a valid
 * ranges= value again. */
static int cdr_print(char *buf, int len, const struct bus_dma_region *m)
{
	for (; m && m->size; m++)
		len += scnprintf(buf + len, PAGE_SIZE - len,
				 "0x%llx:0x%llx:0x%llx\n", (u64)m->cpu_start,
				 (u64)m->dma_start, (u64)m->size);
	return len;
}

static int cdr_table_get(char *buf, const struct kernel_param *kp)
{
	int len;

	mutex_lock(&cdr_lock);
	len = cdr_print(buf, 0, cdr_pdev ? cdr_table : NULL);
	mutex_unlock(&cdr_lock);
	return len;
}

static struct pci_dev *cdr_get_pdev(void)
{
	struct pci_dev *pdev;

	mutex_lock(&cdr_lock);
	pdev = cdr_pdev ? pci_dev_get(cdr_pdev) : NULL;
	mutex_unlock(&cdr_lock);
	return pdev;
}

/* The map the device carries right now; the device lock keeps the driver
 * core from freeing it while it is printed. */
static int cdr_live_get(char *buf, const struct kernel_param *kp)
{
	struct pci_dev *pdev = cdr_get_pdev();
	int len;

	if (!pdev)
		return 0;
	device_lock(&pdev->dev);
	len = cdr_print(buf, 0, pdev->dev.dma_range_map);
	device_unlock(&pdev->dev);
	pci_dev_put(pdev);
	return len;
}

static int cdr_status_get(char *buf, const struct kernel_param *kp)
{
	struct pci_dev *pdev = cdr_get_pdev();
	struct device *dev;
	const char *map;
	int len;

	if (!pdev)
		return scnprintf(buf, PAGE_SIZE, "device=none\n");
	dev = &pdev->dev;
	device_lock(dev);
	mutex_lock(&cdr_lock);
	if (!dev->dma_range_map)
		map = "none";
	else if (dev->dma_range_map == cdr_installed)
		map = "ours";
	else
		map = "foreign";
	len = scnprintf(buf, PAGE_SIZE,
			"device=%s driver=%s map=%s ranges=%u installs=%u "
			"pinned=%d previous=%s\n",
			pci_name(pdev), cdr_drv_name(dev), map, cdr_n,
			cdr_installs, cdr_pinned,
			cdr_previous ? "kept" : "none");
	mutex_unlock(&cdr_lock);
	device_unlock(dev);
	pci_dev_put(pdev);
	return len;
}

static const struct kernel_param_ops cdr_table_ops = {
	.set = cdr_ro_set,
	.get = cdr_table_get,
};
static const struct kernel_param_ops cdr_live_ops = {
	.set = cdr_ro_set,
	.get = cdr_live_get,
};
static const struct kernel_param_ops cdr_status_ops = {
	.set = cdr_ro_set,
	.get = cdr_status_get,
};
module_param_cb(table, &cdr_table_ops, NULL, 0444);
MODULE_PARM_DESC(table, "(read-only) the table this module installs");
module_param_cb(live, &cdr_live_ops, NULL, 0444);
MODULE_PARM_DESC(live, "(read-only) the device's current dma_range_map");
module_param_cb(status, &cdr_status_ops, NULL, 0444);
MODULE_PARM_DESC(status, "(read-only) device, driver and map state");

static int __init cdr_init(void)
{
	struct pci_dev *pdev;
	struct device *dev;
	int ret;

	ret = cdr_parse(ranges);
	if (ret)
		return ret;
	ret = cdr_validate();
	if (!ret)
		ret = cdr_check_coverage();
	if (ret)
		goto free_table;

	pdev = cdr_find_pci_dev(bdf);
	if (IS_ERR(pdev)) {
		ret = PTR_ERR(pdev);
		pr_err("no device %s (%d)\n",
		       bdf && *bdf ? bdf : "(first NVIDIA display controller)",
		       ret);
		goto free_table;
	}
	dev = &pdev->dev;

	device_lock(dev);
	if (dev->driver) {
		dev_err(dev,
			"bound to %s; the map must be in place before the "
			"driver probes, unload it first (rmmod nvidia_uvm "
			"nvidia)\n",
			cdr_drv_name(dev));
		ret = -EBUSY;
		goto unlock;
	}
	if (dev->dma_range_map) {
		if (!force) {
			dev_err(dev, "already has a DMA range map; force=1 "
				     "replaces it\n");
			ret = -EEXIST;
			goto unlock;
		}
		dev_warn(dev, "force=1: replacing the existing DMA range map, "
			      "restored on unload\n");
	}

	mutex_lock(&cdr_lock);
	cdr_previous = dev->dma_range_map;
	ret = cdr_install(dev);
	if (!ret)
		cdr_pdev = pdev;
	else
		cdr_previous = NULL;
	mutex_unlock(&cdr_lock);
	if (ret)
		goto unlock;

	/* Registered under the device lock: no bind can slip in between. */
	ret = bus_register_notifier(&pci_bus_type, &cdr_nb);
	if (ret) {
		mutex_lock(&cdr_lock);
		dev->dma_range_map = cdr_previous;
		kfree(cdr_installed);
		cdr_installed = NULL;
		cdr_previous = NULL;
		cdr_pdev = NULL;
		mutex_unlock(&cdr_lock);
		goto unlock;
	}
	cdr_log_table(dev);
	dev_info(dev, "installed %u ranges covering all System RAM\n", cdr_n);
	device_unlock(dev);
	return 0;

unlock:
	device_unlock(dev);
	pci_dev_put(pdev);
free_table:
	kfree(cdr_table);
	cdr_table = NULL;
	return ret;
}

static void __exit cdr_exit(void)
{
	struct pci_dev *pdev = cdr_pdev;
	struct device *dev = &pdev->dev;

	bus_unregister_notifier(&pci_bus_type, &cdr_nb);

	device_lock(dev);
	mutex_lock(&cdr_lock);
	if (dev->driver) {
		/* Only through a bind racing with rmmod: the driver keeps the
		 * map, and the driver core frees it on unbind. */
		dev_warn(dev, "%s is bound; its map stays in place%s\n",
			 cdr_drv_name(dev),
			 cdr_previous ? ", the replaced map is leaked" : "");
	} else if (cdr_installed && dev->dma_range_map == cdr_installed) {
		kfree(cdr_installed);
		dev->dma_range_map = cdr_previous;
		dev_info(dev, "map removed, %s\n",
			 cdr_previous ? "previous map restored" :
					"no DMA range map left");
	} else if (!dev->dma_range_map) {
		dev->dma_range_map = cdr_previous;
		dev_info(dev, "no map in place, %s\n",
			 cdr_previous ? "previous map restored" :
					"nothing to remove");
	} else {
		dev_warn(dev,
			 "a map this module did not install is in place; "
			 "left alone%s\n",
			 cdr_previous ? ", the replaced map is leaked" : "");
	}
	cdr_installed = NULL;
	cdr_previous = NULL;
	cdr_pdev = NULL;
	mutex_unlock(&cdr_lock);
	device_unlock(dev);

	pci_dev_put(pdev);
	kfree(cdr_table);
	cdr_table = NULL;
}

module_init(cdr_init);
module_exit(cdr_exit);

MODULE_LICENSE("GPL");
MODULE_DESCRIPTION("Caraxes: per-device DMA range map for the NVIDIA GPU "
		   "(temporary aid, D13)");
