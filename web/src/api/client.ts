/** An error from the API (`{"error": {kind, message}}`) or from failing to reach it (`kind: "network"`). */
export class ApiError extends Error {
  kind: string;
  status: number;

  constructor(kind: string, status: number, message: string) {
    super(message);
    this.name = "ApiError";
    this.kind = kind;
    this.status = status;
  }
}

export type Method = "GET" | "POST" | "PATCH" | "PUT" | "DELETE";

/** Calls the API by relative path, so the app works under any host. */
export async function api<T>(method: Method, path: string, body?: unknown): Promise<T> {
  let res: Response;
  try {
    res = await fetch(path, {
      method,
      headers: body === undefined ? undefined : { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch (e) {
    throw new ApiError("network", 0, e instanceof Error ? e.message : String(e));
  }
  if (res.status === 204) return undefined as T;
  const data = await res.json().catch(() => null);
  if (!res.ok) {
    throw new ApiError(
      data?.error?.kind ?? "internal",
      res.status,
      data?.error?.message ?? `HTTP ${res.status}`,
    );
  }
  return data as T;
}
