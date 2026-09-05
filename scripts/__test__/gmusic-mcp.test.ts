// @vitest-environment node
import { mkdtemp, rm } from "node:fs/promises";
import { createServer, type Server } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import {
  getDefaultEnvironment,
  StdioClientTransport,
} from "@modelcontextprotocol/sdk/client/stdio.js";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

type Request = { id: number; method: string; params: Record<string, unknown> };

describe("MCP wire contracts", () => {
  let directory: string;
  let server: Server;
  let client: Client;
  let requests: Request[];

  beforeEach(async () => {
    requests = [];
    directory = await mkdtemp(join(tmpdir(), "gmusic-mcp-"));
    const socketPath = join(directory, "agent.sock");
    server = createServer((socket) => {
      let buffer = "";
      socket.on("data", (chunk) => {
        buffer += chunk.toString();
        if (!buffer.includes("\n")) return;
        const request = JSON.parse(buffer.split("\n")[0]) as Request;
        requests.push(request);
        socket.end(
          `${JSON.stringify({ id: request.id, result: { tracks: [] } })}\n`,
        );
      });
    });
    await new Promise<void>((resolve, reject) => {
      server.once("error", reject);
      server.listen(socketPath, resolve);
    });
    client = new Client({ name: "gmusic-contract-test", version: "1.0.0" });
    await client.connect(
      new StdioClientTransport({
        command: "bun",
        args: [resolve("scripts/gmusic-mcp.ts")],
        env: { ...getDefaultEnvironment(), GMUSIC_AGENT_SOCKET: socketPath },
      }),
    );
  });

  afterEach(async () => {
    await client?.close();
    if (server?.listening) {
      await new Promise<void>((resolve) => server.close(() => resolve()));
    }
    if (directory) await rm(directory, { recursive: true, force: true });
  });

  it("sends durable library reorder to library.move, never queue.move", async () => {
    const result = await client.callTool({
      name: "move_library_track",
      arguments: { from: 2, to: 0, confirmed: true },
    });
    expect(result.isError).not.toBe(true);
    expect(requests).toHaveLength(1);
    expect(requests[0]).toMatchObject({
      method: "library.move",
      params: { from: 2, to: 0 },
    });
  });

  it("nests batch metadata exactly as the Rust agent protocol requires", async () => {
    const result = await client.callTool({
      name: "update_library_track_metadata_batch",
      arguments: {
        confirmed: true,
        updates: [
          { id: "fixture", title: "Title", artist: "Artist", genres: [] },
        ],
      },
    });
    expect(result.isError).not.toBe(true);
    expect(requests).toHaveLength(1);
    expect(requests[0]).toMatchObject({
      method: "tracks.update",
      params: {
        updates: [
          {
            id: "fixture",
            metadata: {
              title: "Title",
              artist: "Artist",
              album: null,
              label: null,
              genres: [],
            },
          },
        ],
      },
    });
  });

  it("does not send an unconfirmed library mutation", async () => {
    const result = await client.callTool({
      name: "move_library_track",
      arguments: { from: 2, to: 0, confirmed: false },
    });
    expect(result.isError).toBe(true);
    expect(requests).toEqual([]);
  });
});

describe("Cross-language MCP ↔ Rust dispatch registration", () => {
  it("every invokeAgent method in gmusic-mcp.ts is handled in agent.rs dispatch", async () => {
    const { readFile } = await import("node:fs/promises");
    const { resolve } = await import("node:path");

    const mcpSource = await readFile(resolve("scripts/gmusic-mcp.ts"), "utf8");
    const agentSource = await readFile(
      resolve("src-tauri/src/agent.rs"),
      "utf8",
    );

    // Extract all string literals passed as first argument to invokeAgent(...)
    const mcpMethods = Array.from(
      mcpSource.matchAll(/invokeAgent\(\s*"([^"]+)"/g),
    ).map((m) => m[1]);

    // Extract all string arms in the Rust dispatch match
    const rustArms = Array.from(
      agentSource.matchAll(/"([a-z]+\.[a-z]+)"\s*=>/g),
    ).map((m) => m[1]);

    // Every method the MCP layer calls must have a Rust handler
    const unregistered = mcpMethods.filter(
      (method) => !rustArms.includes(method),
    );
    expect(unregistered).toEqual([]);
  });
});
