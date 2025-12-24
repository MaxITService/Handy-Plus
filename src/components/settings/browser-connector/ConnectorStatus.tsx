import React, { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { Wifi, WifiOff, Server } from "lucide-react";
import { SettingContainer } from "../../ui/SettingContainer";

// Types matching Rust backend
type ExtensionStatus = "online" | "offline" | "unknown";

interface ConnectorStatusResponse {
  status: ExtensionStatus;
  last_poll_at: number;
  server_running: boolean;
  port: number;
}

interface ConnectorStatusIndicatorProps {
  grouped?: boolean;
  descriptionMode?: "inline" | "tooltip" | "none";
}

/**
 * Format a timestamp into a human-readable "time ago" string
 */
function formatTimeAgo(timestamp: number, t: (key: string) => string): string {
  if (timestamp === 0) {
    return t("settings.browserConnector.status.never");
  }

  const now = Date.now();
  const diffMs = now - timestamp;
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);
  const diffHour = Math.floor(diffMin / 60);

  if (diffSec < 10) {
    return t("settings.browserConnector.status.justNow");
  } else if (diffSec < 60) {
    return t("settings.browserConnector.status.secondsAgo", { count: diffSec });
  } else if (diffMin < 60) {
    return t("settings.browserConnector.status.minutesAgo", { count: diffMin });
  } else {
    return t("settings.browserConnector.status.hoursAgo", { count: diffHour });
  }
}

export const ConnectorStatusIndicator: React.FC<ConnectorStatusIndicatorProps> = ({
  grouped = false,
  descriptionMode = "tooltip",
}) => {
  const { t } = useTranslation();
  const [status, setStatus] = useState<ConnectorStatusResponse | null>(null);
  const [lastSeenText, setLastSeenText] = useState<string>("");

  // Fetch status from backend
  const fetchStatus = useCallback(async () => {
    try {
      const result = await invoke<ConnectorStatusResponse>("connector_get_status");
      setStatus(result);
    } catch (error) {
      console.error("Failed to get connector status:", error);
    }
  }, []);

  // Update "last seen" text periodically
  useEffect(() => {
    if (status && status.status === "offline" && status.last_poll_at > 0) {
      setLastSeenText(formatTimeAgo(status.last_poll_at, t));

      const interval = setInterval(() => {
        setLastSeenText(formatTimeAgo(status.last_poll_at, t));
      }, 10000); // Update every 10 seconds

      return () => clearInterval(interval);
    } else {
      setLastSeenText("");
    }
  }, [status, t]);

  // Initial fetch and periodic polling
  useEffect(() => {
    fetchStatus();

    const interval = setInterval(fetchStatus, 5000); // Poll every 5 seconds

    return () => clearInterval(interval);
  }, [fetchStatus]);

  // Listen for status change events from backend
  useEffect(() => {
    const unlisten = listen<ExtensionStatus>("extension-status-changed", (event) => {
      // Refetch full status when event received
      fetchStatus();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [fetchStatus]);

  const getStatusColor = (): string => {
    if (!status || !status.server_running) {
      return "text-gray-400";
    }
    switch (status.status) {
      case "online":
        return "text-green-500";
      case "offline":
        return "text-red-500";
      default:
        return "text-yellow-500";
    }
  };

  const getStatusIcon = () => {
    if (!status || !status.server_running) {
      return <Server className="w-5 h-5 text-gray-400" />;
    }
    if (status.status === "online") {
      return <Wifi className={`w-5 h-5 ${getStatusColor()}`} />;
    }
    return <WifiOff className={`w-5 h-5 ${getStatusColor()}`} />;
  };

  const getStatusText = (): string => {
    if (!status) {
      return t("settings.browserConnector.status.loading");
    }
    if (!status.server_running) {
      return t("settings.browserConnector.status.serverStopped");
    }
    switch (status.status) {
      case "online":
        return t("settings.browserConnector.status.online");
      case "offline":
        return t("settings.browserConnector.status.offline");
      default:
        return t("settings.browserConnector.status.waiting");
    }
  };

  const getStatusBadgeClass = (): string => {
    if (!status || !status.server_running) {
      return "bg-gray-500/20 border-gray-500/30";
    }
    switch (status.status) {
      case "online":
        return "bg-green-500/20 border-green-500/30";
      case "offline":
        return "bg-red-500/20 border-red-500/30";
      default:
        return "bg-yellow-500/20 border-yellow-500/30";
    }
  };

  return (
    <SettingContainer
      title={t("settings.browserConnector.status.title")}
      description={t("settings.browserConnector.status.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
    >
      <div className="flex items-center gap-3">
        <div
          className={`flex items-center gap-2 px-3 py-1.5 rounded-lg border ${getStatusBadgeClass()}`}
        >
          {getStatusIcon()}
          <span className={`text-sm font-medium ${getStatusColor()}`}>
            {getStatusText()}
          </span>
        </div>

        {/* Show last seen time when offline */}
        {status?.status === "offline" && lastSeenText && (
          <span className="text-xs text-text/50">
            {t("settings.browserConnector.status.lastSeen", { time: lastSeenText })}
          </span>
        )}

        {/* Show port info when server is running */}
        {status?.server_running && (
          <span className="text-xs text-text/40">
            {t("settings.browserConnector.status.port", { port: status.port })}
          </span>
        )}
      </div>
    </SettingContainer>
  );
};
