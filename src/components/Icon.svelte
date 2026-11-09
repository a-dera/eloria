<script lang="ts">
  // Line icon set (24×24, stroke 1.75), drawn for Eloria.
  const P: Record<string, string> = {
    files: "M4 4h6l2 2h8v13a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1z",
    search: "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4",
    tag: "M3 12V4h8l9 9-8 8zM7.5 7.5h.01",
    book: "M4 5a2 2 0 0 1 2-2h13v15H6a2 2 0 0 0-2 2zM4 20a2 2 0 0 0 2 2h13v-4M9 7h6",
    thesis: "M12 3 2 8l10 5 10-5zM6 10v5c2 2 10 2 12 0v-5M22 8v6",
    graph: "M6 6m-2.5 0a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0M18 7m-2 0a2 2 0 1 0 4 0a2 2 0 1 0-4 0M12 18m-2.5 0a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0M8 8l3 7.6M16.3 8.5l-3.3 7.2M8.4 6.3l7.6.5",
    kanban: "M4 4h4v16H4zM10 4h4v10h-4zM16 4h4v7h-4z",
    table: "M4 5h16v14H4zM4 10h16M4 15h16M10 5v14",
    layout: "M4 4h16v16H4zM9 4v16M9 11h11",
    clusters: "M7 8m-3 0a3 3 0 1 0 6 0a3 3 0 1 0-6 0M17 7m-2.5 0a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0M12 17m-3.5 0a3.5 3.5 0 1 0 7 0a3.5 3.5 0 1 0-7 0",
    person: "M12 8m-4 0a4 4 0 1 0 8 0a4 4 0 1 0-8 0M4 21c0-4 3.6-6.5 8-6.5s8 2.5 8 6.5",
    tree: "M9 3h6v4H9zM3 17h6v4H3zM15 17h6v4h-6zM12 7v5M6 17v-5h12v5",
    flask: "M9 3h6M10 3v6L5 19a1.5 1.5 0 0 0 1.3 2h11.4a1.5 1.5 0 0 0 1.3-2L14 9V3M7.5 15h9",
    timeline: "M4 12h16M7 8v8M12 6v12M17 9v6",
    calendar: "M5 5h14v15H5zM5 10h14M9 3v4M15 3v4",
    command: "M9 6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3z",
    settings: "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
    plus: "M12 5v14M5 12h14",
    inbox: "M4 13h4l2 3h4l2-3h4M4 13l2.5-8h11l2.5 8v6H4z",
    pulse: "M3 12h4l3-8 4 16 3-8h4",
    flow: "M5 6m-2 0a2 2 0 1 0 4 0a2 2 0 1 0-4 0M19 18m-2 0a2 2 0 1 0 4 0a2 2 0 1 0-4 0M7 6h6a3 3 0 0 1 3 3v6a3 3 0 0 0 3 3",
    "file-plus": "M14 3H6a1 1 0 0 0-1 1v16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8zM14 3v5h5M12 11v6M9 14h6",
    "folder-plus": "M4 5h6l2 2h8v12H4zM12 10v6M9 13h6",
    folder: "M4 5h6l2 2h8v12H4z",
    "folder-open": "M4 19V5h6l2 2h7v3M4 19l3-8h15l-3 8z",
    note: "M7 3h7l5 5v13H7zM14 3v5h5M10 13h6M10 17h4",
    image: "M4 5h16v14H4zM4 16l5-5 4 4 2-2 5 5M15 9h.01",
    pdf: "M7 3h7l5 5v13H7zM14 3v5h5M9.5 16v-4h1.5a1.2 1.2 0 0 1 0 2.5H9.5",
    x: "M6 6l12 12M18 6 6 18",
    chevron: "M9 6l6 6-6 6",
    "chevron-down": "M6 9l6 6 6-6",
    "panel-left": "M4 4h16v16H4zM9 4v16",
    "panel-right": "M4 4h16v16H4zM15 4v16",
    split: "M4 4h16v16H4zM12 4v16",
    link: "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
    list: "M9 6h11M9 12h11M9 18h11M4.5 6h.01M4.5 12h.01M4.5 18h.01",
    outline: "M4 6h16M8 12h12M12 18h8",
    props: "M4 6h7M4 12h7M4 18h7M14 6h6M14 12h6M14 18h6",
    sync: "M20 11a8 8 0 0 0-14-4.9L4 8M4 4v4h4M4 13a8 8 0 0 0 14 4.9l2-1.9M20 20v-4h-4",
    external: "M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5",
    quote: "M6 17c-1.5 0-2.5-1.2-2.5-3 0-3.5 2-6.5 5-8l.7 1C7.5 8.2 6.8 9.5 6.7 11H9v6zM16 17c-1.5 0-2.5-1.2-2.5-3 0-3.5 2-6.5 5-8l.7 1c-1.7 1.2-2.4 2.5-2.5 4H19v6z",
    at: "M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0zM16 12v1.5a2.5 2.5 0 0 0 5 0V12a9 9 0 1 0-3.5 7.1",
    trash: "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3",
    edit: "M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4",
    history: "M3 12a9 9 0 1 0 3-6.7L3 8M3 3v5h5M12 7v5l3 2",
    download: "M12 4v11M7 10l5 5 5-5M5 20h14",
    focus: "M4 9V5a1 1 0 0 1 1-1h4M15 4h4a1 1 0 0 1 1 1v4M20 15v4a1 1 0 0 1-1 1h-4M9 20H5a1 1 0 0 1-1-1v-4",
    eye: "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12zM12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z",
    code: "M8 7l-5 5 5 5M16 7l5 5-5 5",
    check: "M5 12.5l4.5 4.5L19 7",
    vault: "M4 4h16v16H4zM9 9h6v6H9zM12 4v5M12 15v5",
    zap: "M13 3 5 13h6l-1 8 8-10h-6z",
    sun: "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8zM12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4",
    moon: "M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z",
    more: "M6 12h.01M12 12h.01M18 12h.01",
    sort: "M7 4v16M4 17l3 3 3-3M17 20V4M14 7l3-3 3 3",
    filter: "M4 5h16l-6 8v6l-4-2v-4z",
    help: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM9.2 9.2a2.9 2.9 0 0 1 5.6 1c0 1.9-2.8 2.4-2.8 4M12 17.5h.01",
    map: "M9 4 3 6v14l6-2 6 2 6-2V4l-6 2zM9 4v14M15 6v14",
    bulb: "M9 18h6M10 21h4M12 3a6 6 0 0 0-3.6 10.8c.6.5.9 1.1.9 1.8V16h5.4v-.4c0-.7.3-1.3.9-1.8A6 6 0 0 0 12 3z",
    chart: "M4 20V11M10 20V5M16 20v-8M3 20h18",
    flag: "M5 21V4M5 4h11l-2 3.5L16 11H5",
  };

  let { name, size = 17, stroke = 1.75 }: { name: string; size?: number; stroke?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  style="flex-shrink:0"
>
  <path d={P[name] ?? P.note} />
</svg>
