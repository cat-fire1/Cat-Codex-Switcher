import React from "react";
import { Warning16Regular, Dismiss16Regular } from "@fluentui/react-icons";

interface BannerProps {
  message: string | null;
  onDismiss: () => void;
}

export const Banner: React.FC<BannerProps> = ({ message, onDismiss }) => {
  if (!message) return null;

  return (
    <div className="mx-4 my-2.5 p-2.5 rounded bg-red-500/10 border border-red-500/25 text-red-200 flex items-start justify-between gap-2.5 text-xs">
      <div className="flex items-start gap-2">
        <Warning16Regular className="w-4 h-4 text-red-400 mt-0.5 flex-shrink-0" />
        <div className="space-y-0.5">
          {message.split("\n").map((line, idx) => (
            <p key={idx} className="leading-snug">
              {line}
            </p>
          ))}
        </div>
      </div>
      <button
        onClick={onDismiss}
        className="p-1 rounded text-zinc-400 hover:text-white hover:bg-white/10 transition-colors flex-shrink-0"
      >
        <Dismiss16Regular className="w-3.5 h-3.5" />
      </button>
    </div>
  );
};
