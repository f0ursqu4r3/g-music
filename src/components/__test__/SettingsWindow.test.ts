import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { playbackApi, youtubeAuthApi } from "@/api";

import SettingsWindow from "../SettingsWindow.vue";

vi.mock("@/api", () => ({
  playbackApi: { inspectDiagnostics: vi.fn(), exportLibraryBackup: vi.fn() },
  youtubeAuthApi: {
    inspect: vi.fn(),
    openLogin: vi.fn(),
    saveSession: vi.fn(),
    disconnect: vi.fn(),
  },
}));

describe("SettingsWindow", () => {
  beforeEach(() => {
    vi.mocked(playbackApi.inspectDiagnostics)
      .mockReset()
      .mockResolvedValue({
        appVersion: "1.0",
        platform: "macos",
        dependencies: [
          {
            name: "yt-dlp",
            available: false,
            version: null,
            message: "Install yt-dlp, then refresh diagnostics.",
          },
        ],
        audioOutputPolicy: "System default output",
      });
    vi.mocked(playbackApi.exportLibraryBackup).mockReset();
    vi.mocked(youtubeAuthApi.inspect).mockReset();
    vi.mocked(youtubeAuthApi.openLogin).mockReset();
    vi.mocked(youtubeAuthApi.saveSession).mockReset();
    vi.mocked(youtubeAuthApi.disconnect).mockReset();
    vi.mocked(youtubeAuthApi.inspect).mockResolvedValue({ connected: false });
  });

  it("shows readiness and recovery instructions, refreshes diagnostics, and copies support info", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const wrapper = mount(SettingsWindow);
    await flushPromises();
    expect(wrapper.text()).toContain("Install yt-dlp");
    expect(wrapper.text()).toContain("System default output");
    await wrapper
      .get('button[aria-label="Refresh diagnostics"]')
      .trigger("click");
    await flushPromises();
    expect(playbackApi.inspectDiagnostics).toHaveBeenCalledTimes(2);
    await wrapper
      .get('button[aria-label="Copy support info"]')
      .trigger("click");
    await flushPromises();
    expect(writeText).toHaveBeenCalledWith(
      expect.stringContaining('"appVersion": "1.0"'),
    );
    expect(wrapper.text()).toContain("Support info copied");
  });

  it("retains diagnostics on refresh failure and offers retry", async () => {
    const wrapper = mount(SettingsWindow);
    await flushPromises();
    vi.mocked(playbackApi.inspectDiagnostics).mockRejectedValueOnce({
      message: "Diagnostics unavailable",
    });
    await wrapper
      .get('button[aria-label="Refresh diagnostics"]')
      .trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("Diagnostics unavailable");
    expect(wrapper.text()).toContain("Install yt-dlp");
    await wrapper
      .get('button[aria-label="Refresh diagnostics"]')
      .trigger("click");
    await flushPromises();
    expect(wrapper.text()).not.toContain("Diagnostics unavailable");
  });

  it("exports a backup, keeps its path selectable, and shows copy failures", async () => {
    vi.mocked(playbackApi.exportLibraryBackup).mockResolvedValue({
      path: "/backups/library.sqlite3",
    });
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: vi
          .fn()
          .mockRejectedValue(new Error("Clipboard unavailable")),
      },
    });
    const wrapper = mount(SettingsWindow);
    await flushPromises();
    await wrapper.get('button[aria-label="Back up library"]').trigger("click");
    await flushPromises();
    expect(
      (
        wrapper.get('input[aria-label="Backup path"]')
          .element as HTMLInputElement
      ).value,
    ).toBe("/backups/library.sqlite3");
    await wrapper.get('button[aria-label="Copy backup path"]').trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("Clipboard unavailable");
    expect(wrapper.find('input[aria-label="Backup path"]').exists()).toBe(true);
  });

  it("shows backup failures and emits the selected theme", async () => {
    vi.mocked(playbackApi.exportLibraryBackup).mockRejectedValue({
      message: "Backup disk is full",
    });
    const wrapper = mount(SettingsWindow, { props: { theme: "plum" } });
    await flushPromises();
    const selector = wrapper.get('select[aria-label="Theme"]');
    expect((selector.element as HTMLSelectElement).value).toBe("plum");
    await selector.setValue("ember");
    expect(wrapper.emitted("update:theme")).toEqual([["ember"]]);
    await wrapper.get('button[aria-label="Back up library"]').trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("Backup disk is full");
  });

  it("opens the isolated YouTube login flow and saves its session", async () => {
    vi.mocked(youtubeAuthApi.openLogin).mockResolvedValue({ connected: false });
    vi.mocked(youtubeAuthApi.saveSession).mockResolvedValue({
      connected: true,
    });
    const wrapper = mount(SettingsWindow);
    await flushPromises();

    expect(wrapper.text()).toContain("account can be restricted or banned");
    await wrapper
      .get('button[aria-label="Sign in to YouTube"]')
      .trigger("click");
    expect(youtubeAuthApi.openLogin).toHaveBeenCalledOnce();

    await wrapper
      .get('button[aria-label="Use signed-in session"]')
      .trigger("click");
    await flushPromises();

    expect(youtubeAuthApi.saveSession).toHaveBeenCalledOnce();
    expect(wrapper.get('[data-auth-state="connected"]').text()).toContain(
      "Connected",
    );
  });

  it("disconnects and deletes the dedicated session", async () => {
    vi.mocked(youtubeAuthApi.inspect).mockResolvedValue({ connected: true });
    vi.mocked(youtubeAuthApi.disconnect).mockResolvedValue({
      connected: false,
    });
    const wrapper = mount(SettingsWindow);
    await flushPromises();

    await wrapper
      .get('button[aria-label="Disconnect YouTube"]')
      .trigger("click");
    await flushPromises();

    expect(youtubeAuthApi.disconnect).toHaveBeenCalledOnce();
    expect(wrapper.get('[data-auth-state="disconnected"]')).toBeDefined();
  });
});
