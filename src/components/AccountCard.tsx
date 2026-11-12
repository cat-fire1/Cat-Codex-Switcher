import React from "react";
import {
  ArrowSwap16Regular,
  Delete16Regular,
  Flash16Regular,
  Warning16Regular,
} from "@fluentui/react-icons";
import { Row } from "../types";
import { UsageBar } from "./UsageBar";
import { getT } from "../lib/i18n";

interface AccountCardProps {
  row: Row;
  isActive: boolean;
  showFiveHour: boolean;
  language: string;
  onSwitch: (id: string) => void;
  onRemove: (id: string) => void;
}

export const AccountCard: React.FC<AccountCardProps> = ({
  row,
  isActive,
  showFiveHour,
  language,
  onSwitch,
  onRemove,
}) => {
  const t = getT(language);
  const { profile, usage, stale, title, error } = row;

  // Extract avatar initials (first 2 chars)
  const initials = title.slice(0, 2).toUpperCase();

  return (
    <div
      className={`rounded-lg p-3.5 transition-all duration-150 border select-none ${
        isActive
          ? "fluent-card-active"
          : "fluent-card"
      }`}
    >
      {/* Top Header: Avatar, Name, AccountID, Active Badge or Actions */}
      <div className="flex items-start justify-between gap-3 mb-2.5">
        <div className="flex items-center gap-2.5 min-w-0">
          {/* Fluent Persona Avatar */}
          <div
            className={`w-8 h-8 rounded-full flex-shrink-0 flex items-center justify-center font-semibold text-[11px] tracking-wider shadow-sm ${
              isActive
                ? "bg-[#ff7a45] text-white"
                : "bg-white/10 text-zinc-300 border border-white/10"
            }`}
          >
            {initials}
          </div>

          <div className="min-w-0">
            <div className="flex items-center gap-1.5">
              <span className="font-semibold text-[13px] text-white truncate leading-tight">
                {title}
              </span>
              {stale && (
                <span
                  title={t.staleTooltip}
                  className="w-1.5 h-1.5 rounded-full bg-amber-400 flex-shrink-0 animate-pulse"
                />
              )}
            </div>

            <div className="flex items-center gap-2 mt-0.5 text-[11px] text-zinc-400">
              {profile.accountID && (
                <span className="font-mono text-[10px] text-zinc-500 truncate max-w-[160px]">
                  {profile.accountID}
                </span>
              )}
              {usage?.resetCreditsRemaining !== undefined &&
                usage.resetCreditsRemaining !== null &&
                usage.resetCreditsRemaining > 0 && (
                  <span className="inline-flex items-center gap-1 text-[10px] text-amber-300 bg-amber-400/10 px-1.5 py-0.5 rounded border border-amber-400/20">
                    <Flash16Regular className="w-3 h-3 text-amber-400" />
                    {usage.resetCreditsRemaining} {t.resetCredits}
                  </span>
                )}
            </div>
          </div>
        </div>

        {/* Status / Buttons */}
        <div className="flex items-center gap-1 flex-shrink-0">
          {isActive ? (
            <span className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-[11px] font-medium bg-[#107c41]/20 text-[#6ccb5f] border border-[#107c41]/30">
              <span className="w-1.5 h-1.5 rounded-full bg-[#6ccb5f]" />
              {t.activeBadge}
            </span>
          ) : (
            <div className="flex items-center gap-1">
              <button
                onClick={() => onSwitch(profile.id)}
                className="fluent-btn-standard px-2.5 py-1 flex items-center gap-1"
              >
                <ArrowSwap16Regular className="w-3.5 h-3.5" />
                <span>{t.switchBtn}</span>
              </button>
              <button
                onClick={() => onRemove(profile.id)}
                className="w-7 h-7 flex items-center justify-center rounded text-zinc-400 hover:text-red-400 hover:bg-white/5 active:bg-white/10 transition-colors"
                title={t.removeBtn}
              >
                <Delete16Regular className="w-3.5 h-3.5" />
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Error state if limits failed */}
      {error && (
        <div className="flex items-start gap-2 p-2 mb-2 rounded bg-red-500/10 border border-red-500/20 text-red-300 text-xs">
          <Warning16Regular className="w-4 h-4 flex-shrink-0 text-red-400 mt-0.5" />
          <p className="line-clamp-2">{error}</p>
        </div>
      )}

      {/* Usage Bars: compact, no empty gap */}
      {usage && (
        <div className="pt-2 border-t border-[rgba(255,255,255,0.06)] space-y-1">
          <UsageBar
            label={t.weeklyLimit}
            percent={usage.remainingPercent}
            resetsAt={usage.resetsAt}
            resetsInText={t.resetsIn}
            unknownText={t.resetsUnknown}
          />

          {showFiveHour && usage.fiveHourRemainingPercent !== null && (
            <UsageBar
              label={t.fiveHourLimit}
              percent={usage.fiveHourRemainingPercent}
              resetsAt={usage.fiveHourResetsAt}
              resetsInText={t.resetsIn}
              unknownText={t.resetsUnknown}
            />
          )}
        </div>
      )}
    </div>
  );
};
