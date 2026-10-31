obj-m := g_iphone.o iap2_char.o iap2_hid.o iap2_usb_bulk.o iap2_scan.o

g_iphone-y := gadget/iphone_mod.o gadget/iphone_dev.o gadget/iphone_rs.o gadget/iphone_class.o \
	gadget/f_iphone.o gadget/g_iphone_impl.o gadget/libcomposite/composite.o gadget/iphone_hid.o gadget/iphone_audio.o gadget/iphone_endpoint.o
iap2_char-y := iap2_char/iap2_char_core.o
iap2_hid-y := iap2_hid/iap2_hid.o iap2_hid/hid_framing.o
iap2_usb_bulk-y := iap2_usb_bulk/iap2_usb_bulk.o
iap2_scan-y := scan/iap2_scan.o

KDIR ?= /lib/modules/$(shell uname -r)/build

MODULE_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
DIST_DIR := $(MODULE_DIR)/dist

.PHONY: all clean insmod rmmod install

all:
	mkdir -p $(DIST_DIR)
	@if [ -L "$(DIST_DIR)/source" ]; then unlink "$(DIST_DIR)/source"; fi
	$(MAKE) -C $(KDIR) M=$(MODULE_DIR) MO=$(DIST_DIR) modules

clean:
	mkdir -p $(DIST_DIR)
	@if [ -L "$(DIST_DIR)/source" ]; then unlink "$(DIST_DIR)/source"; fi
	$(MAKE) -C $(KDIR) M=$(MODULE_DIR) MO=$(DIST_DIR) clean

insmod:
	insmod $(DIST_DIR)/iap2_char.ko
	insmod $(DIST_DIR)/iap2_usb_bulk.ko
	insmod $(DIST_DIR)/iap2_scan.ko
	insmod $(DIST_DIR)/iap2_hid.ko
	modprobe libcomposite
	insmod $(DIST_DIR)/g_iphone.ko

rmmod:
	rmmod g_iphone
	rmmod iap2_hid
	rmmod iap2_scan
	rmmod iap2_usb_bulk
	rmmod iap2_char

install: all
	@if [ -L "$(DIST_DIR)/source" ]; then unlink "$(DIST_DIR)/source"; fi
	$(MAKE) -C $(KDIR) M=$(MODULE_DIR) MO=$(DIST_DIR) modules_install

# Android sheldon kernel (4.4): backfill newer APIs instead of patching upstream sources.
ifeq ($(VERSION),4)
# $(src) is relative to the kernel tree when built via Lineage (M=../../../device/...)
GI_DIR := $(if $(filter /%,$(src)),$(src),$(srctree)/$(src))
ccflags-y += -I$(GI_DIR)/compat/include -include $(GI_DIR)/compat/compat.h
endif
