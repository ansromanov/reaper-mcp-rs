-- Project, tempo map, marker, project-tab, batch and plugin-list handlers.
-- Loaded by reaper_mcp_handlers.lua with the shared helpers and the handlers table.

local S, handlers = ...

local bbt, describe_send, describe_track, file_exists, fx_name, get_track_name, project_path, render_state, require_int, set_project_string, undo = S.bbt, S.describe_send, S.describe_track, S.file_exists, S.fx_name, S.get_track_name, S.project_path, S.render_state, S.require_int, S.set_project_string, S.undo

handlers.audio_device = function(args)
  if type(args.action) ~= "string"
      or (args.action ~= "status" and args.action ~= "close" and args.action ~= "open") then
    error("action must be 'status', 'close', or 'open'")
  end
  local before_running = reaper.Audio_IsRunning() ~= 0
  if args.action == "close" and before_running then
    reaper.Audio_Quit()
  elseif args.action == "open" and not before_running then
    reaper.Audio_Init()
  end
  local after_running = reaper.Audio_IsRunning() ~= 0
  return {
    action = args.action,
    before = { running = before_running },
    after = { running = after_running },
    changed = before_running ~= after_running,
  }
end

handlers.get_project_info = function()
  local name = reaper.GetProjectName(0)
  local numerator, denominator, tempo = reaper.TimeMap_GetTimeSigAtTime(0, 0)
  return {
    project_name = name,
    path = project_path(),
    dirty = reaper.IsProjectDirty(0) == 1,
    tempo_bpm = tempo,
    time_signature = { numerator = numerator, denominator = denominator },
    length_seconds = reaper.GetProjectLength(0),
    sample_rate = reaper.GetSetProjectInfo(0, "PROJECT_SRATE", 0, false),
    track_count = reaper.CountTracks(0),
    reaper_version = reaper.GetAppVersion(),
  }
end

-- Sets the project sample rate (Project Settings > Project sample rate, "use" ticked), so that a
-- project assembled from files recorded at 48 or 96 kHz keeps that rate.
handlers.set_project_sample_rate = function(args)
  local rate = args.sample_rate
  if type(rate) ~= "number" or rate < 8000 or rate > 768000 or rate ~= math.floor(rate) then
    error("sample_rate must be a whole number of Hz between 8000 and 768000")
  end
  reaper.GetSetProjectInfo(0, "PROJECT_SRATE_USE", 1, true)
  reaper.GetSetProjectInfo(0, "PROJECT_SRATE", rate, true)
  return { sample_rate = reaper.GetSetProjectInfo(0, "PROJECT_SRATE", 0, false) }
end

