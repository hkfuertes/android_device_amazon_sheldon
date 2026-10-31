package com.amazon.sheldon.carplay;

import android.media.AudioAttributes;
import android.media.AudioFormat;
import android.media.AudioManager;
import android.media.AudioRecordingConfiguration;
import android.media.AudioTrack;
import android.media.IAudioService;
import android.media.MediaRecorder;
import android.media.audiopolicy.AudioMix;
import android.media.audiopolicy.AudioMixingRule;
import android.media.audiopolicy.AudioPolicy;
import android.os.Looper;
import android.os.ServiceManager;

import java.io.InputStream;
import java.lang.reflect.Method;

/**
 * Makes the car's microphone the Android microphone while CarPlay runs.
 *
 * An AudioPolicy loopback mix (the CTS "injection" pattern, backed by the r_submix HAL) captures every
 * AudioRecord using a mic-like source; we feed it the car mic PCM read from stdin (s16le, 24 kHz, mono).
 * stdout reports "REC 1" / "REC 0" when some app starts/stops recording, so the daemon can open the
 * duplex (telephony) audio stream with the car only when needed. EOF on stdin = unregister and exit.
 *
 * Usage (shell uid, has MODIFY_AUDIO_ROUTING):
 *   CLASSPATH=/system/framework/CarplayMic.jar app_process /system/bin com.amazon.sheldon.carplay.Mic
 */
public final class Mic {
    private static final int RATE = 24000;
    private static final int[] SOURCES = {
        MediaRecorder.AudioSource.DEFAULT,
        MediaRecorder.AudioSource.MIC,
        MediaRecorder.AudioSource.VOICE_RECOGNITION,
        MediaRecorder.AudioSource.VOICE_COMMUNICATION,
        MediaRecorder.AudioSource.CAMCORDER,
    };

    public static void main(String[] args) throws Exception {
        Looper.prepareMainLooper(); // AudioPolicy wants a looper; we never need its callbacks

        AudioMixingRule.Builder rule = new AudioMixingRule.Builder();
        for (int s : SOURCES) {
            rule.addRule(new AudioAttributes.Builder().setCapturePreset(s).build(),
                    AudioMixingRule.RULE_MATCH_ATTRIBUTE_CAPTURE_PRESET);
        }
        AudioFormat format = new AudioFormat.Builder()
                .setSampleRate(RATE)
                .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                .build();
        AudioMix mix = new AudioMix.Builder(rule.build())
                .setFormat(format)
                .setRouteFlags(AudioMix.ROUTE_FLAG_LOOP_BACK)
                .build();
        AudioPolicy policy = new AudioPolicy.Builder(null).addMix(mix).build(); // null context: perms via AMS

        Method register = AudioManager.class.getDeclaredMethod("registerAudioPolicyStatic", AudioPolicy.class);
        register.setAccessible(true);
        if ((int) register.invoke(null, policy) != AudioManager.SUCCESS) {
            System.err.println("registerAudioPolicy failed");
            System.exit(1);
        }
        AudioTrack track = policy.createAudioTrackSource(mix);
        track.play();
        System.err.println("car mic injection ready");

        IAudioService audio = IAudioService.Stub.asInterface(ServiceManager.getService(Context_AUDIO));
        Thread watcher = new Thread(() -> {
            boolean last = false;
            while (true) {
                boolean rec = false;
                try {
                    for (AudioRecordingConfiguration c : audio.getActiveRecordingConfigurations()) {
                        for (int s : SOURCES) {
                            rec |= c.getClientAudioSource() == s;
                        }
                    }
                } catch (Exception e) {
                    System.err.println("getActiveRecordingConfigurations: " + e);
                }
                if (rec != last) {
                    System.out.println(rec ? "REC 1" : "REC 0");
                    System.out.flush();
                    last = rec;
                }
                try {
                    Thread.sleep(300);
                } catch (InterruptedException e) {
                    return;
                }
            }
        });
        watcher.setDaemon(true);
        watcher.start();

        InputStream in = System.in;
        byte[] buf = new byte[RATE / 50 * 2]; // 20 ms
        int n;
        while ((n = in.read(buf)) > 0) {
            track.write(buf, 0, n);
        }
        track.stop();
        AudioManager.class.getDeclaredMethod("unregisterAudioPolicyAsyncStatic", AudioPolicy.class)
                .invoke(null, policy);
        System.exit(0);
    }

    private static final String Context_AUDIO = "audio"; // Context.AUDIO_SERVICE
}
