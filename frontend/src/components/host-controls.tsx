import type { Host } from "@/lib/api";

export function HostMark({ host, compact = false }: { host: Host; compact?: boolean }) {
  const initials = host.id === "opencode" ? "OC" : host.id === "claude" ? "CC" : "CX";
  return (
    <span
      className={`host-mark host-mark-${host.id} ${compact ? "host-mark-compact" : ""}`}
      aria-hidden="true"
    >
      {initials}
    </span>
  );
}

export function Switch({
  checked,
  disabled,
  label,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="switch" title={label}>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        aria-label={label}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span className="switch-track">
        <span className="switch-thumb" />
      </span>
    </label>
  );
}

export function StatusDot({ ready }: { ready: boolean }) {
  return (
    <span
      className={`status-dot ${ready ? "status-ready" : "status-offline"}`}
      aria-hidden="true"
    />
  );
}
