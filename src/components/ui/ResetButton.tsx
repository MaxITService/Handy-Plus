import React from "react";
import ResetIcon from "../icons/ResetIcon";

interface ResetButtonProps {
  onClick: () => void;
  disabled?: boolean;
  className?: string;
  ariaLabel?: string;
  title?: string;
  children?: React.ReactNode;
}

export const ResetButton: React.FC<ResetButtonProps> = React.memo(
  ({ onClick, disabled = false, className = "", ariaLabel, title, children }) => (
    <button
      type="button"
      aria-label={ariaLabel}
      title={title}
      className={`p-1.5 rounded-md border border-transparent transition-all duration-200 ${
        disabled
          ? "opacity-40 cursor-not-allowed text-[#4a4a4a]"
          : "hover:bg-[#9b5de5]/20 active:bg-[#9b5de5]/30 active:translate-y-[1px] hover:cursor-pointer hover:border-[#9b5de5]/50 text-[#b8b8b8] hover:text-[#9b5de5]"
      } ${className}`}
      onClick={onClick}
      disabled={disabled}
    >
      {children ?? <ResetIcon />}
    </button>
  ),
);
