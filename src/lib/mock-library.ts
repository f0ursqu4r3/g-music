export interface MockTrack {
  id: string;
  title: string;
  artist: string;
  album: string;
  duration: string;
  cover: string;
}

export interface MockAlbum {
  title: string;
  artist: string;
  cover: string;
}

export interface MockArtist {
  name: string;
  detail: string;
  cover: string;
}

export const mockTracks: MockTrack[] = [
  {
    id: "night-drive",
    title: "Night Drive",
    artist: "Chromatic Skies",
    album: "Afterimage",
    duration: "3:58",
    cover: "violet",
  },
  {
    id: "the-current",
    title: "The Current",
    artist: "Distant Signals",
    album: "Drift Pattern",
    duration: "3:27",
    cover: "cyan",
  },
  {
    id: "soft-focus",
    title: "Soft Focus",
    artist: "Northbound",
    album: "Slow Motion",
    duration: "3:11",
    cover: "amber",
  },
  {
    id: "first-light",
    title: "First Light",
    artist: "Hollow Coast",
    album: "Weather System",
    duration: "4:06",
    cover: "rose",
  },
  {
    id: "granite",
    title: "Granite",
    artist: "Low Season",
    album: "Thin Air",
    duration: "3:44",
    cover: "lime",
  },
  {
    id: "holding-pattern",
    title: "Holding Pattern",
    artist: "Open Circuit",
    album: "Signal Field",
    duration: "4:18",
    cover: "cobalt",
  },
  {
    id: "blue-hour",
    title: "Blue Hour",
    artist: "Ember Lane",
    album: "Stillness",
    duration: "3:36",
    cover: "coral",
  },
];

export const mockAlbums: MockAlbum[] = [
  { title: "Afterimage", artist: "Chromatic Skies", cover: "violet" },
  { title: "Drift Pattern", artist: "Distant Signals", cover: "cyan" },
  { title: "Slow Motion", artist: "Northbound", cover: "amber" },
  { title: "Weather System", artist: "Hollow Coast", cover: "rose" },
  { title: "Thin Air", artist: "Low Season", cover: "lime" },
];

export const mockArtists: MockArtist[] = [
  { name: "Chromatic Skies", detail: "1 album · 12 songs", cover: "violet" },
  { name: "Distant Signals", detail: "2 albums · 21 songs", cover: "cyan" },
  { name: "Northbound", detail: "1 album · 9 songs", cover: "amber" },
  { name: "Hollow Coast", detail: "3 albums · 30 songs", cover: "rose" },
  { name: "Low Season", detail: "2 albums · 18 songs", cover: "lime" },
];
