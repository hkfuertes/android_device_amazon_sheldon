package com.amazon.sheldon.carplay;

import android.view.SurfaceControl;

/**
 * Powers the physical (HDMI) display off/on at SurfaceFlinger level, like scrcpy --turn-screen-off.
 * Android still thinks the screen is on, so the car's virtual display keeps rendering, but the GPU stops
 * composing a 1080p frame nobody sees. Any sleep/wake cycle turns it back on.
 *
 * Usage (shell uid): CLASSPATH=/system/framework/CarplayMic.jar app_process / com.amazon.sheldon.carplay.Screen off|on
 */
public final class Screen {
    public static void main(String[] args) {
        int mode = "on".equals(args.length > 0 ? args[0] : "") ? SurfaceControl.POWER_MODE_NORMAL
                : SurfaceControl.POWER_MODE_OFF;
        for (long id : SurfaceControl.getPhysicalDisplayIds()) {
            SurfaceControl.setDisplayPowerMode(SurfaceControl.getPhysicalDisplayToken(id), mode);
        }
    }
}