-- Writes the project tempo map from markers placed in quarter notes (the unit a MIDI file or a Cubase
-- tempo track uses), so that MIDI imported by tick lands on the same bars as the audio. The first marker
-- must be at quarter note 0; a marker's time in seconds follows from the tempi before it. Clears the
-- existing tempo markers unless clear is false.
handlers.set_tempo_map = function(args)
  local markers = args.markers
  if type(markers) ~= "table" or #markers == 0 then error("markers must be a non-empty list") end
  local previous
  for i, marker in ipairs(markers) do
    if type(marker.quarter_notes) ~= "number" or marker.quarter_notes < 0 then
      error("marker " .. i .. ": quarter_notes must be a number >= 0")
    end
    if type(marker.bpm) ~= "number" or marker.bpm < 10 or marker.bpm > 960 then
      error("marker " .. i .. ": bpm must be a number between 10 and 960")
    end
    if i == 1 and marker.quarter_notes ~= 0 then error("the first marker must be at quarter note 0") end
    if previous and marker.quarter_notes <= previous then error("markers must be in increasing quarter_notes order") end
    previous = marker.quarter_notes
  end
  return undo("set tempo map", function()
    if args.clear ~= false then
      for i = reaper.CountTempoTimeSigMarkers(0) - 1, 0, -1 do reaper.DeleteTempoTimeSigMarker(0, i) end
    end
    local seconds, last = 0, nil
    for _, marker in ipairs(markers) do
      if last then seconds = seconds + (marker.quarter_notes - last.quarter_notes) * 60 / last.bpm end
      if not reaper.SetTempoTimeSigMarker(0, -1, seconds, -1, -1, marker.bpm,
          marker.numerator or 0, marker.denominator or 0, false) then
        error("REAPER refused the tempo marker at quarter note " .. marker.quarter_notes)
      end
      last = marker
    end
    reaper.UpdateTimeline()
    local count = reaper.CountTempoTimeSigMarkers(0)
    local result = {}
    for i = 0, count - 1 do
      local _, time, _, _, bpm, num, den = reaper.GetTempoTimeSigMarker(0, i)
      result[#result + 1] = { time_seconds = time, bpm = bpm, numerator = num, denominator = den }
    end
    return { marker_count = count, markers = result }
  end)
end

local function list_markers_raw()
  local markers = {}
  local i = 0
  while true do
    local retval, is_region, position, region_end, name, number, color = reaper.EnumProjectMarkers3(0, i)
    if retval == 0 then break end
    markers[#markers + 1] = {
      index = i, number = number, name = name, position = position,
      is_region = is_region, region_end = is_region and region_end or nil, color = color,
    }
    i = i + 1
  end
  return markers
end

handlers.list_markers = function()
  return list_markers_raw()
end

-- "#rrggbb" to REAPER's native custom color (0 keeps the default color).
local function marker_color(hex)
  if hex == nil then return 0 end
  local r, g, b = tostring(hex):match("^#?(%x%x)(%x%x)(%x%x)$")
  if not r then error("color must be a hex string like #ff8800") end
  return reaper.ColorToNative(tonumber(r, 16), tonumber(g, 16), tonumber(b, 16)) + 0x1000000
end

-- Adds markers (position only) and regions (position and end) at project times in seconds, so a long
-- project can be navigated by name. Existing markers are kept unless clear is true.
handlers.add_markers = function(args)
  local markers = args.markers
  if type(markers) ~= "table" or #markers == 0 then error("markers must be a non-empty list") end
  for i, marker in ipairs(markers) do
    if type(marker.name) ~= "string" then error("marker " .. i .. ": name must be a string") end
    if type(marker.position_seconds) ~= "number" or marker.position_seconds < 0 then
      error("marker " .. i .. ": position_seconds must be a number >= 0")
    end
    if marker.end_seconds ~= nil and (type(marker.end_seconds) ~= "number" or marker.end_seconds <= marker.position_seconds) then
      error("marker " .. i .. ": end_seconds must be a number greater than position_seconds")
    end
    marker_color(marker.color)
  end
  return undo("add markers", function()
    if args.clear == true then
      for i = #list_markers_raw() - 1, 0, -1 do reaper.DeleteProjectMarkerByIndex(0, i) end
    end
    for i, marker in ipairs(markers) do
      local is_region = marker.end_seconds ~= nil
      local index = reaper.AddProjectMarker2(0, is_region, marker.position_seconds, marker.end_seconds or 0,
        marker.name, -1, marker_color(marker.color))
      if index < 0 then error("REAPER refused marker " .. i .. " (" .. marker.name .. ")") end
    end
    reaper.UpdateTimeline()
    local all = list_markers_raw()
    return { added = #markers, total = #all, markers = all }
  end)
end

handlers.open_project = function(args)
  if type(args.path) ~= "string" or args.path == "" then error("path must be a non-empty string") end
  if not file_exists(args.path) then error("project file does not exist: " .. args.path) end
  if reaper.IsProjectDirty(0) == 1 and not args.discard_unsaved then
    error("the current project has unsaved changes; save it or pass discard_unsaved=true")
  end
  reaper.Main_openProject("noprompt:" .. args.path)
  return { path = project_path(), track_count = reaper.CountTracks(0) }
end

-- Plugin parameters set through the API reach a plugin's audio processor only when REAPER runs
-- the FX. With the audio engine stopped they stay pending, and the plugin state written on save
-- is the old one (changes were lost after a reload). A 0.1 s render runs every FX once.
local function flush_fx_state()
  local running = reaper.Audio_IsRunning() == 1
  if running then return false end
  local folder = reaper.GetResourcePath() .. "/reaper-mcp-bridge/flush"
  reaper.RecursiveCreateDirectory(folder, 0)
  local before = render_state()
  local before_flag = reaper.GetSetProjectInfo(0, "RENDER_BOUNDSFLAG", 0, false)
  local before_start = reaper.GetSetProjectInfo(0, "RENDER_STARTPOS", 0, false)
  local before_end = reaper.GetSetProjectInfo(0, "RENDER_ENDPOS", 0, false)
  set_project_string("RENDER_FILE", folder)
  set_project_string("RENDER_PATTERN", "flush")
  reaper.GetSetProjectInfo(0, "RENDER_SETTINGS", 0, true)
  reaper.GetSetProjectInfo(0, "RENDER_BOUNDSFLAG", 0, true)
  reaper.GetSetProjectInfo(0, "RENDER_STARTPOS", 0, true)
  reaper.GetSetProjectInfo(0, "RENDER_ENDPOS", 0.1, true)
  reaper.Main_OnCommand(42230, 0)
  set_project_string("RENDER_FILE", before.file)
  set_project_string("RENDER_PATTERN", before.pattern)
  reaper.GetSetProjectInfo(0, "RENDER_SETTINGS", before.settings, true)
  reaper.GetSetProjectInfo(0, "RENDER_BOUNDSFLAG", before_flag, true)
  reaper.GetSetProjectInfo(0, "RENDER_STARTPOS", before_start, true)
  reaper.GetSetProjectInfo(0, "RENDER_ENDPOS", before_end, true)
  return true
end

handlers.save_project = function(args)
  local flushed = flush_fx_state()
  if args.path ~= nil then
    if type(args.path) ~= "string" or args.path == "" then error("path must be a non-empty string") end
    reaper.Main_SaveProjectEx(0, args.path, 0)
  else
    -- Save always writes the file: without the dirty flag REAPER may skip it and changes
    -- made through the API (for example plugin parameters) would be lost on reload.
    reaper.MarkProjectDirty(0)
    reaper.Main_SaveProject(0, false)
  end
  return {
    path = project_path(),
    dirty = reaper.IsProjectDirty(0) == 1,
    audio_running = reaper.Audio_IsRunning() == 1,
    flushed_fx = flushed,
  }
end

-- Open project tabs: list, switch, create, close. The other handlers always
-- act on the active tab, so switch first.
handlers.list_projects = function()
  local projects = {}
  local active = reaper.EnumProjects(-1, "")
  local i = 0
  while true do
    local project, path = reaper.EnumProjects(i, "")
    if not project then break end
    projects[#projects + 1] = {
      index = i,
      path = path,
      active = project == active,
      dirty = reaper.IsProjectDirty(project) == 1,
      track_count = reaper.CountTracks(project),
    }
    i = i + 1
  end
  return projects
end

handlers.switch_project = function(args)
  local index = require_int(args.index, "index", 0)
  local project = reaper.EnumProjects(index, "")
  if not project then error("project tab index is out of range") end
  reaper.SelectProjectInstance(project)
  return { index = index, path = project_path() }
end

handlers.new_project_tab = function()
  reaper.Main_OnCommand(40859, 0) -- File: New project tab
  return { path = project_path(), track_count = reaper.CountTracks(0) }
end

-- Closes the project tab named by args.index (see list_projects). The index is required: closing
-- "the active tab" closed a project the caller did not mean to close. Refuses when that tab has
-- unsaved changes, so no save dialog can block REAPER, and keeps the previously active tab active.
handlers.close_project_tab = function(args)
  local count = 0
  while reaper.EnumProjects(count, "") do count = count + 1 end
  if count < 2 then error("refusing to close the last project tab") end
  if args.index == nil then error("index is required: pass the tab to close (see list_projects)") end
  local index = require_int(args.index, "index", 0)
  local target = reaper.EnumProjects(index, "")
  if not target then error("project tab index is out of range") end
  if reaper.IsProjectDirty(target) == 1 then
    error("project tab " .. index .. " has unsaved changes; save it first")
  end
  local active = reaper.EnumProjects(-1, "")
  reaper.SelectProjectInstance(target)
  reaper.Main_OnCommand(40860, 0) -- File: Close current project tab
  if target ~= active then reaper.SelectProjectInstance(active) end
  return { closed_index = index, path = project_path() }
end

-- Runs a REAPER action by numeric command ID or by named command string
-- (for example "_SWS_SAVEALLSELTRK"). Escape hatch for features that have no
-- dedicated tool yet.
handlers.run_action = function(args)
  local id = args.command_id
  if type(id) == "string" then id = reaper.NamedCommandLookup(id) end
  if type(id) ~= "number" or id <= 0 then error("unknown action") end
  reaper.Main_OnCommand(id, 0)
  return { command_id = id }
end

-- Snapshot -------------------------------------------------------------------------------------
-- The whole project in one call: tracks with folder structure, fader, pan, mute, FX (name and
-- state), sends (destination and channels) and item counts. Compare two snapshots to see what
-- changed.
handlers.project_snapshot = function()
  local tracks = {}
  for i = 0, reaper.CountTracks(0) - 1 do
    local track = reaper.GetTrack(0, i)
    local info = describe_track(track, i)
    local fx = {}
    for f = 0, reaper.TrackFX_GetCount(track) - 1 do
      fx[#fx + 1] = { name = fx_name(track, f), enabled = reaper.TrackFX_GetEnabled(track, f) }
    end
    local sends = {}
    for s = 0, reaper.GetTrackNumSends(track, 0) - 1 do
      local send = describe_send(track, 0, s)
      sends[#sends + 1] = {
        to = send.other_name, volume_db = send.volume_db, mute = send.mute, mode = send.mode,
        source_channel = send.source_channel, dest_channel = send.dest_channel,
      }
    end
    tracks[#tracks + 1] = {
      index = i, name = info.name, folder_depth = info.folder_depth, parent = info.parent_index,
      volume_db = info.volume_db, pan = info.pan, mute = info.mute, solo = info.solo,
      phase_inverted = info.phase_inverted, master_send = info.master_send, items = info.item_count,
      fx = fx, sends = sends,
    }
  end
  local master_fx = {}
  local master = reaper.GetMasterTrack(0)
  for f = 0, reaper.TrackFX_GetCount(master) - 1 do
    master_fx[#master_fx + 1] = { name = fx_name(master, f), enabled = reaper.TrackFX_GetEnabled(master, f) }
  end
  return {
    path = project_path(),
    length_seconds = reaper.GetProjectLength(0),
    track_count = #tracks,
    tracks = tracks,
    master_fx = master_fx,
  }
end

-- Plugins --------------------------------------------------------------------------------------
-- Plugins used by the project (name, how many instances, first tracks) with the version found on
-- disk for VST3 bundles (Contents/Resources/moduleinfo.json). REAPER keeps old DLLs loaded after
-- an update, so the version on disk can be newer than the one that runs: restart REAPER.
local VST3_FOLDERS = { "C:/Program Files/Common Files/VST3", "C:/Program Files (x86)/Common Files/VST3" }

local function vst3_version(display)
  local name = display:gsub("^VST3i?: ", ""):gsub("%s*%b()%s*$", "")
  for _, folder in ipairs(VST3_FOLDERS) do
    local info = folder .. "/" .. name .. ".vst3/Contents/Resources/moduleinfo.json"
    local handle = io.open(info, "rb")
    if handle then
      local text = handle:read("*a")
      handle:close()
      return text:match('"Version"%s*:%s*"([^"]+)"'), folder .. "/" .. name .. ".vst3"
    end
  end
  return nil, nil
end

handlers.list_plugins = function(args)
  local seen, order = {}, {}
  local function visit(track, label)
    for f = 0, reaper.TrackFX_GetCount(track) - 1 do
      local name = fx_name(track, f)
      local entry = seen[name]
      if not entry then
        entry = { name = name, instances = 0, tracks = {} }
        seen[name] = entry
        order[#order + 1] = entry
      end
      entry.instances = entry.instances + 1
      if #entry.tracks < 5 then entry.tracks[#entry.tracks + 1] = label end
    end
  end
  visit(reaper.GetMasterTrack(0), "master")
  for i = 0, reaper.CountTracks(0) - 1 do
    local track = reaper.GetTrack(0, i)
    visit(track, get_track_name(track))
  end
  local filter = args.filter and tostring(args.filter):lower() or nil
  local result = {}
  for _, entry in ipairs(order) do
    if not filter or entry.name:lower():find(filter, 1, true) then
      entry.version_on_disk, entry.path = vst3_version(entry.name)
      result[#result + 1] = entry
    end
  end
  return { plugins = result, count = #result }
end

-- Batch ---------------------------------------------------------------------------------------
-- Runs several handlers in one call: calls = [{ tool, args }]. Stops at the first error unless
-- continue_on_error is true. Each call keeps its own undo step.
handlers.batch = function(args)
  if type(args.calls) ~= "table" or #args.calls == 0 then error("calls must be a non-empty list") end
  local results = {}
  for i, call in ipairs(args.calls) do
    local handler = type(call.tool) == "string" and call.tool ~= "batch" and handlers[call.tool] or nil
    if not handler then
      results[i] = { tool = call.tool, ok = false, error = "unknown tool in batch: " .. tostring(call.tool) }
    else
      local ok, value = pcall(handler, call.args or {})
      results[i] = ok and { tool = call.tool, ok = true, result = value } or { tool = call.tool, ok = false, error = tostring(value) }
    end
    if not results[i].ok and not args.continue_on_error then break end
  end
  return { results = results, completed = #results, total = #args.calls }
end

-- Project tempo and grid, to see what "one grid step" means before moving anything.
handlers.get_grid = function()
  local _, division, swing_mode, swing = reaper.GetSetProjectGrid(0, false)
  local numerator, denominator, tempo = reaper.TimeMap_GetTimeSigAtTime(0, 0)
  return {
    grid_division = division,
    grid_quarter_notes = division * 4,
    swing_mode = swing_mode,
    swing = swing,
    tempo_bpm = tempo,
    time_signature = { numerator = numerator, denominator = denominator },
    bar_seconds = reaper.TimeMap2_QNToTime(0, numerator * 4 / denominator) - reaper.TimeMap2_QNToTime(0, 0),
    project_length_bbt = bbt(reaper.GetProjectLength(0)),
  }
end
