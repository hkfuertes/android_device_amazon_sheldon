//! catplay_android: sheldon (Android 13) plays the iPhone towards a wired-CarPlay car.
//!
//! USB/iAP2/AirPlay-TX: catplay (g_iphone kernel gadget + catplay_carplay_tx_gadget).
//! Screen + audio capture: scrcpy-server (Apache-2.0) run as shell via app_process, which already does
//!   SurfaceFlinger mirror -> MediaCodec HW H.264 and REMOTE_SUBMIX PCM. We read its local sockets.
//! Car touch/knob/buttons: the car's own HID descriptors are replayed through /dev/uhid, so the kernel
//!   (hid-generic) + Android InputReader handle them like a real USB HID device.
//! ponytail: no car->Android microphone (Siri/calls) yet; needs a virtual mic in the audio HAL.

use std::{
    collections::HashMap,
    error::Error,
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    os::{
        linux::net::SocketAddrExt,
        unix::{
            net::{SocketAddr, UnixStream},
            process::CommandExt,
        },
    },
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use bytes::BytesMut;
use catplay_carplay::{
    audio::{AudioPlayer, AudioRecorder, AudioSinkBox, AudioSourceBox},
    carplay_tx::{AirPlayTransmitter, AirPlayTransmitterProxyRef, TeardownGuard},
    modes::{EntityEnum, Resource, ResourceConstraint, ResourceController, ResourceID, ResourceTransferPriority},
    msg::{AudioFormat, AudioType, Command as CarCommand, SiriAction, StreamType},
    rtsp_frame::{HttpStatus, RtspResult},
    screen::tx::{ScreenTransmitProxy, ScreenTransmitSink},
    video::{AvccConfig, AvccConfigExtended, EncodedVideoFrame, Pts},
};
use catplay_carplay_tx_gadget::client::CarPlayUsbClientGadget;
use catplay_hap::HomekitStorageFile;
use catplay_util::{EventReconciler, EventSleeper};
use log::{error, info, warn};
use tokio::sync::mpsc;

type Res<T> = Result<T, Box<dyn Error + Send + Sync>>;

const MIC_JAR: &str = "/system/framework/CarplayMic.jar";
const SCRCPY_JAR: &str = "/system/etc/carplay/scrcpy-server.jar";
const SCRCPY_VERSION: &str = "5.0.1"; // must match the jar exactly
const SCRCPY_SCID: &str = "0ca7f00d";
const HOMEKIT_FILE: &str = "/data/vendor/carplay/homekit.bin";
// pid.codes test VID; matched by /vendor/usr/idc/Vendor_1209_Product_ca7a.idc (touch.deviceType = touchScreen)
const UHID_VENDOR: u32 = 0x1209;
const UHID_PRODUCT: u32 = 0xca7a;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if std::env::var_os("RUST_LOG").is_none() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }
    catplay_tracing::logger::setup_prod_logger();

    if std::env::args().nth(1).as_deref() == Some("--selftest") {
        let size = std::env::args().nth(2).unwrap_or("800x480".into());
        let (w, h) = size.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))).unwrap_or((800, 480));
        std::process::exit(match selftest(w, h) {
            Ok(()) => 0,
            Err(e) => {
                error!("SELFTEST FAILED: {e}");
                1
            }
        });
    }

    // The single UDC (mtu3) is held by Android's configfs gadget (adb/mtp). Free it for g_iphone.
    // adb over USB is gone while this runs: use adb over WiFi.
    if let Err(e) = std::fs::write("/config/usb_gadget/g1/UDC", "none") {
        warn!("could not release g1 UDC: {e}");
    }

    let homekit = HomekitStorageFile::file(HOMEKIT_FILE).unwrap_or_else(|e| {
        warn!("homekit storage {HOMEKIT_FILE}: {e:?}, pairing won't persist");
        HomekitStorageFile::memory()
    });
    let udc = std::env::var("CATPLAY_UDC").ok();
    let mut gadget = match CarPlayUsbClientGadget::new("default", "02:ca:7f:00:0d:01", udc.as_deref(), homekit, false) {
        Ok(g) => g,
        Err(e) => {
            error!("g_iphone gadget unavailable (modules loaded? UDC free?): {e:?}");
            std::process::exit(1);
        }
    };
    info!("waiting for a CarPlay car");

    let mut session: Option<tokio::task::JoinHandle<()>> = None;
    loop {
        let _ = gadget.reconcile().await;
        if let Some(tx) = gadget.child_mut().pop_transmitter() {
            if let Some(old) = session.take() {
                old.abort(); // a new transmitter means the old car session is dead anyway
            }
            session = Some(tokio::spawn(async move {
                match run_session(tx).await {
                    Ok(()) => info!("session ended"),
                    Err(e) => error!("session ended: {e}"),
                }
            }));
        }
        gadget.sleep().await;
    }
}

