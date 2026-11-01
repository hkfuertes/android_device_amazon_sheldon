package com.amazon.sheldon.carplay.settings;

import com.android.settingslib.drawer.SwitchController;
import com.android.settingslib.drawer.SwitchesProvider;

import java.util.List;

/** "CarPlay (car mode)" switch injected into Settings > Connected devices, next to USB. */
public class CarPlaySwitches extends SwitchesProvider {
    @Override
    protected List<SwitchController> createSwitchControllers() {
        return List.of(new SwitchController() {
            @Override
            public String getSwitchKey() {
                return "carplay";
            }

            @Override
            protected MetaData getMetaData() {
                return new MetaData("com.android.settings.category.ia.device") // Connected devices
                        .setTitle(R.string.title)
                        .setSummary(R.string.summary)
                        .setIcon(R.drawable.ic_car)
                        .setOrder(-200);
            }

            @Override
            protected boolean isChecked() {
                return CarPlay.on();
            }

            @Override
            protected boolean onCheckedChanged(boolean checked) {
                CarPlay.set(checked);
                return true;
            }

            @Override
            protected String getErrorMessage(boolean attemptedChecked) {
                return null;
            }
        });
    }
}
