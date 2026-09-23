/* SPDX-License-Identifier: GPL-2.0-only */
/* SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> */
/* SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR */
/*
 * Device lookup shared by caraxes_dma_range and its test module.
 */
#ifndef CARAXES_DMA_RANGE_PCI_H
#define CARAXES_DMA_RANGE_PCI_H

#include <linux/err.h>
#include <linux/pci.h>

/*
 * bdf is "DDDD:BB:DD.F" or "BB:DD.F" (hex). Without one, take the first
 * NVIDIA VGA controller, then the first NVIDIA 3D controller. The returned
 * device carries a reference; drop it with pci_dev_put().
 */
static inline struct pci_dev *cdr_find_pci_dev(const char *bdf)
{
	static const unsigned int classes[] = {
		PCI_CLASS_DISPLAY_VGA << 8,
		PCI_CLASS_DISPLAY_3D << 8,
	};
	unsigned int d = 0, b, s, f, i;
	struct pci_dev *pdev;

	if (bdf && *bdf) {
		if (sscanf(bdf, "%x:%x:%x.%x", &d, &b, &s, &f) != 4) {
			d = 0;
			if (sscanf(bdf, "%x:%x.%x", &b, &s, &f) != 3)
				return ERR_PTR(-EINVAL);
		}
		if (d > 0xffff || b > 0xff || s > 0x1f || f > 7)
			return ERR_PTR(-EINVAL);
		pdev = pci_get_domain_bus_and_slot(d, b, PCI_DEVFN(s, f));
		return pdev ? pdev : ERR_PTR(-ENODEV);
	}

	for (i = 0; i < ARRAY_SIZE(classes); i++) {
		pdev = NULL;
		while ((pdev = pci_get_class(classes[i], pdev)))
			if (pdev->vendor == PCI_VENDOR_ID_NVIDIA)
				return pdev;
	}
	return ERR_PTR(-ENODEV);
}

#endif /* CARAXES_DMA_RANGE_PCI_H */
