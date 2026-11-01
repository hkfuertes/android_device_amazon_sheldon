#
# Copyright (C) 2025 The LineageOS Project
#
# SPDX-License-Identifier: Apache-2.0
#

# Inherit from those products. Most specific first.
$(call inherit-product, $(SRC_TARGET_DIR)/product/full_base.mk)

# Inherit from device makefile.
$(call inherit-product, device/amazon/sheldon/device.mk)

# No camera hardware on the stick: LOS knob drops the Aperture camera app.
# Must be set before inheriting the common config below.
PRODUCT_NO_CAMERA := true

# Phone flavour of LineageOS without telephony: common_full_phone.mk minus telephony.mk
# (tablet_wifionly would add window_extensions + tablet characteristics, which misbehaved on the car screen).
$(call inherit-product, vendor/lineage/config/common_full.mk)
$(call inherit-product, vendor/lineage/config/wifionly.mk)
PRODUCT_PACKAGES += LatinIME
PRODUCT_PACKAGE_OVERLAYS += vendor/lineage/overlay/dictionaries
PRODUCT_ENFORCE_RRO_EXCLUDED_OVERLAYS += vendor/lineage/overlay/dictionaries

PRODUCT_NAME := lineage_sheldon_tab
PRODUCT_DEVICE := sheldon
PRODUCT_MANUFACTURER := Amzn
PRODUCT_BRAND := Fire
PRODUCT_MODEL := Fire TV Stick

# Phone UI overrides — device.mk sets tv/tvdpi for the TV product
PRODUCT_CHARACTERISTICS := default
PRODUCT_AAPT_PREF_CONFIG := xhdpi

# Auto-pair BT mice on boot (no touch input on a TV stick).
# Also triggerable on demand:
#   adb shell am broadcast -a com.amazon.sheldon.btmouse.START
PRODUCT_PACKAGES += BtMouseAutoPair
PRODUCT_PACKAGES += FrameworksResOverlaySheldonTab
PRODUCT_PACKAGES += SettingsOverlaySheldonTab

# adb shell btmousepair -> opens the BT mouse pairing window
PRODUCT_COPY_FILES += \
    $(LOCAL_PATH)/btmouse/btmousepair:$(TARGET_COPY_OUT_SYSTEM)/bin/btmousepair \

PRODUCT_COPY_FILES += \
    $(LOCAL_PATH)/btmouse/default-permissions.xml:$(TARGET_COPY_OUT_SYSTEM)/etc/default-permissions/btmouse.xml \
    $(LOCAL_PATH)/btmouse/privapp-permissions.xml:$(TARGET_COPY_OUT_SYSTEM)/etc/permissions/privapp-permissions-com.amazon.sheldon.btmouse.xml \
    $(LOCAL_PATH)/permissions/tablet_core_hardware.xml:$(TARGET_COPY_OUT_SYSTEM)/etc/permissions/tablet_core_hardware.xml

# CarPlay: sheldon plays the iPhone (g_iphone modules via BoardConfig, daemon + scrcpy capture here)
# ponytail: logcat (+kernel) persisted to /data/misc/logd so car tests survive the power cut;
# drop once CarPlay works ("adb shell setprop persist.logd.logpersistd stop" turns it off at runtime)
PRODUCT_SYSTEM_PROPERTIES += persist.logd.logpersistd=logcatd

# GPU composes the 1080p HDMI plane + the CarPlay copy every frame (~20fps cap). Compose the primary
# at 540p and let HWC scale it up. ponytail: global (TV output gets softer); raise/drop if not in a car.
PRODUCT_VENDOR_PROPERTIES += \
    ro.surface_flinger.max_graphics_width=960 \
    ro.surface_flinger.max_graphics_height=540
PRODUCT_PACKAGES += CarplayMic CarPlaySettings
PRODUCT_COPY_FILES += \
    $(LOCAL_PATH)/carplay/prebuilt/catplay_android:$(TARGET_COPY_OUT_SYSTEM)/bin/catplay_android \
    $(LOCAL_PATH)/carplay/prebuilt/scrcpy-server.jar:$(TARGET_COPY_OUT_SYSTEM)/etc/carplay/scrcpy-server.jar \
    $(LOCAL_PATH)/carplay/init.carplay.rc:$(TARGET_COPY_OUT_SYSTEM)/etc/init/carplay.rc \
    $(LOCAL_PATH)/carplay/media/jazz_in_paris.mp3:$(TARGET_COPY_OUT_SYSTEM)/media/carplay/jazz_in_paris.mp3 \
    $(LOCAL_PATH)/carplay/Vendor_1209_Product_ca7a.idc:$(TARGET_COPY_OUT_VENDOR)/usr/idc/Vendor_1209_Product_ca7a.idc

BUILD_FINGERPRINT := google/adt2/adt2:9/PTT5.190307.001/5360859:user/dev-keys
