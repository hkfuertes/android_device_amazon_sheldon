/* Force-included on 4.x kernels (see Makefile). Backfills APIs added after 4.4. */
#pragma once
#include <linux/version.h>
#include <linux/device.h>
#include <linux/idr.h>
#include <linux/poll.h>
#include <linux/fs.h>

typedef unsigned __poll_t;
#define EPOLLIN POLLIN
#define EPOLLOUT POLLOUT
#define EPOLLERR POLLERR
#define EPOLLHUP POLLHUP
#define EPOLLRDNORM POLLRDNORM
#define EPOLLWRNORM POLLWRNORM

#ifndef struct_size
#define struct_size(p, member, n) (sizeof(*(p)) + (n) * sizeof(*(p)->member))
#endif

#define ida_alloc(ida, gfp) ida_simple_get(ida, 0, 0, gfp)
#define ida_free(ida, id) ida_simple_remove(ida, id)

/* 6.4 dropped the owner argument */
#undef class_create
#define class_create(name) ({ static struct lock_class_key __key; __class_create(THIS_MODULE, name, &__key); })

/* fs: 5.12 file_open_root(path,...), 4.14 kernel_write(..., loff_t *) */
#define file_open_root(path, name, flags, mode) \
	file_open_root((path)->dentry, (path)->mnt, name, flags, mode)

/* usb: 5.10 added the SSP descriptor argument */
#include <linux/usb/gadget.h>
#define usb_assign_descriptors(f, fs, hs, ss, ssp) usb_assign_descriptors(f, fs, hs, ss)

#include <asm/unaligned.h>
static inline u32 get_unaligned_le24(const void *p)
{
	const u8 *b = p;
	return b[0] | b[1] << 8 | b[2] << 16;
}

/* class: CLASS_ATTR_WO 4.13, class_find_device_by_name 5.3 */
#define CLASS_ATTR_WO(_name) \
	struct class_attribute class_attr_##_name = __ATTR(_name, 0200, NULL, (void *)_name##_store) /* const-only mismatch */
static inline int compat_match_name(struct device *dev, const void *name)
{
	return sysfs_streq(dev_name(dev), name);
}
#define class_find_device_by_name(cls, name) class_find_device(cls, NULL, name, compat_match_name)

/* workqueue: disable_work* are 6.10; callers set runtime_stopped first and run in process context */
#define disable_work(w) cancel_work_sync(w)
#define disable_delayed_work(w) cancel_delayed_work_sync(w)

/* net headers that usbnet.h assumes on newer kernels */
#include <linux/netdevice.h>
#include <linux/mii.h>

/*
 * driver core: device_driver_attach() is 5.4. ponytail: device_attach() probes
 * every matching driver, so this only works if the wanted driver matches the
 * interface (true for cdc_ncm and iap2_usb_bulk id tables); returns -EBUSY if
 * someone else won. Upgrade: export driver_probe_device if that ever bites.
 */
static inline int compat_device_driver_attach(struct device_driver *drv, struct device *dev)
{
	int r = device_attach(dev);
	if (r < 0)
		return r;
	if (!r)
		return -ENODEV;
	return dev->driver == drv ? 0 : -EBUSY;
}
#define device_driver_attach(drv, dev) compat_device_driver_attach(drv, dev)

/* lives in our libcomposite copy; iphone_audio.c includes the kernel's composite.h */
struct usb_function;
int config_ep_by_speed_and_alt(struct usb_gadget *g, struct usb_function *f,
			       struct usb_ep *_ep, u8 alt);

/* cdc_ncm.h uses struct usbnet without including usbnet.h on 4.4 */
#include <linux/usb.h>
#include <linux/usb/usbnet.h>
