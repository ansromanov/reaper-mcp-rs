-- Command handlers for the reaper-mcp-rs bridge. Loaded by reaper_mcp_bridge.lua and
-- reloaded by the `reload` command, so edits apply without restarting REAPER.

local function get_track_name(track)
  local _, name = reaper.GetSetMediaTrackInfo_String(track, "P_NAME", "", false)
  return name
end

local handlers = {
  create_track = function(args)
    if type(args.name) ~= "string" or args.name:match("^%s*$") or #args.name > 255 then
      error("name must contain 1 to 255 bytes")
    end
    local index = reaper.CountTracks(0)
    reaper.Undo_BeginBlock2(0)
    reaper.InsertTrackAtIndex(index, true)
    local track = reaper.GetTrack(0, index)
    reaper.GetSetMediaTrackInfo_String(track, "P_NAME", args.name, true)
    reaper.TrackList_AdjustWindows(false)
    reaper.UpdateArrange()
    reaper.Undo_EndBlock2(0, "MCP: create track", -1)
    return { index = index, name = get_track_name(track), guid = reaper.GetTrackGUID(track) }
  end,

  rename_track = function(args)
    if type(args.index) ~= "number" or args.index < 0 or args.index % 1 ~= 0 then
      error("index must be a non-negative integer")
    end
    if type(args.name) ~= "string" or args.name:match("^%s*$") or #args.name > 255 then
      error("name must contain 1 to 255 bytes")
    end
    local track = reaper.GetTrack(0, args.index)
    if not track then error("track index is out of range") end
    local old_name = get_track_name(track)
    reaper.Undo_BeginBlock2(0)
    reaper.GetSetMediaTrackInfo_String(track, "P_NAME", args.name, true)
    reaper.TrackList_AdjustWindows(false)
    reaper.UpdateArrange()
    reaper.Undo_EndBlock2(0, "MCP: rename track", -1)
    return { index = args.index, old_name = old_name, name = get_track_name(track) }
  end,
}

-- ---------------------------------------------------------------------------
-- Helpers shared by the richer handlers below.
-- ---------------------------------------------------------------------------

local function require_int(value, field, minimum)
  if type(value) ~= "number" or value % 1 ~= 0 or value < (minimum or 0) then
    error(field .. " must be an integer >= " .. tostring(minimum or 0))
  end
  return math.floor(value)
end

-- Track index -1 addresses the master track.
local function resolve_track(index, field)
  field = field or "track"
  local i = require_int(index, field, -1)
  local track
  if i == -1 then track = reaper.GetMasterTrack(0) else track = reaper.GetTrack(0, i) end
  if not track then error(field .. " index is out of range") end
  return track, i
end

local function resolve_fx(track, fx, field)
  field = field or "fx"
  local i = require_int(fx, field, 0)
  if i >= reaper.TrackFX_GetCount(track) then error(field .. " index is out of range") end
  return i
end

local function db_to_gain(db) return 10 ^ (db / 20) end
local function gain_to_db(gain)
  if gain <= 0 then return -150 end
  return 20 * math.log(gain, 10)
end

local function track_index_of(track)
  if not track then return nil end
  local number = reaper.GetMediaTrackInfo_Value(track, "IP_TRACKNUMBER")
  if number == -1 then return -1 end
  return math.floor(number) - 1
end

local function describe_track(track, index)
  local volume = reaper.GetMediaTrackInfo_Value(track, "D_VOL")
  return {
    index = index,
    name = get_track_name(track),
    guid = reaper.GetTrackGUID(track),
    volume = volume,
    volume_db = gain_to_db(volume),
    pan = reaper.GetMediaTrackInfo_Value(track, "D_PAN"),
    mute = reaper.GetMediaTrackInfo_Value(track, "B_MUTE") ~= 0,
    solo = reaper.GetMediaTrackInfo_Value(track, "I_SOLO"),
    phase_inverted = reaper.GetMediaTrackInfo_Value(track, "B_PHASE") ~= 0,
    folder_depth = reaper.GetMediaTrackInfo_Value(track, "I_FOLDERDEPTH"),
    parent_index = track_index_of(reaper.GetParentTrack(track)),
    channels = reaper.GetMediaTrackInfo_Value(track, "I_NCHAN"),
    master_send = reaper.GetMediaTrackInfo_Value(track, "B_MAINSEND") ~= 0,
    fx_enabled = reaper.GetMediaTrackInfo_Value(track, "I_FXEN") ~= 0,
    fx_count = reaper.TrackFX_GetCount(track),
    send_count = reaper.GetTrackNumSends(track, 0),
    receive_count = reaper.GetTrackNumSends(track, -1),
    item_count = reaper.CountTrackMediaItems(track),
  }
end

local function fx_name(track, fx)
  local _, name = reaper.TrackFX_GetFXName(track, fx, "")
  return name
end

-- Matches an FX parameter by index, exact name, or case-insensitive substring.
local function find_param(track, fx, param)
  if type(param) == "number" then
    local i = require_int(param, "param", 0)
    if i >= reaper.TrackFX_GetNumParams(track, fx) then error("param index is out of range") end
    return i
  end
  if type(param) ~= "string" or param == "" then error("param must be an index or a name") end
  local wanted = param:lower()
  local partial
  for i = 0, reaper.TrackFX_GetNumParams(track, fx) - 1 do
    local _, name = reaper.TrackFX_GetParamName(track, fx, i, "")
    local lowered = name:lower()
    if lowered == wanted then return i end
    if not partial and lowered:find(wanted, 1, true) then partial = i end
  end
  if partial then return partial end
  error("no FX parameter matches '" .. param .. "'")
end

local function describe_param(track, fx, i)
  local _, name = reaper.TrackFX_GetParamName(track, fx, i, "")
  local _, formatted = reaper.TrackFX_GetFormattedParamValue(track, fx, i, "")
  local _, minimum, maximum = reaper.TrackFX_GetParam(track, fx, i)
  return {
    index = i,
    name = name,
    normalized = reaper.TrackFX_GetParamNormalized(track, fx, i),
    formatted = formatted,
    min = minimum,
    max = maximum,
  }
end

local function describe_send(track, category, i)
  local key = category < 0 and "P_SRCTRACK" or "P_DESTTRACK"
  local other = reaper.GetTrackSendInfo_Value(track, category, i, key)
  local volume = reaper.GetTrackSendInfo_Value(track, category, i, "D_VOL")
  return {
    send_index = i,
    other_track = track_index_of(other),
    other_name = other and get_track_name(other) or nil,
    volume = volume,
    volume_db = gain_to_db(volume),
    pan = reaper.GetTrackSendInfo_Value(track, category, i, "D_PAN"),
    mute = reaper.GetTrackSendInfo_Value(track, category, i, "B_MUTE") ~= 0,
    mode = reaper.GetTrackSendInfo_Value(track, category, i, "I_SENDMODE"),
    -- first source channel of the pair (0 = channels 1/2, 2 = 3/4 ...; -1 = none) and first destination channel
    source_channel = reaper.GetTrackSendInfo_Value(track, category, i, "I_SRCCHAN"),
    dest_channel = reaper.GetTrackSendInfo_Value(track, category, i, "I_DSTCHAN"),
    mono = reaper.GetTrackSendInfo_Value(track, category, i, "B_MONO") ~= 0,
    phase_inverted = reaper.GetTrackSendInfo_Value(track, category, i, "B_PHASE") ~= 0,
  }
end

-- Channel routing and phase of a send; shared by create_send and set_send.
local function apply_send_routing(source, send, args)
  if args.source_channel ~= nil then
    reaper.SetTrackSendInfo_Value(source, 0, send, "I_SRCCHAN", require_int(args.source_channel, "source_channel", -1))
  end
  if args.dest_channel ~= nil then
    reaper.SetTrackSendInfo_Value(source, 0, send, "I_DSTCHAN", require_int(args.dest_channel, "dest_channel", 0))
  end
  if args.mono ~= nil then reaper.SetTrackSendInfo_Value(source, 0, send, "B_MONO", args.mono and 1 or 0) end
  if args.mode ~= nil then
    local mode = require_int(args.mode, "mode", 0)
    if mode ~= 0 and mode ~= 1 and mode ~= 3 then error("mode must be 0 (post-fader), 1 (pre-FX) or 3 (post-FX, pre-fader)") end
    reaper.SetTrackSendInfo_Value(source, 0, send, "I_SENDMODE", mode)
  end
  if args.phase_inverted ~= nil then
    reaper.SetTrackSendInfo_Value(source, 0, send, "B_PHASE", args.phase_inverted and 1 or 0)
  end
end

local function apply_track_fields(track, args)
  if args.name ~= nil then
    if type(args.name) ~= "string" or args.name:match("^%s*$") or #args.name > 255 then
      error("name must contain 1 to 255 bytes")
    end
    reaper.GetSetMediaTrackInfo_String(track, "P_NAME", args.name, true)
  end
  if args.volume_db ~= nil then
    reaper.SetMediaTrackInfo_Value(track, "D_VOL", db_to_gain(args.volume_db))
  end
  if args.pan ~= nil then reaper.SetMediaTrackInfo_Value(track, "D_PAN", args.pan) end
  if args.mute ~= nil then reaper.SetMediaTrackInfo_Value(track, "B_MUTE", args.mute and 1 or 0) end
  if args.solo ~= nil then reaper.SetMediaTrackInfo_Value(track, "I_SOLO", args.solo and 2 or 0) end
  if args.phase_inverted ~= nil then
    reaper.SetMediaTrackInfo_Value(track, "B_PHASE", args.phase_inverted and 1 or 0)
  end
  if args.master_send ~= nil then
    reaper.SetMediaTrackInfo_Value(track, "B_MAINSEND", args.master_send and 1 or 0)
  end
  if args.fx_enabled ~= nil then
    reaper.SetMediaTrackInfo_Value(track, "I_FXEN", args.fx_enabled and 1 or 0)
  end
  if args.channels ~= nil then
    local channels = math.floor(args.channels)
    if channels < 2 or channels > 128 or channels % 2 ~= 0 then
      error("channels must be an even number from 2 to 128")
    end
    reaper.SetMediaTrackInfo_Value(track, "I_NCHAN", channels)
  end
end

local function undo(label, body)
  reaper.Undo_BeginBlock2(0)
  local ok, result = pcall(body)
  reaper.Undo_EndBlock2(0, "MCP: " .. label, -1)
  -- Plugin parameter changes made through the API do not always mark the project as changed,
  -- and REAPER then skips writing the file on save; mark it explicitly.
  reaper.MarkProjectDirty(0)
  reaper.TrackList_AdjustWindows(false)
  reaper.UpdateArrange()
  if not ok then error(result, 0) end
  return result
