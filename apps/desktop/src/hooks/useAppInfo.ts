import { useEffect, useState } from "react";
import { getAppInfo } from "../services/appInfo";
import type { AppInfo } from "../types";

export function useAppInfo(): { info: AppInfo | null; error: string | null } {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    getAppInfo()
      .then((i) => active && setInfo(i))
      .catch((e: unknown) => active && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      active = false;
    };
  }, []);
  return { info, error };
}
