import { byteDisplayPrecision, bytesPerKilobyte, byteUnits } from './constants';

export const formatBytes = (bytes: number) => {
  if (bytes < bytesPerKilobyte) return `${bytes} B`;
  const units = byteUnits;
  const unit = Math.min(
    Math.floor(Math.log(bytes) / Math.log(bytesPerKilobyte)),
    units.length,
  );
  return `${(bytes / bytesPerKilobyte ** unit).toFixed(byteDisplayPrecision)} ${units[unit - 1]}`;
};
