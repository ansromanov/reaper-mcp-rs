-- Helpers shared by the handler modules. Loaded first by reaper_mcp_handlers.lua.

-- Command handlers for the reaper-mcp-rs bridge. Loaded by reaper_mcp_bridge.lua and
-- reloaded by the `reload` command, so edits apply without restarting REAPER.

local function get_track_name(track)
  local _, name = reaper.GetSetMediaTrackInfo_String(track, "P_NAME", "", false)
  return name
end

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

local function safe_filename(value)
  return (value:gsub('[<>:"/\\|?*%c]', "_"):gsub("%s+$", ""))
end

-- Musical time: bar.beat position of a project time, "17.1.00" = bar 17, beat 1, 0 % of the beat.
local function bbt(time)
  local beats, measures = reaper.TimeMap2_timeToBeats(0, time)
  local beat = math.floor(beats)
  return string.format("%d.%d.%02d", measures + 1, beat + 1, math.floor((beats - beat) * 100 + 0.5))
end

return {
  bbt = bbt,
  db_to_gain = db_to_gain,
  describe_send = describe_send,
  describe_track = describe_track,
  file_exists = file_exists,
  find_param = find_param,
  fx_name = fx_name,
  gain_to_db = gain_to_db,
  get_project_string = get_project_string,
  get_track_name = get_track_name,
  project_path = project_path,
  render_state = render_state,
  require_int = require_int,
  resolve_fx = resolve_fx,
  resolve_track = resolve_track,
  safe_filename = safe_filename,
  set_project_string = set_project_string,
  track_index_of = track_index_of,
  undo = undo,
}
