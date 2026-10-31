#
# Copyright (C) 2025 The LineageOS Project
#
# SPDX-License-Identifier: Apache-2.0
#

DEVICE_PATH := device/amazon/sheldon

# APEX
OVERRIDE_TARGET_FLATTEN_APEX := true

# Architecture
TARGET_ARCH := arm
TARGET_ARCH_VARIANT := armv8-a
TARGET_CPU_ABI := armeabi-v7a
TARGET_CPU_ABI2 := armeabi
TARGET_CPU_VARIANT := cortex-a53
TARGET_CPU_VARIANT_RUNTIME := cortex-a53

TARGET_USES_64_BIT_BINDER := true

# Bootloader
TARGET_BOOTLOADER_BOARD_NAME := sheldon
TARGET_NO_BOOTLOADER := true

# Boot Image
BOARD_KERNEL_CMDLINE := bootopt=64S3,32N2,32N2 androidboot.selinux=permissive
BOARD_KERNEL_CMDLINE += androidboot.init_fatal_reboot_target=recovery
BOARD_KERNEL_BASE := 0x40000000
BOARD_KERNEL_OFFSET := 0x00008000
BOARD_KERNEL_PAGESIZE := 2048
BOARD_KERNEL_TAGS_OFFSET := 0x14000000
BOARD_RAMDISK_OFFSET := 0x15000000
BOARD_SECOND_OFFSET := 0x00f00000
BOARD_BOOTIMG_HEADER_VERSION := 1
BOARD_RAMDISK_USE_XZ := true

BOARD_MKBOOTIMG_ARGS := --ramdisk_offset $(BOARD_RAMDISK_OFFSET)
BOARD_MKBOOTIMG_ARGS += --tags_offset $(BOARD_KERNEL_TAGS_OFFSET)
BOARD_MKBOOTIMG_ARGS += --second_offset $(BOARD_SECOND_OFFSET)
BOARD_MKBOOTIMG_ARGS += --header_version $(BOARD_BOOTIMG_HEADER_VERSION)
BOARD_MKBOOTIMG_ARGS += --board ""

# Build System
BUILD_BROKEN_DUP_RULES := true
BUILD_BROKEN_PREBUILT_ELF_FILES := true
BUILD_BROKEN_ELF_PREBUILT_PRODUCT_COPY_FILES := true
SOONG_CONFIG_NAMESPACES += surfaceflinger_workarounds
SOONG_CONFIG_surfaceflinger_workarounds += force_hwc_buffer_resubmit_video
SOONG_CONFIG_surfaceflinger_workarounds_force_hwc_buffer_resubmit_video := true
SOONG_CONFIG_surfaceflinger_workarounds += force_hwc_layer_recreate_protected_to_1x1
SOONG_CONFIG_surfaceflinger_workarounds_force_hwc_layer_recreate_protected_to_1x1 := true

# Display
TARGET_ADDITIONAL_GRALLOC_10_USAGE_BITS := 0x00000200

# Filesystems
BOARD_SYSTEMIMAGE_FILE_SYSTEM_TYPE   := ext4
BOARD_CACHEIMAGE_FILE_SYSTEM_TYPE    := ext4
BOARD_USERDATAIMAGE_FILE_SYSTEM_TYPE := ext4
BOARD_VENDORIMAGE_FILE_SYSTEM_TYPE   := ext4
TARGET_USERIMAGES_USE_EXT4 := true

# HIDL
DEVICE_MANIFEST_FILE += $(DEVICE_PATH)/manifest.xml

# HWUI
HWUI_COMPILE_FOR_PERF := true

# Kernel
BOARD_KERNEL_IMAGE_NAME := zImage-dtb
KERNEL_TOOLCHAIN := $(shell pwd)/prebuilts/gcc/$(HOST_OS)-x86/arm/arm-linux-androideabi-4.9/bin
TARGET_KERNEL_ARCH := arm
TARGET_KERNEL_HEADER_ARCH := arm
TARGET_KERNEL_SOURCE := kernel/amazon/mt8695
TARGET_KERNEL_CONFIG := mt8695_defconfig amazon/$(PRODUCT_DEVICE).config
TARGET_KERNEL_CLANG_COMPILE := false
TARGET_KERNEL_CROSS_COMPILE_PREFIX := arm-linux-androideabi-

# Malloc
MALLOC_SVELTE := true

# Partitions
BOARD_FLASH_BLOCK_SIZE := 131072                   # 2048      * 64   (none)
BOARD_BOOTIMAGE_PARTITION_SIZE := 16777216         # 16384     * 1024 (mmcblk0p4)
BOARD_RECOVERYIMAGE_PARTITION_SIZE := 16777216     # 16384     * 1024 (mmcblk0p5)
BOARD_CACHEIMAGE_PARTITION_SIZE := 17179869184     # 524288    * 1024 (mmcblk0p12)
BOARD_USERDATAIMAGE_PARTITION_SIZE := 5500615680   # 5371695   * 1024 (mmcblk0p13)
BOARD_VENDORIMAGE_PARTITION_SIZE := 157286400      # 153600    * 1024 (mmcblk0p10)
BOARD_SYSTEMIMAGE_PARTITION_SIZE := 1572864000     # 1536000   * 1024 (mmcblk0p11)

TARGET_COPY_OUT_VENDOR := vendor
TARGET_COPY_OUT_SYSTEM := system

# Platform
TARGET_BOARD_PLATFORM := mt8695

# Properties
TARGET_SYSTEM_PROP += $(DEVICE_PATH)/system.prop
TARGET_VENDOR_PROP += $(DEVICE_PATH)/vendor.prop

# Recovery
TARGET_RECOVERY_FSTAB := $(DEVICE_PATH)/rootdir/etc/fstab.mt8695
TARGET_OTA_ASSERT_DEVICE := sheldon,sheldonp

# Screen
# Tablet UI: 240dpi => sw >= 600dp, Settings goes two-pane (verified on-device).
# Must live here: BoardConfig is read after the product mk and would override it.
ifeq ($(TARGET_PRODUCT),lineage_sheldon_tab)
TARGET_SCREEN_DENSITY := 240
else
TARGET_SCREEN_DENSITY := 320
endif

# CarPlay display (tablet only): g_iphone USB gadget as external kernel modules
ifeq ($(TARGET_PRODUCT),lineage_sheldon_tab)
TARGET_KERNEL_EXT_MODULE_ROOT := device/amazon/sheldon/carplay
TARGET_KERNEL_EXT_MODULES := g_iphone:kbuild
endif

# Verified Boot
BOARD_AVB_ENABLE := false

# VNDK
BOARD_VNDK_VERSION := current

# Inherit the proprietary files
include vendor/amazon/sheldon/BoardConfigVendor.mk
