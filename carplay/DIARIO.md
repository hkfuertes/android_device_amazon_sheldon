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

## 2026-11-01 — primera prueba en el sheldon (sin coche), ROM sha256 d1cf3834…
Todo OK con `carplay-check.sh` (en /mnt/DATA/los-build, fuera de git por la clave WiFi):
- UDC `11271000.usb`, ruta de cambio de rol `/sys/kernel/debug/usb/11271000.usb/mode` existe (mtu3 manual drd).
- Selftest PASS con el **encoder HW MTK** (`OMX.MTK.VIDEO.ENCODER.AVC`, SPS 67 42 00 29 = Baseline 4.1); audio llega.
- `persist.vendor.carplay.enable=1`: 6 módulos cargados, g_iphone bind con 4 configs, pullup D+, el daemon llega a
  `WaitingForUsbAccessory` → `ReceivedEnable`. El PC (tollan) como host lo enumeró como iPhone (config #1 PTP) y le
  mandó la petición Apple de carga 0x40/0x40 (2900 mA) → **el gadget se ve como iPhone desde un host real**.
- `=0`: g1 recupera el UDC (adb USB vuelve). Los .ko quedan cargados (no hay rmmod; inofensivo).
- SELinux avc module_load denegado pero permissive=1: OK.
Pendiente: coche (cambio de rol a host, NCM, sesión CarPlay, vídeo/touch/audio/mic).

## 2026-11-01 — coche: "compruebe el iPhone", sin logs
- El coche reconoce el gadget como iPhone pero no llega a CarPlay. No hubo logs (logpersist apagado, y el stick
  pierde corriente al desenchufarlo del coche). Tampoco había clave adb guardada → `unauthorized` por WiFi.
- init.carplay.rc: con CarPlay activo se pone `sys.usb.config none` y adbd arranca solo TCP (antes init/adbd
  reintentaban cada segundo enganchar g1 al UDC → EBUSY en bucle, y riesgo de robar el UDC durante el cambio de rol).
- Daemon: nivel de log `debug` por defecto (`persist.vendor.carplay.log` para cambiarlo).
- ROM: `persist.logd.logpersistd=logcatd` de serie → logcat+kernel en /data/misc/logd (quitar cuando funcione).
- Recuperar logs: /mnt/DATA/los-build/carplay-pull-logs.sh (recovery → Enable ADB; sube el tgz a dungeon).

## 2026-11-01 tarde — segunda prueba en coche (LG, vid 1004 pid 771a): llega a NCM
Logs persistentes OK. Secuencia por intento (11 intentos): GadgetStarted → ReceivedEnable → coche pide
cambio de rol (0x51) → ReceivedRoleSwitch → mtu3 pasa a host → coche enumera como accesorio
"LG Electronics / Gadget iAP2" → cdc_ncm registra usb0 → ConfiguringNcm → **falla**: catplay ejecuta
`/sbin/ip` (no existe en Android) → AccessoryFailedNCM(NotFound) → unbind y vuelta a empezar.
Arreglo: build-daemon.sh reescribe `"/sbin/` → `"/system/bin/` en catplay (ip, modprobe, rmmod).
Densidad vuelta a 320 (as-is) a petición del usuario.
Siguiente punto de fallo posible: bonjour/mDNS en usb0 (puerto 5353 vs mdnsd de Android) y la sesión iAP2/AirPlay.

## 2026-11-01 — tercera prueba en coche: sesión iAP2 completa, falla al conectar AirPlay
Coche: LG "Media Display" (ULC…Series, hostname imx6dlsabrebavn). Ahora sí: cambio de rol → usb0 con IP
fe80::…def0 → iAP2 identificación aceptada → autenticación OK → el coche muestra "iPhone SIM" (nombre fijo en
catplay) → Bonjour encuentra `CarPlay._airplay._tcp` del coche en [fe80::…%usb0]:7000 → invitación →
`Connecting` → **ENETUNREACH** al abrir el TCP hacia el coche → coche: "CarPlay no está disponible".
Causa: el policy routing de Android (netd) no tiene regla "lookup main"; la ruta fe80::/64 de usb0 está en
main → todo tráfico saliente por usb0 acaba en la regla 32000 unreachable (el multicast mDNS sí pasaba).
Arreglo: carplay.rc añade `ip -6 rule add oif usb0 lookup main pref 5000` al activar (y la quita al desactivar).

## 2026-11-01 — cuarta prueba: mismo ENETUNREACH
La regla `oif usb0 lookup main` sí se aplicó (después de netd), pero la ruta fe80::/64 de usb0 no está en main:
netd pone `accept_ra_rt_table=-1000` y el kernel Android 4.4 (addrconf_rt_table) mete las rutas de prefijo
de cada iface en la tabla 1000+ifindex (usb0=12 → 1012). Arreglo: patch 0002 en catplay pone
`conf/<iface>/accept_ra_rt_table=0` antes de levantar la NCM → ruta en main. Además el daemon ahora arranca
con `sys.boot_completed=1` (antes a los 9 s, sin system_server para wm/scrcpy).

## 2026-11-01 — ¡FUNCIONA! Quinta prueba: CarPlay con Android en el coche, touch OK
Coche LG ULC4.5.LAN5900WR0: display 800x480@60, 176x99 mm (~115 dpi reales), 3 HID (Touch Screen 0..800/0..480,
Telephony, Button). Sesión limpia ~45 s (hasta cortar corriente): encoder HW MTK, ~10 fps medios (solo envía
cuando cambia la pantalla), 9 keyframes, audio MainAudio con feedback del coche, sin errores/crash.
Los ENETUNREACH del log son de arranques anteriores (antes del patch 0002).
Formatos del coche: MainAudio Default/Telephony entrada/salida PCM 8/16/24/48k mono (+48k estéreo salida),
SpeechRecognition, Media/Alert 48k estéreo, AltAudio 48k estéreo.
Ajuste: el daemon pone `wm density` = persist.vendor.carplay.density (def. 160 → 800x480 dp, UI móvil apaisada)
y lo resetea al desconectar.
Pendiente de verificar: audio audible en el coche, micrófono (REC/Telephony), Siri→VOICE_ASSIST, modo noche.

## 2026-11-01 — nombre, horizontal, interruptor en Ajustes
- Nombre en el coche: build-daemon.sh reemplaza "iPhone SIM" (DeviceInformationUpdate), "iPhone" (AirPlay
  InitialSetup) y "CatPlay" (iPod name) por $CATPLAY_NAME; el daemon lo toma de persist.vendor.carplay.name
  (def. "Fire TV Stick"). Modelo iPhone8.1 e instancias bonjour sin tocar.
- Durante la sesión: `wm user-rotation lock 0` + `wm set-ignore-orientation-request true` (apps verticales con
  bandas), se restauran (free/false) al desconectar.
- carplay/settings → CarPlaySettings (/system/app, cert platform): interruptor inyectado en Ajustes →
  Dispositivos conectados (SwitchesProvider, categoría ia.device, protegido con WRITE_SECURE_SETTINGS) y tile
  de Ajustes rápidos. Ambos cambian persist.vendor.carplay.enable (SELinux permisivo; en enforcing haría falta
  regla set_prop).
Sin probar en dispositivo.

## 2026-11-01 — sexta prueba: vídeo 17-20 fps en movimiento, sin audio
- fps: el daemon seguía en RUST_LOG=debug (~270 líneas/s + hexdumps; logpersist ~100 KB/s a eMMC) y el
  MTK encoder tenía 7-8 buffers de salida pendientes → cuello en el consumo, no en el encoder.
  Ahora por defecto `error,catplay_android=info` (prop persist.vendor.carplay.log sigue mandando).
- audio: el coche nos da resourceID 2 (MainAudio) entity 1, el r_submix "0" de scrcpy se abre y se envían
  48k estéreo sin cortes. El mic del coche entra (Grabadora → swap a Telephony y vuelta). La única
  reproducción fue esa grabación, así que la prueba no distingue salida vs mic.
  Añadido medidor: cada 5 s `audio to car: peak N dBFS` y `audio car mic: peak N dBFS` (-99 = silencio).
- Canción de prueba: carplay/media/jazz_in_paris.mp3 (YouTube Audio Library, de AOSP samples) →
  /system/media/carplay, copiada a /sdcard/Music al arrancar (aparece en Música).

## 2026-11-01 19:35 — composición a 540p + mp3 arreglado
- Navegación solo con "atrás"/sin panel: culpa de carplay-check.sh (user_setup_complete=1 sin pasar el asistente → SetupWizard deja setDisabledForSetup). Arreglo: `pm disable org.lineageos.setupwizard` + reboot.
- fps tope ~20 con encoder sin cola → cuello en composición GPU (1080p HDMI + copia 800x480). HWC solo ofrece 1920x1080.
- ro.surface_flinger.max_graphics_width/height=960x540 (vendor): SF compone el primario a 540p y HWC escala. Global; la salida TV se ve más blanda.
- mp3: exec de init no ve /sdcard (exit 1); ahora copia como media_rw a /data/media/0/Music (aparece en el siguiente escaneo/arranque).
- Zip sha256 f73fa83e...777a. Sin probar.

## 2026-11-01 19:50 — panel HDMI apagado durante la sesión
- CarplayMic.jar gana `com.amazon.sheldon.carplay.Screen off|on` (SurfaceControl.setDisplayPowerMode en los displays físicos, como scrcpy --turn-screen-off). El daemon lo llama al arrancar captura y al cerrarla.
- persist.vendor.carplay.screen_off=0 lo desactiva (por si el HWC de MTK se porta mal). Si el daemon muere en sesión, el panel vuelve con un ciclo sleep/wake.
- Zip sha256 560a357b...1d57. Sin probar.

## 2026-11-01 20:20 — sabor phone en vez de tablet
- lineage_sheldon_tab.mk: common_full.mk + wifionly.mk + LatinIME (common_full_phone sin telephony.mk); fuera window_extensions y ro.build.characteristics=tablet; modelo "Fire TV Stick".
- Se mantienen: sin teléfono (wifionly, sin Dialer/Messaging), sin batería (SettingsOverlaySheldonTab + icon_blacklist), sin cámara (PRODUCT_NO_CAMERA, sin Aperture).
- Nombre de producto sigue lineage_sheldon_tab (lunch/BoardConfig). Zip sha256 3b001553...1904. Sin probar.
