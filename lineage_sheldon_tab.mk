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

BUILD_FINGERPRINT := google/adt2/adt2:9/PTT5.190307.001/5360859:user/dev-keys
