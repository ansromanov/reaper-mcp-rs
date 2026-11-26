-- Render settings, stems and variants.
-- Loaded by reaper_mcp_handlers.lua with the shared helpers and the handlers table.

local S, handlers = ...

local file_exists, get_project_string, get_track_name, project_path, render_state, resolve_track, safe_filename, set_project_string = S.file_exists, S.get_project_string, S.get_track_name, S.project_path, S.render_state, S.resolve_track, S.safe_filename, S.set_project_string

-- Render file formats as REAPER's base64 render config strings (WAV only).
local RENDER_FORMATS = {
  pcm16 = "ZXZhdxAAAAABAA==",
  pcm24 = "ZXZhdxgAAAABAA==",
  float32 = "ZXZhdyAAAAADAA==",
}

-- Switches the project's render format when args.format names one (or is a raw base64 string)
-- and returns a function that restores the previous format.
local function use_render_format(args)
  if args.format == nil then return function() end end
  local code = RENDER_FORMATS[args.format] or args.format
  local before = get_project_string("RENDER_FORMAT")
  set_project_string("RENDER_FORMAT", code)
  return function() set_project_string("RENDER_FORMAT", before) end
end

-- Sets a custom render range (seconds) when both start and end are given; returns a function
-- that restores the previous bounds.
local function use_render_range(args)
  local before_flag = reaper.GetSetProjectInfo(0, "RENDER_BOUNDSFLAG", 0, false)
  local before_start = reaper.GetSetProjectInfo(0, "RENDER_STARTPOS", 0, false)
  local before_end = reaper.GetSetProjectInfo(0, "RENDER_ENDPOS", 0, false)
  if args.start_seconds ~= nil and args.end_seconds ~= nil then
    if type(args.start_seconds) ~= "number" or type(args.end_seconds) ~= "number"
        or args.start_seconds < 0 or args.end_seconds <= args.start_seconds then
      error("start_seconds and end_seconds must be numbers with 0 <= start < end")
    end
    reaper.GetSetProjectInfo(0, "RENDER_BOUNDSFLAG", 0, true) -- custom time range
    reaper.GetSetProjectInfo(0, "RENDER_STARTPOS", args.start_seconds, true)
    reaper.GetSetProjectInfo(0, "RENDER_ENDPOS", args.end_seconds, true)
  else
    reaper.GetSetProjectInfo(0, "RENDER_BOUNDSFLAG", 1, true) -- entire project
  end
  return function()
    reaper.GetSetProjectInfo(0, "RENDER_BOUNDSFLAG", before_flag, true)
    reaper.GetSetProjectInfo(0, "RENDER_STARTPOS", before_start, true)
    reaper.GetSetProjectInfo(0, "RENDER_ENDPOS", before_end, true)
  end
end

handlers.get_render_settings = function()
  local state = render_state()
  state.format = get_project_string("RENDER_FORMAT")
  return state
end

