import { createConnection } from "node:net";
import { homedir } from "node:os";
import { join } from "node:path";

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

type JsonObject = Record<string, unknown>;

interface AgentError {
  code: string;
  message: string;
}

interface AgentResponse {
  id: number;
  result?: JsonObject;
  error?: AgentError;
}

const socketPath =
  process.env.GMUSIC_AGENT_SOCKET ??
  join(
    homedir(),
    "Library",
    "Application Support",
    "com.kyle.gmusic",
    "agent.sock",
  );
let nextRequestId = 1;

function invokeAgent(
  method: string,
  params: JsonObject = {},
): Promise<JsonObject> {
  const id = nextRequestId++;
  const request = `${JSON.stringify({ id, method, params })}\n`;

  return new Promise((resolve, reject) => {
    const socket = createConnection(socketPath);
    let buffer = "";
    const timeout = setTimeout(() => {
      socket.destroy();
      reject(new Error("gMusic did not respond within five seconds."));
    }, 5_000);

    function complete(callback: () => void): void {
      clearTimeout(timeout);
      socket.destroy();
      callback();
    }

    socket.once("error", (error) => {
      complete(() =>
        reject(new Error(`Could not connect to gMusic: ${error.message}`)),
      );
    });
    socket.on("data", (chunk: Buffer) => {
      buffer += chunk.toString("utf8");
      const newline = buffer.indexOf("\n");
      if (newline < 0) {
        return;
      }
      const line = buffer.slice(0, newline);
      let response: AgentResponse;
      try {
        response = JSON.parse(line) as AgentResponse;
      } catch {
        complete(() =>
          reject(new Error("gMusic returned an invalid control response.")),
        );
        return;
      }
      if (response.id !== id) {
        complete(() =>
          reject(new Error("gMusic returned a mismatched control response.")),
        );
        return;
      }
      const responseError = response.error;
      if (responseError) {
        complete(() => reject(new Error(responseError.message)));
        return;
      }
      complete(() => resolve(response.result ?? {}));
    });
    socket.once("connect", () => socket.write(request));
  });
}

function textResult(result: JsonObject) {
  return {
    content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }],
  };
}

function toolError(error: unknown) {
  return {
    content: [
      {
        type: "text" as const,
        text:
          error instanceof Error ? error.message : "The gMusic command failed.",
      },
    ],
    isError: true,
  };
}

function requireConfirmation(confirmed: boolean): void {
  if (!confirmed) {
    throw new Error(
      "This changes the gMusic library. Get the user's explicit approval, then call again with confirmed=true.",
    );
  }
}

function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function libraryPage(
  library: JsonObject,
  offset: number,
  limit: number,
  query?: string,
): JsonObject {
  const tracks = Array.isArray(library.tracks)
    ? library.tracks.filter(isJsonObject)
    : [];
  const normalizedQuery = query?.trim().toLocaleLowerCase();
  const matchingTracks = normalizedQuery
    ? tracks.filter((track) =>
        [
          track.title,
          track.artist,
          track.album,
          track.label,
          ...(Array.isArray(track.genres) ? track.genres : []),
        ]
          .filter((value): value is string => typeof value === "string")
          .some((value) => value.toLocaleLowerCase().includes(normalizedQuery)),
      )
    : tracks;
  const start = Math.min(offset, matchingTracks.length);

  return {
    ...library,
    tracks: matchingTracks.slice(start, start + limit),
    page: {
      limit,
      offset: start,
      returnedTracks: Math.min(limit, matchingTracks.length - start),
      totalMatchingTracks: matchingTracks.length,
      totalTracks: tracks.length,
    },
  };
}

const server = new McpServer({
  name: "gmusic",
  version: "0.1.0",
});

server.registerTool(
  "inspect_library",
  {
    title: "Inspect gMusic library",
    description:
      "Read a bounded page of tracks, editable metadata, playlists, and play statistics from the running local gMusic application.",
    inputSchema: {
      limit: z.number().int().min(1).max(250).default(100),
      offset: z.number().int().nonnegative().default(0),
      query: z.string().min(1).optional(),
    },
  },
  async ({ limit, offset, query }) => {
    try {
      const library = await invokeAgent("library.inspect");
      return textResult(libraryPage(library, offset, limit, query));
    } catch (error) {
      return toolError(error);
    }
  },
);

server.registerTool(
  "update_track_metadata",
  {
    title: "Update gMusic track metadata",
    description:
      "Update a known track's title, artist, album, label, and genres. This is a write operation and requires current explicit user approval.",
    inputSchema: {
      confirmed: z.boolean(),
      id: z.string().min(1),
      title: z.string().min(1),
      artist: z.string().min(1),
      album: z.string().nullable().optional(),
      label: z.string().nullable().optional(),
      genres: z.array(z.string()).default([]),
    },
  },
  async ({ confirmed, id, title, artist, album, label, genres }) => {
    try {
      requireConfirmation(confirmed);
      return textResult(
        await invokeAgent("track.update", {
          id,
          metadata: {
            title,
            artist,
            album: album ?? null,
            label: label ?? null,
            genres,
          },
        }),
      );
    } catch (error) {
      return toolError(error);
    }
  },
);

server.registerTool(
  "upsert_playlist",
  {
    title: "Create or replace a gMusic playlist",
    description:
      "Create a playlist or replace its ordered list of stable track IDs. This is a write operation and requires current explicit user approval.",
    inputSchema: {
      confirmed: z.boolean(),
      id: z.string().min(1),
      name: z.string().min(1),
      trackIds: z.array(z.string().min(1)),
    },
  },
  async ({ confirmed, id, name, trackIds }) => {
    try {
      requireConfirmation(confirmed);
      return textResult(
        await invokeAgent("playlist.upsert", {
          playlist: { id, name, trackIds },
        }),
      );
    } catch (error) {
      return toolError(error);
    }
  },
);

server.registerTool(
  "delete_playlist",
  {
    title: "Delete a gMusic playlist",
    description:
      "Delete one playlist by stable playlist ID. This is a destructive write operation and requires current explicit user approval.",
    inputSchema: {
      confirmed: z.boolean(),
      id: z.string().min(1),
    },
  },
  async ({ confirmed, id }) => {
    try {
      requireConfirmation(confirmed);
      return textResult(await invokeAgent("playlist.delete", { id }));
    } catch (error) {
      return toolError(error);
    }
  },
);

server.registerTool(
  "move_library_track",
  {
    title: "Reorder gMusic library",
    description:
      "Move one track in the durable library order. This is a write operation and requires current explicit user approval.",
    inputSchema: {
      confirmed: z.boolean(),
      from: z.number().int().nonnegative(),
      to: z.number().int().nonnegative(),
    },
  },
  async ({ confirmed, from, to }) => {
    try {
      requireConfirmation(confirmed);
      return textResult(await invokeAgent("queue.move", { from, to }));
    } catch (error) {
      return toolError(error);
    }
  },
);

async function main(): Promise<void> {
  await server.connect(new StdioServerTransport());
}

void main().catch((error: unknown) => {
  console.error(
    error instanceof Error ? error.message : "Could not start gMusic MCP.",
  );
  process.exitCode = 1;
});
