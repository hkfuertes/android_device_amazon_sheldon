/* compat (4.4): enum usb_role only; role switching goes through mtu3, see iphone_rs.c */
#pragma once
enum usb_role { USB_ROLE_NONE, USB_ROLE_HOST, USB_ROLE_DEVICE };
