export interface Profile {
  id: string;
  displayName: string;
  email: string | null;
  accountID: string | null;
  userID?: string | null;
  createdAt: string;
  lastUsedAt: string | null;
}

export interface Credit {
  id: string;
  status: string;
  resetType: string;
  expiresAt: string | null;
}

export interface Usage {
  remainingPercent: number;
  resetsAt: string | null;
  fiveHourRemainingPercent: number | null;
  fiveHourResetsAt: string | null;
  resetCreditsRemaining: number | null;
  resetCredits: Credit[] | null;
}

export interface Row {
  profile: Profile;
  title: string;
  usage: Usage | null;
  stale: boolean;
  fetchedAt: string | null;
  error: string | null;
  subscription: [string, string] | null;
}

export interface Settings {
  formatVersion: number;
  language: string;
  showsMenuBarPercentage: boolean;
  showsFiveHourUsage: boolean;
  statusBarUsageWindow: string;
  resetExpiryExperimentEnabled: boolean;
  launchAtLogin: boolean;
  automaticallyCheckUpdates: boolean;
  updateRepository: string;
}

export interface ImportPreview {
  titles: string[];
  duplicates: number;
  source: string;
}

export interface Snapshot {
  rows: Row[];
  active: string | null;
  settings: Settings;
  busy: boolean;
  adding: boolean;
  refreshing: boolean;
  error: string | null;
  preview: ImportPreview | null;
  updateUrl: string | null;
  updateStatus: string;
}
