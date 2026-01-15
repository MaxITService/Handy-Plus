import React, { useState, useMemo, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  Plus,
  Trash2,
  ChevronDown,
  ChevronUp,
  Globe,
  Check,
  Play,
  RefreshCw,
} from "lucide-react";
import { commands, TranscriptionProfile } from "@/bindings";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { Button } from "../ui/Button";
import { Input } from "../ui/Input";
import { Dropdown } from "../ui/Dropdown";
import { Badge } from "../ui/Badge";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { HandyShortcut } from "./HandyShortcut";
import { useSettings } from "../../hooks/useSettings";
import { useModels } from "../../hooks/useModels";
import { LANGUAGES } from "../../lib/constants/languages";
import { getModelPromptInfo } from "./TranscriptionSystemPrompt";

interface ExtendedTranscriptionProfile extends TranscriptionProfile {
  include_in_cycle: boolean;
  push_to_talk: boolean;
  stt_prompt_override_enabled: boolean;
}

interface ProfileCardProps {
  profile: ExtendedTranscriptionProfile;
  isExpanded: boolean;
  onToggleExpand: () => void;
  onUpdate: (profile: ExtendedTranscriptionProfile) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  canDelete: boolean;
  promptLimit: number;
  isActive: boolean;
  onSetActive: (id: string) => Promise<void>;
}

