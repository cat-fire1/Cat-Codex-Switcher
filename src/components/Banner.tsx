import React from "react";
import { AlertTriangle, X } from "lucide-react";

interface BannerProps {
  message: string | null;
  onDismiss: () => void;
}

export const Banner: React.FC<BannerProps> = ({ message, onDismiss }) => {
  if (!message) return null;

  return (
    <div className="mx-5 my-3 p-3 rounded-xl bg-rose-500/10 border border-rose-500/20 text-rose-300 flex items-start justify-between gap-3 animate-in fade-in slide-in-from-top-2 duration-200">
      <div className="flex items-start gap-2.5">
        <AlertTriangle className="w-4 h-4 text-rose-400 mt-0.5 flex-shrink-0" />
        <div className="text-xs space-y-1">
          {message.split("\n").map((line, idx) => (
            <p key={idx} className="leading-snug">
              {line}
            </p>
          ))}
        </div>
      </div>
      <button
        onClick={onDismiss}
        className="p-1 rounded-lg text-rose-400/80 hover:text-rose-200 hover:bg-rose-500/20 transition flex-shrink-0"
      >
        <X className="w-3.5 h-3.5" />
      </button>
    </div>
  );
};
