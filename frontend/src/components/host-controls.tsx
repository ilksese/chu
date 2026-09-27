import { tv } from "tailwind-variants";
import type { Host } from "@/lib/api";
import { HostIcon } from "@/components/host-icons";

const hostMark = tv({
  base: "grid shrink-0 place-items-center overflow-hidden rounded-md border border-black",
  variants: { compact: { true: "size-[30px]", false: "size-10" } },
  defaultVariants: { compact: false },
});

const statusDot = tv({
  base: "inline-block size-[7px] rounded-full border border-black",
  variants: { ready: { true: "bg-[#229948]", false: "bg-[#cccccc]" } },
});

export function HostMark({ host, compact = false }: { host: Host; compact?: boolean }) {
  return (
    <span className={hostMark({ compact })} title={host.name}>
      <HostIcon id={host.id} />
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
    <label className="inline-flex h-8 w-[38px] items-center justify-center" title={label}>
      <input
        type="checkbox"
        className="peer absolute size-px opacity-0"
        checked={checked}
        disabled={disabled}
        aria-label={label}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span className="relative h-[19px] w-8 rounded-full border-2 border-black bg-[#cccccc] transition peer-checked:bg-primary peer-focus-visible:outline-3 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-[#e92929] peer-disabled:cursor-not-allowed peer-disabled:opacity-45 after:absolute after:top-0.5 after:left-0.5 after:size-[11px] after:rounded-full after:border after:border-black after:bg-white after:transition peer-checked:after:translate-x-[13px]" />
    </label>
  );
}

export function StatusDot({ ready }: { ready: boolean }) {
  return (
    <span
      className={statusDot({ ready })}
      aria-hidden="true"
    />
  );
}
