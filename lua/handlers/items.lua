-- Media item and MIDI handlers.
-- Loaded by reaper_mcp_handlers.lua with the shared helpers and the handlers table.

local S, handlers = ...

local bbt, db_to_gain, file_exists, gain_to_db, get_track_name, require_int, resolve_track, undo = S.bbt, S.db_to_gain, S.file_exists, S.gain_to_db, S.get_track_name, S.require_int, S.resolve_track, S.undo

-- Take channel modes (I_CHANMODE) by name; mono modes matter for plugins that read one input channel.
local CHANNEL_MODES = { [0] = "normal", [1] = "reverse_stereo", [2] = "mono_mix", [3] = "left", [4] = "right" }

local CHANNEL_MODE_VALUES = {}
for value, name in pairs(CHANNEL_MODES) do CHANNEL_MODE_VALUES[name] = value end

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