async fn run_session(tx: TeardownGuard<AirPlayTransmitterProxyRef>) -> Res<()> {
    let info = tx.info_cached().clone();
    let disp = info.displays.first().ok_or("car reported no display")?;
    let (w, h, fps) = (disp.width_pixels, disp.height_pixels, disp.max_fps.unwrap_or(30));
    info!("car {:?} {:?}: display {w}x{h}@{fps}, {} HID devices", info.manufacturer, info.model, info.hid_devices.len());
    for f in &info.audio_formats {
        info!("car audio format: {f:?}");
    }

    night_mode(*info.night_mode.clone().unwrap_or_default());

    let mut hid = HashMap::new();
    for dev in &info.hid_devices {
        match uhid_create(&dev.name, &dev.hid_descriptor) {
            Ok(f) => {
                info!("uhid: created '{}'", dev.name);
                hid.insert(dev.uuid.clone(), f);
            }
            Err(e) => warn!("uhid: '{}' failed: {e}", dev.name),
        }
    }

    tx.record().await?;
    let mut modes = ResourceController::from_change_modes(&info.modes);
    take_screen_and_audio(&mut modes);
    tx.assert_modes(modes.serialize_to_state()).await?;

    // Android -> car audio. MainAudio stays open all session:
    //   Media 48k stereo normally; Telephony 24k mono duplex (car mic -> Android) while an Android app records.
    // ponytail: always-open stream. Tear down when idle if a car complains.
    let sink: Arc<Mutex<Option<AudioSinkBox<i16>>>> = Default::default();
    let audio_on = Arc::new(AtomicBool::new(false));
    let voice = Arc::new(AtomicBool::new(false));
    let gain = Arc::new(AtomicU32::new(1f32.to_bits())); // car-requested ducking
    let (audio_gain, audio_voice, audio_sink, audio_flag) = (gain.clone(), voice.clone(), sink.clone(), audio_on.clone());
    let (mic, mut rec_rx) = match MicHelper::start() {
        Ok((m, rx)) => (Some(m), rx),
        Err(e) => {
            warn!("car mic helper unavailable: {e}");
            (None, mpsc::channel(1).1)
        }
    };
    let mic_in = mic.as_ref().map(|m| m.stdin.clone());
    let mut audio = open_audio(&*tx, false, &sink, &audio_on, None).await;

    let (_android, mut video_sock, mut audio_sock, first_config) = tokio::task::spawn_blocking(move || capture(w, h, fps)).await??;

    let (vtx, mut vrx) = mpsc::channel::<Packet>(8);
    thread::spawn(move || {
        while let Ok(p) = read_packet(&mut video_sock) {
            if vtx.blocking_send(p).is_err() {
                break;
            }
        }
    });
    thread::spawn(move || {
        let mut pcm = Vec::new();
        while let Ok(p) = read_packet(&mut audio_sock) {
            if p.config || !audio_flag.load(Ordering::Relaxed) {
                continue;
            }
            pcm.clear();
            let g = f32::from_bits(audio_gain.load(Ordering::Relaxed));
            let s16 = p.data.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 * g);
            if audio_voice.load(Ordering::Relaxed) {
                // 48k stereo -> 24k mono: average 2 frames x 2 channels (crude low-pass, fine for voice)
                let v: Vec<f32> = s16.collect();
                pcm.extend(v.chunks_exact(4).map(|c| (c.iter().sum::<f32>() / 4.0) as i16));
            } else {
                pcm.extend(s16.map(|x| x as i16));
            }
            if let Some(s) = audio_sink.lock().unwrap().as_mut() {
                s.write(&pcm);
            }
        }
    });

    let closed = tx.closed();
    tokio::pin!(closed);
    let mut screen: Option<TeardownGuard<ScreenTransmitProxy>> = None;
    let mut sps_pps = first_config.data;
    let mut need_key = true;

    loop {
        // Same ordering as catplay c2a: modesChanged before screen SETUP, TEARDOWN before modesChanged.
        let own_screen = modes.screen.entity() == EntityEnum::Controller;
        if own_screen && screen.is_none() {
            info!("screen SETUP");
            screen = Some(tx.setup_screen(Duration::from_millis(25)).await?);
            need_key = true;
        } else if !own_screen && screen.is_some() {
            info!("screen TEARDOWN (car took the screen)");
            drop(screen.take());
            tx.drain_teardown_queue().await?;
        }

        tokio::select! {
            err = &mut closed => return Err(format!("car closed the session: {err}").into()),
            Some(rec) = rec_rx.recv() => {
                info!("Android {} recording: main audio -> {}", if rec { "started" } else { "stopped" }, if rec { "telephony (car mic on)" } else { "media" });
                drop(audio.take());
                tx.drain_teardown_queue().await?;
                voice.store(rec, Ordering::Relaxed);
                audio = open_audio(&*tx, rec, &sink, &audio_on, if rec { mic_in.clone() } else { None }).await;
            }
            cmd = tx.pop_command() => {
                let Some(mut cmd) = cmd else { continue };
                match cmd.command().clone() {
                    CarCommand::ChangeModes(req) => {
                        let resp = modes.process_request_to_response(&req);
                        if resp.is_error() {
                            warn!("rejecting car ChangeModes {req:?}");
                        }
                        if cmd.respond_plist(HttpStatus::Ok, resp).is_err() {
                            cmd.respond_err();
                        }
                        tx.assert_modes(modes.serialize_to_state()).await?;
                    }
                    CarCommand::RequestUI(_) => {
                        take_screen_and_audio(&mut modes);
                        cmd.respond_ok();
                        tx.assert_modes(modes.serialize_to_state()).await?;
                    }
                    CarCommand::HidSendReport(r) => {
                        cmd.respond_ok(); // fast ack, cars queue only one HID command
                        if let Some(f) = hid.get_mut(&r.uuid) {
                            let _ = uhid_input(f, &r.hid_report);
                        }
                    }
                    CarCommand::ForceKeyFrame(_) => {
                        need_key = true; // scrcpy emits an IDR every second (i-frame-interval=1)
                        cmd.respond_ok();
                    }
                    // ponytail: duck without the ramp
                    CarCommand::DuckAudio(d) => {
                        gain.store(10f32.powf(d.volume as f32 / 20.0).to_bits(), Ordering::Relaxed);
                        cmd.respond_ok();
                    }
                    CarCommand::UnduckAudio(_) => {
                        gain.store(1f32.to_bits(), Ordering::Relaxed);
                        cmd.respond_ok();
                    }
                    CarCommand::SetNightMode(n) => {
                        night_mode(n.night_mode);
                        cmd.respond_ok();
                    }
                    CarCommand::RequestSiri(r) => {
                        // the car's voice button opens the Android assistant (no car mic yet, see DIARIO)
                        if r.siri_action == SiriAction::ButtonUp {
                            thread::spawn(|| sh(&["input", "keyevent", "KEYCODE_VOICE_ASSIST"]));
                        }
                        cmd.respond_ok();
                    }
                    other => {
                        info!("ignored car command {}", other.get_type());
                        cmd.respond_ok();
                    }
                }
            }
            pkt = vrx.recv() => {
                let p = pkt.ok_or("scrcpy video stream ended")?;
                if p.config {
                    sps_pps = p.data;
                    continue;
                }
                let Some(s) = screen.as_mut() else { continue };
                if (need_key && !p.key) || sps_pps.is_empty() {
                    continue;
                }
                let frame = EncodedVideoFrame {
                    pts: Pts(Instant::now() + s.stream_latency()),
                    width: w,
                    height: h,
                    data: BytesMut::from(&p.data[..]),
                    config: Some(AvccConfigExtended {
                        hevc: false,
                        avcc: AvccConfig { nal_size_len: 4, sps_pps: sps_pps.clone() },
                        video_latency: Duration::ZERO,
                        width: w,
                        height: h,
                        respect_timestamps: true,
                    }),
                    nal_offsets: None,
                    is_keyframe: Some(p.key),
                    header_buf: BytesMut::new(),
                    chacha_tag_buf: BytesMut::new(),
                };
                match s.push_frame(frame).await {
                    Ok(()) => need_key = false,
                    Err((e, _)) => {
                        warn!("push_frame: {e}");
                        need_key = true;
                    }
                }
            }
        }
    }
}

