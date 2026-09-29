export const defaultStaleAfterDays = 90;
export const staleAfterDayOptions = [30, 90, 180, 365] as const;
export const minimumStaleAfterDays = 1;
export const maximumStaleAfterDays = 3_650;
export const dataHealthStaleTime = 30_000;

export const bytesPerKilobyte = 1_024;
export const byteDisplayPrecision = 1;

export const customThresholdValue = 'custom';
export const staleAfterLabelId = 'stale-after-label';
export const staleAfterSelectMinWidth = 160;
export const customThresholdInputWidth = 110;
export const summaryCardMinWidth = 190;
export const dataHealthExtensionContextVersion = 1;
export const dataHealthExtensionOutlet = 'data_health_card';
export const byteUnits = ['KB', 'MB', 'GB', 'TB'] as const;