const ProfileCard: React.FC<ProfileCardProps> = ({
  profile,
  isExpanded,
  onToggleExpand,
  onUpdate,
  onDelete,
  canDelete,
  promptLimit,
  isActive,
  onSetActive,
}) => {
  const { t } = useTranslation();
  const [isUpdating, setIsUpdating] = useState(false);

  const bindingId = `transcribe_${profile.id}`;

  const languageLabel = useMemo(() => {
    const lang = LANGUAGES.find((l) => l.value === profile.language);
    return lang?.label || t("settings.general.language.auto");
  }, [profile.language, t]);

  const promptLength = (profile.system_prompt || "").length;
  const isOverLimit = promptLimit > 0 && promptLength > promptLimit;

  // Instant update handlers
  const handleNameChange = async (newName: string) => {
    const trimmed = newName.trim();
    if (!trimmed || trimmed === profile.name) return;
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, name: trimmed });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleLanguageChange = async (newLanguage: string) => {
    if (newLanguage === profile.language) return;
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, language: newLanguage });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleTranslateChange = async (newTranslate: boolean) => {
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, translate_to_english: newTranslate });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleSystemPromptChange = async (newPrompt: string) => {
    if (newPrompt === (profile.system_prompt || "")) return;
    if (promptLimit > 0 && newPrompt.length > promptLimit) return;
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, system_prompt: newPrompt });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleIncludeInCycleChange = async (newValue: boolean) => {
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, include_in_cycle: newValue });
    } finally {
      setIsUpdating(false);
    }
  };

  const handlePushToTalkChange = async (newValue: boolean) => {
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, push_to_talk: newValue });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleSttPromptOverrideChange = async (newValue: boolean) => {
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, stt_prompt_override_enabled: newValue });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleDelete = async () => {
    setIsUpdating(true);
    try {
      await onDelete(profile.id);
    } finally {
      setIsUpdating(false);
    }
  };

  return (
    <div
      className={`min-w-0 border rounded-lg transition-colors ${isActive ? "border-purple-500/50 bg-purple-500/5" : "border-mid-gray/30 bg-background/50"}`}
    >
      {/* Header - always visible */}
      <div
        className="flex items-center justify-between gap-3 px-4 py-3 cursor-pointer hover:bg-mid-gray/5 transition-colors"
        onClick={onToggleExpand}
      >
        <div className="flex items-center gap-3 min-w-0">
          <Globe
            className={`w-4 h-4 ${isActive ? "text-purple-400" : "text-logo-primary"}`}
          />
          <div className="flex flex-col min-w-0">
            <div className="flex flex-wrap items-center gap-2 min-w-0">
              <span className="font-medium text-sm break-words">
                {profile.name}
              </span>
              {isActive && (
                <Badge
                  variant="secondary"
                  className="bg-purple-500/20 text-purple-400 border-purple-500/30 text-[10px] px-1.5 py-0 max-w-full truncate"
                >
                  {t("settings.transcriptionProfiles.active")}
                </Badge>
              )}
            </div>
            <span className="text-xs text-mid-gray break-words">
              {languageLabel}
              {profile.translate_to_english && (
                <span className="text-purple-400 ml-1">→ EN</span>
              )}
            </span>
          </div>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <Button
            onClick={(e) => {
              e.stopPropagation();
              onSetActive(profile.id);
            }}
            disabled={isActive || isUpdating}
            variant={isActive ? "secondary" : "primary"}
            size="sm"
            className={isActive ? "opacity-100 cursor-default" : ""}
            title={isActive ? t("settings.transcriptionProfiles.active") : undefined}
          >
            {isActive ? (
              <Check className="w-4 h-4" />
            ) : (
              <span className="text-xs">
                {t("settings.transcriptionProfiles.setActive")}
              </span>
            )}
          </Button>
          {canDelete && (
            <Button
              onClick={(e) => {
                e.stopPropagation();
                handleDelete();
              }}
              variant="secondary"
              size="sm"
              disabled={isUpdating}
              className="text-red-400 hover:text-red-300 hover:border-red-400/50 p-1.5"
            >
              <Trash2 className="w-3.5 h-3.5" />
            </Button>
          )}
          {isExpanded ? (
            <ChevronUp className="w-4 h-4 text-mid-gray" />
          ) : (
            <ChevronDown className="w-4 h-4 text-mid-gray" />
          )}
        </div>
      </div>

      {/* Expanded content */}
      {isExpanded && (
        <div className="px-4 pb-4 pt-3 border-t border-mid-gray/20 space-y-3">
          {/* Cycle & Push-to-Talk Controls */}
          <div className="grid gap-3 md:grid-cols-2 bg-mid-gray/5 p-3 rounded-lg border border-mid-gray/10">
            <div className="min-w-0">
              <label className="text-xs font-semibold text-text/70 block mb-2">
                {t("settings.transcriptionProfiles.includeInCycle")}
              </label>
              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={profile.include_in_cycle}
                  onChange={(e) => handleIncludeInCycleChange(e.target.checked)}
                  disabled={isUpdating}
                  className="w-4 h-4 rounded border-mid-gray bg-background text-purple-500 focus:ring-purple-500/50"
                />
                <span className="text-xs text-mid-gray leading-snug">
                  {t(
                    "settings.transcriptionProfiles.includeInCycleDescription",
                  )}
                </span>
              </div>
            </div>

            <div className="min-w-0">
              <label className="text-xs font-semibold text-text/70 block mb-2">
                {t("settings.general.pushToTalk.label")}
              </label>
              <div className="flex items-center gap-2">
                <ToggleSwitch
                  checked={profile.push_to_talk ?? true}
                  onChange={handlePushToTalkChange}
                  disabled={isUpdating}
                />
                <span className="text-xs text-mid-gray leading-snug">
                  {t("settings.general.pushToTalk.description")}
                </span>
              </div>
            </div>
          </div>

          <div className="grid gap-3 lg:grid-cols-2">
            {/* Shortcut */}
            <div className="space-y-2 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.shortcut")}
              </label>
              <HandyShortcut shortcutId={bindingId} grouped={true} />
            </div>

            {/* Profile Name */}
            <div className="space-y-2 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.profileName")}
              </label>
              <Input
                type="text"
                defaultValue={profile.name}
                onBlur={(e) => handleNameChange(e.target.value)}
                placeholder={t(
                  "settings.transcriptionProfiles.profileNamePlaceholder",
                )}
                variant="compact"
                disabled={isUpdating}
              />
            </div>

            {/* Language Selection */}
            <div className="space-y-2 relative z-20 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.language")}
              </label>
              <Dropdown
                selectedValue={profile.language}
                options={LANGUAGES.map((l) => ({
                  value: l.value,
                  label: l.label,
                }))}
                onSelect={(value) => value && handleLanguageChange(value)}
                placeholder={t("settings.general.language.auto")}
                disabled={isUpdating}
              />
            </div>

            {/* Translate to English Toggle */}
            <div className="space-y-2 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.translateToEnglish")}
              </label>
              <div className="flex flex-col gap-2 rounded-md border border-mid-gray/10 bg-mid-gray/5 px-3 py-2 sm:flex-row sm:items-center sm:justify-between">
                <p className="text-xs text-mid-gray leading-snug">
                  {t(
                    "settings.transcriptionProfiles.translateToEnglishDescription",
                  )}
                </p>
                <button
                  type="button"
                  onClick={() =>
                    handleTranslateChange(!profile.translate_to_english)
                  }
                  disabled={isUpdating}
                  className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors shrink-0 ${
                    profile.translate_to_english
                      ? "bg-purple-500"
                      : "bg-mid-gray/30"
                  } ${isUpdating ? "opacity-50 cursor-not-allowed" : "cursor-pointer"}`}
                >
                  <span
                    className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                      profile.translate_to_english
                        ? "translate-x-6"
                        : "translate-x-1"
                    }`}
                  />
                </button>
              </div>
            </div>
          </div>

          {/* System Prompt Override */}
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.overrideSystemPrompt")}
              </label>
              <ToggleSwitch
                checked={profile.stt_prompt_override_enabled ?? false}
                onChange={handleSttPromptOverrideChange}
                disabled={isUpdating}
              />
            </div>
            <p className="text-xs text-mid-gray">
              {profile.stt_prompt_override_enabled
                ? t("settings.transcriptionProfiles.overrideSystemPromptOnDescription")
                : t("settings.transcriptionProfiles.overrideSystemPromptOffDescription")}
            </p>
            {profile.stt_prompt_override_enabled && (
              <>
                <div className="flex items-center justify-between">
                  <label className="text-xs font-semibold text-text/70">
                    {t("settings.transcriptionProfiles.systemPrompt")}
                  </label>
                  <span
                    className={`text-xs ${isOverLimit ? "text-red-400" : "text-mid-gray"}`}
                  >
                    {promptLength}
                    {promptLimit > 0 && ` / ${promptLimit}`}
                  </span>
                </div>
                <textarea
                  defaultValue={profile.system_prompt || ""}
                  onBlur={(e) => handleSystemPromptChange(e.target.value)}
                  placeholder={t(
                    "settings.transcriptionProfiles.systemPromptPlaceholder",
                  )}
                  disabled={isUpdating}
                  rows={3}
                  className={`w-full px-3 py-2 text-sm bg-[#1e1e1e]/80 border rounded-md resize-none transition-colors ${
                    isOverLimit
                      ? "border-red-400 focus:border-red-400"
                      : "border-[#3c3c3c] focus:border-[#4a4a4a]"
                  } ${isUpdating ? "opacity-40 cursor-not-allowed" : ""} text-[#e8e8e8] placeholder-[#6b6b6b]`}
                />
                <p className="text-xs text-mid-gray">
                  {t("settings.transcriptionProfiles.systemPromptDescription")}
                </p>
                {isOverLimit && (
                  <p className="text-xs text-red-400">
                    {t("settings.transcriptionProfiles.systemPromptTooLong", {
                      limit: promptLimit,
                    })}
                  </p>
                )}
              </>
            )}
          </div>

        </div>
      )}
    </div>
  );
};