fn take_screen_and_audio(m: &mut ResourceController) {
    let p = ResourceTransferPriority::UserInitiated;
    for (r, id) in [(&mut m.screen, ResourceID::MainScreen), (&mut m.main_audio, ResourceID::MainAudio)] {
        if r.entity() != EntityEnum::Controller && r.can_take(p) {
            r.take_by_controller(&Resource::take(id, p, ResourceConstraint::Anytime, ResourceConstraint::Anytime));
        }
    }
}

async fn open_audio(
    tx: &AirPlayTransmitterProxyRef,
    voice: bool,
    sink: &Arc<Mutex<Option<AudioSinkBox<i16>>>>,
    on: &Arc<AtomicBool>,
    mic_in: Option<Arc<Mutex<ChildStdin>>>,
) -> Option<TeardownGuard<StreamType>> {
    let recorder = Box::new(Recorder { sink: sink.clone(), on: on.clone() });
    let (format, kind) = if voice { (AudioFormat::PCM_24000_MONO, AudioType::Telephony) } else { (AudioFormat::PCM_48000_STEREO, AudioType::Media) };
    let player = mic_in.map(|stdin| Box::new(MicPlayer { stdin, source: None, run: Default::default() }) as _);
    tx.setup_audio(Duration::from_millis(32), StreamType::MainAudio, format, kind, player, recorder)
        .await
        .inspect_err(|e| warn!("audio SETUP {kind:?} failed: {e}"))
        .ok()
}

