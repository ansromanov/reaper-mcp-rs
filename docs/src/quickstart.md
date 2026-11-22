# Quick start

1. Open REAPER with a project and make sure the bridge is running
   ([Installation](installation.md#start-the-bridge-in-reaper)).
2. In your MCP client, ask for the bridge state. The agent calls `health`:

   ```json
   { "connected": true, "heartbeat_age_seconds": 0, "ipc_directory": "…/REAPER/reaper-mcp-bridge" }
   ```

3. Ask for an overview. `project_snapshot` returns the whole project in one call: tracks,
   folders, faders, FX, sends, item counts and the master FX.

## Things to ask

- "List the tracks and their FX, and tell me which ones have no output to the master."
- "Create a BASS folder with DI and AMP tracks, and send both to the BASS bus."
- "Set the vocal compressor's threshold to -18 dB and its ratio to 4:1." The agent finds the
  parameters by name and can set them by the value the plugin displays.
- "Move every item on the drum tracks two bars later." Items move by bars and beats, so they stay
  on the grid when the tempo is not round.
- "Render the drum and bass stems as float32 WAV, then compare the mix to `reference.wav`."
  `analyze_audio` and `compare_audio` report loudness, peaks, stereo width and the spectrum.

## Good habits

- **Save or back up first.** Edits run inside REAPER undo blocks, so Ctrl+Z works, but an agent
  can make many changes quickly.
- **Render analysis files as float32**, so that levels above 0 dBFS are not cut off.
- **Prefer real tools over `run_action`.** It runs any REAPER action by ID and is meant as a
  stop-gap until a feature gets its own tool.

Next: [What the tools do](tools.md), or the full [tool reference](reference.md).
