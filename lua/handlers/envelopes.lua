-- Automation envelope handlers.
-- Loaded by reaper_mcp_handlers.lua with the shared helpers and the handlers table.

local S, handlers = ...

local db_to_gain, find_param, fx_name, gain_to_db, get_track_name, resolve_fx, resolve_track, undo = S.db_to_gain, S.find_param, S.fx_name, S.gain_to_db, S.get_track_name, S.resolve_fx, S.resolve_track, S.undo

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
