import React from "react";
import largeLogoUrl from "../../assets/large_logo.jpg";

const HandyTextLogo = ({
  width,
  height,
  className,
}: {
  width?: number;
  height?: number;
  className?: string;
}) => {
  const resolvedWidth = width ?? (height ? undefined : 300);

  return (
    <img
      src={largeLogoUrl}
      alt="AivoRelay"
      width={resolvedWidth}
      height={height}
      className={className}
    />
  );
};

export default HandyTextLogo;
