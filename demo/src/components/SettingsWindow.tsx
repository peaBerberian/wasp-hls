import * as React from "react";
import WaspHlsPlayer, { PlayerState } from "../../../src";
import { VariantInfo } from "../../../src/ts-main";
import SpeedSetting from "./SpeedSetting";
import VariantSetting from "./VariantSetting";

function SettingsWindow({ player }: { player: WaspHlsPlayer }): JSX.Element {
  const [speed, setSpeed] = React.useState(player.getSpeed());
  const [isAutoVariant, setIsAutoVariant] = React.useState(
    player.getLockedVariant() === null
  );
  const [variant, setVariant] = React.useState<VariantInfo | undefined>(
    player.getCurrentVariant()
  );
  const [variantsList, setVariantsList] = React.useState<VariantInfo[]>(
    player.getVariantsList()
  );

  React.useEffect(() => {
    if (speed !== player.getSpeed()) {
      player.setSpeed(speed);
    }
  }, [speed, player]);

  React.useEffect(() => {
    player.addEventListener("variantUpdate", onVariantUpdate);
    player.addEventListener("variantsListUpdate", onVariantsListUpdate);
    player.addEventListener("playerStateChange", onPlayerStateChange);

    setVariantsList(player.getVariantsList());
    setVariant(player.getCurrentVariant());
    setIsAutoVariant(player.getLockedVariant() === null);
    setSpeed(player.getSpeed());
    return () => {
      player.removeEventListener("variantUpdate", onVariantUpdate);
      player.removeEventListener("variantsListUpdate", onVariantsListUpdate);
      player.removeEventListener("playerStateChange", onPlayerStateChange);
    };

    function onVariantUpdate(v: VariantInfo | undefined) {
      setVariant(v);
    }

    function onVariantsListUpdate(vl: VariantInfo[]) {
      setVariantsList(vl);
    }

    function onPlayerStateChange(playerState: PlayerState): void {
      switch (playerState) {
        case PlayerState.Loading:
        case PlayerState.Stopped:
        case PlayerState.Error:
          setVariant(undefined);
          setVariantsList([]);
          break;
      }
    }
  }, [player]);

  const updateVariant = React.useCallback(
    (v: VariantInfo | undefined) => {
      if (v === undefined) {
        player.unlockVariant();
        setIsAutoVariant(true);
      } else {
        player.lockVariant(v.id);
        setIsAutoVariant(false);
      }
    },
    [player]
  );

  return (
    <div className="settings visible">
      <VariantSetting
        variant={variant}
        variantsList={variantsList}
        isAuto={isAutoVariant}
        updateVariant={updateVariant}
      />
      <SpeedSetting speed={speed} updateSpeed={setSpeed} />
    </div>
  );
}

export default React.memo(SettingsWindow);