end

local function project_path()
  local _, path = reaper.EnumProjects(-1, "")
  return path
end

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

local function set_project_string(key, value) reaper.GetSetProjectInfo_String(0, key, value, true) end
local function get_project_string(key)
  local _, value = reaper.GetSetProjectInfo_String(0, key, "", false)
  return value
end

local function render_state()
  return {
    file = get_project_string("RENDER_FILE"),
    pattern = get_project_string("RENDER_PATTERN"),
    sample_rate = reaper.GetSetProjectInfo(0, "RENDER_SRATE", 0, false),
    channels = reaper.GetSetProjectInfo(0, "RENDER_CHANNELS", 0, false),
    bounds = reaper.GetSetProjectInfo(0, "RENDER_BOUNDSFLAG", 0, false),
    settings = reaper.GetSetProjectInfo(0, "RENDER_SETTINGS", 0, false),
  }
end

local function file_exists(path)
  local file = io.open(path, "rb")
  if file then file:close() return true end
  return false
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

-- Export grouped, item-free track templates from the active project. Track state
-- chunks contain the complete FX, envelopes, folder depth, and send configuration.
local MIX_TEMPLATE_GROUPS = { "DRUMS", "BASS", "GUITARS", "VOCALS", "MIX BUS", "RETURNS" }

