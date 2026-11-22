-- A small in-memory stand-in for REAPER's ReaScript API: enough of the project model (tracks and
-- markers) to run the real bridge and handlers outside REAPER. Calling a function that is not
-- faked raises an error that names it, so a test that needs more fails loudly.
--
-- Usage: lua fake_reaper.lua <path to reaper_mcp_bridge.lua> <seconds to run>
-- The bridge reads REAPER_MCP_DIR; the loop ends when a file named `stop` appears there.

local bridge_path, max_seconds = arg[1], tonumber(arg[2] or "30")
local ipc_dir = assert(os.getenv("REAPER_MCP_DIR"), "REAPER_MCP_DIR must be set")

local project = { tracks = {}, markers = {}, dirty = false, next_guid = 1 }
local master = { name = "MASTER", guid = "{MASTER}", info = { IP_TRACKNUMBER = -1, D_VOL = 1 } }

local function new_track()
  local guid = string.format("{00000000-0000-4000-8000-%012d}", project.next_guid)
  project.next_guid = project.next_guid + 1
  return {
    name = "",
    guid = guid,
    info = { D_VOL = 1, D_PAN = 0, B_MUTE = 0, I_SOLO = 0, B_PHASE = 0, I_FOLDERDEPTH = 0,
      I_NCHAN = 2, B_MAINSEND = 1, I_FXEN = 1 },
  }
end

local function index_of(track)
  for i, candidate in ipairs(project.tracks) do
    if candidate == track then return i end
  end
end

local deferred
local fake = {
  -- Script environment.
  GetResourcePath = function() return ipc_dir end,
  RecursiveCreateDirectory = function() return 1 end,
  time_precise = function() return os.time() end,
  defer = function(fn) deferred = fn end,
  GetAppVersion = function() return "7.0/fake" end,

  -- Undo and refresh: no visible effect here.
  Undo_BeginBlock2 = function() end,
  Undo_EndBlock2 = function() end,
  MarkProjectDirty = function() project.dirty = true end,
  IsProjectDirty = function() return project.dirty and 1 or 0 end,
  PreventUIRefresh = function() end,
  TrackList_AdjustWindows = function() end,
  UpdateArrange = function() end,
  UpdateTimeline = function() end,

  -- Tracks.
  CountTracks = function() return #project.tracks end,
  GetTrack = function(_, index) return project.tracks[index + 1] end,
  GetMasterTrack = function() return master end,
  InsertTrackAtIndex = function(index) table.insert(project.tracks, index + 1, new_track()) end,
  DeleteTrack = function(track) table.remove(project.tracks, assert(index_of(track), "unknown track")) end,
  GetTrackGUID = function(track) return track.guid end,
  GetParentTrack = function() return nil end,
  GetSetMediaTrackInfo_String = function(track, key, value, set)
    assert(key == "P_NAME", "fake reaper: only P_NAME is supported, got " .. key)
    if set then track.name = value end
    return true, track.name
  end,
  GetMediaTrackInfo_Value = function(track, key)
    if key == "IP_TRACKNUMBER" and track ~= master then return index_of(track) end
    return track.info[key] or 0
  end,
  SetMediaTrackInfo_Value = function(track, key, value)
    track.info[key] = value
    return true
  end,
  TrackFX_GetCount = function() return 0 end,
  GetTrackNumSends = function() return 0 end,
  CountTrackMediaItems = function() return 0 end,

  -- Markers and regions.
  ColorToNative = function(r, g, b) return r | (g << 8) | (b << 16) end,
  AddProjectMarker2 = function(_, is_region, position, region_end, name, _, color)
    table.insert(project.markers, { is_region = is_region, position = position,
      region_end = region_end, name = name, number = #project.markers + 1, color = color })
    table.sort(project.markers, function(a, b) return a.position < b.position end)
    return #project.markers - 1
  end,
  DeleteProjectMarkerByIndex = function(_, index)
    return table.remove(project.markers, index + 1) ~= nil
  end,
  EnumProjectMarkers3 = function(_, index)
    local m = project.markers[index + 1]
    if not m then return 0 end
    return index + 1, m.is_region, m.position, m.region_end, m.name, m.number, m.color
  end,
}

reaper = setmetatable(fake, {
  __index = function(_, name)
    error("fake reaper: reaper." .. name .. " is not implemented", 2)
  end,
})

local separator = package.config:sub(1, 1)
local function stop_requested()
  local file = io.open(ipc_dir .. separator .. "stop", "rb")
  if file then file:close() return true end
  return false
end

dofile(bridge_path)
local deadline = os.time() + max_seconds
while deferred and os.time() < deadline and not stop_requested() do
  local step = deferred
  deferred = nil
  step()
  os.execute("sleep 0.01")
end
