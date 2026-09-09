import { byteDisplayPrecision, bytesPerKilobyte } from './constants';

export const formatBytes = (bytes: number) => {
  if (bytes < bytesPerKilobyte) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  const unit = Math.min(
    Math.floor(Math.log(bytes) / Math.log(bytesPerKilobyte)),
    units.length,
  );
  return `${(bytes / bytesPerKilobyte ** unit).toFixed(byteDisplayPrecision)} ${units[unit - 1]}`;
};
