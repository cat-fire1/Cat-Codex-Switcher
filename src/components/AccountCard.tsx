import React from "react";
import { Check, Trash2, ArrowRightLeft, AlertCircle, Zap } from "lucide-react";
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

  // Extract avatar initials
  const initials = title.slice(0, 2).toUpperCase();

  return (
    <div
      className={`relative rounded-2xl p-4 transition-all duration-200 border ${
        isActive
          ? "bg-[#181c26] border-orange-500/40 shadow-xl shadow-orange-500/10 ring-1 ring-orange-500/30"
          : "bg-[#14161f] border-white/5 hover:border-white/10 hover:bg-[#171a24]"
      }`}
    >
      {/* Top row: Avatar, Info, Badges */}
      <div className="flex items-start justify-between gap-3 mb-3.5">
        <div className="flex items-center gap-3">
          <div
            className={`w-10 h-10 rounded-xl flex items-center justify-center font-bold text-sm tracking-wider shadow-inner ${
              isActive
                ? "bg-gradient-to-br from-orange-500 to-amber-600 text-white shadow-orange-500/20"
                : "bg-zinc-800 text-zinc-300"
            }`}
          >
            {initials}
          </div>

          <div>
            <div className="flex items-center gap-2">
              <h3 className="font-semibold text-sm text-white tracking-tight">{title}</h3>
              {stale && (
                <span
                  title={t.staleTooltip}
                  className="w-2 h-2 rounded-full bg-amber-400 ring-4 ring-amber-400/20 animate-pulse"
                />
              )}
            </div>

            <div className="flex items-center gap-2 mt-0.5 text-xs text-zinc-400">
              {profile.accountID && (
                <span className="font-mono text-[11px] text-zinc-500">
                  {profile.accountID}
                </span>
              )}
              {usage?.resetCreditsRemaining !== undefined &&
                usage.resetCreditsRemaining !== null &&
                usage.resetCreditsRemaining > 0 && (
                  <span className="flex items-center gap-0.5 text-[11px] text-amber-400 bg-amber-400/10 px-1.5 py-0.5 rounded-full border border-amber-400/20">
                    <Zap className="w-2.5 h-2.5" />
                    {usage.resetCreditsRemaining} {t.resetCredits}
                  </span>
                )}
            </div>
          </div>
        </div>

        {/* Status / Active Badge */}
        <div className="flex items-center gap-2">
          {isActive ? (
            <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-semibold bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
              {t.activeBadge}
            </span>
          ) : (
            <div className="flex items-center gap-1">
              <button
                onClick={() => onSwitch(profile.id)}
                className="flex items-center gap-1 px-3 py-1 rounded-lg text-xs font-medium bg-white/5 hover:bg-orange-500 hover:text-white text-zinc-300 border border-white/10 hover:border-orange-500 transition active:scale-95 shadow-sm"
              >
                <ArrowRightLeft className="w-3 h-3" />
                {t.switchBtn}
              </button>
              <button
                onClick={() => onRemove(profile.id)}
                className="p-1.5 rounded-lg text-zinc-500 hover:text-rose-400 hover:bg-rose-500/10 transition active:scale-95"
                title={t.removeBtn}
              >
                <Trash2 className="w-3.5 h-3.5" />
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Error state if profile limits failed */}
      {error && (
        <div className="flex items-start gap-2 p-2.5 mb-3 rounded-lg bg-rose-500/10 border border-rose-500/20 text-rose-300 text-xs">
          <AlertCircle className="w-4 h-4 flex-shrink-0 mt-0.5 text-rose-400" />
          <p className="line-clamp-2">{error}</p>
        </div>
      )}

      {/* Usage Bars */}
      {usage && (
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-2 border-t border-white/5">
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
