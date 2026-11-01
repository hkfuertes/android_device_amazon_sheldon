package com.amazon.sheldon.carplay.settings;

import android.os.SystemProperties;

final class CarPlay {
    static final String PROP = "persist.vendor.carplay.enable";

    static boolean on() {
        return SystemProperties.getBoolean(PROP, false);
    }

    // ponytail: SELinux is permissive on sheldon; under enforcing this needs a set_prop rule for the app domain
    static void set(boolean on) {
        SystemProperties.set(PROP, on ? "1" : "0");
    }
}
