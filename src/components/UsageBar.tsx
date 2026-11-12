import React from "react";
import { Timer16Regular } from "@fluentui/react-icons";

interface UsageBarProps {
  label: string;
  percent: number;
  resetsAt: string | null;
  resetsInText: string;
  unknownText: string;
}

export const UsageBar: React.FC<UsageBarProps> = ({
  label,
  percent,
  resetsAt,
  resetsInText,
  unknownText,
}) => {
  const clamped = Math.max(0, Math.min(100, percent));

  const formatCountdown = (dateStr: string | null) => {
    if (!dateStr) return unknownText;
    const target = new Date(dateStr).getTime();
    const now = Date.now();
    const diff = target - now;
    if (diff <= 0) return "Зараз / Now";

    const hours = Math.floor(diff / (1000 * 60 * 60));
    const mins = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));
    const days = Math.floor(hours / 24);

    if (days > 0) {
      return `${resetsInText} ${days}д ${hours % 24}г`;
    }
    if (hours > 0) {
      return `${resetsInText} ${hours}г ${mins}хв`;
    }
    return `${resetsInText} ${mins}хв`;
  };

  const getBarColor = (val: number) => {
    if (val > 50) return "bg-[#6ccb5f]";
    if (val > 20) return "bg-[#f59e0b]";
    return "bg-[#f87171]";
  };

  const getTextColor = (val: number) => {
    if (val > 50) return "text-[#6ccb5f]";
    if (val > 20) return "text-amber-400";
    return "text-red-400";
  };

  return (
    <div className="py-1">
      {/* Label and Countdown info */}
      <div className="flex items-center justify-between text-[11px] mb-1.5">
        <span className="text-zinc-300 font-medium">{label}</span>
        <div className="flex items-center gap-1.5">
          <span className="text-[10px] text-zinc-400 flex items-center gap-1 font-mono">
            <Timer16Regular className="w-3 h-3 text-zinc-500" />
            {formatCountdown(resetsAt)}
          </span>
          <span className={`font-semibold text-xs min-w-[32px] text-right ${getTextColor(clamped)}`}>
            {clamped}%
          </span>
        </div>
      </div>

      {/* WinUI 3 styled ProgressBar */}
      <div className="h-1.5 w-full bg-white/10 rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-300 ease-out ${getBarColor(
            clamped
          )}`}
          style={{ width: `${clamped}%` }}
        />
      </div>
    </div>
  );
};