-- Render helpers ------------------------------------------------------------------------------
-- REAPER never overwrites a render: it writes NAME-001.wav and leaves the old file in place, so a
-- caller that reads NAME.wav reads stale audio. The render handlers delete the files they are
-- about to write (args.overwrite = false keeps them) and report what ended up on disk.
local function render_targets(directory, pattern, track_names)
  local base = (directory:gsub("[/" .. string.char(92) .. "]+$", ""))
  if (pattern:gsub("%$track", "")):find("%$") then return nil end -- other wildcards: names are not predictable
  local paths = {}
  if track_names then
    for _, name in ipairs(track_names) do
      paths[#paths + 1] = base .. "/" .. safe_filename((pattern:gsub("%$track", name))) .. ".wav"
    end
  else
    paths[1] = base .. "/" .. safe_filename(pattern) .. ".wav"
  end
  return paths
end

local function clear_targets(paths)
  local removed, failed = {}, {}
  for _, path in ipairs(paths or {}) do
    if file_exists(path) then
      if os.remove(path) then removed[#removed + 1] = path else failed[#failed + 1] = path end
    end
  end
  return removed, failed
end

local function describe_files(paths)
  local files, warnings = {}, {}
  for _, path in ipairs(paths or {}) do
    local handle = io.open(path, "rb")
    if handle then
      local size = handle:seek("end")
      handle:close()
      files[#files + 1] = { path = path, bytes = size }
    else
      files[#files + 1] = { path = path, missing = true }
      local numbered = path:gsub("%.wav$", "-001.wav")
      if file_exists(numbered) then
        warnings[#warnings + 1] = path .. " was not written, but " .. numbered .. " exists: an older file blocked the render"
      else
        warnings[#warnings + 1] = path .. " was not written"
      end
    end
  end
  return files, warnings
end

-- Saves the project and opens it again, so the render sees the saved state (a project edited in
-- place has rendered silent or wrong).
local function refresh_project()
  local path = project_path()
  if path == nil or path == "" then error("the project has never been saved; save it before using fresh") end
  handlers.save_project({})
  handlers.open_project({ path = path, discard_unsaved = true })
end

-- Renders the whole project (master mix) with the project's saved render
-- format. Blocks REAPER's UI thread until the render finishes.
handlers.render_project = function(args)
  if type(args.directory) ~= "string" or args.directory == "" then error("directory must be a non-empty string") end
  if type(args.name) ~= "string" or args.name == "" then error("name must be a non-empty string") end
  if args.fresh then refresh_project() end
  local before = render_state()
  local targets = render_targets(args.directory, args.name, nil)
  local removed, failed = {}, {}
  if args.overwrite ~= false then removed, failed = clear_targets(targets) end
  set_project_string("RENDER_FILE", args.directory)
  set_project_string("RENDER_PATTERN", args.name)
  reaper.GetSetProjectInfo(0, "RENDER_SETTINGS", 0, true)
  local restore_range = use_render_range(args)
  local restore_format = use_render_format(args)
  if args.sample_rate then reaper.GetSetProjectInfo(0, "RENDER_SRATE", args.sample_rate, true) end
  local started = reaper.time_precise()
  -- File: Render project, using the most recent render settings, auto-close render dialog
  reaper.Main_OnCommand(42230, 0)
  local elapsed = reaper.time_precise() - started
  restore_range()
  restore_format()
  local settings = render_state()
  set_project_string("RENDER_FILE", before.file)
  set_project_string("RENDER_PATTERN", before.pattern)
  reaper.GetSetProjectInfo(0, "RENDER_SETTINGS", before.settings, true)
  reaper.GetSetProjectInfo(0, "RENDER_SRATE", before.sample_rate, true)
  local files, warnings = describe_files(targets)
  for _, path in ipairs(failed) do
    warnings[#warnings + 1] = "could not delete the old file (in use?): " .. path
  end
  return {
    directory = args.directory,
    name = args.name,
    elapsed_seconds = elapsed,
    files = files,
    removed_old_files = removed,
    warnings = warnings,
    previous_settings = before,
    settings = settings,
  }
end

-- Renders the given tracks as separate stem files (each track's own output,
-- including everything routed into it) named after the track. Blocks REAPER
-- until done and restores the previous render settings and selection.
handlers.render_stems = function(args)
  if type(args.directory) ~= "string" or args.directory == "" then error("directory must be a non-empty string") end
  if type(args.tracks) ~= "table" or #args.tracks == 0 then error("tracks must be a non-empty list of indexes") end
  if args.fresh then refresh_project() end
  local before = render_state()
  local before_selected = {}
  for i = 0, reaper.CountTracks(0) - 1 do
    local track = reaper.GetTrack(0, i)
    before_selected[i] = reaper.IsTrackSelected(track)
    reaper.SetTrackSelected(track, false)
  end
  local names = {}
  for _, index in ipairs(args.tracks) do
    local track = resolve_track(index, "tracks")
    reaper.SetTrackSelected(track, true)
    names[#names + 1] = get_track_name(track)
  end
  local pattern = args.pattern or "$track"
  local targets = render_targets(args.directory, pattern, names)
  local removed, failed = {}, {}
  if args.overwrite ~= false then removed, failed = clear_targets(targets) end
  set_project_string("RENDER_FILE", args.directory)
  set_project_string("RENDER_PATTERN", pattern)
  reaper.GetSetProjectInfo(0, "RENDER_SETTINGS", 3, true) -- stems: selected tracks
  local restore_range = use_render_range(args)
  local restore_format = use_render_format(args)
  local started = reaper.time_precise()
  reaper.Main_OnCommand(42230, 0)
  local elapsed = reaper.time_precise() - started
  restore_range()
  restore_format()
  set_project_string("RENDER_FILE", before.file)
  set_project_string("RENDER_PATTERN", before.pattern)
  reaper.GetSetProjectInfo(0, "RENDER_SETTINGS", before.settings, true)
  reaper.GetSetProjectInfo(0, "RENDER_BOUNDSFLAG", before.bounds, true)
  for i = 0, reaper.CountTracks(0) - 1 do
    reaper.SetTrackSelected(reaper.GetTrack(0, i), before_selected[i] or false)
  end
  local files, warnings = describe_files(targets)
  for _, path in ipairs(failed) do
    warnings[#warnings + 1] = "could not delete the old file (in use?): " .. path
  end
  if targets == nil then
    warnings[#warnings + 1] = "the pattern has wildcards other than $track: old files were not deleted and the written files are not listed"
  end
  return {
    directory = args.directory,
    tracks = names,
    elapsed_seconds = elapsed,
    files = files,
    removed_old_files = removed,
    warnings = warnings,
  }
end

-- Variants ------------------------------------------------------------------------------------
-- Renders the same tracks several times with different tracks muted or unmuted ("live only",
-- "trigger only", "everything"). Each variant is saved and the project reopened before the render
-- (fresh, unless fresh = false), and the original mute states are restored and saved at the end.
-- variants: [{ name, mute = [names or indexes], unmute = [...] }]. In `pattern`, {variant} is
-- replaced by the variant name.
handlers.render_variants = function(args)
  if type(args.variants) ~= "table" or #args.variants == 0 then error("variants must be a non-empty list") end
  if type(args.tracks) ~= "table" or #args.tracks == 0 then error("tracks must be a non-empty list of indexes") end
  local function find_track(ref)
    if type(ref) == "number" then return (resolve_track(ref, "track")) end
    for i = 0, reaper.CountTracks(0) - 1 do
      local track = reaper.GetTrack(0, i)
      if get_track_name(track) == ref then return track end
    end
    error("no track named " .. tostring(ref))
  end
  local original = {}
  for i = 0, reaper.CountTracks(0) - 1 do
    original[i] = reaper.GetMediaTrackInfo_Value(reaper.GetTrack(0, i), "B_MUTE")
  end
  local results = {}
  local ok, failure = pcall(function()
    for _, variant in ipairs(args.variants) do
      for i = 0, reaper.CountTracks(0) - 1 do
        reaper.SetMediaTrackInfo_Value(reaper.GetTrack(0, i), "B_MUTE", original[i])
      end
      for _, ref in ipairs(variant.mute or {}) do reaper.SetMediaTrackInfo_Value(find_track(ref), "B_MUTE", 1) end
      for _, ref in ipairs(variant.unmute or {}) do reaper.SetMediaTrackInfo_Value(find_track(ref), "B_MUTE", 0) end
      local pattern = (args.pattern or "$track-{variant}"):gsub("{variant}", variant.name or "variant")
      results[#results + 1] = {
        variant = variant.name,
        render = handlers.render_stems({
          directory = args.directory, tracks = args.tracks, pattern = pattern,
          start_seconds = args.start_seconds, end_seconds = args.end_seconds, format = args.format,
          overwrite = args.overwrite, fresh = args.fresh ~= false,
        }),
      }
    end
  end)
  for i = 0, reaper.CountTracks(0) - 1 do
    reaper.SetMediaTrackInfo_Value(reaper.GetTrack(0, i), "B_MUTE", original[i] or 0)
  end
  handlers.save_project({})
  if not ok then error(failure, 0) end
  return { variants = results }
end
