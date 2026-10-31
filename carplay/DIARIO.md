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

## 2026-10-31 noche — Fase 2-5 (todo de golpe, se prueba con la build final)
Arquitectura final (más simple que el plan original):
- **Daemon `catplay_android`** (Rust, `carplay/catplay_android/src/main.rs`, ~350 líneas): miembro del workspace catplay
  @bc6edae, binario **estático armv7 musl** (no NDK) → `carplay/prebuilt/catplay_android` → `/system/bin`.
  Build: `docker build -t catplay-builder carplay/catplay_android` (musl-cross-make + rustup + libusb) y
  `docker run --rm -v /mnt/DATA:/mnt/DATA -v /mnt/DATA/los-build/cargo:/cargo catplay-builder carplay/build-daemon.sh`.
  Gotchas: catplay_hap necesita OpenSSL → `openssl/vendored`; catplay_audio fuerza opus dinámico y el build.rs de
  libopus_sys mira `cfg!(musl)` del HOST → `LIBOPUS_STATIC=1`.
- **Vídeo + audio Android→coche: scrcpy-server 5.0.1** (Apache-2.0, `prebuilt/scrcpy-server.jar` → `/system/etc/carplay/`),
  lanzado por el daemon como uid shell (setgroups/setgid/setuid en pre_exec) vía app_process: espejo SurfaceFlinger →
  MediaCodec HW H.264 (IDR cada 1 s) + REMOTE_SUBMIX PCM 48k estéreo. Sockets abstractos `scrcpy_0ca7f00d`.
  Antes: `wm size <ancho coche>x<alto coche>`, `svc power stayon true`, WAKEUP. Al terminar: `wm size reset`.
- **Touch/mando/botones del coche → `/dev/uhid`** con el descriptor HID del propio coche (CONFIG_UHID=y, hid-generic).
  VID:PID fijo 1209:ca7a + `/vendor/usr/idc/Vendor_1209_Product_ca7a.idc` (touch.deviceType = touchScreen).
- Protocolo: record → take screen+mainAudio (UserInitiated) → modesChanged → audio SETUP (MainAudio/Media PCM_48000_STEREO,
  abierto toda la sesión) → screen SETUP; ChangeModes del coche → process_request_to_response + modesChanged; si el coche
  quita la pantalla → TEARDOWN; RequestUI → retomar. ForceKeyFrame → descartar hasta el próximo IDR (≤1 s).
- **Arranque**: `carplay.rc` (system/etc/init), off por defecto: `setprop persist.vendor.carplay.enable 1` → insmod de los
  5 .ko, mount debugfs, IPv6 on, `start vendor.catplay` (logwrapper, seclabel su). Daemon libera el UDC de g1 (adb USB fuera).
  `=0` → para, quita g_iphone y devuelve el UDC a g1.
- g_iphone: `iphone_rs` trata `-EINVAL` de mtu3 debugfs ("duplicated setting") como "ya está en ese rol".
- Smoke test: binario arranca en un Android armv7 real (Echo radar, como shell): logger OK, reintenta gadget cada 1 s.
- **Build final OK** (BUILD-EXIT 0): zip `lineage-20.0-20261031-UNOFFICIAL-sheldon_tab.zip` sha256 `0ad8b649bf56294b0f8c918da494f822452d095d58d34b7f267026c0e53b93eb`.
  Verificado en target_files: catplay_android (= prebuilt), carplay.rc, scrcpy jar, idc, 5 .ko en vendor/lib/modules.

### NO hecho / riesgos conocidos
- **Mic coche→Android** (Siri/llamadas): no. Necesita mic virtual en el HAL de audio.
- Multitouch: sin CONFIG_HID_MULTITOUCH, solo 1 dedo fiable.
- Display sin HDMI conectado: no verificado que SurfaceFlinger siga componiendo (dumpsys dio display 0 presente).
- Rol host: mtu3 enciende su regulador VBUS a la vez que el coche da VBUS (igual que el modo cable-Y de Amazon).
- Codec H.264 del encoder MTK (perfil por defecto); si el coche no decodifica, probar `video_codec_options=profile:int=8`.
- mDNS 5353 puede chocar con mdnsd de Android si está activo.
- catplay no tiene LICENSE en el repo: el binario lo incluye; uso personal.

## 2026-10-31/11-01 noche — piezas restantes (solo compilado, SIN probar en hardware)
- **hid-multitouch.ko** (fuente 4.4 sin cambios, `carplay/hid_multitouch/`): las pantallas de coche con Contact ID
  caen en HID_GROUP_MULTITOUCH y hid-generic no las coge; sin este módulo no habría touch. Se carga en carplay.rc.
- **Selftest sin coche**: `adb shell su -c "catplay_android --selftest 800x480"` (o como shell). Crea un touch uhid,
  arranca scrcpy al tamaño dado, comprueba SPS/PPS + IDR + paquetes de audio. No toca USB.
- **Encoder**: espera 5 s al SPS/PPS del encoder por defecto (MTK OMX); si no llega, reintenta con
  `c2.android.avc.encoder` (software). Forzar: `setprop persist.vendor.carplay.encoder <nombre>`.
  Otros knobs: `persist.vendor.carplay.bitrate` (8000000), `persist.vendor.carplay.codec_options`
  (`i-frame-interval:float=1`; añadir `,profile:int=1` = Baseline si el coche no decodifica).
- scrcpy con `cleanup=false` (su helper de cleanup borra /data/local/tmp/scrcpy-server.jar).
- **Comandos del coche**: SetNightMode → `cmd uimode night yes|no` (y al inicio según info del coche);
  Duck/Unduck → ganancia en el PCM (sin rampa); botón de voz (RequestSiri ButtonUp) → KEYCODE_VOICE_ASSIST.
- **Mic coche → Android** (`carplay/mic/`, `CarplayMic.jar` en /system/framework): app_process como shell
  (tiene MODIFY_AUDIO_ROUTING) registra un AudioPolicy loopback (patrón "injection" de CTS, HAL r_submix) que captura
  todo AudioRecord con fuente MIC/DEFAULT/VOICE_*/CAMCORDER y le inyecta el PCM del coche (stdin, s16le 24k mono).
  Avisa por stdout `REC 1/0` cuando una app graba → el daemon cambia MainAudio de Media 48k estéreo a
  **Telephony 24k mono dúplex** (salida Android remuestreada 48k st → 24k mono) y vuelve al soltar.
  Riesgo: no sé si el coche acepta Telephony sin declarar appState phoneCall en modes.

- **Build final OK** (BUILD-EXIT 0): `lineage-20.0-20261031-UNOFFICIAL-sheldon_tab.zip` sha256
  `fcdd2fd09386fcc3f09f24936d8139af4b078f3704bade9b3cc4c32383e6af6d`. En target_files: catplay_android, CarplayMic.jar,
  carplay.rc, scrcpy jar, idc y 6 .ko (5 g_iphone + hid-multitouch). Nota: el Makefile del módulo externo debe llamarse
  `Makefile` (modpost de 4.4 no lee `Kbuild`).

### Prueba sugerida (otro día, en el sheldon)
1. `adb shell catplay_android --selftest 800x480` → debe acabar en `selftest: PASS` (dice qué encoder vale).
2. `adb tcpip 5555; adb connect <ip>; adb shell setprop persist.vendor.carplay.enable 1`, enchufar al coche,
   `adb logcat -s catplay_android`.