/// Car mic samples (24k mono) -> CarplayMic helper stdin. The player pulls from the ring at real-time pace.
struct MicPlayer {
    stdin: Arc<Mutex<ChildStdin>>,
    source: Option<AudioSourceBox<i16>>,
    run: Arc<AtomicBool>,
}

impl AudioPlayer for MicPlayer {
    type Sample = i16;
    fn init(&mut self, source: AudioSourceBox<i16>) -> RtspResult<()> {
        self.source = Some(source);
        Ok(())
    }
    fn start(&mut self) {
        let Some(mut source) = self.source.take() else { return }; // ponytail: no resume after stop, the stream is re-SETUP instead
        let (stdin, run) = (self.stdin.clone(), self.run.clone());
        run.store(true, Ordering::Relaxed);
        thread::spawn(move || {
            let mut buf = [0i16; 480]; // 20 ms @ 24 kHz
            let mut next = Instant::now();
            while run.load(Ordering::Relaxed) {
                source.read(&mut buf);
                let bytes: Vec<u8> = buf.iter().flat_map(|s| s.to_le_bytes()).collect();
                if stdin.lock().unwrap().write_all(&bytes).is_err() {
                    break;
                }
                next += Duration::from_millis(20);
                thread::sleep(next.saturating_duration_since(Instant::now()));
            }
        });
    }
    fn stop(&mut self, _drain: bool) {
        self.run.store(false, Ordering::Relaxed);
    }
}

