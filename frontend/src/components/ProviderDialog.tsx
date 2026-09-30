import { useRef, useState, type FormEvent } from "react";
import { Eye, EyeOff, LoaderCircle, X } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Input } from "@/components/ui/Input";
import type { Provider, ProviderInput } from "@/lib/api";

export function ProviderDialog({
  provider,
  providers,
  busy,
  returnFocus,
  onClose,
  onSave,
}: {
  provider: Provider | null;
  providers: Provider[];
  busy: boolean;
  returnFocus?: HTMLElement | null;
  onClose: () => void;
  onSave: (input: ProviderInput) => Promise<boolean>;
}) {
  const [draft, setDraft] = useState<ProviderInput>(() => ({
    name: provider?.name ?? "",
    baseUrl: provider?.baseUrl ?? "",
    envApiKey: provider?.envApiKey ?? "",
    apiKey: "",
  }));
  const [errors, setErrors] = useState<Partial<Record<keyof ProviderInput, string>>>({});
  const [saveError, setSaveError] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const submittingRef = useRef(false);
  const locked = busy || submitting;

  function requestClose() {
    if (!locked && !submittingRef.current) onClose();
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (locked || submittingRef.current) return;
    const form = event.currentTarget;
    const input = {
      ...draft,
      name: draft.name.trim(),
      baseUrl: draft.baseUrl.trim(),
      envApiKey: draft.envApiKey.trim(),
    };
    const nextErrors: typeof errors = {};
    const others = providers.filter((item) => item.id !== provider?.id);
    if (!input.name) nextErrors.name = "请输入供应商名称";
    else if (others.some((item) => item.name.trim().toLowerCase() === input.name.toLowerCase()))
      nextErrors.name = "供应商名称已存在";
    try {
      const url = new URL(input.baseUrl);
      if (!/^https?:\/\//i.test(input.baseUrl) || !url.hostname || /\s/.test(input.baseUrl))
        nextErrors.baseUrl = "请输入完整的 http:// 或 https:// 地址";
      else if (url.username || url.password || url.search || url.hash)
        nextErrors.baseUrl = "地址不能包含凭证、查询参数或片段";
    } catch {
      nextErrors.baseUrl = "请输入完整的 http:// 或 https:// 地址";
    }
    if (input.envApiKey && !/^[A-Za-z_][A-Za-z0-9_]*$/.test(input.envApiKey))
      nextErrors.envApiKey = "请输入有效的环境变量名";
    else if (input.envApiKey && others.some((item) => item.envApiKey === input.envApiKey))
      nextErrors.envApiKey = "环境变量名已被其他供应商使用";
    else if (input.apiKey && !input.envApiKey)
      nextErrors.envApiKey = "填写 API_KEY 时需要环境变量名";
    setErrors(nextErrors);
    setSaveError("");
    const firstInvalid = Object.keys(nextErrors)[0];
    if (firstInvalid) {
      form.querySelector<HTMLInputElement>(`[name="${firstInvalid}"]`)?.focus();
      return;
    }
    const focused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    submittingRef.current = true;
    setSubmitting(true);
    let saved = false;
    try {
      saved = await onSave(input);
      if (saved) onClose();
      else setSaveError("保存失败，请重试");
    } catch {
      setSaveError("保存失败，请重试");
    } finally {
      submittingRef.current = false;
      setSubmitting(false);
      if (!saved) requestAnimationFrame(() => focused?.isConnected && focused.focus());
    }
  }

  return (
    <Dialog
      className="w-[520px] rounded-md"
      aria-labelledby="provider-dialog-title"
      onClose={requestClose}
      returnFocus={returnFocus}
    >
      <header className="flex items-center justify-between border-b-2 border-border px-5 py-4">
        <h2 id="provider-dialog-title" className="m-0 text-lg font-bold">
          {provider ? "编辑供应商" : "新增供应商"}
        </h2>
        <Button
          type="button"
          size="icon"
          title="关闭"
          aria-label="关闭"
          disabled={locked}
          onClick={requestClose}
        >
          <X />
        </Button>
      </header>
      <form className="grid gap-4 p-5" onSubmit={submit} noValidate aria-busy={locked}>
        {(
          [
            ["name", "供应商名称", ""],
            ["baseUrl", "BASE_URL", "https://api.example.com/v1"],
            ["envApiKey", "ENV_API_KEY", "PROVIDER_API_KEY"],
            ["apiKey", "API_KEY", provider ? "留空保留原密钥" : ""],
          ] as const
        ).map(([name, label, placeholder]) => (
          <div key={name} className="grid min-w-0 gap-1.5">
            <label
              htmlFor={`provider-${name}`}
              className="text-[11px] font-bold text-muted-foreground"
            >
              {label}
              {name === "name" || name === "baseUrl" ? " *" : ""}
            </label>
            <div className="relative min-w-0">
              <Input
                id={`provider-${name}`}
                name={name}
                className={`w-full ${name === "apiKey" ? "pr-12" : ""}`}
                type={name === "apiKey" && !showKey ? "password" : "text"}
                autoComplete={name === "apiKey" ? "new-password" : "off"}
                spellCheck={name === "name"}
                required={name === "name" || name === "baseUrl"}
                readOnly={locked}
                value={draft[name]}
                placeholder={placeholder}
                aria-invalid={Boolean(errors[name])}
                aria-describedby={errors[name] ? `provider-${name}-error` : undefined}
                onChange={(event) => {
                  setDraft((current) => ({ ...current, [name]: event.target.value }));
                  setErrors((current) => ({ ...current, [name]: undefined }));
                  setSaveError("");
                }}
              />
              {name === "apiKey" ? (
                <Button
                  type="button"
                  variant="ghost"
                  className="absolute top-1 right-1 size-8 rounded-sm p-0"
                  title={showKey ? "隐藏密钥" : "显示密钥"}
                  aria-label={showKey ? "隐藏密钥" : "显示密钥"}
                  aria-pressed={showKey}
                  disabled={locked}
                  onClick={() => setShowKey((value) => !value)}
                >
                  {showKey ? <EyeOff /> : <Eye />}
                </Button>
              ) : null}
            </div>
            {errors[name] ? (
              <p
                id={`provider-${name}-error`}
                className="m-0 text-xs text-error-foreground"
                role="alert"
              >
                {errors[name]}
              </p>
            ) : null}
          </div>
        ))}
        {saveError ? (
          <p className="m-0 text-xs text-error-foreground" role="alert">
            {saveError}
          </p>
        ) : null}
        <footer className="mt-1 flex justify-end gap-2">
          <Button type="button" className="rounded-md" disabled={locked} onClick={requestClose}>
            取消
          </Button>
          <Button type="submit" variant="primary" className="min-w-24 rounded-md" disabled={locked}>
            {submitting ? <LoaderCircle className="animate-spin" /> : null}
            {submitting ? "保存中" : "保存"}
          </Button>
        </footer>
      </form>
    </Dialog>
  );
}
