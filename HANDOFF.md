# Handoff — Tablet UI de LineageOS 20 para sheldon (Fire TV Stick 3rd Gen/Lite)

## Qué hay en esta rama (`lineage-20.0-tablet`)

Producto **tablet** (UI de teléfono/tablet, sin Leanback/Android TV) junto al TV existente, que queda intacto.

- `lineage_sheldon_phone.mk` (nuevo):
  - Hereda `$(SRC_TARGET_DIR)/product/full_base.mk` + `vendor/lineage/config/common_full_tablet_wifionly.mk` (nombre verificado contra `vendor/lineage` branch `lineage-20.0`).
  - No hereda nada de `device/google/atv`, `device/lineage/atv`, `common_mini_tv` ni `gapps_tv`.
  - Sobrescribe tras heredar `device.mk`: `PRODUCT_CHARACTERISTICS := tablet`, `PRODUCT_AAPT_PREF_CONFIG := xhdpi` (device.mk pone `tv`/`tvdpi`; al ir después en el parse, gana el override).
  - Resto de vars igual que `lineage_sheldon.mk` (device `sheldon`, fingerprint adt2).
- `AndroidProducts.mk`: registra `lineage_sheldon_phone.mk` + lunch `lineage_sheldon_phone-userdebug`.
- `device.mk`, `lineage_sheldon.mk` (TV), `BoardConfig.mk`: **sin cambios**.

## Repos (branch `lineage-20.0`, ignorar `lineage-22.1`)

- Device tree: este fork (`hkfuertes/android_device_amazon_sheldon`), rama **`lineage-20.0-tablet`** ← apuntar aquí en el manifest local
- Blobs: `amazon-oss/android_vendor_amazon_sheldon` (sin fork)
- Kernel: `amazon-oss/android_kernel_amazon_mt8695` (sin fork)
- local_manifests base: `amazon-oss/local_manifests`
- Releases TV de referencia: `amazon-oss/releases` (`lineage-20-sheldon-v0.4`/`v0.5`)

## Build (en la máquina de build — NO en la de 113 GB libres)

Requisitos: Linux x86_64, ~250 GB libres full sync (o **~90–110 GB** con shallow: `repo init --depth=1` + `repo sync -c --no-tags`), 16 GB+ RAM (64 recomendado; 16 va justo, swap/zram si puede ser).

```bash
repo init -u https://github.com/LineageOS/android.git -b lineage-20.0
# local_manifests de amazon-oss, con device/amazon/sheldon apuntando a este fork, rama lineage-20.0-tablet
repo sync -c -j8
source build/envsetup.sh
breakfast lineage_sheldon_phone-userdebug
brt   # o mka bacon
```

GApps opcionales: MindTheGapps **arm normal (no TV)** vía local_manifests; el producto no hereda ninguna.

## Pitfalls

- Si `common_full_tablet_wifionly.mk` no existiera en el tree sincronizado, alternativas verificadas en LOS 20: `common_mini_tablet_wifionly.mk` (más ligero), `common_full_tablet.mk`.
- `PRODUCT_ENFORCE_RRO_TARGETS := *` ya está en device.mk — los overlays siguen igual.
- Sin GPS/cámara/teléfono (es un stick): por eso tablet-**wifionly**. Hay builds tablet de otros Fire TV como precedente.
- Flasheo: TWRP + TZ upgrade package la primera vez (XDA), wipe system_root/data/cache.
- Basarse en HEAD de `lineage-20.0` (v0.5 incluye fixes post-v0.4).
- Kernel config: `mt8695_defconfig amazon/$(PRODUCT_DEVICE).config` — no cambia con el producto nuevo.
