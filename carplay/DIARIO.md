# Diario — CarPlay como display (rama `lineage-20.0-carplay`)

Objetivo: el Fire TV Stick (sheldon, LOS 20, kernel 4.4.302 arm32) se presenta al coche como
iPhone por USB (g_iphone) y emite la pantalla de Android (H.264), recibe touch y audio en
ambos sentidos vía el lado TX de catplay. Sin MFi local: el coche se autentica, nosotros no.

Referencias:
- catplay upstream: https://github.com/catplay-labs/catplay @ `bc6edae` (mismo pin que catplay-buildroot)
- catplay-buildroot rama `sheldon` (Linux 7.2, NO sirve para Android: solo como referencia de parches)
- Kernel Android: `/mnt/DATA/los20/kernel/amazon/mt8695` (4.4.302, ARM32, GCC 4.9)

## Fases
1. g_iphone como módulo externo para 4.4 + role switch sobre mtu3 → Mac ve `05ac:12a8`
2. Binario catplay TX para armv7 Android con frame fijo → prueba en coche (role switch + sesión)
3. Video: espejo display 0 (helper nativo tipo screenrecord, sin límite 180 s, encoder MTK AVC)
4. Touch: HidSendReport del coche → uinput
5. Audio: 5a salida Android → coche (remote submix); 5b micro coche → Android

## Hechos verificados (no repetir)
- Kernel: `CONFIG_MODULES=y`, `MODULE_UNLOAD=y`, `MODVERSIONS=y` → g_iphone como .ko sin tocar kernel (en principio).
- `CONFIG_USB_MTU3=y`, `MTU3_DUAL_ROLE=y`, `XHCI_MTK=y`; mtu3 tiene `ssusb_mode_manual_switch()` / `ssusb_set_force_mode()` (fuerza por registro, no depende de VBUS).
- NCM: tras role switch el coche presenta NCM y nosotros somos host → basta `CONFIG_USB_NET_CDC_NCM=y` (ya está). `CONFIG_USB_CONFIGFS_NCM` NO hace falta.
- `CONFIG_INPUT_UINPUT=y`, `CONFIG_IPV6=y`. SELinux permissive (cmdline).
- Encoder HW: `OMX.MTK.VIDEO.ENCODER.AVC`. `screenrecord` limita a 180 s.
- `dumpsys SurfaceFlinger --display-id` → `Display 0 (HWC display 0)` (pendiente: ¿con o sin HDMI?).
- Módulos externos: Lineage `TARGET_KERNEL_EXT_MODULES` (vendor/lineage/build/tasks/kernel.mk).

