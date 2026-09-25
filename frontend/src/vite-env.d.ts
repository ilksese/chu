/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_CHU_DEBUG_WS?: string;
}

interface Window {
  runtime?: {
    LogInfo?: (message: string) => void;
    LogError?: (message: string) => void;
  };
}