/// CarplayMic.jar (app_process, shell): AudioPolicy loopback so Android apps record the car mic.
/// stdout "REC 1"/"REC 0" when an app starts/stops recording.
struct MicHelper {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
}

impl MicHelper {
    fn start() -> io::Result<(Self, mpsc::Receiver<bool>)> {
        let mut cmd = Command::new("/system/bin/app_process");
        cmd.env("CLASSPATH", MIC_JAR)
            .args(["/system/bin", "com.amazon.sheldon.carplay.Mic"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        as_shell(&mut cmd);
        let mut child = cmd.spawn()?;
        let stdin = Arc::new(Mutex::new(child.stdin.take().unwrap()));
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel(4);
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let rec = match line.trim() {
                    "REC 1" => true,
                    "REC 0" => false,
                    _ => continue,
                };
                if tx.blocking_send(rec).is_err() {
                    break;
                }
            }
        });
        Ok((Self { child, stdin }, rx))
    }
}

impl Drop for MicHelper {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Recorder {
    sink: Arc<Mutex<Option<AudioSinkBox<i16>>>>,
    on: Arc<AtomicBool>,
}

impl AudioRecorder for Recorder {
    type Sample = i16;
    fn init(&mut self, sink: AudioSinkBox<i16>) -> RtspResult<()> {
        *self.sink.lock().unwrap() = Some(sink);
        Ok(())
    }
    fn start(&mut self) {
        self.on.store(true, Ordering::Relaxed);
    }
    fn stop(&mut self, _drain: bool) {
        self.on.store(false, Ordering::Relaxed);
    }
}

/// Android side: display sized like the car, scrcpy-server running as shell.
struct Android {
    scrcpy: Child,
}

impl Android {
    fn start(w: u32, h: u32, fps: u32, encoder: &str) -> io::Result<Self> {
        sh(&["wm", "size", &format!("{w}x{h}")]);
        sh(&["svc", "power", "stayon", "true"]);
        sh(&["input", "keyevent", "KEYCODE_WAKEUP"]);

        let mut cmd = Command::new("/system/bin/app_process");
        cmd.env("CLASSPATH", std::env::var("CATPLAY_SCRCPY_JAR").unwrap_or(SCRCPY_JAR.into())).args([
            "/",
            "com.genymobile.scrcpy.Server",
            SCRCPY_VERSION,
            &format!("scid={SCRCPY_SCID}"),
            "log_level=info",
            "tunnel_forward=true",
            "control=false",
            "video_codec=h264",
            "audio_codec=raw",
            &format!("max_size={}", w.max(h)),
            &format!("max_fps={fps}"),
            &format!("video_bit_rate={}", prop("persist.vendor.carplay.bitrate", "8000000")),
            // e.g. "i-frame-interval:float=1,profile:int=1" (1 = Baseline) if the car can't decode High
            &format!("video_codec_options={}", prop("persist.vendor.carplay.codec_options", "i-frame-interval:float=1")),
            "send_device_meta=false",
            "send_stream_meta=false",
            "send_dummy_byte=false",
            "stay_awake=true",
            "power_on=true",
            "cleanup=false", // the cleanup helper would unlink /data/local/tmp/scrcpy-server.jar
        ]);
        as_shell(&mut cmd);
        if !encoder.is_empty() {
            cmd.arg(format!("video_encoder={encoder}"));
        }
        Ok(Self { scrcpy: cmd.spawn()? })
    }

