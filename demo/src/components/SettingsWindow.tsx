import * as React from "react";
import WaspHlsPlayer, { PlayerState } from "../../../src";
import { AudioTrackInfo, VariantInfo } from "../../../src/ts-main";
import AudioTrackSetting from "./AudioTrackSetting";
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
  const [variantList, setVariantList] = React.useState<VariantInfo[]>(
    player.getVariantList()
  );
  const [audioTrack, setAudioTrack] = React.useState<
    AudioTrackInfo | undefined
  >(player.getAudioTrack());
  const [audioTrackList, setAudioTrackList] = React.useState<AudioTrackInfo[]>(
    player.getAudioTrackList()
  );

  React.useEffect(() => {
    if (speed !== player.getSpeed()) {
      player.setSpeed(speed);
    }
  }, [speed, player]);

  React.useEffect(() => {
    player.addEventListener("variantUpdate", onVariantUpdate);
    player.addEventListener("variantListUpdate", onVariantListUpdate);
    player.addEventListener("audioTrackUpdate", onAudioTrackUpdate);
    player.addEventListener("audioTrackListUpdate", onAudioTrackListUpdate);
    player.addEventListener("playerStateChange", onPlayerStateChange);

    setVariantList(player.getVariantList());
    setVariant(player.getCurrentVariant());
    setIsAutoVariant(player.getLockedVariant() === null);
    setSpeed(player.getSpeed());
    return () => {
      player.removeEventListener("variantUpdate", onVariantUpdate);
      player.removeEventListener("variantListUpdate", onVariantListUpdate);
      player.removeEventListener("playerStateChange", onPlayerStateChange);
    };

    function onVariantUpdate(v: VariantInfo | undefined) {
      setVariant(v);
    }

    function onVariantListUpdate(vl: VariantInfo[]) {
      setVariantList(vl);
    }

    function onAudioTrackUpdate(v: AudioTrackInfo | undefined) {
      setAudioTrack(v);
    }

    function onAudioTrackListUpdate(vl: AudioTrackInfo[]) {
      setAudioTrackList(vl);
    }

    function onPlayerStateChange(playerState: PlayerState): void {
      switch (playerState) {
        case PlayerState.Loading:
        case PlayerState.Stopped:
        case PlayerState.Error:
          setVariant(undefined);
          setVariantList([]);
          setAudioTrack(undefined);
          setAudioTrackList([]);
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

  const updateAudioTrack = React.useCallback(
    (t: AudioTrackInfo) => {
      player.setAudioTrack(t.id);
      setAudioTrack(t);
    },
    [player]
  );

  return (
    <div className="settings visible">
      <AudioTrackSetting
        audioTrack={audioTrack}
        audioTrackList={audioTrackList}
        isAuto={isAutoVariant}
        updateAudioTrack={updateAudioTrack}
      />
      <VariantSetting
        variant={variant}
        variantList={variantList}
        isAuto={isAutoVariant}
        updateVariant={updateVariant}
      />
      <SpeedSetting speed={speed} updateSpeed={setSpeed} />
    </div>
  );
}

export default React.memo(SettingsWindow);