local function remove_item_chunks(chunk)
  local output, skipping, depth = {}, false, 0
  for line in (chunk .. "\n"):gmatch("(.-)\n") do
    local keep = not skipping
    if not skipping and line:match("^%s*<ITEM[%s>]?") then
      skipping = true
      depth = 1
      keep = false
    elseif skipping and line:match("^%s*<") then
      depth = depth + 1
      keep = false
    elseif skipping and line:match("^%s*>%s*$") then
      depth = depth - 1
      keep = false
      if depth == 0 then skipping = false end
    elseif skipping then
      keep = false
    end
    if keep then output[#output + 1] = line end
  end
  return table.concat(output, "\n") .. "\n"
end

local function safe_filename(value)
  return (value:gsub('[<>:"/\\|?*%c]', "_"):gsub("%s+$", ""))
end

local function direct_child_chunk(chunk, wanted_tag)
  local lines, depth, start = {}, 0, nil
  for line in (chunk .. "\n"):gmatch("(.-)\n") do lines[#lines + 1] = line end
  for i, line in ipairs(lines) do
    if line:match("^%s*>%s*$") then
      depth = depth - 1
      if start and depth == 1 then
        local block = {}
        for j = start, i do block[#block + 1] = lines[j] end
        return table.concat(block, "\n") .. "\n"
      end
    else
      local tag = line:match("^%s*<([%w_]+)")
      if tag then
        if depth == 1 and tag == wanted_tag then start = i end
        depth = depth + 1
      end
    end
  end
  return nil
end

local function remap_receives(chunk, local_track_indices)
  local output, depth, skipping, omitted = {}, 0, false, 0
  for line in (chunk .. "\n"):gmatch("(.-)\n") do
    local was_skipping = skipping
    local keep = not was_skipping
    if line:match("^%s*>%s*$") then
      depth = depth - 1
      if skipping and depth == 1 then skipping = false end
      keep = not was_skipping
    else
      local tag = line:match("^%s*<([%w_]+)")
      if tag then
        if depth == 1 and tag == "AUXRECV" then
          local prefix, old_index, suffix = line:match("^(%s*<AUXRECV%s+)(%d+)(.*)$")
          if not prefix then error("malformed AUXRECV chunk: missing source track index") end
          local local_index = local_track_indices[tonumber(old_index)]
          if local_index == nil then
            omitted = omitted + 1
            skipping = true
            keep = false
          else
            line = prefix .. tostring(local_index) .. suffix
          end
        end
        depth = depth + 1
      elseif skipping then
        keep = false
      end
    end
    if keep then output[#output + 1] = line end
  end
  return table.concat(output, "\n") .. "\n", omitted
end

handlers.export_mix_template = function(args)
  if type(args.directory) ~= "string" or args.directory:match("^%s*$") then
    error("directory must be a non-empty path")
  end
  if args.include_master_chain ~= nil and type(args.include_master_chain) ~= "boolean" then
    error("include_master_chain must be a boolean")
  end
  local requested, group_order = {}, {}
  if args.groups == nil then
    for _, group in ipairs(MIX_TEMPLATE_GROUPS) do
      requested[group] = true
      group_order[#group_order + 1] = group
    end
  elseif type(args.groups) == "table" and #args.groups > 0 then
    for _, group in ipairs(args.groups) do
      if type(group) ~= "string" or group:match("^%s*$") then error("groups must contain non-empty names") end
      local normalized = group:upper()
      if not requested[normalized] then group_order[#group_order + 1] = normalized end
      requested[normalized] = true
    end
  else
    error("groups must be a non-empty array of group names")
  end

  local tracks, track_count = {}, reaper.CountTracks(0)
  for i = 0, track_count - 1 do
    local track = reaper.GetTrack(0, i)
    local _, chunk = reaper.GetTrackStateChunk(track, "", false)
    if not chunk or chunk == "" then error("could not read track state for index " .. tostring(i)) end
    tracks[i + 1] = {
      name = get_track_name(track),
      folder_depth = reaper.GetMediaTrackInfo_Value(track, "I_FOLDERDEPTH"),
      chunk = remove_item_chunks(chunk),
    }
  end

  local project_name = reaper.GetProjectName(0, "")
  project_name = project_name:gsub("%.[Rr][Pp][Pp]$", "")
  project_name = safe_filename(project_name)
  reaper.RecursiveCreateDirectory(args.directory, 0)
  local results = {}
  for _, group_name in ipairs(group_order) do
    local included, depth, folder_roots = {}, 0, {}
    for i, track in ipairs(tracks) do
      local match = track.name:upper() == group_name
      for root_level = #folder_roots, 1, -1 do
        if depth <= folder_roots[root_level] then table.remove(folder_roots, root_level) end
      end
      local is_descendant = #folder_roots > 0 and depth > folder_roots[#folder_roots]
      if match or is_descendant then included[#included + 1] = i end
      if match and track.folder_depth > 0 then folder_roots[#folder_roots + 1] = depth end
      depth = depth + track.folder_depth
      if depth < 0 then depth = 0 end
    end
    if #included > 0 then
      local group_file = safe_filename(group_name:lower():gsub("%s+", "-"))
      local path = args.directory .. "/" .. project_name .. "-" .. group_file .. ".RTrackTemplate"
      local file, err = io.open(path, "wb")
      if not file then error("could not open template for writing: " .. tostring(err)) end
      local local_track_indices = {}
      for local_index, source_index in ipairs(included) do
        local_track_indices[source_index - 1] = local_index - 1
      end
      local omitted_receives = 0
      for _, index in ipairs(included) do
        local chunk, omitted = remap_receives(tracks[index].chunk, local_track_indices)
        file:write(chunk)
        omitted_receives = omitted_receives + omitted
      end
      file:close()
      results[#results + 1] = {
        group = group_name,
        path = path,
        track_count = #included,
        omitted_external_receives = omitted_receives,
      }
    end
  end

  local master_chain = nil
  if args.include_master_chain ~= false then
    local master = reaper.GetMasterTrack(0)
    local ok, chunk = reaper.GetTrackStateChunk(master, "", false)
    if not ok or not chunk then error("could not read master track state") end
    -- REAPER's master track uses MASTERFXLIST in its state chunk, while the
    -- portable .RfxChain file format expects an FXCHAIN root.
    local fxchain = direct_child_chunk(chunk, "MASTERFXLIST")
    if fxchain then
      fxchain = fxchain:gsub("^([ \t]*)<MASTERFXLIST", "%1<FXCHAIN", 1)
    else
      fxchain = direct_child_chunk(chunk, "FXCHAIN")
    end
    if fxchain then
      local path = args.directory .. "/master.RfxChain"
      local file, err = io.open(path, "wb")
      if not file then error("could not open master FX chain for writing: " .. tostring(err)) end
      file:write(fxchain)
      file:close()
      local fx_count = 0
      for _ in fxchain:gmatch("\n%s*FXID%s+") do fx_count = fx_count + 1 end
      master_chain = { path = path, fx_count = fx_count }
    else
      master_chain = { path = nil, fx_count = 0 }
    end
  end
  return {
    project_name = project_name,
    directory = args.directory,
    templates = results,
    exported_groups = #results,
    master_chain = master_chain,
    warning = "Receives from tracks outside a group are omitted. Internal receives are remapped to template-local track indexes. The separate master FX chain contains only static FX state.",
  }
end

handlers.get_track_state_chunk = function(args)
  local track, index = resolve_track(args.track)
  local ok, chunk = reaper.GetTrackStateChunk(track, "", false)
  if not ok or not chunk or chunk == "" then
    error("could not read track state chunk for index " .. tostring(index))
  end
  return { track = index, chunk = chunk }
end

handlers.set_track_state_chunk = function(args)
  local track, index = resolve_track(args.track)
  local valid_chunk = type(args.chunk) == "string"
      and (args.chunk:match("^%s*<TRACK[%s>]" ) or args.chunk:match("^%s*<MASTERTRACK[%s>]"))
  if not valid_chunk then
    error("chunk must contain a REAPER <TRACK> or <MASTERTRACK> state chunk")
  end
  local old_fx_count = reaper.TrackFX_GetCount(track)
  reaper.Undo_BeginBlock2(0)
  local ok = reaper.SetTrackStateChunk(track, args.chunk, true)
  reaper.Undo_EndBlock2(0, "MCP: apply track FX template", -1)
  if not ok then error("REAPER rejected the track state chunk for index " .. tostring(index)) end
  if args.close_fx_windows then
    local first_fx = args.preserve_existing_fx_windows and old_fx_count or 0
    if first_fx == 0 then reaper.TrackFX_Show(track, 0, 0) end
    for fx = first_fx, reaper.TrackFX_GetCount(track) - 1 do
      reaper.TrackFX_Show(track, fx, 2)
    end
  end
  reaper.TrackList_AdjustWindows(false)
  reaper.UpdateArrange()
  reaper.MarkProjectDirty(0)
  return { track = index, applied = true }
end

handlers.get_track = function(args)
  local track, index = resolve_track(args.track)
  return describe_track(track, index)
end

handlers.list_tracks = function()
  local tracks = {}
  for index = 0, reaper.CountTracks(0) - 1 do
    tracks[#tracks + 1] = describe_track(reaper.GetTrack(0, index), index)
  end
  return tracks
end

-- Moves one track so it sits before `before_index` (0-based; CountTracks() = to the end).
-- Does not touch folder state: build the order first, then set folder depths.
handlers.move_track = function(args)
  local count = reaper.CountTracks(0)
  if type(args.index) ~= "number" or args.index < 0 or args.index >= count or args.index % 1 ~= 0 then
    error("index is out of range")
  end
  if type(args.before_index) ~= "number" or args.before_index < 0 or args.before_index > count or args.before_index % 1 ~= 0 then
    error("before_index must be between 0 and the track count")
  end
  local track = reaper.GetTrack(0, args.index)
  local guid = reaper.GetTrackGUID(track)
  reaper.Undo_BeginBlock2(0)
  reaper.PreventUIRefresh(1)
  reaper.Main_OnCommand(40297, 0) -- unselect all tracks
  reaper.SetTrackSelected(track, true)
  reaper.ReorderSelectedTracks(args.before_index, 0)
  reaper.Main_OnCommand(40297, 0)
  reaper.PreventUIRefresh(-1)
  reaper.TrackList_AdjustWindows(false)
  reaper.UpdateArrange()
  reaper.Undo_EndBlock2(0, "MCP: move track", -1)
  for i = 0, reaper.CountTracks(0) - 1 do
    if reaper.GetTrackGUID(reaper.GetTrack(0, i)) == guid then return { index = i } end
  end
  error("moved track not found")
end

-- Sets I_FOLDERDEPTH: 1 starts a folder, 0 is a normal track, -1..-n closes that many folder levels.
handlers.set_track_folder_depth = function(args)
  local count = reaper.CountTracks(0)
  if type(args.index) ~= "number" or args.index < 0 or args.index >= count or args.index % 1 ~= 0 then
    error("index is out of range")
  end
  if type(args.depth) ~= "number" or args.depth % 1 ~= 0 or args.depth < -16 or args.depth > 1 then
    error("depth must be an integer from -16 to 1")
  end
  local track = reaper.GetTrack(0, args.index)
  reaper.Undo_BeginBlock2(0)
  reaper.SetMediaTrackInfo_Value(track, "I_FOLDERDEPTH", args.depth)
  reaper.TrackList_AdjustWindows(false)
  reaper.UpdateArrange()
  reaper.Undo_EndBlock2(0, "MCP: set folder depth", -1)
  return { index = args.index, depth = reaper.GetMediaTrackInfo_Value(track, "I_FOLDERDEPTH") }
end

handlers.set_track = function(args)
  local track, index = resolve_track(args.track)
  return undo("set track", function()
    apply_track_fields(track, args)
    return describe_track(track, index)
  end)
end

-- Take channel modes (I_CHANMODE) by name; mono modes matter for plugins that read one input channel.
local CHANNEL_MODES = { [0] = "normal", [1] = "reverse_stereo", [2] = "mono_mix", [3] = "left", [4] = "right" }
local CHANNEL_MODE_VALUES = {}
for value, name in pairs(CHANNEL_MODES) do CHANNEL_MODE_VALUES[name] = value end

-- Musical time: bar.beat position of a project time, "17.1.00" = bar 17, beat 1, 0 % of the beat.
local function bbt(time)
  local beats, measures = reaper.TimeMap2_timeToBeats(0, time)
  local beat = math.floor(beats)
  return string.format("%d.%d.%02d", measures + 1, beat + 1, math.floor((beats - beat) * 100 + 0.5))
end

local function describe_item(item, index)
  local take = reaper.GetActiveTake(item)
  local source, take_name, take_pan, channel_mode, source_channels, start_offset
  if take then
    take_name = reaper.GetTakeName(take)
    start_offset = reaper.GetMediaItemTakeInfo_Value(take, "D_STARTOFFS")
    take_pan = reaper.GetMediaItemTakeInfo_Value(take, "D_PAN")
    local mode = math.floor(reaper.GetMediaItemTakeInfo_Value(take, "I_CHANMODE"))
    channel_mode = CHANNEL_MODES[mode] or tostring(mode)
    local src = reaper.GetMediaItemTake_Source(take)
    if src then
      source = reaper.GetMediaSourceFileName(src, "")
      source_channels = reaper.GetMediaSourceNumChannels(src)
    end
  end
  return {
    index = index,
    position = reaper.GetMediaItemInfo_Value(item, "D_POSITION"),
    position_bbt = bbt(reaper.GetMediaItemInfo_Value(item, "D_POSITION")),
    length = reaper.GetMediaItemInfo_Value(item, "D_LENGTH"),
    volume_db = gain_to_db(reaper.GetMediaItemInfo_Value(item, "D_VOL")),
    mute = reaper.GetMediaItemInfo_Value(item, "B_MUTE") ~= 0,
    -- Item and take pan act before the track FX: a mono source panned hard right reaches a
    -- plugin that reads only its first input channel as silence.
    take_pan = take_pan,
    -- Start offset of the take in its source: a slip edit. The audio of the file appears on the
    -- timeline at position - start_offset, so an item at 0.0099 with offset 0.0196 plays 9.7 ms early.
    start_offset = start_offset,
    channel_mode = channel_mode,
    source_channels = source_channels,
    take_name = take_name,
    source = source,
  }
end

handlers.list_items = function(args)
  local track = resolve_track(args.track)
  local items = {}
  for i = 0, reaper.CountTrackMediaItems(track) - 1 do
    items[#items + 1] = describe_item(reaper.GetTrackMediaItem(track, i), i)
  end
  return items
end

-- Sets the time base of the items (C_BEATATTACHMODE): "time" keeps them at their position in seconds when the
-- tempo map changes, "beats" moves them with the bars, "beats_position" moves only the position, "default" follows
-- the project setting. Items created with beat time base shifted in seconds when set_tempo_map was used.
local TIME_BASES = { default = -1, time = 0, beats = 1, beats_position = 2 }
handlers.set_items_time_base = function(args)
  local mode = TIME_BASES[args.time_base or "time"]
  if mode == nil then error("time_base must be one of time, beats, beats_position, default") end
  local wanted = args.tracks
  local function use_track(index)
    if type(wanted) ~= "table" or #wanted == 0 then return true end
    for _, w in ipairs(wanted) do if w == index then return true end end
    return false
  end
  local changed, total = 0, 0
  undo("set items time base", function()
    for ti = 0, reaper.CountTracks(0) - 1 do
      if use_track(ti) then
        local track = reaper.GetTrack(0, ti)
        for ii = 0, reaper.CountTrackMediaItems(track) - 1 do
          local item = reaper.GetTrackMediaItem(track, ii)
          total = total + 1
          if reaper.GetMediaItemInfo_Value(item, "C_BEATATTACHMODE") ~= mode then
            reaper.SetMediaItemInfo_Value(item, "C_BEATATTACHMODE", mode)
            changed = changed + 1
          end
        end
      end
    end
  end)
  return { time_base = args.time_base or "time", items = total, changed = changed }
end

-- Normalizes media items with REAPER's own normalization (the calculation behind Item processing >
-- Normalize): the item gain is set so that the part of the source the item plays reaches target_db in the
-- chosen measure. Take volume stays as it is (the item gain is divided by it). Existing item gain is replaced.
-- tracks (default: all) and exclude_tracks pick the tracks; dry_run only reports.
local NORMALIZE_MODES = { lufs_i = 0, rms = 1, peak = 2, true_peak = 3, lufs_m = 4, lufs_s = 5 }
handlers.normalize_items = function(args)
  if not reaper.CalculateNormalization then error("this REAPER has no CalculateNormalization (needs REAPER 6.x or newer)") end
  local mode = args.mode or "peak"
  local mode_id = NORMALIZE_MODES[mode]
  if mode_id == nil then error("mode must be one of peak, true_peak, lufs_i, lufs_m, lufs_s, rms") end
  if type(args.target_db) ~= "number" or args.target_db > 0 then error("target_db must be a number <= 0") end
  local skip = {}
  for _, index in ipairs(args.exclude_tracks or {}) do skip[index] = true end
  local wanted = args.tracks
  local function use_track(index)
    if skip[index] then return false end
    if type(wanted) ~= "table" or #wanted == 0 then return true end
    for _, w in ipairs(wanted) do if w == index then return true end end
    return false
  end
  local rows = {}
  local function run()
    for ti = 0, reaper.CountTracks(0) - 1 do
      if use_track(ti) then
        local track = reaper.GetTrack(0, ti)
        for ii = 0, reaper.CountTrackMediaItems(track) - 1 do
          local item = reaper.GetTrackMediaItem(track, ii)
          local take = reaper.GetActiveTake(item)
          local source = take and reaper.GetMediaItemTake_Source(take)
          if source and not reaper.TakeIsMIDI(take) then
            local offset = reaper.GetMediaItemTakeInfo_Value(take, "D_STARTOFFS")
            local rate = reaper.GetMediaItemTakeInfo_Value(take, "D_PLAYRATE")
            local length = reaper.GetMediaItemInfo_Value(item, "D_LENGTH") * rate
            local gain = reaper.CalculateNormalization(source, mode_id, args.target_db, offset, offset + length)
            local take_volume = math.abs(reaper.GetMediaItemTakeInfo_Value(take, "D_VOL"))
            if gain and gain > 0 and take_volume > 0 then
              local before = reaper.GetMediaItemInfo_Value(item, "D_VOL")
              if not args.dry_run then reaper.SetMediaItemInfo_Value(item, "D_VOL", gain / take_volume) end
              rows[#rows + 1] = {
                track = ti, item = ii, take = reaper.GetTakeName(take),
                before_db = gain_to_db(before), after_db = gain_to_db(gain / take_volume),
              }
            else
              rows[#rows + 1] = { track = ti, item = ii, take = reaper.GetTakeName(take), skipped = "no usable gain (silent source?)" }
            end
          end
        end
      end
    end
    reaper.UpdateArrange()
  end
  if args.dry_run then run() else undo("normalize items", run) end
  return { mode = mode, target_db = args.target_db, dry_run = args.dry_run == true, count = #rows, items = rows }
end

handlers.set_item = function(args)
  local track = resolve_track(args.track)
  local index = tonumber(args.item)
  if index == nil or index < 0 or index >= reaper.CountTrackMediaItems(track) then
    error("no such item on this track: " .. tostring(args.item))
  end
  local item = reaper.GetTrackMediaItem(track, index)
  return undo("set item", function()
    if args.volume_db ~= nil then reaper.SetMediaItemInfo_Value(item, "D_VOL", db_to_gain(args.volume_db)) end
    if args.mute ~= nil then reaper.SetMediaItemInfo_Value(item, "B_MUTE", args.mute and 1 or 0) end
    if args.position ~= nil then
      if type(args.position) ~= "number" or args.position < 0 then error("position must be a number of seconds >= 0") end
      reaper.SetMediaItemInfo_Value(item, "D_POSITION", args.position)
    end
    if args.length ~= nil then
      if type(args.length) ~= "number" or args.length <= 0 then error("length must be a number of seconds > 0") end
      reaper.SetMediaItemInfo_Value(item, "D_LENGTH", args.length)
    end
    local take = reaper.GetActiveTake(item)
    if (args.take_pan ~= nil or args.channel_mode ~= nil or args.start_offset ~= nil) and not take then
      error("the item has no active take")
    end
    if args.start_offset ~= nil then
      if type(args.start_offset) ~= "number" or args.start_offset < 0 then error("start_offset must be a number of seconds >= 0") end
      reaper.SetMediaItemTakeInfo_Value(take, "D_STARTOFFS", args.start_offset)
    end
    if args.take_pan ~= nil then
      reaper.SetMediaItemTakeInfo_Value(take, "D_PAN", math.max(-1, math.min(1, args.take_pan)))
    end
    if args.channel_mode ~= nil then
      local mode = CHANNEL_MODE_VALUES[args.channel_mode]
      if mode == nil then error("channel_mode must be one of normal, reverse_stereo, mono_mix, left, right") end
      reaper.SetMediaItemTakeInfo_Value(take, "I_CHANMODE", mode)
    end
    if args.source ~= nil then
      if type(args.source) ~= "string" or not file_exists(args.source) then
        error("source must be the path of an existing audio file")
      end
      if not take then error("the item has no active take") end
      local source = reaper.PCM_Source_CreateFromFile(args.source)
      if not source then error("REAPER could not read the source file") end
      reaper.SetMediaItemTake_Source(take, source)
      if args.set_length ~= false then
        reaper.SetMediaItemInfo_Value(item, "D_LENGTH", reaper.GetMediaSourceLength(source))
      end
    end
    reaper.UpdateItemInProject(item)
    return describe_item(item, index)
  end)
end

handlers.list_fx = function(args)
  local track = resolve_track(args.track)
  local fx = {}
  for i = 0, reaper.TrackFX_GetCount(track) - 1 do
    fx[#fx + 1] = {
      index = i,
      name = fx_name(track, i),
      enabled = reaper.TrackFX_GetEnabled(track, i),
      param_count = reaper.TrackFX_GetNumParams(track, i),
    }
  end
  return fx
end

handlers.add_fx = function(args)
  local track = resolve_track(args.track)
  if type(args.name) ~= "string" or args.name == "" then error("name must be a non-empty string") end
  return undo("add FX", function()
    local index = reaper.TrackFX_AddByName(track, args.name, false, -1)
    if index < 0 then error("FX not found or could not be loaded: " .. args.name) end
    return { index = index, name = fx_name(track, index), param_count = reaper.TrackFX_GetNumParams(track, index) }
  end)
end

handlers.remove_fx = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  return undo("remove FX", function()
    local name = fx_name(track, fx)
    if not reaper.TrackFX_Delete(track, fx) then error("REAPER refused to delete the FX") end
    return { removed = name }
  end)
end

handlers.set_fx_enabled = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  if type(args.enabled) ~= "boolean" then error("enabled must be a boolean") end
  return undo("enable FX", function()
    reaper.TrackFX_SetEnabled(track, fx, args.enabled)
    return { index = fx, name = fx_name(track, fx), enabled = reaper.TrackFX_GetEnabled(track, fx) }
  end)
end

handlers.list_fx_params = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local params = {}
  for i = 0, reaper.TrackFX_GetNumParams(track, fx) - 1 do
    params[#params + 1] = describe_param(track, fx, i)
  end
  return { fx = fx_name(track, fx), params = params }
end

-- Number shown by a plugin for a parameter, for example "-18.0 dB" -> -18,
-- "1.2 kHz" -> 1200, "4.0:1" -> 4.
local INFINITY_SIGN = utf8.char(8734)
local NO_BREAK_SPACE = utf8.char(160)
local NARROW_NO_BREAK_SPACE = utf8.char(8239)
local MINUS_SIGN = utf8.char(8722)

local function parse_display_number(text)
  -- Plugins print infinity as a symbol; that is not a number to solve for.
  if text:find(INFINITY_SIGN, 1, true) or text:lower():find("inf", 1, true) then return nil end
  -- Normalize non-breaking spaces and the Unicode minus (kHs plugins print both); take the unit that follows the number
  -- (plugins sometimes print a second, stale unit: "2.50 s ms").
  local lowered = text:lower():gsub(NO_BREAK_SPACE, " "):gsub(NARROW_NO_BREAK_SPACE, " "):gsub(MINUS_SIGN, "-")
  local number, unit = lowered:match("(-?%d+%.?%d*)%s*(%a*)")
  if not number then return nil end
  local value = tonumber(number)
  if unit == "khz" then
    value = value * 1000
  elseif unit == "s" or unit == "sec" then
    -- Seconds shown for long times: normalize to ms.
    value = value * 1000
  end
  return value
end

local function format_normalized(track, fx, param, normalized)
  local ok, text = reaper.TrackFX_FormatParamValueNormalized(track, fx, param, normalized, "")
  if not ok then return nil end
  return text
end

-- Finds the normalized value whose displayed number matches `target` by
-- bisection (assumes the display is monotonic in the normalized value).
local function solve_display_number(track, fx, param, target)
  local function shown(normalized)
    local text = format_normalized(track, fx, param, normalized)
    return text and parse_display_number(text)
  end
  -- Some parameters show text at an end of the range (for example an infinite
  -- ratio); use the outermost numeric points instead.
  local lo_bound, hi_bound = 0.0, 1.0
  local low, high = shown(0), shown(1)
  if high == nil then
    for _, probe in ipairs({ 0.995, 0.99, 0.98, 0.95, 0.9 }) do
      high = shown(probe)
      if high ~= nil then hi_bound = probe break end
    end
  end
  if low == nil then
    for _, probe in ipairs({ 0.005, 0.01, 0.02, 0.05, 0.1 }) do
      low = shown(probe)
      if low ~= nil then lo_bound = probe break end
    end
  end
  if low == nil or high == nil or low == high then
    error(string.format("parameter does not show a numeric value at its ends (low %s, high %s, texts '%s' / '%s'); use display_text or value",
      tostring(low), tostring(high), tostring(format_normalized(track, fx, param, 0)),
      tostring(format_normalized(track, fx, param, 1))))
  end
  local ascending = high > low
  local lo, hi = lo_bound, hi_bound
  for _ = 1, 40 do
    local mid = (lo + hi) / 2
    local current = shown(mid)
    if current == nil then error("parameter display could not be parsed") end
    if (current < target) == ascending then lo = mid else hi = mid end
  end
  -- Of the two values around the crossing, take the one that displays closer.
  local best, achieved = lo, shown(lo)
  local upper = shown(hi)
  if achieved == nil or (upper ~= nil and math.abs(upper - target) < math.abs(achieved - target)) then
    best, achieved = hi, upper
  end
  local tolerance = math.max(math.abs(high - low) * 0.01, 0.06)
  if achieved == nil or math.abs(achieved - target) > tolerance then
    error(string.format("display value %s is outside the parameter range (%s to %s; end texts '%s' / '%s')",
      tostring(target), tostring(low), tostring(high),
      tostring(format_normalized(track, fx, param, lo_bound)), tostring(format_normalized(track, fx, param, hi_bound))))
  end
  return best
end

-- Finds a normalized value whose displayed text equals `wanted`
-- (case-insensitive), for choice and switch parameters.
local function solve_display_text(track, fx, param, wanted)
  local steps = 1000
  local matches = {}
  local wanted_lower = wanted:lower()
  for i = 0, steps do
    local normalized = i / steps
    local text = format_normalized(track, fx, param, normalized)
    if text and text:lower() == wanted_lower then matches[#matches + 1] = normalized end
  end
  if #matches == 0 then error("no value of the parameter displays '" .. wanted .. "'") end
  return matches[math.ceil(#matches / 2)]
end

local function solve_target(track, fx, param, spec)
  local given = (spec.value ~= nil and 1 or 0) + (spec.display_value ~= nil and 1 or 0)
    + (spec.display_text ~= nil and 1 or 0)
  if given ~= 1 then error("pass exactly one of value, display_value or display_text") end
  if spec.value ~= nil then
    if type(spec.value) ~= "number" then error("value must be a number") end
    return spec.value
  elseif spec.display_value ~= nil then
    if type(spec.display_value) ~= "number" then error("display_value must be a number") end
    return solve_display_number(track, fx, param, spec.display_value)
  end
  if type(spec.display_text) ~= "string" then error("display_text must be a string") end
  return solve_display_text(track, fx, param, spec.display_text)
end

handlers.set_fx_param = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local param = find_param(track, fx, args.param)
  return undo("set FX parameter", function()
    local normalized = solve_target(track, fx, param, args)
    if not reaper.TrackFX_SetParamNormalized(track, fx, param, normalized) then
      error("FX rejected the parameter value")
    end
    return describe_param(track, fx, param)
  end)
end

-- Applies many parameters of one FX in a single undo block. Failures are
-- reported per entry and do not stop the rest.
handlers.set_fx_params = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  if type(args.params) ~= "table" then error("params must be a list") end
  return undo("set FX parameters", function()
    local results = {}
    for _, entry in ipairs(args.params) do
      local ok, result = pcall(function()
        local param = find_param(track, fx, entry.param)
        local normalized = solve_target(track, fx, param, entry)
        if not reaper.TrackFX_SetParamNormalized(track, fx, param, normalized) then
          error("FX rejected the parameter value")
        end
        return describe_param(track, fx, param)
      end)
      if ok then
        results[#results + 1] = { ok = true, name = result.name, formatted = result.formatted }
      else
        results[#results + 1] = { ok = false, param = entry.param, error = tostring(result) }
      end
    end
    return { fx = fx_name(track, fx), results = results }
  end)
end

handlers.set_fx_preset = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  if type(args.preset) ~= "string" or args.preset == "" then error("preset must be a non-empty string") end
  return undo("set FX preset", function()
    if not reaper.TrackFX_SetPreset(track, fx, args.preset) then
      error("preset not found: " .. args.preset)
    end
    local _, name = reaper.TrackFX_GetPreset(track, fx, "")
    return { fx = fx_name(track, fx), preset = name }
  end)
end

handlers.list_fx_presets = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local _, current = reaper.TrackFX_GetPreset(track, fx, "")
  local _, count = reaper.TrackFX_GetPresetIndex(track, fx)
  return { fx = fx_name(track, fx), current = current, count = count }
end

-- Moves an FX to another position in the same chain (to_index is its final index).
handlers.move_fx = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local target = require_int(args.to_index, "to_index", 0)
  local count = reaper.TrackFX_GetCount(track)
  if target >= count then error("to_index is out of range") end
  return undo("move FX", function()
    if target ~= fx then
      -- CopyToTrack with is_move=true inserts before the destination index.
      local destination = target > fx and target + 1 or target
      reaper.TrackFX_CopyToTrack(track, fx, track, destination, true)
    end
    local fx_list = {}
    for i = 0, reaper.TrackFX_GetCount(track) - 1 do fx_list[#fx_list + 1] = fx_name(track, i) end
    return { fx = fx_list }
  end)
end

-- Distinct displayed texts of a parameter across its range, for choice
-- parameters (shapes, modes, styles). Returns each text with the first
-- normalized value that shows it.
handlers.list_fx_param_choices = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local param = find_param(track, fx, args.param)
  local steps = args.steps or 400
  local choices, seen = {}, {}
  for i = 0, steps do
    local normalized = i / steps
    local text = format_normalized(track, fx, param, normalized)
    if text and not seen[text] then
      seen[text] = true
      choices[#choices + 1] = { text = text, normalized = normalized }
      if #choices > 64 then break end
    end
  end
  local _, name = reaper.TrackFX_GetParamName(track, fx, param, "")
  return { param = name, index = param, choices = choices }
end

-- Text a plugin shows for a normalized parameter value, without changing it.
handlers.format_fx_param = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local param = find_param(track, fx, args.param)
  if type(args.value) ~= "number" then error("value must be a number") end
  return { param = param, value = args.value, formatted = format_normalized(track, fx, param, args.value) }
end

-- Replaces the automation envelope of an FX parameter. Point values are
-- normalized 0..1 plugin values; shape follows REAPER (0 linear, 1 square,
-- 2 slow start/end, 3 fast start, 4 fast end, 5 bezier).
handlers.set_fx_envelope = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local param = find_param(track, fx, args.param)
  if type(args.points) ~= "table" then error("points must be a list") end
  return undo("set FX envelope", function()
    local envelope = reaper.GetFXEnvelope(track, fx, param, true)
    if not envelope then error("could not create the FX envelope") end
    reaper.DeleteEnvelopePointRange(envelope, -1e9, 1e9)
    for _, point in ipairs(args.points) do
      if type(point.time) ~= "number" or type(point.value) ~= "number" then
        error("each point needs numeric time and value")
      end
      reaper.InsertEnvelopePoint(envelope, point.time, point.value, point.shape or 0, point.tension or 0, false, true)
    end
    reaper.Envelope_SortPoints(envelope)
    return { param = param, points = reaper.CountEnvelopePoints(envelope) }
  end)
end

-- Pin mappings route track channels to FX pins. Each pin has a bitmask of
-- track channels (bit 0 = channel 1, bit 1 = channel 2, ...), low 32 bits only.
handlers.get_fx_pins = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  local _, input_pins, output_pins = reaper.TrackFX_GetIOSize(track, fx)
  local function read(is_output, count)
    local pins = {}
    for pin = 0, count - 1 do
      local low = reaper.TrackFX_GetPinMappings(track, fx, is_output, pin)
      pins[#pins + 1] = low
    end
    return pins
  end
  return {
    fx = fx_name(track, fx),
    input_pin_count = input_pins,
    output_pin_count = output_pins,
    inputs = read(0, input_pins),
    outputs = read(1, output_pins),
  }
end

handlers.set_fx_pins = function(args)
  local track = resolve_track(args.track)
  local fx = resolve_fx(track, args.fx)
  return undo("set FX pins", function()
    for _, entry in ipairs(args.pins) do
      local pin = require_int(entry.pin, "pin", 0)
      local mask = require_int(entry.mask, "mask", 0)
      local is_output = entry.output and 1 or 0
      if not reaper.TrackFX_SetPinMappings(track, fx, is_output, pin, mask, 0) then
        error("REAPER rejected the pin mapping")
      end
    end
    return handlers.get_fx_pins({ track = args.track, fx = args.fx })
  end)
end

-- Automation envelopes of a track and everything that sets the level after its FX chain.
-- The track volume envelope and the fader act after the plugins, so they move the
-- ceiling of a limiter on the same track; this makes that visible.
local TRACK_ENVELOPES = {
  "Volume", "Volume (Pre-FX)", "Trim Volume", "Pan", "Pan (Pre-FX)", "Width", "Width (Pre-FX)", "Mute",
}

local function envelope_flags(env)
  local ok, chunk = reaper.GetEnvelopeStateChunk(env, "", false)
  if not ok then return nil, nil, chunk end
  local active = chunk:match("\nACT%s+(%d)")
  local visible = chunk:match("\nVIS%s+(%d)")
  return active, visible, chunk
end

local function describe_envelope(env, name)
  local count = reaper.CountEnvelopePoints(env)
  local mode = reaper.GetEnvelopeScalingMode(env)
  local is_volume = name:find("Volume", 1, true) ~= nil
  local low, high, first_time, last_time
  for i = 0, count - 1 do
    local _, time, value = reaper.GetEnvelopePoint(env, i)
    local shown = value
    if is_volume then shown = gain_to_db(reaper.ScaleFromEnvelopeMode(mode, value)) end
    if low == nil or shown < low then low = shown end
    if high == nil or shown > high then high = shown end
    if first_time == nil then first_time = time end
    last_time = time
  end
  local active, visible = envelope_flags(env)
  return {
    name = name,
    active = active == "1",
    visible = visible == "1",
    points = count,
    unit = is_volume and "dB" or (name:find("Mute", 1, true) and "0/1" or "-1..1"),
    min = low,
    max = high,
    first_point_seconds = first_time,
    last_point_seconds = last_time,
  }
end

handlers.list_track_envelopes = function(args)
  local track, index = resolve_track(args.track)
  local envelopes = {}
  for _, name in ipairs(TRACK_ENVELOPES) do
    local env = reaper.GetTrackEnvelopeByName(track, name)
    if env then envelopes[#envelopes + 1] = describe_envelope(env, name) end
  end
  local fx_envelopes = {}
  local checked = args.include_all_parameters and 1e9 or 4096
  for fx = 0, reaper.TrackFX_GetCount(track) - 1 do
    for param = 0, math.min(reaper.TrackFX_GetNumParams(track, fx), checked) - 1 do
      local env = reaper.GetFXEnvelope(track, fx, param, false)
      if env then
        local _, param_name = reaper.TrackFX_GetParamName(track, fx, param, "")
        local entry = describe_envelope(env, param_name)
        entry.fx = fx
        entry.fx_name = fx_name(track, fx)
        entry.param = param
        fx_envelopes[#fx_envelopes + 1] = entry
      end
    end
  end
  return { track = index, name = get_track_name(track), envelopes = envelopes, fx_envelopes = fx_envelopes }
end

-- Switches an automation envelope on or off without deleting its points. This is a debugging
-- aid: a volume envelope that is active changes the level after the FX chain.
handlers.set_envelope_active = function(args)
  local track = resolve_track(args.track)
  if type(args.envelope) ~= "string" or type(args.active) ~= "boolean" then
    error("envelope (name) and active (boolean) are required")
  end
  local env
  if args.fx ~= nil then
    local fx = resolve_fx(track, args.fx)
    env = reaper.GetFXEnvelope(track, fx, find_param(track, fx, args.envelope), false)
  else
    env = reaper.GetTrackEnvelopeByName(track, args.envelope)
  end
  if not env then error("no such envelope on this track: " .. args.envelope) end
  return undo("set envelope active", function()
    local ok, chunk = reaper.GetEnvelopeStateChunk(env, "", false)
    if not ok then error("could not read the envelope state") end
    local wanted = args.active and "1" or "0"
    local updated, replaced = chunk:gsub("(\nACT%s+)%d", "%1" .. wanted, 1)
    if replaced == 0 then error("the envelope state has no ACT line") end
    if not reaper.SetEnvelopeStateChunk(env, updated, false) then error("REAPER refused the envelope state") end
    local active = envelope_flags(env)
    return { envelope = args.envelope, active = active == "1" }
  end)
end

-- One call that answers "what can change the level and sound of this track": FX-chain enable state, FX chain with pins,
-- fader, envelopes, sends, parent, and a note when something acts after the plugins.
handlers.describe_track = function(args)
  local track, index = resolve_track(args.track)
  local info = describe_track(track, index)
  local chain = {}
  for fx = 0, reaper.TrackFX_GetCount(track) - 1 do
    local _, inputs, outputs = reaper.TrackFX_GetIOSize(track, fx)
    local in_pins, out_pins = {}, {}
    for pin = 0, inputs - 1 do in_pins[#in_pins + 1] = (reaper.TrackFX_GetPinMappings(track, fx, 0, pin)) end
    for pin = 0, outputs - 1 do out_pins[#out_pins + 1] = (reaper.TrackFX_GetPinMappings(track, fx, 1, pin)) end
    chain[#chain + 1] = {
      index = fx,
      name = fx_name(track, fx),
      enabled = reaper.TrackFX_GetEnabled(track, fx),
      input_pins = in_pins,
      output_pins = out_pins,
    }
  end
  local report = handlers.list_track_envelopes({ track = index })
  local sends = handlers.list_sends({ track = index })
  local notes = {}
  local gain_low, gain_high = info.volume_db, info.volume_db
  for _, env in ipairs(report.envelopes) do
    if env.name == "Volume" and env.active and env.points > 0 then
      gain_low, gain_high = info.volume_db + env.min, info.volume_db + env.max
      notes[#notes + 1] = string.format(
        "an active Volume envelope runs after the FX chain: %.1f to %.1f dB on top of the fader, so a limiter ceiling on this track is shifted by that amount (post-FX gain %.1f to %.1f dB)",
        env.min, env.max, gain_low, gain_high)
    elseif env.active and env.points > 0 then
      notes[#notes + 1] = string.format("active %s envelope (%d points)", env.name, env.points)
    end
  end
  if #report.fx_envelopes > 0 then
    notes[#notes + 1] = string.format("%d FX parameter envelope(s): parameter values change over time", #report.fx_envelopes)
  end
  if info.volume_db > 0.05 then
    notes[#notes + 1] = string.format("fader is +%.1f dB after the FX chain", info.volume_db)
  end
  info.fx_chain = chain
  info.envelopes = report.envelopes
  info.fx_envelopes = report.fx_envelopes
  info.sends = sends.sends
  info.receives = sends.receives
  info.post_fx_gain_db = { minimum = gain_low, maximum = gain_high }
  info.notes = notes
  return info
end

handlers.list_sends = function(args)
  local track = resolve_track(args.track)
  local sends, receives = {}, {}
  for i = 0, reaper.GetTrackNumSends(track, 0) - 1 do sends[#sends + 1] = describe_send(track, 0, i) end
  for i = 0, reaper.GetTrackNumSends(track, -1) - 1 do receives[#receives + 1] = describe_send(track, -1, i) end
  return { sends = sends, receives = receives }
end

handlers.create_send = function(args)
  local source = resolve_track(args.source, "source")
  local destination = resolve_track(args.destination, "destination")
  return undo("create send", function()
    local send = reaper.CreateTrackSend(source, destination)
    if send < 0 then error("REAPER could not create the send") end
    if args.volume_db ~= nil then
      reaper.SetTrackSendInfo_Value(source, 0, send, "D_VOL", db_to_gain(args.volume_db))
    end
    if args.pan ~= nil then reaper.SetTrackSendInfo_Value(source, 0, send, "D_PAN", args.pan) end
    apply_send_routing(source, send, args)
    return describe_send(source, 0, send)
  end)
end

handlers.set_send = function(args)
  local source = resolve_track(args.source, "source")
  local send = require_int(args.send_index, "send_index", 0)
  if send >= reaper.GetTrackNumSends(source, 0) then error("send_index is out of range") end
  return undo("set send", function()
    if args.volume_db ~= nil then
      reaper.SetTrackSendInfo_Value(source, 0, send, "D_VOL", db_to_gain(args.volume_db))
    end
    if args.pan ~= nil then reaper.SetTrackSendInfo_Value(source, 0, send, "D_PAN", args.pan) end
    if args.mute ~= nil then reaper.SetTrackSendInfo_Value(source, 0, send, "B_MUTE", args.mute and 1 or 0) end
    apply_send_routing(source, send, args)
    return describe_send(source, 0, send)
  end)
end

-- Removes one send. The send indexes of that track after it shift down by one.
handlers.remove_send = function(args)
  local source = resolve_track(args.source, "source")
  local send = require_int(args.send_index, "send_index", 0)
  if send >= reaper.GetTrackNumSends(source, 0) then error("send_index is out of range") end
  return undo("remove send", function()
    if not reaper.RemoveTrackSend(source, 0, send) then error("REAPER could not remove the send") end
    return handlers.list_sends({ track = args.source })
  end)
end

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

-- Puts an audio file on a track as a new item at the given position (seconds).
handlers.insert_media = function(args)
  local track = resolve_track(args.track)
  if type(args.path) ~= "string" or args.path == "" then error("path must be a non-empty string") end
  if not file_exists(args.path) then error("media file does not exist: " .. args.path) end
  local position = args.position or 0
  return undo("insert media", function()
    local source = reaper.PCM_Source_CreateFromFile(args.path)
    if not source then error("REAPER could not read the media file") end
    local length = reaper.GetMediaSourceLength(source)
    local item = reaper.AddMediaItemToTrack(track)
    local take = reaper.AddTakeToMediaItem(item)
    reaper.SetMediaItemTake_Source(take, source)
    reaper.SetMediaItemInfo_Value(item, "D_POSITION", position)
    reaper.SetMediaItemInfo_Value(item, "D_LENGTH", length)
    reaper.UpdateItemInProject(item)
    return { position = position, length = length }
  end)
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

-- Multi-out instruments (Kontakt, drum samplers) ---------------------------------------------
-- An instrument such as Kontakt can play every part of a kit on its own output pair. REAPER only
-- passes those outputs on when the instrument track has enough channels and other tracks receive
-- the pairs through sends.

local function instrument_output_channels(track)
  local most = 2
  for fx = 0, reaper.TrackFX_GetCount(track) - 1 do
    local _, _, outputs = reaper.TrackFX_GetIOSize(track, fx)
    if outputs and outputs > most then most = outputs end
  end
  if most % 2 == 1 then most = most + 1 end
  return math.min(most, 128)
end

local function ensure_channels(track, needed)
  local current = math.floor(reaper.GetMediaTrackInfo_Value(track, "I_NCHAN"))
  if needed % 2 == 1 then needed = needed + 1 end
  if needed > current then reaper.SetMediaTrackInfo_Value(track, "I_NCHAN", math.min(needed, 128)) end
  return current
end

-- outputs = [{ name, source_channel (0 = outputs 1/2, 2 = 3/4 ...), mono?, volume_db?, pan? }].
-- The new tracks go right after the instrument, inside an optional new folder track.
handlers.create_multiout_tracks = function(args)
  local instrument, index = resolve_track(args.track)
  if index < 0 then error("the master track cannot be an instrument") end
  if type(args.outputs) ~= "table" or #args.outputs == 0 then error("outputs must be a non-empty list") end
  for i, output in ipairs(args.outputs) do
    if type(output.name) ~= "string" or output.name:match("^%s*$") then error("outputs[" .. i .. "].name is missing") end
    require_int(output.source_channel, "outputs[" .. i .. "].source_channel", 0)
  end
  return undo("create multi-out tracks", function()
    local highest = 0
    for _, output in ipairs(args.outputs) do
      highest = math.max(highest, output.source_channel + (output.mono and 1 or 2))
    end
    local before_channels = ensure_channels(instrument, highest)
    local closing = reaper.GetMediaTrackInfo_Value(instrument, "I_FOLDERDEPTH")
    if closing < 0 then reaper.SetMediaTrackInfo_Value(instrument, "I_FOLDERDEPTH", 0) end
    local at = index + 1
    local created = {}
    local folder
    if args.folder then
      reaper.InsertTrackAtIndex(at, true)
      folder = reaper.GetTrack(0, at)
      reaper.GetSetMediaTrackInfo_String(folder, "P_NAME", args.folder, true)
      reaper.SetMediaTrackInfo_Value(folder, "I_FOLDERDEPTH", 1)
      at = at + 1
    end
    local last
    for _, output in ipairs(args.outputs) do
      reaper.InsertTrackAtIndex(at, true)
      local track = reaper.GetTrack(0, at)
      reaper.GetSetMediaTrackInfo_String(track, "P_NAME", output.name, true)
      reaper.SetMediaTrackInfo_Value(track, "I_FOLDERDEPTH", 0)
      if output.volume_db ~= nil then reaper.SetMediaTrackInfo_Value(track, "D_VOL", db_to_gain(output.volume_db)) end
      if output.pan ~= nil then reaper.SetMediaTrackInfo_Value(track, "D_PAN", output.pan) end
      local send = reaper.CreateTrackSend(instrument, track)
      if send < 0 then error("REAPER could not create the send to " .. output.name) end
      apply_send_routing(instrument, send, {
        source_channel = output.mono and (1024 + output.source_channel) or output.source_channel,
        dest_channel = 0, mode = args.send_mode or 0,
      })
      created[#created + 1] = { index = at, name = output.name, source_channel = output.source_channel, mono = output.mono == true }
      last = track
      at = at + 1
    end
    -- close the new folder, then whatever folders the instrument used to close
    local tail_depth = (folder and -1 or 0) + (closing < 0 and closing or 0)
    reaper.SetMediaTrackInfo_Value(last, "I_FOLDERDEPTH", tail_depth)
    if args.instrument_to_parent == false then reaper.SetMediaTrackInfo_Value(instrument, "B_MAINSEND", 0) end
    return {
      instrument = get_track_name(instrument),
      channels = math.floor(reaper.GetMediaTrackInfo_Value(instrument, "I_NCHAN")),
      channels_before = before_channels,
      folder = folder and { index = index + 1, name = args.folder } or nil,
      tracks = created,
    }
  end)
end

-- Renders every output pair of an instrument to its own file so the caller can see which pairs
-- carry sound (the MCP server measures the files). Temporary receive tracks are removed again and
-- the track's channel count and mute state are restored; nothing else in the project changes.
handlers.probe_instrument_outputs = function(args)
  local instrument, index = resolve_track(args.track)
  if index < 0 then error("the master track cannot be an instrument") end
  if type(args.directory) ~= "string" or args.directory == "" then error("directory must be a non-empty string") end
  local channels = args.channels and require_int(args.channels, "channels", 2) or instrument_output_channels(instrument)
  local before_channels = math.floor(reaper.GetMediaTrackInfo_Value(instrument, "I_NCHAN"))
  local before_mute = reaper.GetMediaTrackInfo_Value(instrument, "B_MUTE")
  local before_sends = reaper.GetTrackNumSends(instrument, 0)
  local probes = {}
  reaper.PreventUIRefresh(1)
  local ok, result = pcall(function()
    ensure_channels(instrument, channels)
    reaper.SetMediaTrackInfo_Value(instrument, "B_MUTE", 0)
    for pair = 0, channels - 2, 2 do
      local at = reaper.CountTracks(0)
      reaper.InsertTrackAtIndex(at, false)
      local track = reaper.GetTrack(0, at)
      reaper.GetSetMediaTrackInfo_String(track, "P_NAME", string.format("__probe_out_%02d-%02d", pair + 1, pair + 2), true)
      local send = reaper.CreateTrackSend(instrument, track)
      apply_send_routing(instrument, send, { source_channel = pair, dest_channel = 0, mode = 3 })
      probes[#probes + 1] = { track = track, pair = pair }
    end
    local indexes = {}
    for _, probe in ipairs(probes) do
      indexes[#indexes + 1] = math.floor(reaper.GetMediaTrackInfo_Value(probe.track, "IP_TRACKNUMBER")) - 1
    end
    return handlers.render_stems({
      directory = args.directory, tracks = indexes, format = "float32",
      start_seconds = args.start_seconds, end_seconds = args.end_seconds, fresh = false,
    })
  end)
  for i = #probes, 1, -1 do reaper.DeleteTrack(probes[i].track) end
  for send = reaper.GetTrackNumSends(instrument, 0) - 1, before_sends, -1 do reaper.RemoveTrackSend(instrument, 0, send) end
  reaper.SetMediaTrackInfo_Value(instrument, "B_MUTE", before_mute)
  reaper.SetMediaTrackInfo_Value(instrument, "I_NCHAN", before_channels)
  reaper.PreventUIRefresh(-1)
  if not ok then error(result, 0) end
  local pairs_out = {}
  for i, probe in ipairs(probes) do
    pairs_out[i] = { source_channel = probe.pair, outputs = string.format("%d/%d", probe.pair + 1, probe.pair + 2),
                     file = result.files[i] and result.files[i].path or nil }
  end
  return { instrument = get_track_name(instrument), channels = channels, pairs = pairs_out,
           elapsed_seconds = result.elapsed_seconds, warnings = result.warnings }
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

-- ---------------------------------------------------------------------------
-- Deleting, media files, MIDI, envelopes, snapshots, plugins, variants, batches.
-- ---------------------------------------------------------------------------

-- Deletes a track; with_children also deletes everything inside a folder track. The master track
-- cannot be deleted. Without with_children the children of a folder stay and move up one level.
handlers.delete_track = function(args)
  local track, index = resolve_track(args.track)
  if index == -1 then error("the master track cannot be deleted") end
  return undo("delete track", function()
    local victims = { index }
    if args.with_children and reaper.GetMediaTrackInfo_Value(track, "I_FOLDERDEPTH") == 1 then
      local level = 0
      for i = index, reaper.CountTracks(0) - 1 do
        level = level + reaper.GetMediaTrackInfo_Value(reaper.GetTrack(0, i), "I_FOLDERDEPTH")
        if i > index then victims[#victims + 1] = i end
        if level <= 0 then break end
      end
    end
    table.sort(victims, function(a, b) return a > b end)
    local names = {}
    for _, i in ipairs(victims) do
      local victim = reaper.GetTrack(0, i)
      table.insert(names, 1, get_track_name(victim)) -- keep the project order
      reaper.DeleteTrack(victim)
    end
    return { deleted = names, track_count = reaper.CountTracks(0) }
  end)
end

handlers.delete_item = function(args)
  local track = resolve_track(args.track)
  local index = require_int(args.item, "item", 0)
  if index >= reaper.CountTrackMediaItems(track) then error("item index is out of range") end
  return undo("delete item", function()
    local item = reaper.GetTrackMediaItem(track, index)
    local description = describe_item(item, index)
    if not reaper.DeleteTrackMediaItem(track, item) then error("REAPER could not delete the item") end
    return { deleted = description, item_count = reaper.CountTrackMediaItems(track) }
  end)
end

-- Sets every item that plays `path` offline (REAPER closes the file, so it can be overwritten or
-- deleted) or online again. Needed to rewrite a WAV that a project item points at.
local function norm_path(path) return ((path or ""):gsub("\\", "/"):lower()) end

handlers.set_media_offline = function(args)
  if type(args.path) ~= "string" or args.path == "" then error("path must be a non-empty string") end
  if type(args.offline) ~= "boolean" then error("offline must be true or false") end
  local wanted = norm_path(args.path)
  local selection, matched = {}, {}
  for i = 0, reaper.CountMediaItems(0) - 1 do
    local item = reaper.GetMediaItem(0, i)
    selection[#selection + 1] = { item, reaper.IsMediaItemSelected(item) }
    reaper.SetMediaItemSelected(item, false)
    for t = 0, reaper.CountTakes(item) - 1 do
      local take = reaper.GetTake(item, t)
      local source = take and reaper.GetMediaItemTake_Source(take)
      if source and norm_path(reaper.GetMediaSourceFileName(source, "")) == wanted then
        matched[#matched + 1] = item
        break
      end
    end
  end
  for _, item in ipairs(matched) do reaper.SetMediaItemSelected(item, true) end
  -- 40440: Item: Set selected media offline, 40439: Item: Set selected media online
  if #matched > 0 then reaper.Main_OnCommand(args.offline and 40440 or 40439, 0) end
  for _, entry in ipairs(selection) do reaper.SetMediaItemSelected(entry[1], entry[2]) end
  reaper.UpdateArrange()
  return { path = args.path, offline = args.offline, items = #matched }
end

-- MIDI -----------------------------------------------------------------------------------------
local function midi_take(args)
  local track = resolve_track(args.track)
  local index = require_int(args.item, "item", 0)
  local item = reaper.GetTrackMediaItem(track, index)
  if not item then error("item index is out of range") end
  local take = reaper.GetActiveTake(item)
  if not take or not reaper.TakeIsMIDI(take) then error("the item has no MIDI take") end
  return take, item, track, index
end

handlers.list_midi_notes = function(args)
  local take = midi_take(args)
  local _, note_count = reaper.MIDI_CountEvts(take)
  local limit = args.limit and require_int(args.limit, "limit", 1) or 5000
  local offset = args.offset and require_int(args.offset, "offset", 0) or 0
  local notes, pitch_counts = {}, {}
  for i = 0, note_count - 1 do
    local _, selected, muted, start_ppq, end_ppq, channel, pitch, velocity = reaper.MIDI_GetNote(take, i)
    local key = tostring(pitch)
    pitch_counts[key] = (pitch_counts[key] or 0) + 1
    if (args.pitch == nil or args.pitch == pitch) and i >= offset and #notes < limit then
      notes[#notes + 1] = {
        index = i,
        start_seconds = reaper.MIDI_GetProjTimeFromPPQPos(take, start_ppq),
        end_seconds = reaper.MIDI_GetProjTimeFromPPQPos(take, end_ppq),
        pitch = pitch,
        velocity = velocity,
        channel = channel,
        muted = muted,
      }
    end
  end
  return { note_count = note_count, returned = #notes, pitch_counts = pitch_counts, notes = notes }
end

handlers.add_midi_notes = function(args)
  local take = midi_take(args)
  if type(args.notes) ~= "table" or #args.notes == 0 then error("notes must be a non-empty list") end
  return undo("add MIDI notes", function()
    for _, note in ipairs(args.notes) do
      local start_ppq = reaper.MIDI_GetPPQPosFromProjTime(take, note.start_seconds)
      local end_ppq = reaper.MIDI_GetPPQPosFromProjTime(take, note.end_seconds)
      reaper.MIDI_InsertNote(take, false, false, start_ppq, end_ppq, note.channel or 0,
        require_int(note.pitch, "pitch", 0), require_int(note.velocity or 100, "velocity", 1), true)
    end
    reaper.MIDI_Sort(take)
    local _, note_count = reaper.MIDI_CountEvts(take)
    return { added = #args.notes, note_count = note_count }
  end)
end

handlers.create_midi_item = function(args)
  local track = resolve_track(args.track)
  if type(args.start_seconds) ~= "number" or type(args.end_seconds) ~= "number"
      or args.end_seconds <= args.start_seconds then
    error("start_seconds and end_seconds must be numbers with start < end")
  end
  return undo("create MIDI item", function()
    local item = reaper.CreateNewMIDIItemInProj(track, args.start_seconds, args.end_seconds, false)
    if not item then error("REAPER could not create the MIDI item") end
    for i = 0, reaper.CountTrackMediaItems(track) - 1 do
      if reaper.GetTrackMediaItem(track, i) == item then return describe_item(item, i) end
    end
    return {}
  end)
end

-- Envelope points -----------------------------------------------------------------------------
-- Replaces (or, with clear = false, adds to) the points of a track envelope ("Volume", "Pan", ...)
-- or of an FX parameter envelope (give fx and the parameter as `envelope`). For Volume give
-- value_db; for the others give value (the envelope's own range).
handlers.set_envelope_points = function(args)
  local track = resolve_track(args.track)
  if type(args.points) ~= "table" then error("points must be a list of {time, value} or {time, value_db}") end
  local env
  if args.fx ~= nil then
    local fx = resolve_fx(track, args.fx)
    env = reaper.GetFXEnvelope(track, fx, find_param(track, fx, args.envelope), true)
  else
    if type(args.envelope) ~= "string" then error("envelope must be a name such as Volume or Pan") end
    env = reaper.GetTrackEnvelopeByName(track, args.envelope)
    if not env then
      -- show the envelope: 40406 toggles the track volume envelope, 40407 the pan envelope
      local command = args.envelope == "Volume" and 40406 or (args.envelope == "Pan" and 40407 or nil)
      if not command then error("no such envelope on this track: " .. args.envelope) end
      for i = 0, reaper.CountTracks(0) - 1 do reaper.SetTrackSelected(reaper.GetTrack(0, i), false) end
      reaper.SetTrackSelected(track, true)
      reaper.Main_OnCommand(command, 0)
      env = reaper.GetTrackEnvelopeByName(track, args.envelope)
      if not env then error("could not create the " .. args.envelope .. " envelope") end
    end
  end
  return undo("set envelope points", function()
    if args.clear ~= false then reaper.DeleteEnvelopePointRange(env, -1e9, 1e9) end
    local mode = reaper.GetEnvelopeScalingMode(env)
    for _, point in ipairs(args.points) do
      local value = point.value
      if point.value_db ~= nil then value = reaper.ScaleToEnvelopeMode(mode, db_to_gain(point.value_db)) end
      if type(point.time) ~= "number" or type(value) ~= "number" then error("each point needs time and value (or value_db)") end
      reaper.InsertEnvelopePoint(env, point.time, value, point.shape or 0, 0, false, true)
    end
    reaper.Envelope_SortPoints(env)
    return { points = reaper.CountEnvelopePoints(env) }
  end)
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


-- Moving in musical time -----------------------------------------------------------------------
-- Items are moved by bars, beats or note values and can be snapped to a grid, instead of by a
-- number of seconds (which drifts off the grid as soon as the tempo is not a round number).
local UNIT_QN = { ["1/1"] = 4, ["1/2"] = 2, ["1/4"] = 1, ["1/8"] = 0.5, ["1/16"] = 0.25, ["1/32"] = 0.125, ["1/64"] = 0.0625 }

-- Length of one unit in quarter notes at a project time (a bar and a beat depend on the time signature).
local function unit_qn(unit, at_time)
  if unit == "bar" then
    local _, _, beats_per_bar, _, denominator = reaper.TimeMap2_timeToBeats(0, at_time)
    return beats_per_bar * 4 / denominator
  elseif unit == "beat" then
    local _, _, _, _, denominator = reaper.TimeMap2_timeToBeats(0, at_time)
    return 4 / denominator
  elseif unit == "grid" then
    local _, division = reaper.GetSetProjectGrid(0, false)
    return division * 4
  elseif UNIT_QN[unit] then
    return UNIT_QN[unit]
  end
  error("unit must be bar, beat, grid, 1/1, 1/2, 1/4, 1/8, 1/16, 1/32 or 1/64")
end

local function shifted_time(time, value, unit, snap_to)
  local qn = reaper.TimeMap2_timeToQN(0, time) + value * unit_qn(unit, time)
  local result = reaper.TimeMap2_QNToTime(0, qn)
  if snap_to ~= nil then
    local step = unit_qn(snap_to, result)
    result = reaper.TimeMap2_QNToTime(0, math.floor(qn / step + 0.5) * step)
  end
  return result
end

-- Track indexes plus, with_children, everything inside the folder tracks among them.
local function expand_tracks(indexes, with_children)
  local wanted, order = {}, {}
  local function add(i)
    if not wanted[i] then wanted[i] = true; order[#order + 1] = i end
  end
  for _, i in ipairs(indexes) do
    local _, index = resolve_track(i, "tracks")
    add(index)
    local track = reaper.GetTrack(0, index)
    if with_children and track and reaper.GetMediaTrackInfo_Value(track, "I_FOLDERDEPTH") == 1 then
      local level = 0
      for j = index, reaper.CountTracks(0) - 1 do
        level = level + reaper.GetMediaTrackInfo_Value(reaper.GetTrack(0, j), "I_FOLDERDEPTH")
        if j > index then add(j) end
        if level <= 0 then break end
      end
    end
  end
  return order
end

-- Moves the items of the given tracks (or one item, with `item`) by `value` units of musical time:
-- unit = bar | beat | grid | 1/1 .. 1/64. `value` may be negative or fractional. With snap_to
-- (same units) every new position is rounded to that grid. Automation points of the tracks move
-- by the same musical distance unless include_envelopes is false. dry_run only reports.
handlers.move_items = function(args)
  if type(args.tracks) ~= "table" or #args.tracks == 0 then error("tracks must be a non-empty list of indexes") end
  if type(args.value) ~= "number" then error("value must be a number of units (negative moves earlier)") end
  local unit = args.unit or "bar"
  unit_qn(unit, 0)
  if args.snap_to ~= nil then unit_qn(args.snap_to, 0) end
  local indexes = expand_tracks(args.tracks, args.with_children)
  local moves, envelopes = {}, {}
  for _, index in ipairs(indexes) do
    local track = reaper.GetTrack(0, index)
    for i = 0, reaper.CountTrackMediaItems(track) - 1 do
      if args.item == nil or (#indexes == 1 and i == args.item) then
        local item = reaper.GetTrackMediaItem(track, i)
        local from = reaper.GetMediaItemInfo_Value(item, "D_POSITION")
        local to = shifted_time(from, args.value, unit, args.snap_to)
        if to < 0 then error("the move puts " .. get_track_name(track) .. " item " .. i .. " before the project start") end
        moves[#moves + 1] = {
          track = index, track_name = get_track_name(track), item = i, item_object = item,
          from = from, to = to, from_bbt = bbt(from), to_bbt = bbt(to),
        }
      end
    end
    if args.include_envelopes ~= false and args.item == nil then
      for e = 0, reaper.CountTrackEnvelopes(track) - 1 do envelopes[#envelopes + 1] = reaper.GetTrackEnvelope(track, e) end
    end
  end
  local points = 0
  local function move_all()
    for _, move in ipairs(moves) do
      reaper.SetMediaItemInfo_Value(move.item_object, "D_POSITION", move.to)
      reaper.UpdateItemInProject(move.item_object)
    end
    for _, env in ipairs(envelopes) do
      for p = 0, reaper.CountEnvelopePoints(env) - 1 do
        local _, time = reaper.GetEnvelopePoint(env, p)
        local to = shifted_time(time, args.value, unit, args.snap_to)
        if to >= 0 then
          reaper.SetEnvelopePoint(env, p, to, nil, nil, nil, nil, true)
          points = points + 1
        end
      end
      reaper.Envelope_SortPoints(env)
    end
  end
  if not args.dry_run then undo("move items", move_all) end
  local report = {}
  for i, move in ipairs(moves) do
    report[i] = {
      track = move.track, track_name = move.track_name, item = move.item,
      from_seconds = move.from, to_seconds = move.to, from_bbt = move.from_bbt, to_bbt = move.to_bbt,
    }
  end
  return { moved = #report, envelope_points = points, dry_run = args.dry_run == true, items = report }
end

-- Puts one item at an absolute musical position: bar (1-based) and beat (1-based, may be
-- fractional), or in seconds.
handlers.set_item_position = function(args)
  local track = resolve_track(args.track)
  local index = require_int(args.item, "item", 0)
  local item = reaper.GetTrackMediaItem(track, index)
  if not item then error("item index is out of range") end
  local time
  if args.bar ~= nil then
    local bar = require_int(args.bar, "bar", 1)
    time = reaper.TimeMap2_beatsToTime(0, (args.beat or 1) - 1, bar - 1)
  elseif type(args.seconds) == "number" then
    time = args.seconds
  else
    error("give bar (and beat) or seconds")
  end
  if args.snap_to ~= nil then time = shifted_time(time, 0, "beat", args.snap_to) end
  if time < 0 then error("the position is before the project start") end
  return undo("set item position", function()
    reaper.SetMediaItemInfo_Value(item, "D_POSITION", time)
    reaper.UpdateItemInProject(item)
    return describe_item(item, index)
  end)
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


-- FX chain files ---------------------------------------------------------------------------------
-- REAPER has no function that writes a plugin preset file, so the state of plugins travels as an FX chain
-- (.RfxChain, the same text REAPER's own "Save FX chain" writes): one unit per plugin, with its state inside.
local function split_lines(text)
  local lines = {}
  for line in (text .. "\n"):gmatch("(.-)\n") do lines[#lines + 1] = line end
  if lines[#lines] == "" then lines[#lines] = nil end
  return lines
end

-- The lines of the track's <FXCHAIN ...> block: lines, first and last line index.
local function fxchain_block(lines)
  local first, depth
  for i, line in ipairs(lines) do
    if not first then
      if line:match("^%s*<FXCHAIN%s*$") then first, depth = i, 1 end
    elseif line:match("^%s*<") then
      depth = depth + 1
    elseif line:match("^%s*>%s*$") then
      depth = depth - 1
      if depth == 0 then return first, i end
    end
  end
  return nil, nil
end

-- Cuts chain lines into plugin units: each starts with a BYPASS line (at depth 0 of the block) and holds
-- the <VST/<JS/<AU/<CLAP block, FXID and WAK. FXID lines are dropped: REAPER makes new ones.
local function chain_units(lines)
  local units, current, depth = {}, nil, 0
  for _, line in ipairs(lines) do
    if depth == 0 and line:match("^%s*BYPASS%s") then
      current = { lines = {}, name = nil }
      units[#units + 1] = current
    end
    if current and not (depth == 0 and line:match("^%s*FXID%s")) then
      current.lines[#current.lines + 1] = line
      if not current.name then current.name = line:match('^%s*<[%u]+%s+"([^"]+)"') or line:match("^%s*<[%u]+%s+(%S+)") end
    end
    if line:match("^%s*<") then depth = depth + 1 elseif line:match("^%s*>%s*$") then depth = depth - 1 end
  end
  return units
end

local function wanted_units(units, selection)
  if selection == nil then return units end
  if type(selection) ~= "table" then error("fx must be a list of indexes or names") end
  local chosen = {}
  for _, ref in ipairs(selection) do
    if type(ref) == "number" then
      local unit = units[ref + 1]
      if not unit then error("fx index is out of range: " .. tostring(ref)) end
      chosen[#chosen + 1] = unit
    else
      local found
      for _, unit in ipairs(units) do
        if unit.name and unit.name:lower():find(tostring(ref):lower(), 1, true) then found = unit; break end
      end
      if not found then error("no FX matches '" .. tostring(ref) .. "'") end
      chosen[#chosen + 1] = found
    end
  end
  return chosen
end

local function track_chain(track)
  local ok, chunk = reaper.GetTrackStateChunk(track, "", false)
  if not ok then error("could not read the track state") end
  local lines = split_lines(chunk)
  local first, last = fxchain_block(lines)
  local inner = {}
  if first then for i = first + 1, last - 1 do inner[#inner + 1] = lines[i] end end
  return lines, first, last, chain_units(inner)
end

-- Writes the FX of a track (all, or those named in `fx`: indexes or name parts) to an .RfxChain file.
handlers.export_fx_chain = function(args)
  local track = resolve_track(args.track)
  if type(args.path) ~= "string" or args.path == "" then error("path must be a non-empty string") end
  local _, _, _, units = track_chain(track)
  if #units == 0 then error("the track has no FX") end
  local chosen = wanted_units(units, args.fx)
  local out = {}
  for _, unit in ipairs(chosen) do for _, line in ipairs(unit.lines) do out[#out + 1] = line end end
  local file, err = io.open(args.path, "wb")
  if not file then error("could not write " .. args.path .. ": " .. tostring(err)) end
  file:write(table.concat(out, "\n") .. "\n")
  file:close()
  local names = {}
  for _, unit in ipairs(chosen) do names[#names + 1] = unit.name end
  return { path = args.path, count = #chosen, plugins = names }
end

-- Puts the FX of an .RfxChain file on a track: mode append (default, after the existing FX) or replace.
handlers.import_fx_chain = function(args)
  local track = resolve_track(args.track)
  if type(args.path) ~= "string" or not file_exists(args.path) then error("path must be an existing .RfxChain file") end
  local mode = args.mode or "append"
  if mode ~= "append" and mode ~= "replace" then error("mode must be append or replace") end
  local file = io.open(args.path, "rb")
  local text = file:read("*a")
  file:close()
  local imported = chain_units(split_lines(text))
  if #imported == 0 then error("the file holds no FX (no BYPASS lines)") end
  local lines, first, last, current = track_chain(track)
  local block = { "<FXCHAIN", "SHOW 0", "LASTSEL 0", "DOCKED 0" }
  if mode == "append" then
    for _, unit in ipairs(current) do for _, line in ipairs(unit.lines) do block[#block + 1] = line end end
  end
  for _, unit in ipairs(imported) do for _, line in ipairs(unit.lines) do block[#block + 1] = line end end
  block[#block + 1] = ">"
  local result = {}
  if first then
    for i = 1, first - 1 do result[#result + 1] = lines[i] end
    for _, line in ipairs(block) do result[#result + 1] = line end
    for i = last + 1, #lines do result[#result + 1] = lines[i] end
  else  -- no FX chain yet: before the closing > of the track
    for i = 1, #lines - 1 do result[#result + 1] = lines[i] end
    for _, line in ipairs(block) do result[#result + 1] = line end
    result[#result + 1] = lines[#lines]
  end
  return undo("import FX chain", function()
    if not reaper.SetTrackStateChunk(track, table.concat(result, "\n") .. "\n", false) then
      error("REAPER refused the track state")
    end
    local names = {}
    for _, unit in ipairs(imported) do names[#names + 1] = unit.name end
    return { path = args.path, mode = mode, added = #imported, plugins = names, fx_count = reaper.TrackFX_GetCount(track) }
  end)
end

return handlers
