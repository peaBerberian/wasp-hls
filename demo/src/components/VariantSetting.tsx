import * as React from "react";
import { VariantInfo } from "../../../src/ts-main";

/**
 * @param {Object} props
 * @returns {Object}
 */
function VariantSetting({
  variant,
  variantsList,
  isAuto,
  updateVariant,
}: {
  variant: VariantInfo | undefined;
  variantsList: VariantInfo[];
  isAuto: boolean;
  updateVariant: (v: VariantInfo | undefined) => void;
}): JSX.Element | null {
  if (variantsList.length < 1) {
    return null;
  }
  const onSelectChange = React.useCallback((
    evt: React.SyntheticEvent<HTMLSelectElement>
  ) => {
    if (variantsList.length < 2) {
      return;
    }
    const index = +(evt.target as HTMLSelectElement).value;
    if (index === 0) {
      updateVariant(undefined);
    } else if (index > 0) {
      const selected = variantsList[index - 1];
      updateVariant(selected);
    }
  }, [variantsList]);

  const selectedIndex = variantsList.length < 2 || isAuto ?
    0 :
    variantsList.findIndex(v => v.id === variant?.id) + 1;

  const optionsEl = React.useMemo(() => {
    const variantChoices = variantsList.map((v, index) => {
      return <option key={index + 1} value={index + 1}>
        {getVariantLine(v)}
      </option>;
    });
    if (variantsList.length < 2) {
      return variantChoices;
    }
    const autoString = isAuto && variant !== undefined ?
      "auto (" + getVariantLine(variant) + ")" :
      "auto";
    return [
      <option key={0} value={0}>
        {autoString}
      </option>,
      variantChoices,
    ];
  }, [isAuto, variant, variantsList]);

  return (
    <div className="video-setting variant-setting">
      <span className="setting-name" >
        {"Quality"}
      </span>
      <select
        disabled={variantsList.length < 2}
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
  if (variant.height !== 0) {
    info.push(`${variant.height}p`);
  }
  if (variant.bandwidth !== 0) {
    info.push(`${Math.round(variant.bandwidth / 1000)}kbps`);
  }
  if (info.length === 0) {
    info.push("id: " + String(variant.id));
  }
  return info.join(", ");
}
