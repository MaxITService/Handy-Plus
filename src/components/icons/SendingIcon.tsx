import React from "react";

interface SendingIconProps {
  width?: number;
  height?: number;
  color?: string;
  className?: string;
}

const SendingIcon: React.FC<SendingIconProps> = ({
  width = 24,
  height = 24,
  color = "#FAA2CA",
  className = "",
}) => {
  return (
    <svg
      width={width}
      height={height}
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
    >
      {/* Upload arrow icon */}
      <path
        d="M12 4L12 16"
        stroke={color}
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M7 9L12 4L17 9"
        stroke={color}
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M5 20H19"
        stroke={color}
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
};

export default SendingIcon;
