#
# Copyright (C) 2025 The LineageOS Project
#
# SPDX-License-Identifier: Apache-2.0
#

# Inherit from those products. Most specific first.
$(call inherit-product, $(SRC_TARGET_DIR)/product/full_base.mk)

# Inherit from device makefile.
$(call inherit-product, device/amazon/sheldon/device.mk)

# Inherit some common LineageOS stuff.
$(call inherit-product, vendor/lineage/config/common_full_tablet_wifionly.mk)

PRODUCT_NAME := lineage_sheldon_tab
PRODUCT_DEVICE := sheldon
PRODUCT_MANUFACTURER := Amzn
PRODUCT_BRAND := Fire
PRODUCT_MODEL := Fire TV Stick (Tablet UI)

# Tablet UI overrides — device.mk sets tv/tvdpi for the TV product
PRODUCT_CHARACTERISTICS := tablet
PRODUCT_AAPT_PREF_CONFIG := xhdpi

# Auto-pair BT mice on boot (no touch input on a TV stick).
# Also triggerable on demand:
#   adb shell am broadcast -a com.amazon.sheldon.btmouse.START
PRODUCT_PACKAGES += BtMouseAutoPair
PRODUCT_PACKAGES += FrameworksResOverlaySheldonTab

# 240dpi => sw >= 600dp: Settings (and other apps) split into two panes.
# Verified on-device: 320/288 don't split at this panel size, 240 does.
PRODUCT_PROPERTY_OVERRIDES += ro.sf.lcd_density=240
PRODUCT_COPY_FILES += \
    $(LOCAL_PATH)/btmouse/default-permissions.xml:$(TARGET_COPY_OUT_SYSTEM)/etc/default-permissions/btmouse.xml \
    $(LOCAL_PATH)/btmouse/privapp-permissions.xml:$(TARGET_COPY_OUT_SYSTEM)/etc/permissions/privapp-permissions-com.amazon.sheldon.btmouse.xml \
    $(LOCAL_PATH)/permissions/tablet_core_hardware.xml:$(TARGET_COPY_OUT_SYSTEM)/etc/permissions/tablet_core_hardware.xml

BUILD_FINGERPRINT := google/adt2/adt2:9/PTT5.190307.001/5360859:user/dev-keys
