import * as React from "react";
import { VariantInfo } from "wasp-hls";

/**
 * @param {Object} props
 * @returns {Object}
 */
function VariantSetting({
  variant,
  variantList,
  isAuto,
  updateVariant,
}: {
  variant: VariantInfo | undefined;
  variantList: VariantInfo[];
  isAuto: boolean;
  updateVariant: (v: VariantInfo | undefined) => void;
}): JSX.Element | null {
  const onSelectChange = React.useCallback(
    (evt: React.SyntheticEvent<HTMLSelectElement>) => {
      if (variantList.length < 2) {
        return;
      }
      const index = +(evt.target as HTMLSelectElement).value;
      if (index === 0) {
        updateVariant(undefined);
      } else if (index > 0) {
        const selected = variantList[index - 1];
        updateVariant(selected);
      }
    },
    [variantList]
  );

  const selectedIndex =
    variantList.length < 2 || isAuto
      ? 0
      : variantList.findIndex((v) => v.id === variant?.id) + 1;

  const optionsEl = React.useMemo(() => {
    const variantChoices = variantList.map((v, index) => {
      return (
        <option key={v.id} value={index + 1}>
          {getVariantLine(v)}
        </option>
      );
    });
    if (variantList.length < 2) {
      return variantChoices;
    }
    const autoString =
      isAuto && variant !== undefined
        ? "auto (" + getVariantLine(variant) + ")"
        : "auto";
    return [
      <option key={0} value={0}>
        {autoString}
      </option>,
      variantChoices,
    ];
  }, [isAuto, variant, variantList]);

  if (variantList.length < 1) {
    return null;
  }

  return (
    <div className="video-setting variant-setting">
      <span className="setting-name">{"Quality"}</span>
      <select
        disabled={variantList.length < 2}
        aria-label="Update the current playback variant"
        className="setting-value"
        onChange={onSelectChange}
        value={selectedIndex || 0}
      >
        {optionsEl}
      </select>
    </div>
  );
}

export default React.memo(VariantSetting);

function getVariantLine(variant: VariantInfo): string {
  const info = [];
  if (variant.height !== undefined) {
    info.push(`${variant.height}p`);
  }
  if (variant.bandwidth !== undefined) {
    info.push(`${Math.round(variant.bandwidth / 1000)}kbps`);
  }
  if (info.length === 0) {
    info.push("id: " + String(variant.id));
  }
  return info.join(", ");
}
