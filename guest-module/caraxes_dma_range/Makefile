# SPDX-License-Identifier: GPL-2.0-only
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# caraxes_dma_range guest module: build inside the GUEST (needs
# linux-headers-$(uname -r)). Temporary aid, see README.md.
#
#   make
#   sudo insmod caraxes_dma_range.ko ranges=0:0:40000000,...
#
# caraxes_dma_range_test.ko is the check of the DMA API against the table;
# load it only while no driver is bound to the GPU.
obj-m := caraxes_dma_range.o caraxes_dma_range_test.o

KDIR ?= /lib/modules/$(shell uname -r)/build

all: modules

modules:
	$(MAKE) -C $(KDIR) M=$(CURDIR) modules

clean:
	$(MAKE) -C $(KDIR) M=$(CURDIR) clean

.PHONY: all modules clean
