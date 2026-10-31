package com.amazon.sheldon.btmouse;

import android.app.AlarmManager;
import android.app.PendingIntent;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothClass;
import android.bluetooth.BluetoothDevice;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.hardware.input.InputDevice;
import android.hardware.input.InputManager;
import android.os.SystemClock;
import android.util.Log;

/**
 * Fire-TV-style auto pairing for Bluetooth mice.
 *
 * A self-chaining alarm loop runs for a 15-min window on boot (or when
 * triggered via adb: am broadcast -a com.amazon.sheldon.btmouse.START).
 * Each cycle starts a ~12s inquiry scan; any discovered mouse-class
 * peripheral that is not bonded gets createBond(). The chain stops early
 * if a usable pointer already exists (USB mouse plugged, BT mouse
 * connected, or a strict-mouse-class device already bonded).
 *
 * The mouse must be in pairing mode (usually a held button) during the window.
 */
public class BtMouseAutoPair extends BroadcastReceiver {
    private static final String TAG = "BtMouseAutoPair";
    static final String ACTION_START = "com.amazon.sheldon.btmouse.START";
    static final String ACTION_CYCLE = "com.amazon.sheldon.btmouse.CYCLE";
    static final String EXTRA_DEADLINE = "deadline";
    private static final long WINDOW_MS = 15 * 60 * 1000;
    private static final long SCAN_EVERY_MS = 16 * 1000; // ~12s inquiry + gap

    @Override
    public void onReceive(Context ctx, Intent intent) {
        String action = intent.getAction();
        if (action == null) return;
        switch (action) {
            case Intent.ACTION_BOOT_COMPLETED:
            case ACTION_START:
                schedule(ctx, SystemClock.elapsedRealtime() + WINDOW_MS);
                break;
            case BluetoothAdapter.ACTION_STATE_CHANGED:
                if (intent.getIntExtra(BluetoothAdapter.EXTRA_STATE, -1)
                        == BluetoothAdapter.STATE_ON) {
                    schedule(ctx, SystemClock.elapsedRealtime() + WINDOW_MS);
                }
                break;
            case ACTION_CYCLE: {
                long deadline = intent.getLongExtra(EXTRA_DEADLINE, 0);
                if (SystemClock.elapsedRealtime() > deadline) return;
                if (hasUsablePointer(ctx)) return; // done: no more cycles
                BluetoothAdapter ad = BluetoothAdapter.getDefaultAdapter();
                if (ad != null && ad.isEnabled()) {
                    try {
                        ad.startDiscovery();
                    } catch (SecurityException e) {
                        Log.e(TAG, "missing BLUETOOTH_SCAN grant; aborting", e);
                        return;
                    }
                    schedule(ctx, deadline);
                }
                break;
            }
            case BluetoothDevice.ACTION_FOUND: {
                BluetoothDevice d = intent.getParcelableExtra(BluetoothDevice.EXTRA_DEVICE);
                BluetoothClass cls = intent.getParcelableExtra(BluetoothDevice.EXTRA_CLASS);
                if (d == null || cls == null || !isPairableClass(cls)
                        || d.getBondState() != BluetoothDevice.BOND_NONE) return;
                try {
                    Log.i(TAG, "auto-bonding pointer: " + d.getName());
                    d.createBond();
                } catch (SecurityException e) {
                    Log.e(TAG, "missing BLUETOOTH_CONNECT grant", e);
                }
                break;
            }
            default:
                break;
        }
    }

    /**
     * True if a pointer is already usable: any connected mouse (USB or BT,
     * via InputManager) or a strict mouse-class device already bonded (the
     * stack reconnects bonded devices on its own).
     */
    private static boolean hasUsablePointer(Context ctx) {
        for (int id : ctx.getSystemService(InputManager.class).getInputDeviceIds()) {
            InputDevice dev = InputDevice.getDevice(id);
            if (dev != null && !dev.isVirtual()
                    && (dev.getSources() & InputDevice.SOURCE_MOUSE) != 0) {
                Log.i(TAG, "pointer already connected: " + dev.getName());
                return true;
            }
        }
        try {
            BluetoothAdapter ad = BluetoothAdapter.getDefaultAdapter();
            if (ad != null) {
                for (BluetoothDevice d : ad.getBondedDevices()) {
                    // ponytail: strict CoD filter (POINTING only) so a bonded
                    // Fire remote / keyboard does NOT stop the loop; loosen if
                    // real mice with odd CoDs ever need to block it.
                    BluetoothClass c = d.getBluetoothClass();
                    if (c != null && c.getDeviceClass()
                            == BluetoothClass.Device.PERIPHERAL_POINTING) {
                        Log.i(TAG, "mouse already bonded: " + d.getName());
                        return true;
                    }
                }
            }
        } catch (SecurityException e) {
            Log.w(TAG, "cannot check bonded devices: " + e);
        }
        return false;
    }

    /** CoDs we are willing to auto-bond: mice, mouse+kbd combos, generic
     *  non-keyboard peripherals (some mice report this while pairing). */
    private static boolean isPairableClass(BluetoothClass cls) {
        if (cls.getMajorDeviceClass() != BluetoothClass.Device.Major.PERIPHERAL) return false;
        int dc = cls.getDeviceClass();
        return dc == BluetoothClass.Device.PERIPHERAL_POINTING
                || dc == BluetoothClass.Device.PERIPHERAL_KEYBOARD_POINTING
                || dc == BluetoothClass.Device.PERIPHERAL_NON_KEYBOARD_NON_POINTING;
    }

    private static void schedule(Context ctx, long deadline) {
        AlarmManager am = ctx.getSystemService(AlarmManager.class);
        Intent i = new Intent(ctx, BtMouseAutoPair.class)
                .setAction(ACTION_CYCLE)
                .putExtra(EXTRA_DEADLINE, deadline);
        PendingIntent pi = PendingIntent.getBroadcast(ctx, 0, i, PendingIntent.FLAG_IMMUTABLE);
        am.set(AlarmManager.ELAPSED_REALTIME,
                SystemClock.elapsedRealtime() + SCAN_EVERY_MS, pi);
    }
}
