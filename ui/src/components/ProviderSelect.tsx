import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Check, ChevronDown } from "lucide-react";
import type { ProviderSummary } from "../api";
import { useTranslation } from "../i18n";

export type ProviderSelectProps = {
  providers: ProviderSummary[];
  selectedProviderId: string;
  onChange?: (providerId: string) => void;
  defaultProvider?: ProviderSummary;
  workspacePath?: string;
  ariaLabel: string;
  ariaDescribedBy?: string;
  emptyOptionLabel?: string;
  allowEmptyOption?: boolean;
  className?: string;
  disabled?: boolean;
};

type ProviderOption = {
  id: string;
  name: string;
};

export function ProviderSelect({
  providers,
  selectedProviderId,
  onChange,
  defaultProvider,
  workspacePath,
  ariaLabel,
  ariaDescribedBy,
  emptyOptionLabel,
  allowEmptyOption = false,
  className,
  disabled = false,
}: ProviderSelectProps) {
  const t = useTranslation();
  const controlId = `provider-select-${useId().replace(/:/g, "")}`;
  const listboxId = `${controlId}-options`;
  const containerRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const options: ProviderOption[] = [
    ...(allowEmptyOption
      ? [{ id: "", name: emptyOptionLabel ?? `${t("useDefault")}${defaultProvider ? ` · ${defaultProvider.name}` : " route"}` }]
      : []),
    ...providers.map((provider) => ({ id: provider.id, name: provider.name })),
  ];
  const selectedIndex = Math.max(0, options.findIndex((option) => option.id === selectedProviderId));
  const [value, setValue] = useState(selectedProviderId);
  const [activeIndex, setActiveIndex] = useState(selectedIndex);
  const [open, setOpen] = useState(false);
  const selectedOption = options.find((option) => option.id === value)
    ?? (!value && emptyOptionLabel ? { id: "", name: emptyOptionLabel } : options[0])
    ?? { id: "", name: t("chooseProvider") };
  const isDisabled = disabled || providers.length === 0 || !onChange;

  useEffect(() => {
    setValue(selectedProviderId);
  }, [selectedProviderId]);

  useEffect(() => {
    setActiveIndex((index) => Math.min(index, Math.max(options.length - 1, 0)));
  }, [options.length]);

  useEffect(() => {
    if (!open) return;

    const handlePointerDown = (event: MouseEvent) => {
      if (!containerRef.current?.contains(event.target as Node)) setOpen(false);
    };

    document.addEventListener("mousedown", handlePointerDown);
    return () => document.removeEventListener("mousedown", handlePointerDown);
  }, [open]);

  const openMenu = (index = selectedIndex) => {
    if (isDisabled || options.length === 0) return;
    setActiveIndex(index);
    setOpen(true);
  };

  const closeMenu = (restoreFocus = false) => {
    setOpen(false);
    if (restoreFocus) requestAnimationFrame(() => triggerRef.current?.focus());
  };

  const selectOption = (optionId: string) => {
    setValue(optionId);
    setOpen(false);
    onChange?.(optionId);
    requestAnimationFrame(() => triggerRef.current?.focus());
  };

  const handleTriggerKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (isDisabled) return;

    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) {
        openMenu(selectedIndex);
      } else {
        const delta = event.key === "ArrowDown" ? 1 : -1;
        setActiveIndex((index) => (index + delta + options.length) % options.length);
      }
      return;
    }
    if (event.key === "Home" && open) {
      event.preventDefault();
      setActiveIndex(0);
      return;
    }
    if (event.key === "End" && open) {
      event.preventDefault();
      setActiveIndex(options.length - 1);
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (open) selectOption(options[activeIndex].id);
      else openMenu();
      return;
    }
    if (event.key === "Escape" && open) {
      event.preventDefault();
      closeMenu(true);
    }
  };

  return (
    <div className={`provider-select${className ? ` ${className}` : ""}`} ref={containerRef}>
      <button
        ref={triggerRef}
        className="provider-select-trigger"
        type="button"
        aria-label={workspacePath ? `${ariaLabel} for ${workspacePath}` : ariaLabel}
        aria-describedby={ariaDescribedBy}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listboxId}
        aria-activedescendant={open ? `${listboxId}-option-${activeIndex}` : undefined}
        disabled={isDisabled}
        onClick={() => (open ? closeMenu() : openMenu())}
        onKeyDown={handleTriggerKeyDown}
      >
        <span className="provider-select-trigger-label">{selectedOption.name}</span>
        <ChevronDown size={16} aria-hidden="true" />
      </button>
      {open && (
        <div className="provider-select-menu" id={listboxId} role="listbox" aria-label={ariaLabel}>
          {options.map((option, index) => {
            const isSelected = option.id === value;
            return (
              <button
                className="provider-select-option"
                type="button"
                role="option"
                aria-selected={isSelected}
                id={`${listboxId}-option-${index}`}
                data-active={index === activeIndex || undefined}
                key={option.id}
                onMouseEnter={() => setActiveIndex(index)}
                onClick={() => selectOption(option.id)}
              >
                <span className="provider-select-option-check" aria-hidden="true">{isSelected ? <Check size={15} /> : null}</span>
                <span>{option.name}</span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