    /// scrcpy accepts video first, then audio, on the same abstract socket.
    fn connect(&self) -> io::Result<UnixStream> {
        let addr = SocketAddr::from_abstract_name(format!("scrcpy_{SCRCPY_SCID}"))?;
        let mut last = io::Error::other("scrcpy did not listen");
        for _ in 0..100 {
            match UnixStream::connect_addr(&addr) {
                Ok(s) => return Ok(s),
                Err(e) => last = e,
            }
            thread::sleep(Duration::from_millis(100));
        }
        Err(last)
    }
}

impl Drop for Android {
    fn drop(&mut self) {
        let _ = self.scrcpy.kill();
        let _ = self.scrcpy.wait();
        sh(&["wm", "size", "reset"]);
        sh(&["svc", "power", "stayon", "false"]);
    }
}

/// Drop to shell (uid 2000) with adbd's groups: shell holds the capture/surface/audio-routing permissions.
fn as_shell(cmd: &mut Command) {
    unsafe {
        cmd.pre_exec(|| {
            if libc::getuid() != 0 {
                return Ok(()); // --selftest from adb shell: already shell
            }
            let groups: [libc::gid_t; 13] = [1004, 1007, 1011, 1015, 1028, 1078, 1079, 3001, 3002, 3003, 3006, 3009, 3011];
            if libc::setgroups(groups.len() as _, groups.as_ptr()) != 0 || libc::setgid(2000) != 0 || libc::setuid(2000) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

fn night_mode(on: bool) {
    thread::spawn(move || sh(&["cmd", "uimode", "night", if on { "yes" } else { "no" }]));
}

fn sh(args: &[&str]) {
    match Command::new(args[0]).args(&args[1..]).status() {
        Ok(s) if s.success() => {}
        r => warn!("{args:?}: {r:?}"),
    }
}

const SW_ENCODER: &str = "c2.android.avc.encoder";

/// Starts scrcpy and waits for the H.264 config (SPS/PPS). The configured encoder (default: scrcpy's pick,
/// the MTK OMX HW one) gets 5 s; if it produces nothing, retry with the software encoder.
/// Force one with persist.vendor.carplay.encoder (or CATPLAY_ENCODER).
fn capture(w: u32, h: u32, fps: u32) -> Res<(Android, UnixStream, UnixStream, Packet)> {
    let configured = std::env::var("CATPLAY_ENCODER").unwrap_or_else(|_| prop("persist.vendor.carplay.encoder", ""));
    let encoders = if configured == SW_ENCODER { vec![SW_ENCODER] } else { vec![configured.as_str(), SW_ENCODER] };
    for enc in encoders {
        let android = Android::start(w, h, fps, enc)?;
        let mut video = android.connect()?;
        let audio = android.connect()?;
        video.set_read_timeout(Some(Duration::from_secs(5)))?;
        match read_packet(&mut video) {
            Ok(p) if p.config => {
                video.set_read_timeout(None)?;
                info!("H.264 encoder '{}' OK", if enc.is_empty() { "default" } else { enc });
                return Ok((android, video, audio, p));
            }
            r => warn!("encoder '{enc}' gave no H.264 config ({:?})", r.err()),
        }
    }
    Err("no working H.264 encoder".into())
}

struct Packet {
    config: bool,
    key: bool,
    data: Vec<u8>,
}

/// scrcpy frame meta: be64 pts|flags (bit62 config, bit61 keyframe), be32 size, payload (Annex-B / PCM s16le).
fn read_packet(s: &mut UnixStream) -> io::Result<Packet> {
    let mut hdr = [0u8; 12];
    s.read_exact(&mut hdr)?;
    let flags = u64::from_be_bytes(hdr[..8].try_into().unwrap());
    let mut data = vec![0u8; u32::from_be_bytes(hdr[8..].try_into().unwrap()) as usize];
    s.read_exact(&mut data)?;
    Ok(Packet { config: flags & (1 << 62) != 0, key: flags & (1 << 61) != 0, data })
}

// struct uhid_event (packed): u32 type + union, largest member uhid_create2_req = 4372 bytes.
const UHID_EVENT_SIZE: usize = 4376;
const UHID_CREATE2: u32 = 11;
const UHID_INPUT2: u32 = 12;

fn uhid_create(name: &str, desc: &[u8]) -> io::Result<File> {
    if desc.len() > 4096 {
        return Err(io::Error::other("HID descriptor too large"));
    }
    let mut f = OpenOptions::new().read(true).write(true).open("/dev/uhid")?;
    let mut ev = vec![0u8; UHID_EVENT_SIZE];
    ev[0..4].copy_from_slice(&UHID_CREATE2.to_ne_bytes());
    let n = name.len().min(127);
    ev[4..4 + n].copy_from_slice(&name.as_bytes()[..n]); // name[128]
    ev[132..139].copy_from_slice(b"carplay"); // phys[64]
    ev[260..262].copy_from_slice(&(desc.len() as u16).to_ne_bytes()); // rd_size
    ev[262..264].copy_from_slice(&3u16.to_ne_bytes()); // bus = BUS_USB
    ev[264..268].copy_from_slice(&UHID_VENDOR.to_ne_bytes());
    ev[268..272].copy_from_slice(&UHID_PRODUCT.to_ne_bytes());
    ev[280..280 + desc.len()].copy_from_slice(desc); // rd_data
    f.write_all(&ev)?;
    Ok(f)
}

fn uhid_input(f: &mut File, report: &[u8]) -> io::Result<()> {
    let n = report.len().min(4096);
    let mut ev = vec![0u8; 6 + n];
    ev[0..4].copy_from_slice(&UHID_INPUT2.to_ne_bytes());
    ev[4..6].copy_from_slice(&(n as u16).to_ne_bytes());
    ev[6..].copy_from_slice(&report[..n]);
    f.write_all(&ev)
}

fn prop(name: &str, default: &str) -> String {
    let v = Command::new("/system/bin/getprop").arg(name).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    match v {
        Ok(v) if !v.is_empty() => v,
        _ => default.to_string(),
    }
}

/// Everything except the car: uhid touchscreen + scrcpy capture at WxH. Safe to run from `adb shell`
/// (no UDC/USB changes). Usage: catplay_android --selftest [800x480]
fn selftest(w: u32, h: u32) -> Res<()> {
    let name = "catplay selftest touch";
    let mut touch = uhid_create(name, &catplay_hid::TouchScreenSingleReport::descriptor(w as u16, h as u16))?;
    thread::sleep(Duration::from_millis(500));
    // hover report (tip switch off): exercises the path without clicking anything
    uhid_input(&mut touch, &catplay_hid::TouchScreenSingleReport::new(false, w as u16 / 2, h as u16 / 2).to_bytes())?;
    let devices = std::fs::read_to_string("/proc/bus/input/devices").unwrap_or_default();
    if !devices.contains(name) {
        return Err("uhid device did not show up in /proc/bus/input/devices".into());
    }
    info!("selftest: uhid touchscreen OK");

    let (android, mut video, mut audio, first) = capture(w, h, 30)?;
    info!("selftest: SPS/PPS {} bytes, starts {:02x?}", first.data.len(), &first.data[..first.data.len().min(8)]);
    let audio_pkts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let ap = audio_pkts.clone();
    thread::spawn(move || {
        while let Ok(p) = read_packet(&mut audio) {
            if !p.config {
                ap.fetch_add(1, Ordering::Relaxed);
            }
        }
    });
    video.set_read_timeout(Some(Duration::from_secs(5)))?;
    let (mut config, mut keys, mut frames, mut bytes) = (1, 0, 0, 0); // 1: the config capture() already read
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(5) {
        let Ok(p) = read_packet(&mut video) else { break };
        if p.config {
            config += 1;
            info!("selftest: SPS/PPS {} bytes, starts {:02x?}", p.data.len(), &p.data[..p.data.len().min(8)]);
        } else {
            frames += 1;
            keys += p.key as u32;
            bytes += p.data.len();
        }
    }
    let secs = t0.elapsed().as_secs_f32();
    info!(
        "selftest: video {frames} frames ({keys} key, {config} config) {:.1} fps {:.0} kbit/s; audio {} packets",
        frames as f32 / secs,
        bytes as f32 * 8.0 / 1000.0 / secs,
        audio_pkts.load(Ordering::Relaxed)
    );
    drop(android);
    if config == 0 || keys == 0 {
        return Err("no H.264 config/keyframe from scrcpy (static screen gives few frames, but config+IDR must arrive)".into());
    }
    info!("selftest: PASS");
    Ok(())
}