export const TranscriptionProfiles: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings, updateSetting } = useSettings();
  const { getModelInfo } = useModels();
  const [expandedIds, setExpandedIds] = useState<Set<string>>(
    () => new Set(["default"]),
  );
  const [isCreating, setIsCreating] = useState(false);
  const [newName, setNewName] = useState("");
  const [newLanguage, setNewLanguage] = useState("auto");
  const [newTranslate, setNewTranslate] = useState(false);
  const [newSystemPrompt, setNewSystemPrompt] = useState("");
  const [newPushToTalk, setNewPushToTalk] = useState(true);

  const isExpanded = (id: string) => expandedIds.has(id);

  const toggleExpanded = (id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const profiles = (settings?.transcription_profiles ||
    []) as ExtendedTranscriptionProfile[];
  const activeProfileId = (settings as any)?.active_profile_id || "default";
  const overlayEnabled =
    (settings as any)?.profile_switch_overlay_enabled ?? true;

  // Listen for active profile changes from shortcuts
  useEffect(() => {
    const unlistenPromise = listen("active-profile-changed", () => {
      refreshSettings();
    });

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [refreshSettings]);

  // Compute the active model ID based on provider - used for prompt limits and prompt storage
  const activeModelId = useMemo(() => {
    const isRemote =
      settings?.transcription_provider === "remote_openai_compatible";
    return isRemote
      ? settings?.remote_stt?.model_id || ""
      : settings?.selected_model || "";
  }, [
    settings?.transcription_provider,
    settings?.remote_stt?.model_id,
    settings?.selected_model,
  ]);

  // Get model info for prompt configuration (same logic as TranscriptionSystemPrompt)
  const modelInfo = useMemo(() => {
    if (!activeModelId) {
      return { supportsPrompt: true, charLimit: 896, modelId: "" };
    }
    const isRemote = settings?.transcription_provider === "remote_openai_compatible";
    if (isRemote) {
      return getModelPromptInfo(activeModelId);
    }
    // For local models, get engine_type from model info
    const localModelInfo = getModelInfo(activeModelId);
    return getModelPromptInfo(activeModelId, localModelInfo?.engine_type);
  }, [activeModelId, settings?.transcription_provider, getModelInfo]);

  // Get prompt limit based on active transcription settings
  const promptLimit = modelInfo.supportsPrompt ? modelInfo.charLimit : 0;

  const newPromptLength = newSystemPrompt.length;
  const isNewPromptOverLimit = promptLimit > 0 && newPromptLength > promptLimit;

  const handleCreate = async () => {
    if (!newName.trim()) return;
    if (isNewPromptOverLimit) return;
    setIsCreating(true);
    try {
      const result = await commands.addTranscriptionProfile(
        newName.trim(),
        newLanguage,
        newTranslate,
        newSystemPrompt,
        newPushToTalk,
        null,
      );
      if (result.status === "ok") {
        await refreshSettings();
        setNewName("");
        setNewLanguage("auto");
        setNewTranslate(false);
        setNewSystemPrompt("");
        setNewPushToTalk(true);
        setExpandedIds((prev) => {
          const next = new Set(prev);
          next.add(result.data.id);
          return next;
        });
      }
    } catch (error) {
      console.error("Failed to create profile:", error);
    } finally {
      setIsCreating(false);
    }
  };

  const handleUpdate = async (profile: ExtendedTranscriptionProfile) => {
    try {
      await invoke("update_transcription_profile", {
        id: profile.id,
        name: profile.name,
        language: profile.language,
        translateToEnglish: profile.translate_to_english,
        systemPrompt: profile.system_prompt || "",
        sttPromptOverrideEnabled: profile.stt_prompt_override_enabled ?? false,
        includeInCycle: profile.include_in_cycle,
        pushToTalk: profile.push_to_talk,
        llmSettings: {
          enabled: profile.llm_post_process_enabled ?? false,
          promptOverride: profile.llm_prompt_override ?? null,
          modelOverride: profile.llm_model_override ?? null,
        },
      });
      await refreshSettings();
    } catch (error) {
      console.error("Failed to update profile:", error);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await commands.deleteTranscriptionProfile(id);
      await refreshSettings();
      setExpandedIds((prev) => {
        if (!prev.has(id)) return prev;
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    } catch (error) {
      console.error("Failed to delete profile:", error);
    }
  };

  const handleSetActive = async (id: string) => {
    try {
      await invoke("set_active_profile", { id });
      await refreshSettings();
    } catch (e) {
      console.error("Failed to set active profile", e);
    }
  };

  const handleOverlayChange = async (enabled: boolean) => {
    if (updateSetting) {
      await updateSetting("profile_switch_overlay_enabled" as any, enabled);
    }
  };

  return (
    <SettingsGroup title={t("settings.transcriptionProfiles.title")}>
      {/* Help text */}
      <SettingContainer
        title=""
        description=""
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="p-3 bg-purple-500/10 border border-purple-500/30 rounded-lg">
          <p className="text-sm text-text/80">
            {t("settings.transcriptionProfiles.help")}
          </p>
        </div>
      </SettingContainer>

      {/* Transcribe with Active Profile - MAIN SHORTCUT */}
      <SettingContainer
        title=""
        description=""
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="p-3 bg-gradient-to-r from-purple-500/15 to-pink-500/10 border border-purple-500/40 rounded-lg">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex flex-col gap-1 min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-sm font-semibold text-text">
                  {t("settings.transcriptionProfiles.transcribeActiveProfile")}
                </span>
                <Badge
                  variant="secondary"
                  className="bg-purple-500/20 text-purple-400 border-purple-500/30 text-[10px] px-1.5 py-0"
                >
                  {activeProfileId === "default"
                    ? t("settings.transcriptionProfiles.defaultProfile")
                    : profiles.find((p) => p.id === activeProfileId)?.name ||
                      activeProfileId}
                </Badge>
              </div>
              <span className="text-xs text-mid-gray">
                {t("settings.transcriptionProfiles.transcribeActiveProfileDescription")}
              </span>
            </div>
            <div className="shrink-0">
              <HandyShortcut shortcutId="transcribe" />
            </div>
          </div>
        </div>
      </SettingContainer>

      {/* General Settings: Cycle Shortcut & Overlay */}
      <SettingContainer
        title="General Settings"
        description=""
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="space-y-3">
          {/* Cycle Shortcut */}
          <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
            <div className="flex flex-col min-w-0">
              <span className="text-sm font-medium">
                {t("settings.transcriptionProfiles.includeInCycleDescription")}
              </span>
              <span className="text-xs text-mid-gray leading-snug">
                Global shortcut to cycle through active profiles
              </span>
            </div>
            <div className="shrink-0">
              <HandyShortcut shortcutId="cycle_profile" />
            </div>
          </div>

          {/* Overlay Toggle */}
          <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
            <div className="flex flex-col min-w-0">
              <span className="text-sm font-medium">
                {t("settings.transcriptionProfiles.showOverlayOnSwitch")}
              </span>
              <span className="text-xs text-mid-gray leading-snug">
                {t(
                  "settings.transcriptionProfiles.showOverlayOnSwitchDescription",
                )}
              </span>
            </div>
            <div className="shrink-0">
              <ToggleSwitch
                checked={overlayEnabled}
                onChange={handleOverlayChange}
              />
            </div>
          </div>
        </div>
      </SettingContainer>

      <SettingContainer
        title={t("settings.transcriptionProfiles.existingProfiles")}
        description=""
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="space-y-3">
          {/* Default Profile Card - Expandable with global settings */}
          <div
            className={`min-w-0 border rounded-lg transition-colors ${activeProfileId === "default" ? "border-purple-500/50 bg-purple-500/5" : "border-mid-gray/30 bg-background/50"}`}
          >
            {/* Header - clickable to expand */}
            <div
              className="flex items-center justify-between gap-3 px-4 py-3 cursor-pointer hover:bg-mid-gray/5 transition-colors"
              onClick={() => toggleExpanded("default")}
            >
              <div className="flex items-center gap-3 min-w-0">
                <RefreshCw
                  className={`w-4 h-4 ${activeProfileId === "default" ? "text-purple-400" : "text-mid-gray"}`}
                />
                <div className="flex flex-col min-w-0">
                  <div className="flex flex-wrap items-center gap-2 min-w-0">
                    <span className="font-medium text-sm break-words">
                      {t("settings.transcriptionProfiles.defaultProfile")}
                    </span>
                    {activeProfileId === "default" && (
                      <Badge
                        variant="secondary"
                        className="bg-purple-500/20 text-purple-400 border-purple-500/30 text-[10px] px-1.5 py-0"
                      >
                        {t("settings.transcriptionProfiles.active")}
                      </Badge>
                    )}
                  </div>
                  <span className="text-xs text-mid-gray break-words">
                    {t(
                      "settings.transcriptionProfiles.defaultProfileDescription",
                    )}
                  </span>
                </div>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                <Button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleSetActive("default");
                  }}
                  disabled={activeProfileId === "default"}
                  variant={
                    activeProfileId === "default" ? "secondary" : "primary"
                  }
                  size="sm"
                  className={
                    activeProfileId === "default"
                      ? "opacity-100 cursor-default"
                      : ""
                  }
                  title={activeProfileId === "default" ? t("settings.transcriptionProfiles.active") : undefined}
                >
                  {activeProfileId === "default" ? (
                    <Check className="w-4 h-4" />
                  ) : (
                    <span className="text-xs">
                      {t("settings.transcriptionProfiles.setActive")}
                    </span>
                  )}
                </Button>
                {isExpanded("default") ? (
                  <ChevronUp className="w-4 h-4 text-mid-gray" />
                ) : (
                  <ChevronDown className="w-4 h-4 text-mid-gray" />
                )}
              </div>
            </div>

            {/* Expanded content - Global settings */}
            {isExpanded("default") && (
              <div className="px-4 pb-4 pt-3 border-t border-mid-gray/20 space-y-3">
                {/* Optional Shortcut for Default profile */}
                <div className="space-y-2">
                  <label className="text-xs font-semibold text-text/70">
                    {t("settings.transcriptionProfiles.shortcut")}
                  </label>
                  <HandyShortcut shortcutId="transcribe_default" grouped={true} />
                </div>

                {/* Language & Push-to-Talk in a row */}
                <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                  <div className="space-y-2 min-w-0">
                    <label className="text-xs font-semibold text-text/70">
                      {t("settings.general.language.title")}
                    </label>
                    <Dropdown
                      selectedValue={settings?.selected_language || "auto"}
                      onSelect={(value: string) =>
                        updateSetting &&
                        updateSetting("selected_language" as any, value)
                      }
                      options={LANGUAGES.map((lang) => ({
                        value: lang.value,
                        label: lang.label,
                      }))}
                    />
                  </div>
                  <div className="space-y-2 min-w-0">
                    <label className="text-xs font-semibold text-text/70">
                      {t("settings.general.pushToTalk.label")}
                    </label>
                    <div className="flex items-start gap-2">
                      <ToggleSwitch
                        checked={settings?.push_to_talk ?? true}
                        onChange={(checked) =>
                          updateSetting &&
                          updateSetting("push_to_talk" as any, checked)
                        }
                      />
                      <span className="text-xs text-mid-gray leading-snug">
                        {t("settings.general.pushToTalk.description")}
                      </span>
                    </div>
                  </div>
                </div>

                {/* System Prompt - only show if model supports prompts */}
                {modelInfo.supportsPrompt && (
                  <div className="space-y-2">
                    <div className="flex items-center justify-between">
                      <label className="text-xs font-semibold text-text/70">
                        {t("settings.general.transcriptionSystemPrompt.title")}
                      </label>
                      {promptLimit > 0 && (
                        <span
                          className={`text-xs ${(settings?.transcription_prompts?.[activeModelId] || "").length > promptLimit ? "text-red-400" : "text-mid-gray"}`}
                        >
                          {
                            (
                              settings?.transcription_prompts?.[
                                activeModelId
                              ] || ""
                            ).length
                          }
                          /{promptLimit}
                        </span>
                      )}
                    </div>
                    <textarea
                      value={
                        settings?.transcription_prompts?.[
                          activeModelId
                        ] || ""
                      }
                      onChange={async (e) => {
                        if (activeModelId) {
                          await commands.changeTranscriptionPromptSetting(
                            activeModelId,
                            e.target.value,
                          );
                          refreshSettings();
                        }
                      }}
                      placeholder={t(
                        "settings.general.transcriptionSystemPrompt.placeholder",
                      )}
                      className="w-full h-20 px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm text-text placeholder-mid-gray/50 resize-none focus:outline-none focus:border-purple-500/50"
                    />
                  </div>
                )}
              </div>
            )}
          </div>

          {/* Custom Profiles */}
          {profiles.map((profile) => (
            <ProfileCard
              key={profile.id}
              profile={profile}
              isExpanded={isExpanded(profile.id)}
              onToggleExpand={() => toggleExpanded(profile.id)}
              onUpdate={handleUpdate}
              onDelete={handleDelete}
              canDelete={true}
              promptLimit={promptLimit}
              isActive={activeProfileId === profile.id}
              onSetActive={handleSetActive}
            />
          ))}
        </div>
      </SettingContainer>

      {/* Create new profile */}
      <SettingContainer
        title={t("settings.transcriptionProfiles.createNew")}
        description={t("settings.transcriptionProfiles.createNewDescription")}
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="space-y-3 p-3 border border-dashed border-mid-gray/30 rounded-lg overflow-visible">
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
            <div className="space-y-1 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.profileName")}
              </label>
              <Input
                type="text"
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                placeholder={t(
                  "settings.transcriptionProfiles.profileNamePlaceholder",
                )}
                variant="compact"
                disabled={isCreating}
              />
            </div>
            <div className="space-y-1 relative z-10 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.language")}
              </label>
              <Dropdown
                selectedValue={newLanguage}
                options={LANGUAGES.map((l) => ({
                  value: l.value,
                  label: l.label,
                }))}
                onSelect={(value) => value && setNewLanguage(value)}
                placeholder={t("settings.general.language.auto")}
                disabled={isCreating}
              />
            </div>
          </div>

          {/* System Prompt for new profile */}
          <div className="space-y-1">
            <div className="flex items-center justify-between">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.systemPrompt")}
              </label>
              <span
                className={`text-xs ${isNewPromptOverLimit ? "text-red-400" : "text-mid-gray"}`}
              >
                {newPromptLength}
                {promptLimit > 0 && ` / ${promptLimit}`}
              </span>
            </div>
            <textarea
              value={newSystemPrompt}
              onChange={(e) => setNewSystemPrompt(e.target.value)}
              placeholder={t(
                "settings.transcriptionProfiles.systemPromptPlaceholder",
              )}
              disabled={isCreating}
              rows={2}
              className={`w-full px-3 py-2 text-sm bg-[#1e1e1e]/80 border rounded-md resize-none transition-colors ${
                isNewPromptOverLimit
                  ? "border-red-400 focus:border-red-400"
                  : "border-[#3c3c3c] focus:border-[#4a4a4a]"
              } ${isCreating ? "opacity-40 cursor-not-allowed" : ""} text-[#e8e8e8] placeholder-[#6b6b6b]`}
            />
            {isNewPromptOverLimit && (
              <p className="text-xs text-red-400">
                {t("settings.transcriptionProfiles.systemPromptTooLong", {
                  limit: promptLimit,
                })}
              </p>
            )}
          </div>

          <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.translateToEnglish")}
              </label>
            </div>
            <button
              type="button"
              onClick={() => setNewTranslate(!newTranslate)}
              disabled={isCreating}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors shrink-0 ${
                newTranslate ? "bg-purple-500" : "bg-mid-gray/30"
              } ${isCreating ? "opacity-50 cursor-not-allowed" : "cursor-pointer"}`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  newTranslate ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>

          <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.general.pushToTalk.label")}
              </label>
            </div>
            <button
              type="button"
              onClick={() => setNewPushToTalk(!newPushToTalk)}
              disabled={isCreating}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors shrink-0 ${
                newPushToTalk ? "bg-purple-500" : "bg-mid-gray/30"
              } ${isCreating ? "opacity-50 cursor-not-allowed" : "cursor-pointer"}`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  newPushToTalk ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>

          {/* Create Button */}
          <div className="flex justify-end pt-1">
            <Button
              onClick={handleCreate}
              variant="primary"
              size="sm"
              disabled={!newName.trim() || isCreating || isNewPromptOverLimit}
              className="inline-flex items-center"
            >
              <Plus className="w-3.5 h-3.5 mr-1.5" />
              {t("settings.transcriptionProfiles.addProfile")}
            </Button>
          </div>
        </div>
      </SettingContainer>
    </SettingsGroup>
  );
};
