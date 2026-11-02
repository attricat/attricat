const bytesPerKilobyte = 1_024;
const byteDisplayPrecision = 1;
const byteUnits = ['KB', 'MB', 'GB', 'TB'] as const;

export const formatBytes = (bytes: number) => {
  if (bytes < bytesPerKilobyte) return `${bytes} B`;
  const unit = Math.min(
    Math.floor(Math.log(bytes) / Math.log(bytesPerKilobyte)),
    byteUnits.length,
  );
  return `${(bytes / bytesPerKilobyte ** unit).toFixed(byteDisplayPrecision)} ${byteUnits[unit - 1]}`;
};
