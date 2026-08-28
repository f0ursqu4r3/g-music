import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { youtubeAuthApi } from "@/api";

import SettingsWindow from "../SettingsWindow.vue";

vi.mock("@/api", () => ({
  youtubeAuthApi: {
    inspect: vi.fn(),
    openLogin: vi.fn(),
    saveSession: vi.fn(),
    disconnect: vi.fn(),
  },
}));

describe("SettingsWindow", () => {
  beforeEach(() => {
    vi.mocked(youtubeAuthApi.inspect).mockReset();
    vi.mocked(youtubeAuthApi.openLogin).mockReset();
    vi.mocked(youtubeAuthApi.saveSession).mockReset();
    vi.mocked(youtubeAuthApi.disconnect).mockReset();
    vi.mocked(youtubeAuthApi.inspect).mockResolvedValue({ connected: false });
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
