import React from "react";
import { Clock } from "lucide-react";

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
    if (val > 50) return "from-emerald-500 to-teal-400";
    if (val > 20) return "from-amber-500 to-yellow-400";
    return "from-rose-500 to-red-400";
  };

  const getTextColor = (val: number) => {
    if (val > 50) return "text-emerald-400";
    if (val > 20) return "text-amber-400";
    return "text-rose-400";
  };

  return (
    <div className="space-y-1.5">
      <div className="flex justify-between items-center text-xs">
        <span className="text-zinc-400 font-medium">{label}</span>
        <span className={`font-semibold ${getTextColor(clamped)}`}>{clamped}%</span>
      </div>

      <div className="h-2 w-full bg-zinc-800/80 rounded-full overflow-hidden p-[1px] border border-white/5">
        <div
          className={`h-full rounded-full bg-gradient-to-r ${getBarColor(
            clamped
          )} transition-all duration-500 ease-out`}
          style={{ width: `${clamped}%` }}
        />
      </div>

      <div className="flex items-center gap-1 text-[11px] text-zinc-500 font-mono">
        <Clock className="w-3 h-3 text-zinc-500" />
        <span>{formatCountdown(resetsAt)}</span>
      </div>
    </div>
  );
};
