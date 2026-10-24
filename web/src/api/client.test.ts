import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, api } from "./client";

function respond(status: number, body: unknown) {
  return vi.fn().mockResolvedValue(
    new Response(body === undefined ? null : JSON.stringify(body), {
      status,
      headers: { "content-type": "application/json" },
    }),
  );
}

describe("api", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("api_throws_ApiError_with_kind_and_status", async () => {
    vi.stubGlobal("fetch", respond(404, { error: { kind: "not_found", message: "question 9 not found" } }));
    const err = await api<never>("GET", "/api/questions/9").catch((e: ApiError) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err.kind).toBe("not_found");
    expect(err.status).toBe(404);
    expect(err.message).toBe("question 9 not found");
  });

  it("api_returns_json_on_ok", async () => {
    const fetchMock = respond(200, { id: 3 });
    vi.stubGlobal("fetch", fetchMock);
    await expect(api("PATCH", "/api/topics/3", { name: "x" })).resolves.toEqual({ id: 3 });
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/topics/3",
      expect.objectContaining({ method: "PATCH", body: JSON.stringify({ name: "x" }) }),
    );
  });

  it("api_returns_undefined_on_204", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(null, { status: 204 })));
    await expect(api("DELETE", "/api/questions/1/pin")).resolves.toBeUndefined();
  });

  it("network_failure_becomes_ApiError", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("Failed to fetch")));
    const err = await api<never>("GET", "/api/home").catch((e: ApiError) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err.kind).toBe("network");
  });
});
