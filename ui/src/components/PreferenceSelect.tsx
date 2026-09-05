import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Check, ChevronDown } from "lucide-react";

export type PreferenceOption<T extends string> = { value: T; label: string };

type PreferenceSelectProps<T extends string> = {
  value: T;
  options: PreferenceOption<T>[];
  onChange: (value: T) => void;
  ariaLabel: string;
  disabled?: boolean;
};

export function PreferenceSelect<T extends string>({ value, options, onChange, ariaLabel, disabled = false }: PreferenceSelectProps<T>) {
  const id = `preference-select-${useId().replace(/:/g, "")}`;
  const ref = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const selectedIndex = Math.max(0, options.findIndex((option) => option.value === value));
  const [activeIndex, setActiveIndex] = useState(selectedIndex);
  const selected = options[selectedIndex] ?? options[0];

  useEffect(() => setActiveIndex(Math.max(0, options.findIndex((option) => option.value === value))), [options, value]);
  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: MouseEvent) => {
      if (!ref.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onPointerDown);
    return () => document.removeEventListener("mousedown", onPointerDown);
  }, [open]);

  const choose = (next: T) => {
    onChange(next);
    setOpen(false);
    requestAnimationFrame(() => triggerRef.current?.focus());
  };
  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (disabled) return;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) setOpen(true);
      else setActiveIndex((current) => (current + (event.key === "ArrowDown" ? 1 : -1) + options.length) % options.length);
    } else if (event.key === "Home" && open) {
      event.preventDefault(); setActiveIndex(0);
    } else if (event.key === "End" && open) {
      event.preventDefault(); setActiveIndex(options.length - 1);
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (open) choose(options[activeIndex].value); else setOpen(true);
    } else if (event.key === "Escape" && open) {
      event.preventDefault(); setOpen(false);
    }
  };

  return (
    <div className="provider-select settings-preference-select" ref={ref}>
      <button ref={triggerRef} className="provider-select-trigger" type="button" aria-label={ariaLabel} aria-haspopup="listbox" aria-expanded={open} aria-controls={`${id}-options`} disabled={disabled} onClick={() => setOpen((current) => !current)} onKeyDown={onKeyDown}>
        <span className="provider-select-trigger-label">{selected?.label}</span>
        <ChevronDown size={16} aria-hidden="true" />
      </button>
      {open && (
        <div className="provider-select-menu" id={`${id}-options`} role="listbox" aria-label={ariaLabel}>
          {options.map((option, index) => (
            <button className="provider-select-option" type="button" role="option" aria-selected={option.value === value} data-active={index === activeIndex || undefined} key={option.value} onMouseEnter={() => setActiveIndex(index)} onClick={() => choose(option.value)}>
              <span className="provider-select-option-check" aria-hidden="true">{option.value === value ? <Check size={15} /> : null}</span>
              <span>{option.label}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
