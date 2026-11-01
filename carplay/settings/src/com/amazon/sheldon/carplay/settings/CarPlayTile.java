package com.amazon.sheldon.carplay.settings;

import android.service.quicksettings.Tile;
import android.service.quicksettings.TileService;

/** Quick Settings tile: same toggle as the Settings switch. */
public class CarPlayTile extends TileService {
    @Override
    public void onStartListening() {
        refresh(CarPlay.on());
    }

    @Override
    public void onClick() {
        boolean on = !CarPlay.on();
        CarPlay.set(on);
        refresh(on);
    }

    private void refresh(boolean on) {
        Tile t = getQsTile();
        if (t == null) return;
        t.setState(on ? Tile.STATE_ACTIVE : Tile.STATE_INACTIVE);
        t.updateTile();
    }
}