## Log
- 2026-10-31 21:2x — Rama creada desde `lineage-20.0` (post-merge PR #1).
- g_iphone copiado de catplay `bc6edae` a `carplay/g_iphone/` + parches 0001–0003 de
  catplay-buildroot (`buildroot/external/package/g_iphone/`) aplicados con `patch -p4`. Commit "vendor" sin cambios propios.
- 22:2x — **Backport g_iphone a 4.4 COMPILA** (5 .ko, vermagic `4.4.302+ SMP preempt mod_unload modversions ARMv7` = ROM).
  Build manual: `/mnt/DATA/los-build/gi-build.sh` (docker los20-builder) → `/mnt/DATA/los-build/gi-out/*.ko`.
  Cambios (todos marcados `catplay/4.4` o en `compat/`):
  - `compat/compat.h` (force-include solo si `VERSION=4`, ver Makefile) + `compat/include/` (cabeceras stub):
    `__poll_t/EPOLL*`, `struct_size`, `ida_alloc/free`, `class_create(name)`, `file_open_root(path)`,
    `usb_assign_descriptors` 5→4 args, `get_unaligned_le24`, `CLASS_ATTR_WO`, `class_find_device_by_name`,
    `disable_work*`→`cancel_*_sync`, `device_driver_attach`→`device_attach` (ponytail: solo vale si el driver
    deseado hace match por id_table), includes de net/usbnet.
  - **libcomposite**: sustituida la copia moderna (6.x, SSP/LPM/WebUSB) por la `composite.{c,h}` del kernel 4.4 +
    re-aplicadas las modificaciones de catplay (diff por tokens vs v6.15): sin EXPORT_SYMBOL, `c2a_usb_composite_probe`,
    bcdDevice 0x1602, sin selfpowered, `os_desc_req=NULL`, `udc_name` (ignorado en 4.4: 1 UDC).
    + backport de `config_ep_by_speed_and_alt` (FS/HS) + **ruta config0 de `req_match`** (3 args) para que el
    `0x51` llegue a f_iphone antes de SET_CONFIGURATION (crítico para el role switch).
  - gnu89: 4 `for (int i...)` reescritos (NO usar `-std=gnu11`: rompe inicializadores de spinlock/mutex en 4.4).
  - `iphone_dev.c`: `disable_work(&role_switch_work.work)` → `disable_delayed_work(&role_switch_work)`.
  - **Role switch**: `iphone_rs.c` en <4.19 escribe `host`/`device` en `/sys/kernel/debug/usb/<udc>/mode`
    (mtu3 con `enable-manual-drd` en sheldon.dts → `ssusb_mode_manual_switch`, fuerza IDDIG por registro y
    activa el regulador vbus (GPIO, mismo caso que cable Y)). **Requiere debugfs montado.**
- Integración: `BoardConfig.mk` → `TARGET_KERNEL_EXT_MODULES := g_iphone:kbuild` (solo `lineage_sheldon_tab`).
  Ojo: Lineage solo construye módulos externos si la .config tiene algún `=m` (hay 3: ok).

### Pendiente / NO hecho
- No probado en hardware. Comprobar en el stick: `ls /sys/class/udc`, `ls /sys/kernel/debug/usb/` (¿montado?).
- Carga de módulos al arranque: NO (de momento insmod manual; orden: iap2_char, iap2_usb_bulk, iap2_scan, iap2_hid, g_iphone).
- El UDC lo tiene el gadget configfs de Android (adb). Para g_iphone hay que soltarlo → se pierde adb USB (usar adb WiFi).
- 22:3x — Integrado en el build: `m bootimage` construye e instala los 5 .ko en `vendor/lib/modules` con
  `modules.dep` correcto (g_iphone → iap2_hid, iap2_scan, iap2_usb_bulk, iap2_char). Makefile: `GI_DIR` absoluto
  (kbuild 4.4 compila desde O= y `$(src)` es relativo al kernel).
  Símbolos: todos resueltos contra `Module.symvers` (el kernel Amazon ya trae `sysfs_emit`). Script: `/mnt/DATA/los-build/gi-syms.sh`.

### Fase 1 — prueba en banco (sin reflashear: el kernel no cambió)
```
adb tcpip 5555 && adb connect <ip>:5555          # al soltar el UDC se pierde adb USB
adb root && adb push /mnt/DATA/los-build/gi-out/*.ko /data/local/tmp/
adb shell
  mount -t debugfs none /sys/kernel/debug 2>/dev/null; ls /sys/class/udc; ls /sys/kernel/debug/usb
  cd /data/local/tmp && for m in iap2_char iap2_usb_bulk iap2_scan iap2_hid g_iphone; do insmod $m.ko; done
  echo -n "" > /config/usb_gadget/g1/UDC          # suelta el UDC del gadget Android (adb USB)
  echo default > /sys/class/iphone/create
  cat /sys/class/udc/* ; echo <UDC> > /sys/class/iphone/default/udc
  echo 1 > /sys/class/iphone/default/bind; cat /sys/class/iphone/default/status; dmesg | grep -i iphone
```
Éxito = el host (Mac/PC) ve `05ac:12a8` "iPhone" con 4 configuraciones (`lsusb -v` / `ioreg -p IOUSB`).
Role switch: solo lo prueba un coche real (0x51); en banco se puede forzar `echo host > /sys/kernel/debug/usb/<udc>/mode`.

### Notas para fase 2 (userspace catplay)
- `GadgetHelper::change_usb_otg_role/get_usb_otg_role` usan `/sys/class/usb_role` → en 4.4 devolverá "auto-managed"
  (no existe): OK porque el role switch lo hace el kernel (iphone_rs). Revisar `get_usb_otg_role` si algo depende de él.
- `cleanup_once()` = `usb_gadget::unbind_all/remove_all` → desmonta TAMBIÉN el gadget configfs de Android (adb). Asumido en modo coche.
